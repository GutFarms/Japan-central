//! LilyGO T-Display-S3 (ST7789, 320×170, 8-bit parallel) mining UI.

use core::fmt::Write as _;

use embedded_graphics::Drawable;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_6X12};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::prelude::{Primitive, WebColors};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use embedded_hal::delay::DelayNs;
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};
use heapless::String;
use mipidsi::interface::{Generic8BitBus, ParallelError, ParallelInterface};
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};
use mipidsi::{Builder, Display as MipiDisplay};

use crate::config::{PoolConfig, SetupField};
use crate::miner::{hash_to_hex, MinerStats, SCRYPT_LOG_N, SCRYPT_N};

pub const DISPLAY_WIDTH: u16 = 320;
pub const DISPLAY_HEIGHT: u16 = 170;

const BRAND_STYLE: MonoTextStyle<'_, Rgb565> =
    MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_ORANGE);
const LABEL_STYLE: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_6X12, Rgb565::CSS_GRAY);
const VALUE_STYLE: MonoTextStyle<'_, Rgb565> = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
const SHARE_STYLE: MonoTextStyle<'_, Rgb565> =
    MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_LIMEGREEN);
const ACCENT: Rgb565 = Rgb565::CSS_DARK_ORANGE;

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
    /// Kept alive so LCD power / bus control lines stay driven.
    _power_en: Output<'a>,
    _cs: Output<'a>,
    _rd: Output<'a>,
    delay: D,
    ready: bool,
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

        // Chip-select active, RD held high (write-only path).
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
            ready: false,
        })
    }

    fn ensure_ready(&mut self) -> Result<(), Error> {
        if self.ready {
            return Ok(());
        }
        self.backlight.set_high();
        self.display
            .wake(&mut self.delay)
            .map_err(|_| Error::InitError)?;
        self.display
            .clear(Rgb565::BLACK)
            .map_err(|_| Error::DisplayInterface("clear"))?;

        // Accent bar
        self.draw_ok(
            Rectangle::new(Point::new(0, 0), Size::new(DISPLAY_WIDTH as u32, 4))
                .into_styled(PrimitiveStyle::with_fill(ACCENT))
                .draw(&mut self.display),
        )?;

        self.draw_ok(Text::new("SCRYPT", Point::new(12, 22), BRAND_STYLE).draw(&mut self.display))?;

        let mut params: String<48> = String::new();
        let _ = write!(
            params,
            "ESP32-S3  N={} (2^{})  r=1 p=1",
            SCRYPT_N, SCRYPT_LOG_N
        );
        self.draw_ok(Text::new(&params, Point::new(100, 26), LABEL_STYLE).draw(&mut self.display))?;

        self.draw_ok(Text::new("rate", Point::new(12, 52), LABEL_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new("nonce", Point::new(12, 82), LABEL_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new("shares", Point::new(12, 112), LABEL_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new("best", Point::new(12, 142), LABEL_STYLE).draw(&mut self.display))?;

        self.ready = true;
        Ok(())
    }

    fn clear_value(&mut self, y: i32, x: i32, w: u32) -> Result<(), Error> {
        self.draw_ok(
            Rectangle::new(Point::new(x, y), Size::new(w, 22))
                .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
                .draw(&mut self.display),
        )
    }

    /// Full-screen post-boot setup prompt for one credential field.
    pub fn draw_setup(&mut self, field: SetupField, typed: &str) -> Result<(), Error> {
        self.backlight.set_high();
        self.display
            .wake(&mut self.delay)
            .map_err(|_| Error::InitError)?;
        self.display
            .clear(Rgb565::BLACK)
            .map_err(|_| Error::DisplayInterface("clear"))?;
        self.ready = false;

        self.draw_ok(
            Rectangle::new(Point::new(0, 0), Size::new(DISPLAY_WIDTH as u32, 4))
                .into_styled(PrimitiveStyle::with_fill(ACCENT))
                .draw(&mut self.display),
        )?;

        self.draw_ok(Text::new("SCRYPT", Point::new(12, 22), BRAND_STYLE).draw(&mut self.display))?;
        self.draw_ok(
            Text::new("SETUP", Point::new(240, 22), SHARE_STYLE).draw(&mut self.display),
        )?;

        let mut step: String<32> = String::new();
        let step_n = match field {
            SetupField::Address => 1,
            SetupField::Password => 2,
            SetupField::Stratum => 3,
        };
        let _ = write!(step, "step {step_n}/3  {}", field.label());
        self.draw_ok(Text::new(&step, Point::new(12, 52), LABEL_STYLE).draw(&mut self.display))?;

        self.draw_ok(
            Text::new(field.prompt(), Point::new(12, 78), VALUE_STYLE).draw(&mut self.display),
        )?;

        self.draw_ok(
            Text::new("Enter via USB serial, then Enter", Point::new(12, 110), LABEL_STYLE)
                .draw(&mut self.display),
        )?;

        let shown = if field == SetupField::Password && !typed.is_empty() {
            PoolConfig::ellipsize("********", 40)
        } else {
            PoolConfig::ellipsize(typed, 40)
        };
        self.draw_ok(Text::new(&shown, Point::new(12, 140), VALUE_STYLE).draw(&mut self.display))?;

        Ok(())
    }

    /// Summary of credentials before mining starts.
    /// `from_flash` shows whether values were restored from saved storage.
    pub fn draw_config_summary(&mut self, cfg: &PoolConfig, from_flash: bool) -> Result<(), Error> {
        self.backlight.set_high();
        self.display
            .wake(&mut self.delay)
            .map_err(|_| Error::InitError)?;
        self.display
            .clear(Rgb565::BLACK)
            .map_err(|_| Error::DisplayInterface("clear"))?;
        self.ready = false;

        self.draw_ok(
            Rectangle::new(Point::new(0, 0), Size::new(DISPLAY_WIDTH as u32, 4))
                .into_styled(PrimitiveStyle::with_fill(ACCENT))
                .draw(&mut self.display),
        )?;
        self.draw_ok(Text::new("SCRYPT", Point::new(12, 22), BRAND_STYLE).draw(&mut self.display))?;
        let badge = if from_flash { "SAVED" } else { "READY" };
        self.draw_ok(Text::new(badge, Point::new(240, 22), SHARE_STYLE).draw(&mut self.display))?;

        let addr = PoolConfig::ellipsize(cfg.address.as_str(), 28);
        let pass = cfg.password_masked();
        let stratum = PoolConfig::ellipsize(cfg.stratum.as_str(), 28);

        self.draw_ok(Text::new("address", Point::new(12, 50), LABEL_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new(&addr, Point::new(80, 50), VALUE_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new("password", Point::new(12, 80), LABEL_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new(&pass, Point::new(80, 80), VALUE_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new("stratum", Point::new(12, 110), LABEL_STYLE).draw(&mut self.display))?;
        self.draw_ok(Text::new(&stratum, Point::new(80, 110), VALUE_STYLE).draw(&mut self.display))?;

        let hint = if from_flash {
            "serial: change = edit (needs password)"
        } else {
            "saved to flash for next boot"
        };
        self.draw_ok(Text::new(hint, Point::new(12, 145), LABEL_STYLE).draw(&mut self.display))?;

        Ok(())
    }

    /// Redraw dynamic mining stats (includes truncated address / stratum).
    pub fn draw_stats(
        &mut self,
        stats: &MinerStats,
        cfg: &PoolConfig,
        mining: bool,
    ) -> Result<(), Error> {
        self.ensure_ready()?;

        let mut rate: String<32> = String::new();
        let whole = stats.hashrate_x100 / 100;
        let frac = stats.hashrate_x100 % 100;
        let _ = write!(rate, "{}.{:02} H/s", whole, frac);

        let mut nonce: String<32> = String::new();
        let _ = write!(nonce, "{:08x}", stats.nonce);

        let mut shares: String<32> = String::new();
        let _ = write!(shares, "{}", stats.shares);

        let mut best: String<128> = String::new();
        hash_to_hex(&stats.best_hash, 8, &mut best);

        let mut status: String<16> = String::new();
        if mining {
            let _ = write!(status, "MINING");
        } else {
            let _ = write!(status, "IDLE");
        }

        self.clear_value(48, 70, 160)?;
        self.draw_ok(Text::new(&rate, Point::new(70, 58), VALUE_STYLE).draw(&mut self.display))?;

        self.clear_value(48, 240, 70)?;
        let st_style = if mining { SHARE_STYLE } else { LABEL_STYLE };
        self.draw_ok(Text::new(&status, Point::new(240, 58), st_style).draw(&mut self.display))?;

        self.clear_value(78, 70, 200)?;
        self.draw_ok(Text::new(&nonce, Point::new(70, 88), VALUE_STYLE).draw(&mut self.display))?;

        self.clear_value(108, 70, 120)?;
        let share_style = if stats.shares > 0 {
            SHARE_STYLE
        } else {
            VALUE_STYLE
        };
        self.draw_ok(Text::new(&shares, Point::new(70, 118), share_style).draw(&mut self.display))?;

        let addr = PoolConfig::ellipsize(cfg.address.as_str(), 14);
        self.clear_value(108, 160, 150)?;
        self.draw_ok(Text::new(&addr, Point::new(160, 122), LABEL_STYLE).draw(&mut self.display))?;

        self.clear_value(138, 70, 100)?;
        self.draw_ok(Text::new(&best, Point::new(70, 148), VALUE_STYLE).draw(&mut self.display))?;

        let stratum = PoolConfig::ellipsize(cfg.stratum.as_str(), 18);
        self.clear_value(138, 180, 130)?;
        self.draw_ok(
            Text::new(&stratum, Point::new(180, 152), LABEL_STYLE).draw(&mut self.display),
        )?;

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
