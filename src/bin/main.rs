//! ESP32-S3 scrypt miner firmware for LilyGO T-Display-S3.
//!
//! After first boot, enter wallet **address**, pool **password**, and **stratum**
//! over USB serial. Values are saved to flash and auto-loaded on later boots.
//!
//! To change saved info later, type `change` and enter the **current password**
//! first (also works while mining). Hold BOOT at power-on to jump into that
//! password-gated change flow.
//!
//! Flash (ESP Rust toolchain + espflash required):
//! ```text
//! cargo +esp run -Zbuild-std=core,alloc --release \
//!   --target xtensa-esp32s3-none-elf --features esp
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
use esp_hal::gpio::{Input, InputConfig, Pin, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::usb_serial_jtag::UsbSerialJtag;
use heapless::String;
use log::info;

use esp32_s3_scrypt_miner::config::{ConfigError, PoolConfig, SetupField};
use esp32_s3_scrypt_miner::display::{Display, DisplayPeripherals};
use esp32_s3_scrypt_miner::gui::GuiState;
use esp32_s3_scrypt_miner::miner::ScryptMiner;
use esp32_s3_scrypt_miner::persist::ConfigStore;

esp_bootloader_esp_idf::esp_app_desc!();

const BATCH_SIZE: usize = 4;
const DEMO_ZERO_NIBBLES: u8 = 4;
/// How long to show the saved-config screen and accept a `change` command.
const SAVED_CONFIRM_SECS: u64 = 8;
const MAX_PASSWORD_ATTEMPTS: u8 = 3;

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    info!("esp32-s3-scrypt-miner starting");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 192 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let mut usb = UsbSerialJtag::new(peripherals.USB_DEVICE);
    let mut store = ConfigStore::new(peripherals.FLASH);

    // BOOT (GPIO0) + custom button (GPIO14) drive the on-screen GUI.
    let boot_btn = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );
    let action_btn = Input::new(
        peripherals.GPIO14,
        InputConfig::default().with_pull(Pull::Up),
    );
    let force_change = boot_btn.is_low();

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

    let _ = display.draw_splash();
    Timer::after(Duration::from_millis(600)).await;

    let (mut pool, from_flash) =
        resolve_pool_config(&mut usb, &mut display, &mut store, force_change).await;

    let _ = display.draw_config_summary(&pool, from_flash);
    serial_writeln(&mut usb, "");
    if from_flash {
        serial_writeln(&mut usb, "Loaded saved credentials from flash.");
        serial_writeln(
            &mut usb,
            "GUI: BOOT=tabs, btn=menu. Serial 'change' also works (password required).",
        );
    } else {
        serial_writeln(&mut usb, "Credentials saved to flash for next boot.");
    }
    serial_write(&mut usb, "  address = ");
    serial_writeln(&mut usb, pool.address.as_str());
    serial_write(&mut usb, "  password = ");
    serial_writeln(&mut usb, pool.password_masked().as_str());
    serial_write(&mut usb, "  stratum  = ");
    serial_writeln(&mut usb, pool.stratum.as_str());
    Timer::after(Duration::from_secs(2)).await;

    info!(
        "miner ready (N={}, log_n={}) stratum={} from_flash={}",
        esp32_s3_scrypt_miner::SCRYPT_N,
        esp32_s3_scrypt_miner::SCRYPT_LOG_N,
        pool.stratum,
        from_flash
    );

    let mut miner = ScryptMiner::new_demo(DEMO_ZERO_NIBBLES);
    let mut window_start = Instant::now();
    let mut window_hashes: u64 = 0;
    let mut cmd_line: String<32> = String::new();
    let mut gui = GuiState::default();
    let mut boot_was_down = boot_btn.is_low();
    let mut action_was_down = action_btn.is_low();

    let mut stats = miner.stats();
    let _ = display.draw_gui(&gui, &stats, &pool, true);

    loop {
        // Button edge detection for on-screen GUI navigation.
        let boot_down = boot_btn.is_low();
        if boot_down && !boot_was_down {
            gui.on_boot_short_press();
            let _ = display.draw_gui(&gui, &stats, &pool, true);
        }
        boot_was_down = boot_down;

        let action_down = action_btn.is_low();
        if action_down && !action_was_down {
            gui.on_action_press();
            if gui.take_change_request() {
                if let Some(updated) =
                    password_gated_change(&mut usb, &mut display, &mut store, &pool).await
                {
                    pool = updated;
                    gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Config;
                    let _ = display.draw_config_summary(&pool, true);
                    Timer::after(Duration::from_secs(2)).await;
                }
                gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Mining;
            }
            let _ = display.draw_gui(&gui, &stats, &pool, true);
        }
        action_was_down = action_down;

        // Non-blocking serial command poll (password-gated change while mining).
        if poll_command_byte(&mut usb, &mut cmd_line) {
            if is_change_command(cmd_line.as_str().trim()) {
                if let Some(updated) =
                    password_gated_change(&mut usb, &mut display, &mut store, &pool).await
                {
                    pool = updated;
                    let _ = display.draw_config_summary(&pool, true);
                    Timer::after(Duration::from_secs(2)).await;
                }
                gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Mining;
                let _ = display.draw_gui(&gui, &stats, &pool, true);
            } else if !cmd_line.is_empty() {
                serial_writeln(&mut usb, "Unknown command. Type 'change' to edit credentials.");
            }
            cmd_line.clear();
        }

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
            if let Err(e) = display.draw_gui(&gui, &stats, &pool, true) {
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

/// Load saved credentials, or prompt + save on first run / after authorized change.
async fn resolve_pool_config<D: embedded_hal::delay::DelayNs>(
    usb: &mut UsbSerialJtag<'_>,
    display: &mut Display<'_, D>,
    store: &mut ConfigStore<'_>,
    force_change: bool,
) -> (PoolConfig, bool) {
    match store.load() {
        Ok(saved) => {
            serial_writeln(usb, "");
            serial_writeln(usb, "=== Saved credentials found ===");
            serial_write(usb, "  address = ");
            serial_writeln(usb, saved.address.as_str());
            serial_write(usb, "  password = ");
            serial_writeln(usb, saved.password_masked().as_str());
            serial_write(usb, "  stratum  = ");
            serial_writeln(usb, saved.stratum.as_str());
            serial_writeln(
                usb,
                "Type 'change' within 8s to edit (current password required), or wait.",
            );

            let _ = display.draw_config_summary(&saved, true);

            let want_change = force_change
                || wait_for_change_command(usb, Duration::from_secs(SAVED_CONFIRM_SECS)).await;

            if want_change {
                if force_change {
                    serial_writeln(usb, "BOOT held — password required to change credentials.");
                }
                if let Some(updated) = password_gated_change(usb, display, store, &saved).await {
                    return (updated, true);
                }
                serial_writeln(usb, "Keeping previously saved credentials.");
            }
            (saved, true)
        }
        Err(_) => {
            serial_writeln(usb, "No saved credentials — starting setup.");
            let cfg = collect_pool_config(usb, display).await;
            match store.save(&cfg) {
                Ok(()) => serial_writeln(usb, "Saved credentials to flash."),
                Err(_) => serial_writeln(usb, "WARNING: failed to save credentials to flash."),
            }
            (cfg, false)
        }
    }
}

/// Prompt for current password, then collect and persist new credentials.
async fn password_gated_change<D: embedded_hal::delay::DelayNs>(
    usb: &mut UsbSerialJtag<'_>,
    display: &mut Display<'_, D>,
    store: &mut ConfigStore<'_>,
    current: &PoolConfig,
) -> Option<PoolConfig> {
    serial_writeln(usb, "");
    serial_writeln(usb, "=== Change credentials (password required) ===");

    for attempt in 1..=MAX_PASSWORD_ATTEMPTS {
        let _ = display.draw_auth_prompt(attempt, MAX_PASSWORD_ATTEMPTS);
        serial_write(usb, "current password");
        if attempt > 1 {
            serial_write(usb, " (retry)");
        }
        serial_write(usb, ": ");
        let _ = usb.flush();

        let mut line: String<128> = String::new();
        read_line_secret(usb, &mut line).await;

        match current.authorize(line.as_str()) {
            Ok(()) => {
                serial_writeln(usb, "  ok — enter new values");
                let cfg = collect_pool_config(usb, display).await;
                match store.save(&cfg) {
                    Ok(()) => {
                        serial_writeln(usb, "Updated credentials saved to flash.");
                        return Some(cfg);
                    }
                    Err(_) => {
                        serial_writeln(usb, "WARNING: auth ok but flash save failed.");
                        return Some(cfg);
                    }
                }
            }
            Err(ConfigError::BadPassword) => {
                serial_writeln(usb, "  incorrect password");
            }
            Err(_) => serial_writeln(usb, "  auth error"),
        }
    }

    serial_writeln(usb, "Too many failed attempts — change cancelled.");
    None
}

async fn wait_for_change_command(usb: &mut UsbSerialJtag<'_>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut line: String<32> = String::new();
    let mut byte = [0u8; 1];

    while Instant::now() < deadline {
        match usb.read(&mut byte) {
            Ok(0) | Err(_) => {
                Timer::after(Duration::from_millis(20)).await;
            }
            Ok(_) => match byte[0] {
                b'\n' | b'\r' => {
                    if !line.is_empty() {
                        let hit = is_change_command(line.as_str().trim());
                        line.clear();
                        if hit {
                            return true;
                        }
                    }
                }
                c if (32..127).contains(&c) => {
                    let _ = line.push(c as char);
                }
                _ => {}
            },
        }
    }
    false
}

/// Returns true when a full line was completed.
fn poll_command_byte(usb: &mut UsbSerialJtag<'_>, line: &mut String<32>) -> bool {
    let mut byte = [0u8; 1];
    match usb.read(&mut byte) {
        Ok(0) | Err(_) => false,
        Ok(_) => match byte[0] {
            b'\n' | b'\r' => !line.is_empty(),
            c if (32..127).contains(&c) => {
                let _ = line.push(c as char);
                false
            }
            _ => false,
        },
    }
}

fn is_change_command(cmd: &str) -> bool {
    eq_ignore_ascii_case(cmd, "change")
        || eq_ignore_ascii_case(cmd, "edit")
        || eq_ignore_ascii_case(cmd, "update")
        || eq_ignore_ascii_case(cmd, "clear")
}

fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

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
            if field == SetupField::Password {
                read_line_secret(usb, &mut line).await;
            } else {
                read_line(usb, &mut line).await;
            }

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
                    serial_writeln(usb, config_error_msg(e));
                    serial_writeln(usb, " — try again");
                }
            }
        }
    }

    cfg
}

fn config_error_msg(e: ConfigError) -> &'static str {
    match e {
        ConfigError::Empty => "value cannot be empty",
        ConfigError::TooLong => "value too long",
        ConfigError::InvalidChar => "invalid character",
        ConfigError::UnknownField => "unknown field",
        ConfigError::Corrupt => "corrupt value",
        ConfigError::BadPassword => "incorrect password",
    }
}

fn serial_write(usb: &mut UsbSerialJtag<'_>, text: &str) {
    let _ = usb.write_all(text.as_bytes());
}

fn serial_writeln(usb: &mut UsbSerialJtag<'_>, text: &str) {
    let _ = usb.write_all(text.as_bytes());
    let _ = usb.write_all(b"\r\n");
}

async fn read_line(usb: &mut UsbSerialJtag<'_>, line: &mut String<128>) {
    read_line_inner(usb, line, false).await;
}

/// Echo `*` instead of characters (for password entry).
async fn read_line_secret(usb: &mut UsbSerialJtag<'_>, line: &mut String<128>) {
    read_line_inner(usb, line, true).await;
}

async fn read_line_inner(usb: &mut UsbSerialJtag<'_>, line: &mut String<128>, secret: bool) {
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
                            if secret {
                                let _ = usb.write_all(b"*");
                            } else {
                                let _ = usb.write_all(&[c]);
                            }
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
