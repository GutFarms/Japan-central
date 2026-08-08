//! XPT2046 resistive touch on ESP32-2432S028 (CYD).
//!
//! Pins (dedicated bus, not shared with TFT):
//! CLK=25, MOSI=32, MISO=39, CS=33, IRQ=36
//!
//! GPIO bitbang at ~250 kHz (Mode 0), ESPHome-style 24-bit ADC framing.
//! Contact = Z pressure **or** PENIRQ low. Landscape mapping is cycleable
//! via [`Touch::cycle_map`] (BOOT during setup) for panel variants.

use embedded_hal::delay::DelayNs;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

use crate::keyboard::TouchPoint;

/// Common CYD landscape (display Rotation::Deg90) calibration.
const RAW_X_MIN: i32 = 200;
const RAW_X_MAX: i32 = 3900;
const RAW_Y_MIN: i32 = 200;
const RAW_Y_MAX: i32 = 3900;

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 240;
/// Primary Z threshold (ESPHome CYD demos use ~400; we allow softer presses).
const Z_THRESHOLD: u16 = 200;
/// When PENIRQ is active, accept a weaker Z (noisy panels / light press).
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
    /// Default for many CYD + Deg90 setups (ESPHome yellowtft1 + invert X).
    pub const CYD_DEG90: Self = Self {
        swap_xy: false,
        invert_x: true,
        invert_y: false,
    };

    /// Alternate used when swap_xy is needed (some panels / rotations).
    pub const CYD_DEG90_SWAP: Self = Self {
        swap_xy: true,
        invert_x: true,
        invert_y: true,
    };

    pub fn next(self) -> Self {
        match (self.swap_xy, self.invert_x, self.invert_y) {
            (false, true, false) => Self::CYD_DEG90_SWAP,
            (true, true, true) => Self {
                swap_xy: false,
                invert_x: false,
                invert_y: true,
            },
            (false, false, true) => Self {
                swap_xy: true,
                invert_x: false,
                invert_y: false,
            },
            _ => Self::CYD_DEG90,
        }
    }

    pub fn label(self) -> &'static str {
        match (self.swap_xy, self.invert_x, self.invert_y) {
            (false, true, false) => "map A ix",
            (true, true, true) => "map B swap",
            (false, false, true) => "map C iy",
            (true, false, false) => "map D sw",
            _ => "map ?",
        }
    }
}

pub struct Touch {
    clk: Output<'static>,
    mosi: Output<'static>,
    miso: Input<'static>,
    cs: Output<'static>,
    irq: Input<'static>,
    last: Option<TouchPoint>,
    down: bool,
    pub map: TouchMap,
    /// (x_raw, y_raw, z, irq_low)
    pub last_raw: Option<(u16, u16, u16, bool)>,
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
        let mut t = Self {
            clk: Output::new(p.clk, Level::Low, OutputConfig::default()),
            mosi: Output::new(p.mosi, Level::Low, OutputConfig::default()),
            miso: Input::new(p.miso, InputConfig::default().with_pull(Pull::None)),
            cs: Output::new(p.cs, Level::High, OutputConfig::default()),
            // GPIO36 has no internal pull; XPT2046 pulls PENIRQ when PD0=0.
            irq: Input::new(p.irq, InputConfig::default().with_pull(Pull::None)),
            last: None,
            down: false,
            map: TouchMap::CYD_DEG90,
            last_raw: None,
        };
        // Enable PENIRQ (PD=00) with a short settle.
        t.cs.set_low();
        let _ = t.xfer24_spin(CMD_PD);
        t.cs.set_high();
        t
    }

    pub fn pressed_raw(&self) -> bool {
        self.irq.is_low()
    }

    pub fn cycle_map(&mut self) {
        self.map = self.map.next();
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
        delay.delay_us(10);

        let z1 = self.xfer24(CMD_Z1, delay);
        let z2 = self.xfer24(CMD_Z2, delay);
        let z = pressure(z1, z2);

        // Prefer Z; also accept light press when PENIRQ is asserted.
        let contact = z >= Z_THRESHOLD || (irq_low && z >= Z_IRQ_THRESHOLD);
        if !contact {
            let _ = self.xfer24(CMD_PD, delay);
            self.cs.set_high();
            delay.delay_us(2);
            self.last_raw = Some((0, 0, z, irq_low));
            return None;
        }

        // ESPHome sequence: dummy X, then Y/X/Y/X/Y, last with power-down.
        let _ = self.xfer24(CMD_X, delay);
        let mut ys = [0u16; 3];
        let mut xs = [0u16; 3];
        ys[0] = self.xfer24(CMD_Y, delay);
        xs[0] = self.xfer24(CMD_X, delay);
        ys[1] = self.xfer24(CMD_Y, delay);
        xs[1] = self.xfer24(CMD_X, delay);
        ys[2] = self.xfer24(CMD_Y, delay);
        xs[2] = self.xfer24(CMD_PD, delay);

        self.cs.set_high();
        delay.delay_us(2);

        let x_raw = best_two_avg(xs[0], xs[1], xs[2]);
        let y_raw = best_two_avg(ys[0], ys[1], ys[2]);
        self.last_raw = Some((x_raw, y_raw, z, irq_low));

        // Dead bus: MISO stuck low → zeros. Reject.
        if x_raw < 40 && y_raw < 40 {
            return None;
        }
        // MISO stuck high / open → near full-scale on both.
        if x_raw > 4080 && y_raw > 4080 {
            return None;
        }

        Some(map_raw(x_raw, y_raw, self.map))
    }

    /// 24-bit full-duplex bitbang with ~250 kHz clock (Mode 0).
    fn xfer24<D: DelayNs>(&mut self, cmd: u8, delay: &mut D) -> u16 {
        let mut buf = [cmd, 0u8, 0u8];
        for b in &mut buf {
            let mut send = *b;
            let mut recv = 0u8;
            for _ in 0..8 {
                if send & 0x80 != 0 {
                    self.mosi.set_high();
                } else {
                    self.mosi.set_low();
                }
                send <<= 1;
                delay.delay_us(1);
                self.clk.set_high();
                delay.delay_us(1);
                recv <<= 1;
                if self.miso.is_high() {
                    recv |= 1;
                }
                self.clk.set_low();
                delay.delay_us(1);
            }
            *b = recv;
        }
        (u16::from(buf[1]) << 8 | u16::from(buf[2])) >> 3
    }

    /// Spin-only transfer for early init before a Delay is available.
    fn xfer24_spin(&mut self, cmd: u8) -> u16 {
        let mut buf = [cmd, 0u8, 0u8];
        for b in &mut buf {
            let mut send = *b;
            let mut recv = 0u8;
            for _ in 0..8 {
                if send & 0x80 != 0 {
                    self.mosi.set_high();
                } else {
                    self.mosi.set_low();
                }
                send <<= 1;
                for _ in 0..40 {
                    core::hint::spin_loop();
                }
                self.clk.set_high();
                for _ in 0..40 {
                    core::hint::spin_loop();
                }
                recv <<= 1;
                if self.miso.is_high() {
                    recv |= 1;
                }
                self.clk.set_low();
                for _ in 0..40 {
                    core::hint::spin_loop();
                }
            }
            *b = recv;
        }
        (u16::from(buf[1]) << 8 | u16::from(buf[2])) >> 3
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
        let p = map_raw(2000, 2000, TouchMap::CYD_DEG90);
        assert!(p.x < 320 && p.y < 240);
    }

    #[test]
    fn map_cycles() {
        let m = TouchMap::CYD_DEG90.next();
        assert!(m.swap_xy);
        assert_eq!(TouchMap::CYD_DEG90.label(), "map A ix");
    }

    #[test]
    fn best_two_avg_picks_closest_pair() {
        assert_eq!(best_two_avg(100, 102, 500), 101);
    }
}
