//! ESP32 scrypt miner firmware for **ESP32-2432S028** (Cheap Yellow Display).
//!
//! After first boot, enter wallet **address**, pool **password**, **stratum**,
//! optional **WiFi**, and **BLE name** over USB serial (CH340 UART0). Values are
//! saved to flash and auto-loaded on later boots. Onboard WiFi (STA+DHCP) +
//! optional BLE start from those settings. When WiFi is configured, a
//! **stratum TCP client** connects to the pool and mines real jobs.
//!
//! Controls: **BOOT** short-press = next tab / menu highlight; long-press = menu
//! activate. Type `change` / `radio` / `stratum` on serial.
//!
//! Flash (ESP Rust toolchain + espflash required):
//! ```text
//! cargo +esp run -Zbuild-std=core,alloc --release \
//!   --target xtensa-esp32-none-elf --features esp,lite
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
use embedded_io::Write;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Pin, Pull};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::{Config as UartConfig, Uart};
use esp_hal::Blocking;
use heapless::String;
use log::info;

use esp32_s3_scrypt_miner::config::{ConfigError, PoolConfig, SetupField};
use esp32_s3_scrypt_miner::display::{Display, DisplayPeripherals};
use esp32_s3_scrypt_miner::gui::GuiState;
use esp32_s3_scrypt_miner::miner::ScryptMiner;
use esp32_s3_scrypt_miner::persist::ConfigStore;
use esp32_s3_scrypt_miner::radio::{self, RadioStatus};
use esp32_s3_scrypt_miner::stratum::{self, JobMeta, StratumStatus};

esp_bootloader_esp_idf::esp_app_desc!();

const BATCH_SIZE: usize = 2;
const DEMO_ZERO_NIBBLES: u8 = 4;
const SAVED_CONFIRM_SECS: u64 = 8;
const MAX_PASSWORD_ATTEMPTS: u8 = 3;
const BOOT_LONG_PRESS_MS: u64 = 700;

type Serial<'d> = Uart<'d, Blocking>;

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    info!("esp32-2432s028 scrypt miner starting");

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Classic ESP32 DRAM is tight (static heap lives in .bss). Keep modest;
    // WiFi + lite scrypt (N=64 ≈ 8 KiB ROMix) share this pool.
    esp_alloc::heap_allocator!(size: 48 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    // CYD USB-UART bridge on UART0 (GPIO1 TX / GPIO3 RX).
    let mut usb = Uart::new(peripherals.UART0, UartConfig::default())
        .expect("UART0")
        .with_tx(peripherals.GPIO1)
        .with_rx(peripherals.GPIO3);

    let mut store = ConfigStore::new(peripherals.FLASH);

    // Only physical user button is BOOT (GPIO0). Short = next, long = action.
    let boot_btn = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );
    let force_change = boot_btn.is_low();

    // CYD ILI9341 HSPI pins.
    let dp = DisplayPeripherals {
        spi: peripherals.SPI2,
        sclk: peripherals.GPIO14.degrade(),
        mosi: peripherals.GPIO13.degrade(),
        miso: peripherals.GPIO12.degrade(),
        cs: peripherals.GPIO15.degrade(),
        dc: peripherals.GPIO2.degrade(),
        backlight: peripherals.GPIO21.degrade(),
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

    let stratum_enabled = if let Some(stack) =
        radio::start(&spawner, peripherals.WIFI, peripherals.BT, &pool)
    {
        stratum::start(&spawner, stack, &pool);
        serial_writeln(&mut usb, "Stratum client starting (needs WiFi + DHCP).");
        true
    } else {
        serial_writeln(
            &mut usb,
            "No WiFi SSID — local demo mining only (no stratum).",
        );
        false
    };

    let _ = display.draw_config_summary(&pool, from_flash);
    serial_writeln(&mut usb, "");
    if from_flash {
        serial_writeln(&mut usb, "Loaded saved credentials from flash.");
        serial_writeln(
            &mut usb,
            "GUI: BOOT short=tabs, long=menu. Serial: change | radio | stratum",
        );
    } else {
        serial_writeln(&mut usb, "Credentials saved to flash for next boot.");
    }
    print_config_serial(&mut usb, &pool);
    Timer::after(Duration::from_secs(2)).await;

    info!(
        "miner ready (N={}, log_n={}) stratum={} wifi={} ble={} from_flash={}",
        esp32_s3_scrypt_miner::SCRYPT_N,
        esp32_s3_scrypt_miner::SCRYPT_LOG_N,
        pool.stratum,
        pool.wifi_ssid,
        pool.ble_name_or_default(),
        from_flash
    );

    let mut miner = ScryptMiner::new_demo(DEMO_ZERO_NIBBLES);
    let mut active_job: Option<JobMeta> = None;
    let mut pool_mode = false;
    let mut window_start = Instant::now();
    let mut window_hashes: u64 = 0;
    let mut cmd_line: String<32> = String::new();
    let mut gui = GuiState::default();
    let mut boot_was_down = boot_btn.is_low();
    let mut boot_down_since: Option<Instant> = None;

    let mut stats = miner.stats();
    let mut radio_status = radio::snapshot().await;
    let mut stratum_status = if stratum_enabled {
        stratum::snapshot().await
    } else {
        StratumStatus::default()
    };
    let _ = display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);

    loop {
        if let Some(job) = stratum::try_take_job() {
            let diff = job.difficulty;
            let clean = job.clean;
            let job_id = job.meta.job_id.clone();
            info!("new stratum job={job_id} diff={diff} clean={clean}");
            miner.set_job(job.header, job.target, 0);
            active_job = Some(job.meta);
            pool_mode = true;
            let mut m: String<96> = String::new();
            let _ = core::fmt::Write::write_fmt(
                &mut m,
                format_args!("JOB {job_id} diff={diff}"),
            );
            serial_writeln(&mut usb, m.as_str());
        }

        let boot_down = boot_btn.is_low();
        if boot_down && !boot_was_down {
            boot_down_since = Some(Instant::now());
        }
        if !boot_down && boot_was_down {
            if let Some(start) = boot_down_since.take() {
                let long = start.elapsed() >= Duration::from_millis(BOOT_LONG_PRESS_MS);
                if long {
                    gui.on_action_press();
                    if gui.take_change_request() {
                        if let Some(updated) =
                            password_gated_change(&mut usb, &mut display, &mut store, &pool).await
                        {
                            pool = updated;
                            if stratum_enabled {
                                stratum::apply_pool_config(&pool).await;
                                serial_writeln(
                                    &mut usb,
                                    "Stratum worker/endpoint reloaded. WiFi/BLE still need reboot.",
                                );
                            } else {
                                serial_writeln(
                                    &mut usb,
                                    "Note: WiFi/BLE keep prior session until reboot.",
                                );
                            }
                            gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Config;
                            let _ = display.draw_config_summary(&pool, true);
                            Timer::after(Duration::from_secs(2)).await;
                        }
                        gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Mining;
                    }
                } else {
                    gui.on_boot_short_press();
                }
                radio_status = radio::snapshot().await;
                if stratum_enabled {
                    stratum_status = stratum::snapshot().await;
                }
                let _ = display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
            }
        }
        boot_was_down = boot_down;

        if poll_command_byte(&mut usb, &mut cmd_line) {
            let cmd = cmd_line.as_str().trim();
            if is_change_command(cmd) {
                if let Some(updated) =
                    password_gated_change(&mut usb, &mut display, &mut store, &pool).await
                {
                    pool = updated;
                    if stratum_enabled {
                        stratum::apply_pool_config(&pool).await;
                        serial_writeln(
                            &mut usb,
                            "Stratum worker/endpoint reloaded. WiFi/BLE still need reboot.",
                        );
                    } else {
                        serial_writeln(
                            &mut usb,
                            "Note: WiFi/BLE keep prior session until reboot.",
                        );
                    }
                    let _ = display.draw_config_summary(&pool, true);
                    Timer::after(Duration::from_secs(2)).await;
                }
                gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Mining;
                radio_status = radio::snapshot().await;
                if stratum_enabled {
                    stratum_status = stratum::snapshot().await;
                }
                let _ = display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
            } else if is_radio_command(cmd) {
                radio_status = radio::snapshot().await;
                if stratum_enabled {
                    stratum_status = stratum::snapshot().await;
                }
                print_radio_serial(&mut usb, &pool, &radio_status, &stratum_status);
                gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Radio;
                let _ = display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
            } else if !cmd.is_empty() {
                serial_writeln(
                    &mut usb,
                    "Unknown command. Type 'change', 'radio', or 'stratum'.",
                );
            }
            cmd_line.clear();
        }

        let (last, found_share) = miner.mine_batch(BATCH_SIZE);
        window_hashes = window_hashes.saturating_add(BATCH_SIZE as u64);

        if found_share {
            info!(
                "share! nonce={:08x} hash={:02x}{:02x}{:02x}{:02x}... pool_mode={}",
                last.nonce,
                last.hash[0],
                last.hash[1],
                last.hash[2],
                last.hash[3],
                pool_mode
            );
            let mut msg: String<96> = String::new();
            let _ = core::fmt::Write::write_fmt(
                &mut msg,
                format_args!("SHARE nonce={:08x} address={}", last.nonce, pool.address),
            );
            serial_writeln(&mut usb, msg.as_str());

            if pool_mode {
                if let Some(meta) = active_job.as_ref() {
                    let share = stratum::make_share(pool.address.as_str(), meta, last.nonce);
                    stratum::queue_share(share).await;
                }
            }
        }

        let elapsed = window_start.elapsed();
        if elapsed >= Duration::from_millis(750) {
            let ms = elapsed.as_millis().max(1);
            let hashrate_x100 = ((window_hashes as u128 * 100_000) / u128::from(ms)) as u32;

            stats = miner.stats();
            stats.hashrate_x100 = hashrate_x100;
            radio_status = radio::snapshot().await;
            if stratum_enabled {
                stratum_status = stratum::snapshot().await;
            }
            if let Err(e) =
                display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true)
            {
                info!("display error: {e}");
            }

            info!(
                "H/s={}.{:02} nonce={:08x} shares={} wifi={} stratum={} acc={}/{}",
                hashrate_x100 / 100,
                hashrate_x100 % 100,
                stats.nonce,
                stats.shares,
                radio_status.wifi.label(),
                stratum_status.phase.label(),
                stratum_status.accepted,
                stratum_status.rejected
            );

            window_start = Instant::now();
            window_hashes = 0;
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}

fn print_config_serial(usb: &mut Serial<'_>, pool: &PoolConfig) {
    serial_write(usb, "  address = ");
    serial_writeln(usb, pool.address.as_str());
    serial_write(usb, "  password = ");
    serial_writeln(usb, pool.password_masked().as_str());
    serial_write(usb, "  stratum  = ");
    serial_writeln(usb, pool.stratum.as_str());
    serial_write(usb, "  wifi_ssid = ");
    if pool.wifi_enabled() {
        serial_writeln(usb, pool.wifi_ssid.as_str());
    } else {
        serial_writeln(usb, "(disabled)");
    }
    serial_write(usb, "  wifi_password = ");
    serial_writeln(usb, pool.wifi_password_masked().as_str());
    serial_write(usb, "  ble_name = ");
    serial_writeln(usb, pool.ble_name_or_default());
}

fn print_radio_serial(
    usb: &mut Serial<'_>,
    pool: &PoolConfig,
    radio: &RadioStatus,
    stratum: &StratumStatus,
) {
    serial_writeln(usb, "");
    serial_writeln(usb, "=== Radio / stratum status ===");
    serial_write(usb, "  wifi = ");
    serial_write(usb, radio.wifi.label());
    serial_write(usb, "  ssid = ");
    if pool.wifi_enabled() {
        serial_writeln(usb, pool.wifi_ssid.as_str());
    } else {
        serial_writeln(usb, "(disabled)");
    }
    serial_write(usb, "  ip = ");
    serial_writeln(usb, radio.ip_string().as_str());
    serial_write(usb, "  ble = ");
    if radio.ble_connected {
        serial_write(usb, "connected");
    } else if radio.ble_advertising {
        serial_write(usb, "advertising");
    } else {
        serial_write(usb, "off");
    }
    serial_write(usb, "  name = ");
    serial_writeln(usb, pool.ble_name_or_default());
    serial_write(usb, "  stratum = ");
    serial_write(usb, stratum.phase.label());
    serial_write(usb, "  endpoint = ");
    serial_writeln(usb, pool.stratum.as_str());
    serial_write(usb, "  difficulty = ");
    let mut diff: String<16> = String::new();
    let _ = core::fmt::Write::write_fmt(&mut diff, format_args!("{}", stratum.difficulty));
    serial_writeln(usb, diff.as_str());
    serial_write(usb, "  accepted/rejected/dropped = ");
    let mut ar: String<32> = String::new();
    let _ = core::fmt::Write::write_fmt(
        &mut ar,
        format_args!(
            "{}/{}/{}",
            stratum.accepted, stratum.rejected, stratum.dropped
        ),
    );
    serial_writeln(usb, ar.as_str());
    serial_write(usb, "  reconnects = ");
    let mut rc: String<12> = String::new();
    let _ = core::fmt::Write::write_fmt(&mut rc, format_args!("{}", stratum.reconnects));
    serial_writeln(usb, rc.as_str());
    serial_write(usb, "  detail = ");
    if stratum.detail.is_empty() {
        serial_writeln(usb, "(none)");
    } else {
        serial_writeln(usb, stratum.detail.as_str());
    }
    serial_write(usb, "  job = ");
    if stratum.job_id.is_empty() {
        serial_writeln(usb, "(none)");
    } else {
        serial_writeln(usb, stratum.job_id.as_str());
    }
}

async fn resolve_pool_config<D: embedded_hal::delay::DelayNs>(
    usb: &mut Serial<'_>,
    display: &mut Display<'_, D>,
    store: &mut ConfigStore<'_>,
    force_change: bool,
) -> (PoolConfig, bool) {
    match store.load() {
        Ok(saved) => {
            serial_writeln(usb, "");
            serial_writeln(usb, "=== Saved credentials found ===");
            print_config_serial(usb, &saved);
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
                return (saved, true);
            }
            (saved, true)
        }
        Err(_) => {
            serial_writeln(usb, "");
            serial_writeln(usb, "No saved credentials — first-time setup.");
            let cfg = collect_pool_config(usb, display).await;
            match store.save(&cfg) {
                Ok(()) => serial_writeln(usb, "Credentials saved to flash."),
                Err(_) => serial_writeln(usb, "WARNING: flash save failed."),
            }
            (cfg, false)
        }
    }
}

async fn password_gated_change<D: embedded_hal::delay::DelayNs>(
    usb: &mut Serial<'_>,
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

async fn wait_for_change_command(usb: &mut Serial<'_>, timeout: Duration) -> bool {
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

fn poll_command_byte(usb: &mut Serial<'_>, line: &mut String<32>) -> bool {
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

fn is_radio_command(cmd: &str) -> bool {
    eq_ignore_ascii_case(cmd, "radio")
        || eq_ignore_ascii_case(cmd, "wifi")
        || eq_ignore_ascii_case(cmd, "ble")
        || eq_ignore_ascii_case(cmd, "bt")
        || eq_ignore_ascii_case(cmd, "stratum")
        || eq_ignore_ascii_case(cmd, "pool")
}

fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

async fn collect_pool_config<D: embedded_hal::delay::DelayNs>(
    usb: &mut Serial<'_>,
    display: &mut Display<'_, D>,
) -> PoolConfig {
    let mut cfg = PoolConfig::new();

    serial_writeln(usb, "");
    serial_writeln(usb, "=== ESP32-2432S028 Scrypt Miner setup ===");
    serial_writeln(
        usb,
        "Enter address, password, stratum, then WiFi/BLE (one field at a time).",
    );
    serial_writeln(
        usb,
        "Prefixes: address | password | stratum | wifi_ssid | wifi_password | ble_name",
    );
    serial_writeln(usb, "WiFi SSID '-' skips WiFi. Empty WiFi password = open AP.");
    serial_writeln(usb, "BLE name '-' / empty skips BLE (saves RAM).");
    serial_writeln(usb, "");

    for field in SetupField::ALL {
        if !field.allows_empty() && !cfg.get(field).is_empty() {
            continue;
        }
        loop {
            let _ = display.draw_setup(field, "");
            serial_write(usb, field.label());
            serial_write(usb, ": ");
            let _ = usb.flush();

            let mut line: String<128> = String::new();
            if field.is_secret() {
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
                    if target == field {
                        break;
                    }
                    if !field.allows_empty() && !cfg.get(field).is_empty() {
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

fn serial_write(usb: &mut Serial<'_>, text: &str) {
    let _ = usb.write_all(text.as_bytes());
}

fn serial_writeln(usb: &mut Serial<'_>, text: &str) {
    let _ = usb.write_all(text.as_bytes());
    let _ = usb.write_all(b"\r\n");
}

async fn read_line(usb: &mut Serial<'_>, line: &mut String<128>) {
    read_line_inner(usb, line, false).await;
}

async fn read_line_secret(usb: &mut Serial<'_>, line: &mut String<128>) {
    read_line_inner(usb, line, true).await;
}

async fn read_line_inner(usb: &mut Serial<'_>, line: &mut String<128>, secret: bool) {
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
                        let _ = usb.write_all(b"\r\n");
                        break;
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
