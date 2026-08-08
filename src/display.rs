//! ESP32-2432S028 (Cheap Yellow Display) GUI — ILI9341 SPI, 320×240 landscape.

use core::fmt::Write as _;

use embedded_graphics::Drawable;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X10, FONT_6X12, FONT_8X13_BOLD};
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
use crate::keyboard::Keyboard;
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
const KEY_TXT: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
const KEY_TXT_DIM: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_ORANGE);

const ACCENT: Rgb565 = Rgb565::CSS_DARK_ORANGE;
const ACCENT_HOT: Rgb565 = Rgb565::CSS_ORANGE;
const PANEL: Rgb565 = Rgb565::new(3, 6, 3);
const PANEL_HI: Rgb565 = Rgb565::new(5, 10, 5);
const BAR_BG: Rgb565 = Rgb565::new(4, 8, 4);
const BAR_FG: Rgb565 = Rgb565::CSS_ORANGE;
const SELECT: Rgb565 = Rgb565::new(8, 12, 4);
const KEY_BG: Rgb565 = Rgb565::new(6, 10, 6);
const KEY_BG_HOT: Rgb565 = Rgb565::new(10, 14, 4);
const KEY_OK: Rgb565 = Rgb565::new(4, 14, 4);
const BG_DEEP: Rgb565 = Rgb565::new(1, 2, 1);

type SpiBus<'a> = Spi<'a, Blocking>;
type SpiDev<'a> = ExclusiveDevice<SpiBus<'a>, Output<'a>, Delay>;
type SpiDi<'a> = SpiInterface<'a, SpiDev<'a>, Output<'a>>;
type MipiDisplayWrapper<'a> = MipiDisplay<SpiDi<'a>, ILI9341Rgb565, NoResetPin>;

pub struct Display<'a, D: DelayNs> {
    display: MipiDisplayWrapper<'a>,
    backlight: Output<'a>,
    delay: D,
    last_screen: Option<GuiScreen>,
    /// Animation phase 0..255 for subtle pulse.
    pub tick: u8,
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

        let spi_dev = ExclusiveDevice::new(spi, cs, Delay::new());

        static SPI_BUF: StaticCell<[u8; 512]> = StaticCell::new();
        let buf = SPI_BUF.init([0u8; 512]);
        let di = SpiInterface::new(spi_dev, dc, buf);

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
            tick: 0,
        })
    }

    pub fn bump_tick(&mut self) {
        self.tick = self.tick.wrapping_add(3);
    }

    fn wake_clear(&mut self) -> Result<(), Error> {
        self.backlight.set_high();
        self.display
            .wake(&mut self.delay)
            .map_err(|_| Error::InitError)?;
        self.display
            .clear(BG_DEEP)
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

    fn accent_stripe(&mut self) -> Result<(), Error> {
        // Pulsing top edge
        let hot = self.tick < 128;
        let c = if hot { ACCENT_HOT } else { ACCENT };
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 3, c)?;
        // Side rail flare
        self.fill_rect(0, 3, 3, DISPLAY_HEIGHT as u32 - 3, ACCENT)?;
        Ok(())
    }

    fn header_bar(&mut self, tab: &str) -> Result<(), Error> {
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 28, PANEL)?;
        self.accent_stripe()?;
        self.draw_text("SCRYPT", Point::new(10, 20), BRAND)?;
        self.draw_text(tab, Point::new(250, 20), OK)?;
        Ok(())
    }

    fn tab_strip(&mut self, active: GuiScreen) -> Result<(), Error> {
        self.fill_rect(0, 28, DISPLAY_WIDTH as u32, 26, PANEL_HI)?;
        for (i, screen) in GuiScreen::ALL.iter().enumerate() {
            let x = 4 + i as i32 * 80;
            let selected = *screen == active;
            let bg = if selected { ACCENT } else { BAR_BG };
            self.round_panel(x, 30, 72, 20, bg)?;
            let style = if selected {
                MonoTextStyle::new(&FONT_6X12, Rgb565::BLACK)
            } else {
                MUTED
            };
            self.draw_text(screen.title(), Point::new(x + 18, 44), style)?;
        }
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
        // Diagonal-ish flare bands
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 8, ACCENT_HOT)?;
        self.fill_rect(0, 8, DISPLAY_WIDTH as u32, 4, ACCENT)?;
        self.fill_rect(0, 200, DISPLAY_WIDTH as u32, 40, PANEL)?;
        self.round_panel(40, 70, 240, 90, PANEL_HI)?;
        self.draw_text("SCRYPT", Point::new(110, 100), BRAND)?;
        let mut line: String<48> = String::new();
        let _ = write!(line, "ESP32-CYD  N={}  touch+serial", SCRYPT_N);
        self.draw_text(&line, Point::new(48, 130), LABEL)?;
        self.draw_text("warming up…", Point::new(118, 160), MUTED)?;
        self.draw_text("tap the glass · type on keys", Point::new(70, 220), KEY_TXT_DIM)?;
        Ok(())
    }

    pub fn draw_setup(&mut self, field: SetupField, typed: &str) -> Result<(), Error> {
        self.draw_setup_keyboard(field, typed, &Keyboard::default())
    }

    pub fn draw_setup_keyboard(
        &mut self,
        field: SetupField,
        typed: &str,
        kb: &Keyboard,
    ) -> Result<(), Error> {
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
            let x = 12 + (i as i32 - 1) * 18;
            let color = if i < step_n {
                ACCENT
            } else if i == step_n {
                ACCENT_HOT
            } else {
                BAR_BG
            };
            self.round_panel(x, 34, 14, 8, color)?;
        }

        let mut step: String<40> = String::new();
        let _ = write!(step, "{}/6 {}", step_n, field.label());
        self.draw_text(&step, Point::new(130, 42), LABEL)?;

        // Value field
        self.round_panel(6, 48, 308, 42, PANEL_HI)?;
        self.draw_text(field.prompt(), Point::new(12, 60), MUTED)?;
        let shown: String<96> = if field.is_secret() && !typed.is_empty() {
            let n = typed.len().min(24);
            let mut stars: String<96> = String::new();
            for _ in 0..n {
                let _ = stars.push('*');
            }
            stars
        } else if typed.is_empty() {
            let mut s: String<96> = String::new();
            let _ = s.push_str("tap keys...");
            s
        } else {
            PoolConfig::ellipsize(typed, 34)
        };
        let style = if typed.is_empty() { MUTED } else { VALUE_SM };
        self.draw_text(shown.as_str(), Point::new(12, 78), style)?;

        self.draw_keyboard(kb)?;
        Ok(())
    }

    pub fn draw_keyboard(&mut self, kb: &Keyboard) -> Result<(), Error> {
        self.fill_rect(0, kb.origin_y - 4, DISPLAY_WIDTH as u32, 240 - kb.origin_y as u32 + 4, PANEL)?;
        for key in kb.keys() {
            let bg = match key.action {
                crate::keyboard::KeyAction::Enter => KEY_OK,
                crate::keyboard::KeyAction::Shift | crate::keyboard::KeyAction::Symbols => KEY_BG_HOT,
                crate::keyboard::KeyAction::Skip => ACCENT,
                _ => KEY_BG,
            };
            self.round_panel(key.x, key.y, key.w, key.h, bg)?;
            // Center-ish label
            let tx = key.x + 4;
            let ty = key.y + (key.h as i32 / 2) + 3;
            let style = match key.action {
                crate::keyboard::KeyAction::Enter => OK,
                crate::keyboard::KeyAction::Skip => MonoTextStyle::new(&FONT_6X10, Rgb565::BLACK),
                _ => KEY_TXT,
            };
            self.draw_text(key.label, Point::new(tx, ty), style)?;
        }
        Ok(())
    }

    pub fn draw_config_summary(&mut self, cfg: &PoolConfig, from_flash: bool) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar(if from_flash { "SAVED" } else { "READY" })?;
        self.tab_strip(GuiScreen::Config)?;

        self.round_panel(8, 60, 304, 150, PANEL)?;
        self.draw_row(78, "address", &PoolConfig::ellipsize(cfg.address.as_str(), 26))?;
        self.draw_row(104, "password", cfg.password_masked().as_str())?;
        self.draw_row(130, "stratum", &PoolConfig::ellipsize(cfg.stratum.as_str(), 26))?;
        let wifi = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 22)
        } else {
            PoolConfig::ellipsize("(wifi off)", 22)
        };
        self.draw_row(156, "wifi", wifi.as_str())?;
        self.draw_row(182, "ble", cfg.ble_name_or_default())?;

        let hint = if from_flash {
            "tap tabs · long BOOT=menu · serial: change"
        } else {
            "saved to flash — tap glass to explore"
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
        self.bump_tick();
        let screen_changed = self.last_screen != Some(gui.screen);
        if screen_changed {
            self.wake_clear()?;
            self.header_bar(gui.screen.title())?;
            self.tab_strip(gui.screen)?;
            self.last_screen = Some(gui.screen);
        } else {
            // Refresh accent pulse without full clear
            let _ = self.accent_stripe();
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
            self.round_panel(8, 60, 200, 100, PANEL)?;
            self.round_panel(216, 60, 96, 100, PANEL)?;
            self.round_panel(8, 170, 304, 48, PANEL)?;
            self.footer_hint("tap tabs · BOOT short=next long=menu")?;
        }

        self.fill_rect(16, 68, 184, 36, PANEL)?;
        let mut rate: String<24> = String::new();
        let _ = write!(
            rate,
            "{}.{:02} H/s",
            stats.hashrate_x100 / 100,
            stats.hashrate_x100 % 100
        );
        self.draw_text("hashrate", Point::new(16, 76), LABEL)?;
        self.draw_text(&rate, Point::new(16, 100), VALUE)?;

        let pct = core::cmp::min(100u32, stats.hashrate_x100 / 20);
        self.fill_rect(16, 130, 184, 12, BAR_BG)?;
        let pulse = if mining {
            4 + (self.tick as u32 % 12)
        } else {
            0
        };
        let w = (184 * pct / 100).max(pulse);
        if w > 0 {
            let fg = if self.tick < 128 { BAR_FG } else { ACCENT_HOT };
            self.fill_rect(16, 130, w.min(184), 12, fg)?;
        }

        // Status chip with soft border
        let chip = if mining { KEY_OK } else { PANEL_HI };
        self.round_panel(224, 70, 80, 36, chip)?;
        let st = if mining { "LIVE" } else { "IDLE" };
        let st_style = if mining { OK } else { LABEL };
        self.draw_text(st, Point::new(240, 94), st_style)?;

        let mut shares: String<16> = String::new();
        let _ = write!(shares, "{} sh", stats.shares);
        self.draw_text(&shares, Point::new(236, 130), VALUE_SM)?;

        // Activity dots
        for i in 0..4u8 {
            let on = mining && ((self.tick.wrapping_add(i.wrapping_mul(40))) > 120);
            let c = if on { ACCENT_HOT } else { BAR_BG };
            self.fill_rect(236 + i as i32 * 14, 148, 8, 8, c)?;
        }

        self.fill_rect(16, 178, 288, 32, PANEL)?;
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
            "{} {} {} a{}/r{}",
            nonce,
            job,
            stratum.phase.label(),
            stratum.accepted,
            stratum.rejected
        );
        self.draw_text(&line, Point::new(16, 198), LABEL)?;

        let _ = SCRYPT_LOG_N;
        Ok(())
    }

    fn draw_config_body(&mut self, cfg: &PoolConfig) -> Result<(), Error> {
        self.round_panel(8, 60, 304, 150, PANEL)?;
        self.draw_row(78, "address", &PoolConfig::ellipsize(cfg.address.as_str(), 26))?;
        self.draw_row(104, "password", cfg.password_masked().as_str())?;
        self.draw_row(130, "stratum", &PoolConfig::ellipsize(cfg.stratum.as_str(), 26))?;
        let wifi = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 22)
        } else {
            PoolConfig::ellipsize("(off)", 22)
        };
        self.draw_row(156, "wifi", wifi.as_str())?;
        self.draw_row(182, "ble", cfg.ble_name_or_default())?;
        self.footer_hint("tap body to change · serial also works")?;
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
            self.round_panel(8, 60, 304, 150, PANEL)?;
            self.footer_hint("tap tabs · BOOT short=next")?;
        }

        self.fill_rect(16, 68, 288, 130, PANEL)?;

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
        self.draw_text(&wifi_line, Point::new(18, 88), VALUE_SM)?;

        let mut ip_line: String<40> = String::new();
        let _ = write!(ip_line, "IP   {}", radio.ip_string().as_str());
        self.draw_text(&ip_line, Point::new(18, 114), VALUE_SM)?;

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
        self.draw_text(&ble_line, Point::new(18, 140), VALUE_SM)?;

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
        self.draw_text(&st_line, Point::new(18, 166), VALUE_SM)?;
        if !stratum.detail.is_empty() {
            let detail = PoolConfig::ellipsize(stratum.detail.as_str(), 28);
            self.draw_text(detail.as_str(), Point::new(18, 190), VALUE_SM)?;
        }
        Ok(())
    }

    fn draw_menu_body(&mut self, selected: MenuItem) -> Result<(), Error> {
        self.round_panel(8, 60, 304, 150, PANEL)?;
        self.draw_text("Options — tap a row", Point::new(18, 78), LABEL)?;

        for (i, item) in MenuItem::ALL.iter().enumerate() {
            let y = 100 + i as i32 * 36;
            let bg = if *item == selected { SELECT } else { PANEL_HI };
            self.round_panel(18, y - 14, 284, 30, bg)?;
            if *item == selected {
                self.fill_rect(18, y - 14, 4, 30, ACCENT_HOT)?;
            }
            let style = if *item == selected { OK } else { VALUE_SM };
            self.draw_text(item.label(), Point::new(32, y), style)?;
        }
        self.footer_hint("tap row or BOOT short=select long=go")?;
        Ok(())
    }

    pub fn draw_auth_prompt(&mut self, attempt: u8, max: u8) -> Result<(), Error> {
        self.draw_auth_keyboard(attempt, max, "", &Keyboard::default())
    }

    pub fn draw_auth_keyboard(
        &mut self,
        attempt: u8,
        max: u8,
        typed: &str,
        kb: &Keyboard,
    ) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar("AUTH")?;
        self.round_panel(6, 36, 308, 52, PANEL_HI)?;
        self.draw_text("Current password", Point::new(14, 52), VALUE_SM)?;
        let mut tries: String<40> = String::new();
        let _ = write!(tries, "attempt {attempt}/{max}");
        self.draw_text(&tries, Point::new(200, 52), LABEL)?;
        let shown = if typed.is_empty() {
            "tap keys…"
        } else {
            "********"
        };
        self.draw_text(shown, Point::new(14, 74), if typed.is_empty() { MUTED } else { VALUE_SM })?;
        self.draw_keyboard(kb)?;
        Ok(())
    }

    /// Invalidate cached screen so next draw_gui does a full redraw.
    pub fn invalidate(&mut self) {
        self.last_screen = None;
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
