//! ESP32-S3 scrypt miner firmware for LilyGO T-Display-S3.
//!
//! After boot, enter wallet **address**, pool **password**, and **stratum**
//! location one at a time over USB serial, then mining starts.
//!
//! Flash (ESP Rust toolchain + espflash required):
//! ```text
//! cargo +esp build --release --target xtensa-esp32s3-none-elf --features esp
//! cargo +esp run --release --target xtensa-esp32s3-none-elf --features esp
//! ```
//!
//! Optional lighter params (less RAM, higher demo H/s):
//! ```text
//! --features esp,lite
//! ```

#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe with esp_hal types"
)]

extern crate alloc;

use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_io::{Read, Write};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Pin;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use heapless::String;
use log::info;

use esp32_s3_scrypt_miner::config::{PoolConfig, SetupField};
use esp32_s3_scrypt_miner::display::{Display, DisplayPeripherals};
use esp32_s3_scrypt_miner::miner::ScryptMiner;

esp_bootloader_esp_idf::esp_app_desc!();

/// Hashes between on-screen refresh updates.
const BATCH_SIZE: usize = 4;
/// Demo difficulty: leading zero nibbles of the scrypt hash (hex).
/// 4 ≈ find shares occasionally on-device; raise for harder work.
const DEMO_ZERO_NIBBLES: u8 = 4;

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    info!("esp32-s3-scrypt-miner starting");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Litecoin scrypt needs ~128 KiB for ROMix V; keep extra for UI/runtime.
    esp_alloc::heap_allocator!(size: 192 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let mut usb = UsbSerialJtag::new(peripherals.USB_DEVICE);

    let dp = DisplayPeripherals {
        rst: peripherals.GPIO5.degrade(),
        cs: peripherals.GPIO6.degrade(),
        dc: peripherals.GPIO7.degrade(),
        wr: peripherals.GPIO8.degrade(),
        rd: peripherals.GPIO9.degrade(),
        power_en: peripherals.GPIO15.degrade(),
        backlight: peripherals.GPIO38.degrade(),
        d0: peripherals.GPIO39.degrade(),
        d1: peripherals.GPIO40.degrade(),
        d2: peripherals.GPIO41.degrade(),
        d3: peripherals.GPIO42.degrade(),
        d4: peripherals.GPIO45.degrade(),
        d5: peripherals.GPIO46.degrade(),
        d6: peripherals.GPIO47.degrade(),
        d7: peripherals.GPIO48.degrade(),
    };

    let mut display = match Display::new(dp, Delay::new()) {
        Ok(d) => d,
        Err(e) => {
            info!("display init failed: {e}");
            serial_writeln(&mut usb, "display init failed");
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    };

    let pool = collect_pool_config(&mut usb, &mut display).await;
    let _ = display.draw_config_summary(&pool);
    serial_writeln(&mut usb, "");
    serial_writeln(&mut usb, "Config accepted. Starting miner...");
    serial_write(&mut usb, "  address = ");
    serial_writeln(&mut usb, pool.address.as_str());
    serial_write(&mut usb, "  password = ");
    serial_writeln(&mut usb, pool.password_masked().as_str());
    serial_write(&mut usb, "  stratum  = ");
    serial_writeln(&mut usb, pool.stratum.as_str());
    Timer::after(Duration::from_secs(2)).await;

    info!(
        "miner ready (N={}, log_n={}) stratum={}",
        esp32_s3_scrypt_miner::SCRYPT_N,
        esp32_s3_scrypt_miner::SCRYPT_LOG_N,
        pool.stratum
    );

    let mut miner = ScryptMiner::new_demo(DEMO_ZERO_NIBBLES);
    let mut window_start = Instant::now();
    let mut window_hashes: u64 = 0;

    let mut stats = miner.stats();
    let _ = display.draw_stats(&stats, &pool, true);

    loop {
        let (last, found_share) = miner.mine_batch(BATCH_SIZE);
        window_hashes = window_hashes.saturating_add(BATCH_SIZE as u64);

        if found_share {
            info!(
                "share! nonce={:08x} hash={:02x}{:02x}{:02x}{:02x}... addr={}",
                last.nonce,
                last.hash[0],
                last.hash[1],
                last.hash[2],
                last.hash[3],
                pool.address
            );
            let mut msg: String<96> = String::new();
            let _ = core::fmt::Write::write_fmt(
                &mut msg,
                format_args!("SHARE nonce={:08x} address={}", last.nonce, pool.address),
            );
            serial_writeln(&mut usb, msg.as_str());
        }

        let elapsed = window_start.elapsed();
        if elapsed >= Duration::from_millis(750) {
            let ms = elapsed.as_millis().max(1);
            let hashrate_x100 = ((window_hashes as u128 * 100_000) / ms) as u32;

            stats = miner.stats();
            stats.hashrate_x100 = hashrate_x100;
            if let Err(e) = display.draw_stats(&stats, &pool, true) {
                info!("display error: {e}");
            }

            info!(
                "H/s={}.{:02} nonce={:08x} shares={} stratum={}",
                hashrate_x100 / 100,
                hashrate_x100 % 100,
                stats.nonce,
                stats.shares,
                pool.stratum
            );

            window_start = Instant::now();
            window_hashes = 0;
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}

/// Prompt for address, password, and stratum individually over USB serial.
async fn collect_pool_config<D: embedded_hal::delay::DelayNs>(
    usb: &mut UsbSerialJtag<'_>,
    display: &mut Display<'_, D>,
) -> PoolConfig {
    let mut cfg = PoolConfig::new();

    serial_writeln(usb, "");
    serial_writeln(usb, "=== ESP32-S3 Scrypt Miner setup ===");
    serial_writeln(
        usb,
        "Enter address, password, and stratum location (one field at a time).",
    );
    serial_writeln(
        usb,
        "You can also send:  address <value> | password <value> | stratum <value>",
    );
    serial_writeln(usb, "");

    for field in SetupField::ALL {
        if !cfg.get(field).is_empty() {
            continue;
        }
        loop {
            let _ = display.draw_setup(field, "");
            serial_write(usb, field.label());
            serial_write(usb, ": ");
            let _ = usb.flush();

            let mut line: String<128> = String::new();
            read_line(usb, &mut line).await;

            let (target, value) =
                if let Ok((parsed_field, value)) = PoolConfig::parse_assignment(line.as_str()) {
                    (parsed_field, value)
                } else {
                    (field, line.as_str())
                };

            match cfg.set(target, value) {
                Ok(()) => {
                    serial_write(usb, "  ok (");
                    serial_write(usb, target.label());
                    serial_writeln(usb, ")");
                    let _ = display.draw_setup(target, cfg.get(target));
                    if target == field || !cfg.get(field).is_empty() {
                        break;
                    }
                }
                Err(e) => {
                    serial_write(usb, "  error: ");
                    serial_writeln(usb, match e {
                        esp32_s3_scrypt_miner::config::ConfigError::Empty => "value cannot be empty",
                        esp32_s3_scrypt_miner::config::ConfigError::TooLong => "value too long",
                        esp32_s3_scrypt_miner::config::ConfigError::InvalidChar => "invalid character",
                        esp32_s3_scrypt_miner::config::ConfigError::UnknownField => "unknown field",
                    });
                    serial_writeln(usb, " — try again");
                }
            }
        }
    }

    cfg
}

fn serial_write(usb: &mut UsbSerialJtag<'_>, text: &str) {
    let _ = usb.write_all(text.as_bytes());
}

fn serial_writeln(usb: &mut UsbSerialJtag<'_>, text: &str) {
    let _ = usb.write_all(text.as_bytes());
    let _ = usb.write_all(b"\r\n");
}

async fn read_line(usb: &mut UsbSerialJtag<'_>, line: &mut String<128>) {
    line.clear();
    let mut byte = [0u8; 1];
    loop {
        match usb.read(&mut byte) {
            Ok(0) => {
                Timer::after(Duration::from_millis(10)).await;
            }
            Ok(_) => {
                let c = byte[0];
                match c {
                    b'\n' | b'\r' => {
                        if !line.is_empty() {
                            let _ = usb.write_all(b"\r\n");
                            break;
                        }
                    }
                    0x08 | 0x7f => {
                        if line.pop().is_some() {
                            let _ = usb.write_all(b"\x08 \x08");
                        }
                    }
                    c if (32..127).contains(&c) => {
                        if line.push(c as char).is_ok() {
                            let _ = usb.write_all(&[c]);
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => {
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
}
