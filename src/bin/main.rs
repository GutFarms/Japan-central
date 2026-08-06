//! ESP32-S3 scrypt miner firmware for LilyGO T-Display-S3.
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
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Pin;
use esp_hal::timer::timg::TimerGroup;
use log::info;

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
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    };

    info!(
        "miner ready (N={}, log_n={})",
        esp32_s3_scrypt_miner::SCRYPT_N,
        esp32_s3_scrypt_miner::SCRYPT_LOG_N
    );

    let mut miner = ScryptMiner::new_demo(DEMO_ZERO_NIBBLES);
    let mut window_start = Instant::now();
    let mut window_hashes: u64 = 0;

    // Initial paint
    let mut stats = miner.stats();
    let _ = display.draw_stats(&stats, true);

    loop {
        let (last, found_share) = miner.mine_batch(BATCH_SIZE);
        window_hashes = window_hashes.saturating_add(BATCH_SIZE as u64);

        if found_share {
            info!(
                "share! nonce={:08x} hash={:02x}{:02x}{:02x}{:02x}...",
                last.nonce, last.hash[0], last.hash[1], last.hash[2], last.hash[3]
            );
        }

        let elapsed = window_start.elapsed();
        if elapsed >= Duration::from_millis(750) {
            let ms = elapsed.as_millis().max(1);
            // hashrate * 100 for two decimal places without floats in formatting
            let hashrate_x100 = ((window_hashes as u128 * 100_000) / ms) as u32;

            stats = miner.stats();
            stats.hashrate_x100 = hashrate_x100;
            if let Err(e) = display.draw_stats(&stats, true) {
                info!("display error: {e}");
            }

            info!(
                "H/s={}.{:02} nonce={:08x} shares={} hashes={}",
                hashrate_x100 / 100,
                hashrate_x100 % 100,
                stats.nonce,
                stats.shares,
                stats.hashes
            );

            window_start = Instant::now();
            window_hashes = 0;
        }

        // Yield so the embassy timer/executor stays healthy under sustained hashing.
        Timer::after(Duration::from_millis(1)).await;
    }
}
