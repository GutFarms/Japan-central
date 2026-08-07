//! ESP32-2432S028 (Cheap Yellow Display) GUI — ILI9341 SPI, 320×240 landscape.

use core::fmt::Write as _;

use embedded_graphics::Drawable;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X12, FONT_8X13_BOLD};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::prelude::{Primitive, WebColors};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle, RoundedRectangle};
use embedded_graphics::text::Text;
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::delay::Delay;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;
use esp_hal::Blocking;
use heapless::String;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ILI9341Rgb565;
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use mipidsi::{Builder, Display as MipiDisplay, NoResetPin};
use static_cell::StaticCell;

use crate::config::{PoolConfig, SetupField};
use crate::gui::{GuiScreen, GuiState, MenuItem};
use crate::miner::{hash_to_hex, MinerStats, SCRYPT_LOG_N, SCRYPT_N};
use crate::radio::RadioStatus;
use crate::stratum::StratumStatus;

/// Landscape resolution after Deg90 rotation of the native 240×320 panel.
pub const DISPLAY_WIDTH: u16 = 320;
pub const DISPLAY_HEIGHT: u16 = 240;

const BRAND: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_ORANGE);
const LABEL: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X12, Rgb565::CSS_GRAY);
const VALUE: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
const VALUE_SM: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_8X13_BOLD, Rgb565::WHITE);
const OK: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_LIME_GREEN);
const MUTED: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X12, Rgb565::CSS_DIM_GRAY);
const ACCENT: Rgb565 = Rgb565::CSS_DARK_ORANGE;
const PANEL: Rgb565 = Rgb565::new(3, 6, 3);
const BAR_BG: Rgb565 = Rgb565::new(4, 8, 4);
const BAR_FG: Rgb565 = Rgb565::CSS_ORANGE;
const SELECT: Rgb565 = Rgb565::new(8, 12, 4);

type SpiBus<'a> = Spi<'a, Blocking>;
type SpiDev<'a> = ExclusiveDevice<SpiBus<'a>, Output<'a>, Delay>;
type SpiDi<'a> = SpiInterface<'a, SpiDev<'a>, Output<'a>>;
type MipiDisplayWrapper<'a> = MipiDisplay<SpiDi<'a>, ILI9341Rgb565, NoResetPin>;

pub struct Display<'a, D: DelayNs> {
    display: MipiDisplayWrapper<'a>,
    backlight: Output<'a>,
    delay: D,
    last_screen: Option<GuiScreen>,
}

/// CYD TFT pins + SPI2 (HSPI).
pub struct DisplayPeripherals {
    pub spi: esp_hal::peripherals::SPI2<'static>,
    pub sclk: AnyPin<'static>,
    pub mosi: AnyPin<'static>,
    pub miso: AnyPin<'static>,
    pub cs: AnyPin<'static>,
    pub dc: AnyPin<'static>,
    pub backlight: AnyPin<'static>,
}

impl<'a, D: DelayNs> Display<'a, D> {
    fn draw_text(
        &mut self,
        text: &str,
        position: Point,
        style: MonoTextStyle<'_, Rgb565>,
    ) -> Result<(), Error> {
        Text::new(text, position, style)
            .draw(&mut self.display)
            .map(|_| ())
            .map_err(|_| Error::DisplayInterface("text"))
    }

    pub fn new(p: DisplayPeripherals, mut delay: D) -> Result<Self, Error> {
        let backlight = Output::new(p.backlight, Level::High, OutputConfig::default());
        let dc = Output::new(p.dc, Level::Low, OutputConfig::default());
        let cs = Output::new(p.cs, Level::High, OutputConfig::default());

        let spi = Spi::new(
            p.spi,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(40))
                .with_mode(SpiMode::_0),
        )
        .map_err(|_| Error::InitError)?
        .with_sck(p.sclk)
        .with_mosi(p.mosi)
        .with_miso(p.miso);

        // Short CS settle delay between transactions.
        let spi_dev = ExclusiveDevice::new(spi, cs, Delay::new());

        static SPI_BUF: StaticCell<[u8; 512]> = StaticCell::new();
        let buf = SPI_BUF.init([0u8; 512]);
        let di = SpiInterface::new(spi_dev, dc, buf);

        // Native panel is 240×320; Deg90 → 320×240 landscape. CYD ILI9341 is BGR.
        let display = Builder::new(ILI9341Rgb565, di)
            .display_size(240, 320)
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .color_order(ColorOrder::Bgr)
            .init(&mut delay)
            .map_err(|_| Error::InitError)?;

        Ok(Self {
            display,
            backlight,
            delay,
            last_screen: None,
        })
    }

    fn wake_clear(&mut self) -> Result<(), Error> {
        self.backlight.set_high();
        self.display
            .wake(&mut self.delay)
            .map_err(|_| Error::InitError)?;
        self.display
            .clear(Rgb565::BLACK)
            .map_err(|_| Error::DisplayInterface("clear"))?;
        Ok(())
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Rgb565) -> Result<(), Error> {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut self.display)
            .map(|_| ())
            .map_err(|_| Error::DisplayInterface("rect"))
    }

    fn round_panel(&mut self, x: i32, y: i32, w: u32, h: u32, color: Rgb565) -> Result<(), Error> {
        RoundedRectangle::with_equal_corners(
            Rectangle::new(Point::new(x, y), Size::new(w, h)),
            Size::new(6, 6),
        )
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(&mut self.display)
        .map(|_| ())
        .map_err(|_| Error::DisplayInterface("panel"))
    }

    fn header_bar(&mut self, tab: &str) -> Result<(), Error> {
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 28, PANEL)?;
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 3, ACCENT)?;
        self.draw_text("SCRYPT", Point::new(10, 20), BRAND)?;
        self.draw_text(tab, Point::new(270, 20), OK)?;
        Ok(())
    }

    fn footer_hint(&mut self, text: &str) -> Result<(), Error> {
        self.fill_rect(0, 226, DISPLAY_WIDTH as u32, 14, PANEL)?;
        self.draw_text(text, Point::new(8, 236), MUTED)?;
        Ok(())
    }

    pub fn draw_splash(&mut self) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32, Rgb565::BLACK)?;
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 4, ACCENT)?;
        self.draw_text("SCRYPT", Point::new(110, 90), BRAND)?;
        let mut line: String<48> = String::new();
        let _ = write!(line, "ESP32-2432S028  N={}  r=1 p=1", SCRYPT_N);
        self.draw_text(&line, Point::new(40, 130), LABEL)?;
        self.draw_text("loading GUI…", Point::new(110, 160), MUTED)?;
        Ok(())
    }

    pub fn draw_setup(&mut self, field: SetupField, typed: &str) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar("SETUP")?;

        let step_n = match field {
            SetupField::Address => 1,
            SetupField::Password => 2,
            SetupField::Stratum => 3,
            SetupField::WifiSsid => 4,
            SetupField::WifiPassword => 5,
            SetupField::BleName => 6,
        };
        for i in 1..=6u8 {
            let x = 12 + (i as i32 - 1) * 14;
            let color = if i <= step_n { ACCENT } else { BAR_BG };
            self.fill_rect(x, 40, 10, 6, color)?;
        }

        let mut step: String<40> = String::new();
        let _ = write!(step, "{}/6  {}", step_n, field.label());
        self.draw_text(&step, Point::new(100, 48), LABEL)?;

        self.round_panel(8, 70, 304, 100, PANEL)?;
        self.draw_text(field.prompt(), Point::new(18, 100), VALUE_SM)?;

        let shown = if field.is_secret() && !typed.is_empty() {
            PoolConfig::ellipsize("********", 36)
        } else if typed.is_empty() {
            PoolConfig::ellipsize("(waiting for USB serial…)", 36)
        } else {
            PoolConfig::ellipsize(typed, 36)
        };
        self.draw_text(&shown, Point::new(18, 140), VALUE)?;
        self.footer_hint("type value on USB serial, then Enter")?;
        Ok(())
    }

    pub fn draw_config_summary(&mut self, cfg: &PoolConfig, from_flash: bool) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar(if from_flash { "SAVED" } else { "READY" })?;

        self.round_panel(8, 40, 304, 160, PANEL)?;
        self.draw_row(60, "address", &PoolConfig::ellipsize(cfg.address.as_str(), 26))?;
        self.draw_row(90, "password", cfg.password_masked().as_str())?;
        self.draw_row(120, "stratum", &PoolConfig::ellipsize(cfg.stratum.as_str(), 26))?;
        let wifi = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 22)
        } else {
            PoolConfig::ellipsize("(wifi off)", 22)
        };
        self.draw_row(150, "wifi", wifi.as_str())?;
        self.draw_row(180, "ble", cfg.ble_name_or_default())?;

        let hint = if from_flash {
            "BOOT short=tabs  long=menu  serial: change"
        } else {
            "saved to flash for next boot"
        };
        self.footer_hint(hint)?;
        Ok(())
    }

    fn draw_row(&mut self, y: i32, label: &str, value: &str) -> Result<(), Error> {
        self.draw_text(label, Point::new(18, y), LABEL)?;
        self.draw_text(value, Point::new(90, y), VALUE_SM)?;
        Ok(())
    }

    pub fn draw_gui(
        &mut self,
        gui: &GuiState,
        stats: &MinerStats,
        cfg: &PoolConfig,
        radio: &RadioStatus,
        stratum: &StratumStatus,
        mining: bool,
    ) -> Result<(), Error> {
        let screen_changed = self.last_screen != Some(gui.screen);
        if screen_changed {
            self.wake_clear()?;
            self.header_bar(gui.screen.title())?;
            self.last_screen = Some(gui.screen);
        }

        match gui.screen {
            GuiScreen::Mining => self.draw_mining_body(stats, cfg, stratum, mining, screen_changed)?,
            GuiScreen::Config => {
                if screen_changed {
                    self.draw_config_body(cfg)?;
                }
            }
            GuiScreen::Radio => self.draw_radio_body(cfg, radio, stratum, screen_changed)?,
            GuiScreen::Menu => self.draw_menu_body(gui.menu)?,
        }
        Ok(())
    }

    fn draw_mining_body(
        &mut self,
        stats: &MinerStats,
        cfg: &PoolConfig,
        stratum: &StratumStatus,
        mining: bool,
        full: bool,
    ) -> Result<(), Error> {
        if full {
            self.round_panel(8, 40, 200, 100, PANEL)?;
            self.round_panel(216, 40, 96, 100, PANEL)?;
            self.round_panel(8, 152, 304, 56, PANEL)?;
            self.footer_hint("BOOT short=next  long=menu")?;
        }

        self.fill_rect(16, 48, 184, 36, PANEL)?;
        let mut rate: String<24> = String::new();
        let _ = write!(
            rate,
            "{}.{:02} H/s",
            stats.hashrate_x100 / 100,
            stats.hashrate_x100 % 100
        );
        self.draw_text("hashrate", Point::new(16, 56), LABEL)?;
        self.draw_text(&rate, Point::new(16, 80), VALUE)?;

        let pct = core::cmp::min(100u32, stats.hashrate_x100 / 20);
        self.fill_rect(16, 110, 184, 10, BAR_BG)?;
        let w = (184 * pct / 100).max(if mining { 4 } else { 0 });
        if w > 0 {
            self.fill_rect(16, 110, w, 10, BAR_FG)?;
        }

        self.fill_rect(224, 48, 80, 80, PANEL)?;
        let st = if mining { "MINING" } else { "IDLE" };
        let st_style = if mining { OK } else { LABEL };
        self.draw_text(st, Point::new(228, 70), st_style)?;
        let mut shares: String<16> = String::new();
        let _ = write!(shares, "{} sh", stats.shares);
        self.draw_text(&shares, Point::new(228, 110), VALUE_SM)?;

        self.fill_rect(16, 160, 288, 40, PANEL)?;
        let mut nonce: String<20> = String::new();
        let _ = write!(nonce, "{:08x}", stats.nonce);
        let job = if stratum.job_id.is_empty() {
            PoolConfig::ellipsize(cfg.address.as_str(), 10)
        } else {
            PoolConfig::ellipsize(stratum.job_id.as_str(), 10)
        };
        let mut best: String<128> = String::new();
        hash_to_hex(&stats.best_hash, 2, &mut best);
        let mut line: String<72> = String::new();
        let _ = write!(
            line,
            "{} {} {} a{}/r{}/d{} {}",
            nonce,
            job,
            stratum.phase.label(),
            stratum.accepted,
            stratum.rejected,
            stratum.dropped,
            best
        );
        self.draw_text(&line, Point::new(16, 180), LABEL)?;

        let _ = SCRYPT_LOG_N;
        Ok(())
    }

    fn draw_config_body(&mut self, cfg: &PoolConfig) -> Result<(), Error> {
        self.round_panel(8, 40, 304, 160, PANEL)?;
        self.draw_row(60, "address", &PoolConfig::ellipsize(cfg.address.as_str(), 26))?;
        self.draw_row(90, "password", cfg.password_masked().as_str())?;
        self.draw_row(120, "stratum", &PoolConfig::ellipsize(cfg.stratum.as_str(), 26))?;
        let wifi = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 22)
        } else {
            PoolConfig::ellipsize("(off)", 22)
        };
        self.draw_row(150, "wifi", wifi.as_str())?;
        self.draw_row(180, "ble", cfg.ble_name_or_default())?;
        self.footer_hint("BOOT short=next  long=menu  serial: change")?;
        Ok(())
    }

    fn draw_radio_body(
        &mut self,
        cfg: &PoolConfig,
        radio: &RadioStatus,
        stratum: &StratumStatus,
        full: bool,
    ) -> Result<(), Error> {
        if full {
            self.round_panel(8, 40, 304, 160, PANEL)?;
            self.footer_hint("BOOT short=next  long=menu")?;
        }

        self.fill_rect(16, 48, 288, 140, PANEL)?;

        let ssid = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 18)
        } else {
            PoolConfig::ellipsize("(disabled)", 18)
        };
        let mut wifi_line: String<48> = String::new();
        let _ = write!(
            wifi_line,
            "WiFi {} {}",
            radio.wifi.label(),
            ssid.as_str()
        );
        self.draw_text(&wifi_line, Point::new(18, 70), VALUE_SM)?;

        let mut ip_line: String<40> = String::new();
        let _ = write!(ip_line, "IP   {}", radio.ip_string().as_str());
        self.draw_text(&ip_line, Point::new(18, 100), VALUE_SM)?;

        let ble_state = if radio.ble_connected {
            "conn"
        } else if radio.ble_advertising {
            "adv"
        } else {
            "off"
        };
        let mut ble_line: String<48> = String::new();
        let _ = write!(
            ble_line,
            "BLE  {} {}",
            ble_state,
            PoolConfig::ellipsize(cfg.ble_name_or_default(), 14).as_str()
        );
        self.draw_text(&ble_line, Point::new(18, 130), VALUE_SM)?;

        let mut st_line: String<56> = String::new();
        let _ = write!(
            st_line,
            "POOL {} d{} a{}/r{}/d{}",
            stratum.phase.label(),
            stratum.difficulty,
            stratum.accepted,
            stratum.rejected,
            stratum.dropped
        );
        self.draw_text(&st_line, Point::new(18, 160), VALUE_SM)?;
        if !stratum.detail.is_empty() {
            let detail = PoolConfig::ellipsize(stratum.detail.as_str(), 28);
            self.draw_text(detail.as_str(), Point::new(18, 185), VALUE_SM)?;
        }
        Ok(())
    }

    fn draw_menu_body(&mut self, selected: MenuItem) -> Result<(), Error> {
        self.round_panel(8, 40, 304, 160, PANEL)?;
        self.draw_text("Options", Point::new(18, 65), LABEL)?;

        for (i, item) in MenuItem::ALL.iter().enumerate() {
            let y = 100 + i as i32 * 36;
            let bg = if *item == selected { SELECT } else { PANEL };
            self.round_panel(18, y - 14, 284, 30, bg)?;
            let style = if *item == selected { OK } else { VALUE_SM };
            self.draw_text(item.label(), Point::new(28, y), style)?;
        }
        self.footer_hint("BOOT short=select  long=activate")?;
        Ok(())
    }

    pub fn draw_auth_prompt(&mut self, attempt: u8, max: u8) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar("AUTH")?;
        self.round_panel(8, 70, 304, 100, PANEL)?;
        self.draw_text("Enter current password", Point::new(40, 110), VALUE_SM)?;
        let mut tries: String<32> = String::new();
        let _ = write!(tries, "attempt {attempt}/{max}  (USB serial)");
        self.draw_text(&tries, Point::new(60, 145), LABEL)?;
        self.footer_hint("password required to change credentials")?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    DisplayInterface(&'static str),
    InitError,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::DisplayInterface(msg) => write!(f, "display: {msg}"),
            Error::InitError => write!(f, "display init failed"),
        }
    }
}

