//! XPT2046 resistive touch on ESP32-2432S028 (CYD).
//!
//! Dedicated pins (separate from TFT SPI):
//! CLK=25, MOSI=32, MISO=39, CS=33, IRQ=36

use embedded_hal::delay::DelayNs;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

use crate::keyboard::TouchPoint;


/// Calibration for landscape Deg90 / common CYD panels.
/// Tweak if taps land off-target on a specific unit.
const RAW_X_MIN: i32 = 280;
const RAW_X_MAX: i32 = 3800;
const RAW_Y_MIN: i32 = 280;
const RAW_Y_MAX: i32 = 3800;
const INVERT_X: bool = true;
const INVERT_Y: bool = true;
const SWAP_XY: bool = false;

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 240;
const Z_THRESHOLD: u16 = 200;

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
        Self {
            clk: Output::new(p.clk, Level::Low, OutputConfig::default()),
            mosi: Output::new(p.mosi, Level::Low, OutputConfig::default()),
            miso: Input::new(p.miso, InputConfig::default().with_pull(Pull::Up)),
            cs: Output::new(p.cs, Level::High, OutputConfig::default()),
            irq: Input::new(p.irq, InputConfig::default().with_pull(Pull::Up)),
            last: None,
            down: false,
        }
    }

    /// True when panel reports contact (IRQ active-low when supported).
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
        // Prefer IRQ when wired; still sample if IRQ floats high on some boards.
        let irq_down = self.irq.is_low();

        self.cs.set_low();
        delay.delay_us(2);

        let z1 = self.xfer16(0xB1, delay); // Z1
        let z2 = self.xfer16(0xC1, delay); // Z2
        let z = if z2 > z1 { z2 - z1 } else { 0 };

        let mut xs = [0u16; 3];
        let mut ys = [0u16; 3];
        for i in 0..3 {
            xs[i] = self.xfer16(0x91, delay); // X
            ys[i] = self.xfer16(0xD1, delay); // Y
        }

        self.cs.set_high();
        delay.delay_us(2);

        let x_raw = median3(xs[0], xs[1], xs[2]);
        let y_raw = median3(ys[0], ys[1], ys[2]);

        let contact = z > Z_THRESHOLD || (irq_down && x_raw > 100 && y_raw > 100);
        if !contact {
            return None;
        }

        Some(map_raw(x_raw, y_raw))
    }

    fn xfer16<D: DelayNs>(&mut self, cmd: u8, delay: &mut D) -> u16 {
        self.write8(cmd, delay);
        // Extra clocks for conversion; read 12 data bits MSB-first after busy.
        delay.delay_us(6);
        let mut v = 0u16;
        for _ in 0..12 {
            self.clk.set_high();
            delay.delay_us(1);
            v <<= 1;
            if self.miso.is_high() {
                v |= 1;
            }
            self.clk.set_low();
            delay.delay_us(1);
        }
        // discard trailing bits
        for _ in 0..4 {
            self.clk.set_high();
            delay.delay_us(1);
            self.clk.set_low();
            delay.delay_us(1);
        }
        v
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

fn median3(a: u16, b: u16, c: u16) -> u16 {
    if (a <= b && b <= c) || (c <= b && b <= a) {
        b
    } else if (b <= a && a <= c) || (c <= a && a <= b) {
        a
    } else {
        c
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
    fn map_clamps_to_screen() {
        let p = map_raw(0, 0);
        assert!(p.x < 320 && p.y < 240);
        let p2 = map_raw(4095, 4095);
        assert!(p2.x < 320 && p2.y < 240);
    }
}
