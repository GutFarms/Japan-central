//! XPT2046 resistive touch on ESP32-2432S028 (CYD).
//!
//! Dedicated **VSPI / SPI3** bus (separate from TFT HSPI):
//! CLK=25, MOSI=32, MISO=39, CS=33, IRQ=36
//!
//! 1 MHz Mode 0 + ESPHome-style 24-bit ADC framing. Taps fire on **release**
//! (more reliable on resistive glass). Axis map is cycleable via BOOT.

use embedded_hal::delay::DelayNs;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;
use esp_hal::Blocking;

use crate::keyboard::TouchPoint;

/// Common CYD landscape (display Rotation::Deg90) calibration.
const RAW_X_MIN: i32 = 280;
const RAW_X_MAX: i32 = 3860;
const RAW_Y_MIN: i32 = 340;
const RAW_Y_MAX: i32 = 3860;

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 240;
/// Soft enough for light presses; ESPHome demos use ~400.
const Z_THRESHOLD: u16 = 180;
/// When PENIRQ is active, accept a weaker Z.
const Z_IRQ_THRESHOLD: u16 = 40;

const CMD_Z1: u8 = 0xB1;
const CMD_Z2: u8 = 0xC1;
const CMD_X: u8 = 0xD1;
const CMD_Y: u8 = 0x91;
const CMD_PD: u8 = 0xD0;

/// How raw axes map onto landscape screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TouchMap {
    pub swap_xy: bool,
    pub invert_x: bool,
    pub invert_y: bool,
}

impl TouchMap {
    /// ESPHome yellowtft1 (invert X, no swap).
    pub const CYD_DEG90: Self = Self {
        swap_xy: false,
        invert_x: true,
        invert_y: false,
    };

    /// Common on some 2432S028 panels (swap + invert both).
    pub const CYD_DEG90_SWAP: Self = Self {
        swap_xy: true,
        invert_x: true,
        invert_y: true,
    };

    /// Default: swapped axes — matches many GUITION / Sunton boards.
    pub const DEFAULT: Self = Self::CYD_DEG90_SWAP;

    pub fn next(self) -> Self {
        match (self.swap_xy, self.invert_x, self.invert_y) {
            (true, true, true) => Self::CYD_DEG90,
            (false, true, false) => Self {
                swap_xy: false,
                invert_x: false,
                invert_y: true,
            },
            (false, false, true) => Self {
                swap_xy: true,
                invert_x: false,
                invert_y: false,
            },
            _ => Self::CYD_DEG90_SWAP,
        }
    }

    pub fn label(self) -> &'static str {
        match (self.swap_xy, self.invert_x, self.invert_y) {
            (true, true, true) => "map A swap",
            (false, true, false) => "map B ix",
            (false, false, true) => "map C iy",
            (true, false, false) => "map D sw",
            _ => "map ?",
        }
    }
}

pub struct Touch {
    spi: Spi<'static, Blocking>,
    cs: Output<'static>,
    irq: Input<'static>,
    last: Option<TouchPoint>,
    down: bool,
    pub map: TouchMap,
    /// (x_raw, y_raw, z, irq_low)
    pub last_raw: Option<(u16, u16, u16, bool)>,
    /// SPI transfer errors since boot (diagnostic).
    pub xfer_errors: u32,
}

pub struct TouchPins {
    pub spi: esp_hal::peripherals::SPI3<'static>,
    pub clk: esp_hal::gpio::AnyPin<'static>,
    pub mosi: esp_hal::gpio::AnyPin<'static>,
    pub miso: esp_hal::gpio::AnyPin<'static>,
    pub cs: esp_hal::gpio::AnyPin<'static>,
    pub irq: esp_hal::gpio::AnyPin<'static>,
}

impl Touch {
    pub fn new(p: TouchPins) -> Self {
        // 1 MHz Mode 0 — ESPHome / LovyanGFX CYD profiles.
        let spi = Spi::new(
            p.spi,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(1))
                .with_mode(SpiMode::_0),
        )
        .expect("touch SPI3")
        .with_sck(p.clk)
        .with_mosi(p.mosi)
        .with_miso(p.miso);

        let mut t = Self {
            spi,
            cs: Output::new(p.cs, Level::High, OutputConfig::default()),
            irq: Input::new(p.irq, InputConfig::default().with_pull(Pull::None)),
            last: None,
            down: false,
            map: TouchMap::DEFAULT,
            last_raw: None,
            xfer_errors: 0,
        };
        t.cs.set_low();
        let _ = t.read_adc(CMD_PD);
        t.cs.set_high();
        t
    }

    pub fn pressed_raw(&self) -> bool {
        self.irq.is_low()
    }

    pub fn is_down(&self) -> bool {
        self.down
    }

    pub fn last_point(&self) -> Option<TouchPoint> {
        self.last
    }

    pub fn cycle_map(&mut self) {
        self.map = self.map.next();
    }

    /// Poll once. Returns a point on **release edge** (finger up), not press.
    pub fn poll_tap<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let sample = self.read_sample(delay);
        match (self.down, sample) {
            (false, Some(p)) => {
                self.down = true;
                self.last = Some(p);
                None
            }
            (true, None) => {
                self.down = false;
                self.last
            }
            (true, Some(p)) => {
                self.last = Some(p);
                None
            }
            (false, None) => None,
        }
    }

    /// Continuous sample while pressed.
    pub fn poll_point<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let sample = self.read_sample(delay);
        self.down = sample.is_some();
        if let Some(p) = sample {
            self.last = Some(p);
        }
        sample
    }

    fn read_sample<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let irq_low = self.irq.is_low();

        self.cs.set_low();
        delay.delay_us(5);

        let z1 = self.read_adc(CMD_Z1);
        let z2 = self.read_adc(CMD_Z2);
        let z = pressure(z1, z2);

        let contact = z >= Z_THRESHOLD || (irq_low && z >= Z_IRQ_THRESHOLD);
        if !contact {
            let _ = self.read_adc(CMD_PD);
            self.cs.set_high();
            delay.delay_us(2);
            self.last_raw = Some((0, 0, z, irq_low));
            return None;
        }

        let _ = self.read_adc(CMD_X);
        let mut ys = [0u16; 3];
        let mut xs = [0u16; 3];
        ys[0] = self.read_adc(CMD_Y);
        xs[0] = self.read_adc(CMD_X);
        ys[1] = self.read_adc(CMD_Y);
        xs[1] = self.read_adc(CMD_X);
        ys[2] = self.read_adc(CMD_Y);
        xs[2] = self.read_adc(CMD_PD);

        self.cs.set_high();
        delay.delay_us(2);

        let x_raw = best_two_avg(xs[0], xs[1], xs[2]);
        let y_raw = best_two_avg(ys[0], ys[1], ys[2]);
        self.last_raw = Some((x_raw, y_raw, z, irq_low));

        if x_raw < 40 && y_raw < 40 {
            return None;
        }
        if x_raw > 4080 && y_raw > 4080 {
            return None;
        }

        Some(map_raw(x_raw, y_raw, self.map))
    }

    fn read_adc(&mut self, cmd: u8) -> u16 {
        let mut data = [cmd, 0, 0];
        if self.spi.transfer(&mut data).is_err() {
            self.xfer_errors = self.xfer_errors.saturating_add(1);
            return 0;
        }
        (u16::from(data[1]) << 8 | u16::from(data[2])) >> 3
    }
}

fn pressure(z1: u16, z2: u16) -> u16 {
    z1.saturating_add(4095).saturating_sub(z2)
}

fn best_two_avg(a: u16, b: u16, c: u16) -> u16 {
    let da = a.abs_diff(b);
    let db = a.abs_diff(c);
    let dc = b.abs_diff(c);
    if da <= db && da <= dc {
        ((u32::from(a) + u32::from(b)) / 2) as u16
    } else if db <= da && db <= dc {
        ((u32::from(a) + u32::from(c)) / 2) as u16
    } else {
        ((u32::from(b) + u32::from(c)) / 2) as u16
    }
}

fn map_raw(x_raw: u16, y_raw: u16, map: TouchMap) -> TouchPoint {
    let (mut rx, mut ry) = (x_raw as i32, y_raw as i32);
    if map.swap_xy {
        core::mem::swap(&mut rx, &mut ry);
    }
    let mut x = ((rx - RAW_X_MIN) * (SCREEN_W - 1)) / (RAW_X_MAX - RAW_X_MIN).max(1);
    let mut y = ((ry - RAW_Y_MIN) * (SCREEN_H - 1)) / (RAW_Y_MAX - RAW_Y_MIN).max(1);
    if map.invert_x {
        x = (SCREEN_W - 1) - x;
    }
    if map.invert_y {
        y = (SCREEN_H - 1) - y;
    }
    TouchPoint {
        x: x.clamp(0, SCREEN_W - 1) as u16,
        y: y.clamp(0, SCREEN_H - 1) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_and_map() {
        assert!(pressure(2000, 500) > Z_THRESHOLD);
        let p = map_raw(2000, 2000, TouchMap::DEFAULT);
        assert!(p.x < 320 && p.y < 240);
    }

    #[test]
    fn map_cycles_from_default() {
        assert!(TouchMap::DEFAULT.swap_xy);
        let m = TouchMap::DEFAULT.next();
        assert!(!m.swap_xy);
        assert_eq!(TouchMap::DEFAULT.label(), "map A swap");
    }

    #[test]
    fn best_two_avg_picks_closest_pair() {
        assert_eq!(best_two_avg(100, 102, 500), 101);
    }
}
