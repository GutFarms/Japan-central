//! LilyGO T-Display-S3 GUI (ST7789, 320×170, 8-bit parallel).

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
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use heapless::String;
use mipidsi::interface::{Generic8BitBus, ParallelError, ParallelInterface};
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};
use mipidsi::{Builder, Display as MipiDisplay};

use crate::config::{PoolConfig, SetupField};
use crate::gui::{GuiScreen, GuiState, MenuItem};
use crate::miner::{hash_to_hex, MinerStats, SCRYPT_LOG_N, SCRYPT_N};
use crate::radio::RadioStatus;
use crate::stratum::StratumStatus;

pub const DISPLAY_WIDTH: u16 = 320;
pub const DISPLAY_HEIGHT: u16 = 170;

const BRAND: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_ORANGE);
const LABEL: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X12, Rgb565::CSS_GRAY);
const VALUE: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
const VALUE_SM: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_8X13_BOLD, Rgb565::WHITE);
const OK: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_LIMEGREEN);
const MUTED: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X12, Rgb565::CSS_DIM_GRAY);
const ACCENT: Rgb565 = Rgb565::CSS_DARK_ORANGE;
const PANEL: Rgb565 = Rgb565::new(3, 6, 3); // dark slate-ish in RGB565
const BAR_BG: Rgb565 = Rgb565::new(4, 8, 4);
const BAR_FG: Rgb565 = Rgb565::CSS_ORANGE;
const SELECT: Rgb565 = Rgb565::new(8, 12, 4);

type MipiDisplayWrapper<'a> = MipiDisplay<
    ParallelInterface<
        Generic8BitBus<
            Output<'a>,
            Output<'a>,
            Output<'a>,
            Output<'a>,
            Output<'a>,
            Output<'a>,
            Output<'a>,
            Output<'a>,
        >,
        Output<'a>,
        Output<'a>,
    >,
    ST7789,
    Output<'a>,
>;

pub struct Display<'a, D: DelayNs> {
    display: MipiDisplayWrapper<'a>,
    backlight: Output<'a>,
    _power_en: Output<'a>,
    _cs: Output<'a>,
    _rd: Output<'a>,
    delay: D,
    /// Last painted operational screen (forces full redraw on switch).
    last_screen: Option<GuiScreen>,
}

pub struct DisplayPeripherals {
    pub rst: AnyPin<'static>,
    pub cs: AnyPin<'static>,
    pub dc: AnyPin<'static>,
    pub wr: AnyPin<'static>,
    pub rd: AnyPin<'static>,
    pub power_en: AnyPin<'static>,
    pub backlight: AnyPin<'static>,
    pub d0: AnyPin<'static>,
    pub d1: AnyPin<'static>,
    pub d2: AnyPin<'static>,
    pub d3: AnyPin<'static>,
    pub d4: AnyPin<'static>,
    pub d5: AnyPin<'static>,
    pub d6: AnyPin<'static>,
    pub d7: AnyPin<'static>,
}

impl<'a, D: DelayNs> Display<'a, D> {
    fn draw_ok<T, E>(&mut self, result: Result<T, E>) -> Result<(), Error>
    where
        Error: From<E>,
    {
        result.map(|_| ()).map_err(Error::from)
    }

    pub fn new(p: DisplayPeripherals, mut delay: D) -> Result<Self, Error> {
        let mut power_en = Output::new(p.power_en, Level::High, OutputConfig::default());
        power_en.set_high();

        let backlight = Output::new(p.backlight, Level::Low, OutputConfig::default());

        let dc = Output::new(p.dc, Level::Low, OutputConfig::default());
        let mut cs = Output::new(p.cs, Level::Low, OutputConfig::default());
        let rst = Output::new(p.rst, Level::Low, OutputConfig::default());
        let wr = Output::new(p.wr, Level::Low, OutputConfig::default());
        let mut rd = Output::new(p.rd, Level::Low, OutputConfig::default());

        cs.set_low();
        rd.set_high();

        let d0 = Output::new(p.d0, Level::Low, OutputConfig::default());
        let d1 = Output::new(p.d1, Level::Low, OutputConfig::default());
        let d2 = Output::new(p.d2, Level::Low, OutputConfig::default());
        let d3 = Output::new(p.d3, Level::Low, OutputConfig::default());
        let d4 = Output::new(p.d4, Level::Low, OutputConfig::default());
        let d5 = Output::new(p.d5, Level::Low, OutputConfig::default());
        let d6 = Output::new(p.d6, Level::Low, OutputConfig::default());
        let d7 = Output::new(p.d7, Level::Low, OutputConfig::default());

        let bus = Generic8BitBus::new((d0, d1, d2, d3, d4, d5, d6, d7));
        let di = ParallelInterface::new(bus, dc, wr);

        let display = Builder::new(ST7789, di)
            .display_size(DISPLAY_HEIGHT, DISPLAY_WIDTH)
            .display_offset((240 - DISPLAY_HEIGHT) / 2, 0)
            .orientation(Orientation::new().rotate(Rotation::Deg270))
            .invert_colors(ColorInversion::Inverted)
            .reset_pin(rst)
            .init(&mut delay)
            .map_err(|_| Error::InitError)?;

        Ok(Self {
            display,
            backlight,
            _power_en: power_en,
            _cs: cs,
            _rd: rd,
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
        self.draw_ok(
            Rectangle::new(Point::new(x, y), Size::new(w, h))
                .into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut self.display),
        )
    }

    fn round_panel(&mut self, x: i32, y: i32, w: u32, h: u32, color: Rgb565) -> Result<(), Error> {
        self.draw_ok(
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(x, y), Size::new(w, h)),
                Size::new(6, 6),
            )
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut self.display),
        )
    }

    fn header_bar(&mut self, tab: &str) -> Result<(), Error> {
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 28, PANEL)?;
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 3, ACCENT)?;
        self.draw_ok(Text::new("SCRYPT", Point::new(10, 20), BRAND).draw(&mut self.display))?;
        self.draw_ok(Text::new(tab, Point::new(270, 20), OK).draw(&mut self.display))?;
        Ok(())
    }

    fn footer_hint(&mut self, text: &str) -> Result<(), Error> {
        self.fill_rect(0, 156, DISPLAY_WIDTH as u32, 14, PANEL)?;
        self.draw_ok(Text::new(text, Point::new(8, 166), MUTED).draw(&mut self.display))?;
        Ok(())
    }

    /// Boot splash.
    pub fn draw_splash(&mut self) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32, Rgb565::BLACK)?;
        self.fill_rect(0, 0, DISPLAY_WIDTH as u32, 4, ACCENT)?;
        self.draw_ok(Text::new("SCRYPT", Point::new(110, 70), BRAND).draw(&mut self.display))?;
        let mut line: String<48> = String::new();
        let _ = write!(line, "ESP32-S3  N={}  r=1 p=1", SCRYPT_N);
        self.draw_ok(Text::new(&line, Point::new(70, 100), LABEL).draw(&mut self.display))?;
        self.draw_ok(
            Text::new("loading GUI…", Point::new(110, 130), MUTED).draw(&mut self.display),
        )?;
        Ok(())
    }

    /// Full-screen post-boot setup prompt for one credential field.
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
        // Step dots
        for i in 1..=6u8 {
            let x = 12 + (i as i32 - 1) * 14;
            let color = if i <= step_n { ACCENT } else { BAR_BG };
            self.fill_rect(x, 40, 10, 6, color)?;
        }

        let mut step: String<40> = String::new();
        let _ = write!(step, "{}/6  {}", step_n, field.label());
        self.draw_ok(Text::new(&step, Point::new(100, 48), LABEL).draw(&mut self.display))?;

        self.round_panel(8, 60, 304, 70, PANEL)?;
        self.draw_ok(
            Text::new(field.prompt(), Point::new(18, 85), VALUE_SM).draw(&mut self.display),
        )?;

        let shown = if field.is_secret() && !typed.is_empty() {
            PoolConfig::ellipsize("********", 36)
        } else if typed.is_empty() {
            PoolConfig::ellipsize("(waiting for USB serial…)", 36)
        } else {
            PoolConfig::ellipsize(typed, 36)
        };
        self.draw_ok(Text::new(&shown, Point::new(18, 112), VALUE).draw(&mut self.display))?;
        self.footer_hint("type value on USB serial, then Enter")?;
        Ok(())
    }

    /// Summary of credentials before mining starts.
    pub fn draw_config_summary(&mut self, cfg: &PoolConfig, from_flash: bool) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar(if from_flash { "SAVED" } else { "READY" })?;

        self.round_panel(8, 36, 304, 110, PANEL)?;
        self.draw_row(48, "address", &PoolConfig::ellipsize(cfg.address.as_str(), 26))?;
        self.draw_row(72, "password", cfg.password_masked().as_str())?;
        self.draw_row(96, "stratum", &PoolConfig::ellipsize(cfg.stratum.as_str(), 26))?;
        let wifi = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 22)
        } else {
            PoolConfig::ellipsize("(wifi off)", 22)
        };
        self.draw_row(120, "wifi", wifi.as_str())?;

        let hint = if from_flash {
            "BOOT=tabs  btn=menu  serial: change"
        } else {
            "saved to flash for next boot"
        };
        self.footer_hint(hint)?;
        Ok(())
    }

    fn draw_row(&mut self, y: i32, label: &str, value: &str) -> Result<(), Error> {
        self.draw_ok(Text::new(label, Point::new(18, y), LABEL).draw(&mut self.display))?;
        self.draw_ok(Text::new(value, Point::new(90, y), VALUE_SM).draw(&mut self.display))?;
        Ok(())
    }

    /// Paint the active GUI screen (full redraw when the tab changes).
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
            GuiScreen::Menu => {
                if screen_changed {
                    self.draw_menu_body(gui.menu)?;
                } else {
                    // Refresh selection highlight cheaply by redrawing menu body.
                    self.draw_menu_body(gui.menu)?;
                }
            }
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
            self.round_panel(8, 36, 200, 72, PANEL)?;
            self.round_panel(216, 36, 96, 72, PANEL)?;
            self.round_panel(8, 116, 304, 36, PANEL)?;
            self.footer_hint("BOOT=next  btn=menu")?;
        }

        // Hashrate
        self.fill_rect(16, 44, 184, 36, PANEL)?;
        let mut rate: String<24> = String::new();
        let _ = write!(
            rate,
            "{}.{:02} H/s",
            stats.hashrate_x100 / 100,
            stats.hashrate_x100 % 100
        );
        self.draw_ok(Text::new("hashrate", Point::new(16, 52), LABEL).draw(&mut self.display))?;
        self.draw_ok(Text::new(&rate, Point::new(16, 74), VALUE).draw(&mut self.display))?;

        // Activity bar (0–100% of a soft cap ~20 H/s for visual scale)
        let pct = core::cmp::min(100u32, stats.hashrate_x100 / 20);
        self.fill_rect(16, 88, 184, 10, BAR_BG)?;
        let w = (184 * pct / 100).max(if mining { 4 } else { 0 });
        if w > 0 {
            self.fill_rect(16, 88, w, 10, BAR_FG)?;
        }

        // Status + shares
        self.fill_rect(224, 44, 80, 56, PANEL)?;
        let st = if mining { "MINING" } else { "IDLE" };
        let st_style = if mining { OK } else { LABEL };
        self.draw_ok(Text::new(st, Point::new(228, 58), st_style).draw(&mut self.display))?;
        let mut shares: String<16> = String::new();
        let _ = write!(shares, "{} sh", stats.shares);
        self.draw_ok(Text::new(&shares, Point::new(228, 86), VALUE_SM).draw(&mut self.display))?;

        // Bottom identity / stratum strip
        self.fill_rect(16, 122, 288, 24, PANEL)?;
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
            "{} {} {} a{}/r{} {}",
            nonce,
            job,
            stratum.phase.label(),
            stratum.accepted,
            stratum.rejected,
            best
        );
        self.draw_ok(Text::new(&line, Point::new(16, 138), LABEL).draw(&mut self.display))?;

        let _ = SCRYPT_LOG_N;
        Ok(())
    }

    fn draw_config_body(&mut self, cfg: &PoolConfig) -> Result<(), Error> {
        self.round_panel(8, 36, 304, 110, PANEL)?;
        self.draw_row(52, "address", &PoolConfig::ellipsize(cfg.address.as_str(), 26))?;
        self.draw_row(76, "password", cfg.password_masked().as_str())?;
        self.draw_row(100, "stratum", &PoolConfig::ellipsize(cfg.stratum.as_str(), 26))?;
        let wifi = if cfg.wifi_enabled() {
            PoolConfig::ellipsize(cfg.wifi_ssid.as_str(), 22)
        } else {
            PoolConfig::ellipsize("(off)", 22)
        };
        self.draw_row(124, "wifi", wifi.as_str())?;
        self.footer_hint("BOOT=next  btn=menu  serial: change")?;
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
            self.round_panel(8, 36, 304, 110, PANEL)?;
            self.footer_hint("BOOT=next  btn=menu")?;
        }

        self.fill_rect(16, 44, 288, 96, PANEL)?;

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
        self.draw_ok(Text::new(&wifi_line, Point::new(18, 58), VALUE_SM).draw(&mut self.display))?;

        let mut ip_line: String<40> = String::new();
        let _ = write!(ip_line, "IP   {}", radio.ip_string().as_str());
        self.draw_ok(Text::new(&ip_line, Point::new(18, 80), VALUE_SM).draw(&mut self.display))?;

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
        self.draw_ok(Text::new(&ble_line, Point::new(18, 102), VALUE_SM).draw(&mut self.display))?;

        let mut st_line: String<56> = String::new();
        let _ = write!(
            st_line,
            "POOL {} d{} a{}/r{}",
            stratum.phase.label(),
            stratum.difficulty,
            stratum.accepted,
            stratum.rejected
        );
        self.draw_ok(Text::new(&st_line, Point::new(18, 124), VALUE_SM).draw(&mut self.display))?;
        Ok(())
    }

    fn draw_menu_body(&mut self, selected: MenuItem) -> Result<(), Error> {
        self.round_panel(8, 36, 304, 110, PANEL)?;
        self.draw_ok(
            Text::new("Options", Point::new(18, 55), LABEL).draw(&mut self.display),
        )?;

        for (i, item) in MenuItem::ALL.iter().enumerate() {
            let y = 78 + i as i32 * 30;
            let bg = if *item == selected { SELECT } else { PANEL };
            self.round_panel(18, y - 14, 284, 26, bg)?;
            let style = if *item == selected { OK } else { VALUE_SM };
            self.draw_ok(Text::new(item.label(), Point::new(28, y), style).draw(&mut self.display))?;
        }
        self.footer_hint("BOOT=select item  btn=activate")?;
        Ok(())
    }

    /// Password prompt screen while waiting on serial.
    pub fn draw_auth_prompt(&mut self, attempt: u8, max: u8) -> Result<(), Error> {
        self.wake_clear()?;
        self.last_screen = None;
        self.header_bar("AUTH")?;
        self.round_panel(8, 50, 304, 80, PANEL)?;
        self.draw_ok(
            Text::new("Enter current password", Point::new(40, 80), VALUE_SM)
                .draw(&mut self.display),
        )?;
        let mut tries: String<32> = String::new();
        let _ = write!(tries, "attempt {attempt}/{max}  (USB serial)");
        self.draw_ok(Text::new(&tries, Point::new(60, 110), LABEL).draw(&mut self.display))?;
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

impl<BUS, DC, WR> From<ParallelError<BUS, DC, WR>> for Error {
    fn from(e: ParallelError<BUS, DC, WR>) -> Self {
        match e {
            ParallelError::Bus(_) => Self::DisplayInterface("bus"),
            ParallelError::Dc(_) => Self::DisplayInterface("dc"),
            ParallelError::Wr(_) => Self::DisplayInterface("wr"),
        }
    }
}
