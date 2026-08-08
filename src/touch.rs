//! XPT2046 resistive touch on ESP32-2432S028 (CYD).
//!
//! Dedicated **VSPI / SPI3** bus (separate from TFT HSPI):
//! CLK=25, MOSI=32, MISO=39, CS=33, IRQ=36
//!
//! Protocol matches ESPHome `xpt2046` (24-bit full-duplex ADC reads) and
//! calibration matches the common CYD ESPHome landscape profile.

use embedded_hal::delay::DelayNs;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;
use esp_hal::Blocking;

use crate::keyboard::TouchPoint;

/// ESPHome CYD landscape calibration (rotation 90°).
/// `x_min > x_max` in ESPHome ≡ invert X here.
const RAW_X_MIN: i32 = 280;
const RAW_X_MAX: i32 = 3860;
const RAW_Y_MIN: i32 = 340;
const RAW_Y_MAX: i32 = 3860;
const INVERT_X: bool = true;
const INVERT_Y: bool = false;
const SWAP_XY: bool = false;

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 240;
/// ESPHome CYD demos use ~400.
const Z_THRESHOLD: u16 = 400;

const CMD_Z1: u8 = 0xB1; // Z1 + ADC on
const CMD_Z2: u8 = 0xC1; // Z2 + ADC on
const CMD_X: u8 = 0xD1; // X + ADC on
const CMD_Y: u8 = 0x91; // Y + ADC on
const CMD_X_POWERDOWN: u8 = 0xD0; // X, PD=00 → enable PENIRQ

pub struct Touch {
    spi: Spi<'static, Blocking>,
    cs: Output<'static>,
    irq: Input<'static>,
    last: Option<TouchPoint>,
    down: bool,
    /// Last raw sample for serial debug.
    pub last_raw: Option<(u16, u16, u16)>,
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
        // 2 MHz Mode 0 — same as Paul Stoffregen / typical CYD Arduino setups.
        let spi = Spi::new(
            p.spi,
            SpiConfig::default()
                .with_frequency(Rate::from_mhz(2))
                .with_mode(SpiMode::_0),
        )
        .expect("touch SPI3")
        .with_sck(p.clk)
        .with_mosi(p.mosi)
        .with_miso(p.miso);

        let mut t = Self {
            spi,
            cs: Output::new(p.cs, Level::High, OutputConfig::default()),
            // GPIO36 has no internal pull; XPT2046 pulls PENIRQ up when PD0=0.
            irq: Input::new(p.irq, InputConfig::default().with_pull(Pull::None)),
            last: None,
            down: false,
            last_raw: None,
        };
        // Power-down ADC / enable PENIRQ (ESPHome setup).
        t.cs.set_low();
        let _ = t.read_adc(CMD_X_POWERDOWN);
        t.cs.set_high();
        t
    }

    pub fn pressed_raw(&self) -> bool {
        self.irq.is_low()
    }

    /// Poll once. Returns a point only on **press edge** (tap), not while held.
    pub fn poll_tap<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let sample = self.read_sample(delay);
        // IRQ high ⇒ definitely released (avoids stuck-down when Z is noisy).
        if !self.irq.is_low() && sample.is_none() {
            self.down = false;
            return None;
        }
        match (self.down, sample) {
            (false, Some(p)) => {
                self.down = true;
                self.last = Some(p);
                Some(p)
            }
            (true, None) => {
                self.down = false;
                None
            }
            (true, Some(p)) => {
                self.last = Some(p);
                None
            }
            (false, None) => None,
        }
    }

    pub fn poll_point<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let sample = self.read_sample(delay);
        if !self.irq.is_low() && sample.is_none() {
            self.down = false;
            return None;
        }
        self.down = sample.is_some();
        if let Some(p) = sample {
            self.last = Some(p);
        }
        sample
    }

    fn read_sample<D: DelayNs>(&mut self, _delay: &mut D) -> Option<TouchPoint> {
        self.cs.set_low();

        let z1 = self.read_adc(CMD_Z1);
        let z2 = self.read_adc(CMD_Z2);
        let z = pressure(z1, z2);

        if z < Z_THRESHOLD {
            let _ = self.read_adc(CMD_X_POWERDOWN);
            self.cs.set_high();
            self.last_raw = Some((0, 0, z));
            return None;
        }

        // ESPHome: dummy X, then Y/X/Y/X/Y, last X with power-down.
        let _ = self.read_adc(CMD_X);
        let mut ys = [0u16; 3];
        let mut xs = [0u16; 3];
        ys[0] = self.read_adc(CMD_Y);
        xs[0] = self.read_adc(CMD_X);
        ys[1] = self.read_adc(CMD_Y);
        xs[1] = self.read_adc(CMD_X);
        ys[2] = self.read_adc(CMD_Y);
        xs[2] = self.read_adc(CMD_X_POWERDOWN); // also enables PENIRQ

        self.cs.set_high();

        let x_raw = best_two_avg(xs[0], xs[1], xs[2]);
        let y_raw = best_two_avg(ys[0], ys[1], ys[2]);
        self.last_raw = Some((x_raw, y_raw, z));

        // Reject dead bus (MISO stuck low → zeros) or open-circuit junk.
        if x_raw < 50 || y_raw < 50 || x_raw > 4090 || y_raw > 4090 {
            return None;
        }

        Some(map_raw(x_raw, y_raw))
    }

    /// ESPHome-style 24-bit full-duplex transfer: cmd + 16 clocks → 12-bit ADC.
    fn read_adc(&mut self, cmd: u8) -> u16 {
        let mut data = [cmd, 0, 0];
        if self.spi.transfer(&mut data).is_err() {
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

fn map_raw(x_raw: u16, y_raw: u16) -> TouchPoint {
    let (mut rx, mut ry) = (x_raw as i32, y_raw as i32);
    if SWAP_XY {
        core::mem::swap(&mut rx, &mut ry);
    }
    let mut x = ((rx - RAW_X_MIN) * (SCREEN_W - 1)) / (RAW_X_MAX - RAW_X_MIN).max(1);
    let mut y = ((ry - RAW_Y_MIN) * (SCREEN_H - 1)) / (RAW_Y_MAX - RAW_Y_MIN).max(1);
    if INVERT_X {
        x = (SCREEN_W - 1) - x;
    }
    if INVERT_Y {
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
    fn pressure_formula_matches_reference() {
        assert!(pressure(2000, 500) > Z_THRESHOLD);
        assert_eq!(pressure(100, 4000), 0);
        assert_eq!(pressure(0, 0), 4095);
    }

    #[test]
    fn map_clamps_to_screen() {
        let p = map_raw(0, 0);
        assert!(p.x < 320 && p.y < 240);
        let p2 = map_raw(4095, 4095);
        assert!(p2.x < 320 && p2.y < 240);
    }

    #[test]
    fn best_two_avg_picks_closest_pair() {
        assert_eq!(best_two_avg(100, 102, 500), 101);
    }
}
