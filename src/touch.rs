//! XPT2046 resistive touch on ESP32-2432S028 (CYD).
//!
//! Dedicated pins (separate from TFT SPI):
//! CLK=25, MOSI=32, MISO=39, CS=33, IRQ=36
//!
//! Contact detection follows the Paul Stoffregen / ESPHome pattern:
//! `z = z1 + 4095 - z2`, and the final ADC command uses PD=00 (`0xD0`)
//! so PENIRQ stays enabled between polls.

use embedded_hal::delay::DelayNs;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

use crate::keyboard::TouchPoint;

/// Calibration for landscape [`mipids_display::options::Rotation::Deg90`].
/// Tweak if taps land off-target on a specific unit.
const RAW_X_MIN: i32 = 200;
const RAW_X_MAX: i32 = 3700;
const RAW_Y_MIN: i32 = 240;
const RAW_Y_MAX: i32 = 3800;
/// Match common CYD + Deg90 mapping (ESPHome / TFT_eSPI-style).
const INVERT_X: bool = true;
const INVERT_Y: bool = false;
const SWAP_XY: bool = false;

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 240;
const Z_THRESHOLD: u16 = 300;

pub struct Touch {
    clk: Output<'static>,
    mosi: Output<'static>,
    miso: Input<'static>,
    cs: Output<'static>,
    irq: Input<'static>,
    last: Option<TouchPoint>,
    down: bool,
}

pub struct TouchPins {
    pub clk: esp_hal::gpio::AnyPin<'static>,
    pub mosi: esp_hal::gpio::AnyPin<'static>,
    pub miso: esp_hal::gpio::AnyPin<'static>,
    pub cs: esp_hal::gpio::AnyPin<'static>,
    pub irq: esp_hal::gpio::AnyPin<'static>,
}

impl Touch {
    pub fn new(p: TouchPins) -> Self {
        // GPIO36/39 are input-only and have no internal pulls; leave floating.
        Self {
            clk: Output::new(p.clk, Level::Low, OutputConfig::default()),
            mosi: Output::new(p.mosi, Level::Low, OutputConfig::default()),
            miso: Input::new(p.miso, InputConfig::default().with_pull(Pull::None)),
            cs: Output::new(p.cs, Level::High, OutputConfig::default()),
            irq: Input::new(p.irq, InputConfig::default().with_pull(Pull::None)),
            last: None,
            down: false,
        }
    }

    /// True when panel reports contact (IRQ active-low when PENIRQ enabled).
    pub fn pressed_raw(&self) -> bool {
        self.irq.is_low()
    }

    /// Poll once. Returns a point only on **press edge** (tap), not while held.
    pub fn poll_tap<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let sample = self.read_sample(delay);
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

    /// Continuous sample while pressed (for drawing feedback).
    pub fn poll_point<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        let sample = self.read_sample(delay);
        self.down = sample.is_some();
        if let Some(p) = sample {
            self.last = Some(p);
        }
        sample
    }

    fn read_sample<D: DelayNs>(&mut self, delay: &mut D) -> Option<TouchPoint> {
        self.cs.set_low();
        delay.delay_us(2);

        // Keep ADC on (PD=01) during the sequence; finish with 0xD0 (PD=00)
        // so PENIRQ works again for the next poll.
        let z1 = self.read_adc(0xB1, delay);
        let z2 = self.read_adc(0xC1, delay);
        let z = pressure(z1, z2);

        if z < Z_THRESHOLD {
            let _ = self.read_adc(0xD0, delay);
            self.cs.set_high();
            delay.delay_us(2);
            return None;
        }

        // First sample is noisy; discard, then oversample. Command labels follow
        // Stoffregen: 0x91 / 0xD1. Screen X comes from the 0xD* reads, Y from 0x9*.
        let _ = self.read_adc(0x91, delay);
        let mut d1 = [0u16; 3]; // 0xD1 / 0xD0 → screen X (rotation 1)
        let mut d9 = [0u16; 3]; // 0x91 / trailing → screen Y
        for i in 0..2 {
            d1[i] = self.read_adc(0xD1, delay);
            d9[i] = self.read_adc(0x91, delay);
        }
        // Final conversion with power-down (re-enable PENIRQ), then clock out last axis.
        d1[2] = self.read_adc(0xD0, delay);
        d9[2] = self.read_adc(0x00, delay);

        self.cs.set_high();
        delay.delay_us(2);

        let x_raw = best_two_avg(d1[0], d1[1], d1[2]);
        let y_raw = best_two_avg(d9[0], d9[1], d9[2]);
        if x_raw < 50 || y_raw < 50 {
            return None;
        }

        Some(map_raw(x_raw, y_raw))
    }

    /// Write an 8-bit command, then clock 16 bits and return the 12-bit ADC (`>> 3`).
    fn read_adc<D: DelayNs>(&mut self, cmd: u8, delay: &mut D) -> u16 {
        self.write8(cmd, delay);
        // Conversion time; datasheet ~3µs typical with internal clock.
        delay.delay_us(6);
        let mut v = 0u16;
        for _ in 0..16 {
            self.clk.set_high();
            delay.delay_us(1);
            v <<= 1;
            if self.miso.is_high() {
                v |= 1;
            }
            self.clk.set_low();
            delay.delay_us(1);
        }
        v >> 3
    }

    fn write8<D: DelayNs>(&mut self, mut byte: u8, delay: &mut D) {
        for _ in 0..8 {
            if byte & 0x80 != 0 {
                self.mosi.set_high();
            } else {
                self.mosi.set_low();
            }
            byte <<= 1;
            self.clk.set_high();
            delay.delay_us(1);
            self.clk.set_low();
            delay.delay_us(1);
        }
    }
}

/// XPT2046 touch pressure: `z1 + 4095 - z2`.
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
        // Strong press: z1 high, z2 lower → large z.
        assert!(pressure(2000, 500) > Z_THRESHOLD);
        // No touch / open: z1≈0, z2≈0 → z≈4095 is "max" but real idle is
        // often small; verify inverted formula is not used.
        assert_eq!(pressure(100, 4000), 0); // saturating_sub
        assert_eq!(pressure(0, 0), 4095);
        assert_eq!(pressure(1000, 1000), 4095);
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
        assert_eq!(best_two_avg(10, 400, 402), 401);
    }
}
