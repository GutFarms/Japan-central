//! ESP32 scrypt miner firmware for **ESP32-2432S028** (Cheap Yellow Display).
//!
//! After first boot, enter wallet **address**, pool **password**, **stratum**,
//! optional **WiFi**, and **BLE name** over USB serial (CH340 UART0). Values are
//! saved to flash and auto-loaded on later boots. Onboard WiFi (STA+DHCP) +
//! optional BLE start from those settings. When WiFi is configured, a
//! **stratum TCP client** connects to the pool and mines real jobs.
//!
//! Controls: **touch** tabs/menu/on-screen keyboard; **BOOT** short=next tab,
//! long=menu. Serial still accepts `change` / `radio` / `stratum`.
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
use esp_hal::ram;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::{Config as UartConfig, Uart};
use esp_hal::Blocking;
use heapless::String;
use log::info;

use esp32_s3_scrypt_miner::config::{ConfigError, PoolConfig, SetupField};
use esp32_s3_scrypt_miner::display::{Display, DisplayPeripherals};
use esp32_s3_scrypt_miner::gui::GuiState;
use esp32_s3_scrypt_miner::gui::GuiScreen;
use esp32_s3_scrypt_miner::keyboard::{hit_gui, hit_wifi_scan, GuiHit, Keyboard, WifiScanHit};
use esp32_s3_scrypt_miner::miner::ScryptMiner;
use esp32_s3_scrypt_miner::persist::ConfigStore;
use esp32_s3_scrypt_miner::radio::{self, RadioStatus, ScannedNetwork};
use esp32_s3_scrypt_miner::stratum::{self, JobMeta, StratumPhase, StratumStatus};
use esp32_s3_scrypt_miner::touch::{Touch, TouchPins};
use esp32_s3_scrypt_miner::web::{self, WebStatus};
use esp_hal::peripherals::WIFI;

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

    // Classic ESP32: WiFi STA alone wants ~47–57 KiB. Use bootloader-reclaimed
    // DRAM for the radio blobs, plus a smaller .bss heap for app buffers
    // (lite scrypt ROMix ≈ 8 KiB, embassy-net, etc.).
    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 28 * 1024);

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

    // XPT2046 on dedicated VSPI (SPI3) — same pins as Arduino CYD examples.
    let mut touch = Touch::new(TouchPins {
        spi: peripherals.SPI3,
        clk: peripherals.GPIO25.degrade(),
        mosi: peripherals.GPIO32.degrade(),
        miso: peripherals.GPIO39.degrade(),
        cs: peripherals.GPIO33.degrade(),
        irq: peripherals.GPIO36.degrade(),
    });
    let mut touch_delay = Delay::new();
    serial_writeln(&mut usb, "Touch: SPI3 XPT2046 CLK25/MOSI32/MISO39/CS33/IRQ36");
    // Probe once so serial shows whether the panel answers.
    let _ = touch.poll_point(&mut touch_delay);
    if let Some((x, y, z)) = touch.last_raw {
        let mut m: String<48> = String::new();
        let _ = core::fmt::Write::write_fmt(&mut m, format_args!("Touch probe raw x={x} y={y} z={z}"));
        serial_writeln(&mut usb, m.as_str());
    }

    let _ = display.draw_splash();
    Timer::after(Duration::from_millis(700)).await;

    let mut wifi_token: Option<WIFI<'static>> = Some(peripherals.WIFI);
    let (mut pool, from_flash) = resolve_pool_config(
        &mut usb,
        &mut display,
        &mut touch,
        &mut touch_delay,
        &mut store,
        force_change,
        &mut wifi_token,
    )
    .await;

    let _ = display.draw_config_summary(&pool, from_flash);
    serial_writeln(&mut usb, "");
    if from_flash {
        serial_writeln(&mut usb, "Loaded saved credentials from flash.");
        serial_writeln(
            &mut usb,
            "GUI: tap tabs/keyboard · BOOT short=tabs, long=menu · serial: change",
        );
    } else {
        serial_writeln(&mut usb, "Credentials saved to flash for next boot.");
    }
    print_config_serial(&mut usb, &pool);

    // Reserve scrypt buffers *before* WiFi eats the heap (OOM panic otherwise).
    serial_writeln(&mut usb, "Allocating miner buffers…");
    let mut miner = ScryptMiner::new_demo(DEMO_ZERO_NIBBLES);
    serial_writeln(&mut usb, "Starting radio…");

    // Scan may have consumed WIFI; reclaim via steal after controller drop.
    let wifi = wifi_token
        .take()
        .unwrap_or_else(|| unsafe { WIFI::steal() });

    let stratum_enabled = if let Some(stack) = radio::start(&spawner, wifi, peripherals.BT, &pool)
    {
        stratum::start(&spawner, stack, &pool);
        web::start(&spawner, stack);
        serial_writeln(&mut usb, "Stratum client starting (needs WiFi + DHCP).");
        serial_writeln(
            &mut usb,
            "Web UI on port 80 after DHCP — open http://<board-ip>/",
        );
        true
    } else {
        serial_writeln(
            &mut usb,
            "No WiFi SSID — local demo mining only (no stratum / no web UI).",
        );
        false
    };

    if stratum_enabled {
        wait_for_ip_screen(&mut usb, &mut display, &mut touch, &mut touch_delay, &pool).await;
    } else {
        Timer::after(Duration::from_secs(2)).await;
    }

    info!(
        "miner ready (N={}, log_n={}) stratum={} wifi={} ble={} from_flash={}",
        esp32_s3_scrypt_miner::SCRYPT_N,
        esp32_s3_scrypt_miner::SCRYPT_LOG_N,
        pool.stratum,
        pool.wifi_ssid,
        pool.ble_name_or_default(),
        from_flash
    );
    let mut active_job: Option<JobMeta> = None;
    let mut pool_mode = false;
    let mut window_start = Instant::now();
    let mut window_hashes: u64 = 0;
    let mut cmd_line: String<32> = String::new();
    let mut gui = GuiState::default();
    let mut boot_was_down = boot_btn.is_low();
    let mut boot_down_since: Option<Instant> = None;
    let mut saw_ip = false;
    let mut saw_stratum = false;

    let mut stats = miner.stats();
    let mut radio_status = radio::snapshot().await;
    let mut stratum_status = if stratum_enabled {
        stratum::snapshot().await
    } else {
        StratumStatus::default()
    };
    if radio_status.ip.is_some() {
        saw_ip = true;
    }
    // Prefer MINE so live H/s is visible once hashing starts.
    gui.screen = GuiScreen::Mining;
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
                        if let Some(updated) = password_gated_change(
                        &mut usb,
                        &mut display,
                        &mut touch,
                        &mut touch_delay,
                        &mut store,
                        &pool,
                        &mut None,
                    )
                        .await
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
                display.invalidate();
                radio_status = radio::snapshot().await;
                if stratum_enabled {
                    stratum_status = stratum::snapshot().await;
                }
                let _ = display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
            }
        }
        boot_was_down = boot_down;

        // Touch: tab strip, menu rows, config "change" band.
        if let Some(p) = touch.poll_tap(&mut touch_delay) {
            let on_menu = gui.screen == esp32_s3_scrypt_miner::gui::GuiScreen::Menu;
            match hit_gui(p, on_menu) {
                Some(GuiHit::Tab(i)) => {
                    gui.set_tab(i);
                    display.invalidate();
                    let _ =
                        display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
                }
                Some(GuiHit::MenuRow(i)) => {
                    gui.select_menu_row(i);
                    gui.activate_menu();
                    if gui.take_change_request() {
                        if let Some(updated) = password_gated_change(
                        &mut usb,
                        &mut display,
                        &mut touch,
                        &mut touch_delay,
                        &mut store,
                        &pool,
                        &mut None,
                    )
                        .await
                        {
                            pool = updated;
                            if stratum_enabled {
                                stratum::apply_pool_config(&pool).await;
                            }
                            let _ = display.draw_config_summary(&pool, true);
                            Timer::after(Duration::from_secs(2)).await;
                        }
                        gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Mining;
                    }
                    display.invalidate();
                    let _ =
                        display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
                }
                Some(GuiHit::ChangeBanner)
                    if gui.screen == esp32_s3_scrypt_miner::gui::GuiScreen::Config =>
                {
                    if let Some(updated) = password_gated_change(
                        &mut usb,
                        &mut display,
                        &mut touch,
                        &mut touch_delay,
                        &mut store,
                        &pool,
                        &mut None,
                    )
                    .await
                    {
                        pool = updated;
                        if stratum_enabled {
                            stratum::apply_pool_config(&pool).await;
                        }
                        let _ = display.draw_config_summary(&pool, true);
                        Timer::after(Duration::from_secs(2)).await;
                    }
                    gui.screen = esp32_s3_scrypt_miner::gui::GuiScreen::Mining;
                    display.invalidate();
                    let _ =
                        display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true);
                }
                _ => {}
            }
        }

        if poll_command_byte(&mut usb, &mut cmd_line) {
            let cmd = cmd_line.as_str().trim();
            if is_change_command(cmd) {
                if let Some(updated) = password_gated_change(
                        &mut usb,
                        &mut display,
                        &mut touch,
                        &mut touch_delay,
                        &mut store,
                        &pool,
                        &mut None,
                    )
                .await
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
                display.invalidate();
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
            if !saw_ip {
                if let Some(ip) = radio_status.ip {
                    saw_ip = true;
                    serial_write(&mut usb, "IP address: ");
                    serial_writeln(&mut usb, radio_status.ip_string().as_str());
                    let _ = display.draw_online(pool.wifi_ssid.as_str(), ip);
                    Timer::after(Duration::from_secs(3)).await;
                    gui.screen = GuiScreen::Mining;
                    display.invalidate();
                }
            }
            if !saw_stratum && stratum_status.phase.is_connected() {
                saw_stratum = true;
                serial_writeln(&mut usb, "Stratum CONNECTED — showing live H/s");
                let mut msg: String<64> = String::new();
                let _ = core::fmt::Write::write_fmt(
                    &mut msg,
                    format_args!(
                        "H/s={}.{:02}  phase={}",
                        hashrate_x100 / 100,
                        hashrate_x100 % 100,
                        stratum_status.phase.label()
                    ),
                );
                serial_writeln(&mut usb, msg.as_str());
                let _ = display.draw_pool_connected(pool.stratum.as_str(), hashrate_x100);
                Timer::after(Duration::from_secs(3)).await;
                gui.screen = GuiScreen::Mining;
                display.invalidate();
            }
            if saw_stratum
                && !matches!(
                    stratum_status.phase,
                    StratumPhase::Idle | StratumPhase::Mining | StratumPhase::Disabled
                )
            {
                // Allow re-announce after reconnect.
                if matches!(
                    stratum_status.phase,
                    StratumPhase::Error
                        | StratumPhase::Connecting
                        | StratumPhase::WaitingWifi
                ) {
                    saw_stratum = false;
                }
            }
            if let Err(e) =
                display.draw_gui(&gui, &stats, &pool, &radio_status, &stratum_status, true)
            {
                info!("display error: {e}");
            }

            {
                let mut ws = WebStatus::default();
                ws.hashrate_x100 = hashrate_x100;
                ws.shares = stats.shares;
                ws.nonce = stats.nonce;
                let _ = ws.address.push_str(pool.address.as_str());
                let _ = ws.stratum.push_str(pool.stratum.as_str());
                ws.wifi = radio_status.wifi;
                ws.ip = radio_status.ip;
                ws.pool_phase = stratum_status.phase;
                ws.accepted = stratum_status.accepted;
                ws.rejected = stratum_status.rejected;
                ws.dropped = stratum_status.dropped;
                ws.difficulty = stratum_status.difficulty;
                web::publish(ws);
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
    serial_write(usb, "  stratum  = ");
    serial_writeln(usb, pool.stratum.as_str());
    serial_write(usb, "  worker  = ");
    serial_writeln(usb, pool.address.as_str());
    serial_write(usb, "  password = ");
    serial_writeln(usb, pool.password_masked().as_str());
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
    touch: &mut Touch,
    touch_delay: &mut Delay,
    store: &mut ConfigStore<'_>,
    force_change: bool,
    wifi_token: &mut Option<WIFI<'static>>,
) -> (PoolConfig, bool) {
    match store.load() {
        Ok(saved) => {
            serial_writeln(usb, "");
            serial_writeln(usb, "=== Saved credentials found ===");
            print_config_serial(usb, &saved);
            serial_writeln(
                usb,
                "Tap CONF or type 'change' within 8s to edit, or wait.",
            );

            let _ = display.draw_config_summary(&saved, true);

            let want_change = force_change
                || wait_for_change_or_touch(
                    usb,
                    touch,
                    touch_delay,
                    Duration::from_secs(SAVED_CONFIRM_SECS),
                )
                .await;

            if want_change {
                if force_change {
                    serial_writeln(usb, "BOOT held — password required to change credentials.");
                }
                if let Some(updated) = password_gated_change(
                    usb,
                    display,
                    touch,
                    touch_delay,
                    store,
                    &saved,
                    wifi_token,
                )
                .await
                {
                    return (updated, true);
                }
                serial_writeln(usb, "Keeping previously saved credentials.");
                return (saved, true);
            }
            (saved, true)
        }
        Err(_) => {
            serial_writeln(usb, "");
            serial_writeln(usb, "No saved credentials — first-time setup (touch or serial).");
            let cfg = collect_pool_config(usb, display, touch, touch_delay, wifi_token).await;
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
    touch: &mut Touch,
    touch_delay: &mut Delay,
    store: &mut ConfigStore<'_>,
    current: &PoolConfig,
    wifi_token: &mut Option<WIFI<'static>>,
) -> Option<PoolConfig> {
    serial_writeln(usb, "");
    serial_writeln(usb, "=== Change credentials (password required) ===");
    serial_writeln(usb, "Use on-screen keyboard or USB serial.");

    for attempt in 1..=MAX_PASSWORD_ATTEMPTS {
        serial_write(usb, "current password");
        if attempt > 1 {
            serial_write(usb, " (retry)");
        }
        serial_write(usb, ": ");
        let _ = usb.flush();

        let mut line: String<128> = String::new();
        read_field_touch_or_serial(
            usb,
            display,
            touch,
            touch_delay,
            SetupField::Password,
            &mut line,
            true,
            Some((attempt, MAX_PASSWORD_ATTEMPTS)),
        )
        .await;

        match current.authorize(line.as_str()) {
            Ok(()) => {
                serial_writeln(usb, "  ok — enter new values");
                let cfg =
                    collect_pool_config(usb, display, touch, touch_delay, wifi_token).await;
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

async fn wait_for_change_or_touch(
    usb: &mut Serial<'_>,
    touch: &mut Touch,
    touch_delay: &mut Delay,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    let mut line: String<32> = String::new();
    let mut byte = [0u8; 1];

    while Instant::now() < deadline {
        if let Some(p) = touch.poll_tap(touch_delay) {
            if matches!(hit_gui(p, false), Some(GuiHit::Tab(1) | GuiHit::ChangeBanner)) {
                return true;
            }
        }
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
    touch: &mut Touch,
    touch_delay: &mut Delay,
    wifi_token: &mut Option<WIFI<'static>>,
) -> PoolConfig {
    let mut cfg = PoolConfig::new();
    let mut skip_wifi_password = false;

    serial_writeln(usb, "");
    serial_writeln(usb, "=== ESP32-2432S028 Scrypt Miner setup ===");
    serial_writeln(usb, "Step 1: scan & tap a WiFi network (or type / skip).");
    serial_writeln(usb, "Serial: number from scan list, SSID text, or '-' to skip.");
    serial_writeln(usb, "Then WiFi password (if needed), stratum, worker, pool password, BLE.");
    serial_writeln(usb, "");

    for field in SetupField::ALL {
        if field == SetupField::WifiPassword && (!cfg.wifi_enabled() || skip_wifi_password) {
            continue;
        }
        if !field.allows_empty() && !cfg.get(field).is_empty() {
            continue;
        }

        if field == SetupField::WifiSsid {
            match pick_wifi_ssid(usb, display, touch, touch_delay, wifi_token).await {
                WifiPick::Network { ssid, open } => {
                    match cfg.set(SetupField::WifiSsid, ssid.as_str()) {
                        Ok(()) => {
                            serial_write(usb, "  ok (wifi_ssid=");
                            serial_write(usb, ssid.as_str());
                            serial_writeln(usb, ")");
                            skip_wifi_password = open;
                            if open {
                                let _ = cfg.set(SetupField::WifiPassword, "");
                                serial_writeln(usb, "  open network — no password");
                            }
                        }
                        Err(e) => {
                            serial_write(usb, "  error: ");
                            serial_writeln(usb, config_error_msg(e));
                        }
                    }
                }
                WifiPick::Skip => {
                    let _ = cfg.set(SetupField::WifiSsid, "-");
                    serial_writeln(usb, "  ok (wifi skipped)");
                }
            }
            continue;
        }

        loop {
            serial_write(usb, field.label());
            serial_write(usb, ": ");
            let _ = usb.flush();

            let mut line: String<128> = String::new();
            read_field_touch_or_serial(
                usb,
                display,
                touch,
                touch_delay,
                field,
                &mut line,
                field.is_secret(),
                None,
            )
            .await;

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

enum WifiPick {
    Network { ssid: String<32>, open: bool },
    Skip,
}

async fn pick_wifi_ssid<D: embedded_hal::delay::DelayNs>(
    usb: &mut Serial<'_>,
    display: &mut Display<'_, D>,
    touch: &mut Touch,
    touch_delay: &mut Delay,
    wifi_token: &mut Option<WIFI<'static>>,
) -> WifiPick {
    let mut networks: heapless::Vec<ScannedNetwork, 8> = heapless::Vec::new();
    let mut scroll = 0usize;
    let mut status: String<40> = String::new();
    let mut dirty = true;
    let mut byte = [0u8; 1];
    let mut serial_buf: String<64> = String::new();

    // Initial scan when the WIFI peripheral is still available.
    if let Some(wifi) = wifi_token.take() {
        let _ = status.push_str("scanning…");
        let _ = display.draw_wifi_scan(&[], 0, status.as_str());
        serial_writeln(usb, "Scanning WiFi…");
        match radio::scan_networks(wifi).await {
            Ok(list) => {
                networks = list;
                status.clear();
                if networks.is_empty() {
                    let _ = status.push_str("no networks — tap scan/type/skip");
                }
                print_scan_list(usb, &networks);
            }
            Err(()) => {
                status.clear();
                let _ = status.push_str("scan failed — tap type or skip");
                serial_writeln(usb, "WiFi scan failed.");
            }
        }
        dirty = true;
    } else {
        let _ = status.push_str("tap type to enter SSID");
        serial_writeln(usb, "WiFi scan unavailable — type SSID or '-'.");
    }

    loop {
        if dirty {
            let _ = display.draw_wifi_scan(&networks, scroll, status.as_str());
            dirty = false;
        }

        if let Some(p) = touch.poll_tap(touch_delay) {
            log_touch(usb, &touch, p);
            match hit_wifi_scan(p, scroll, networks.len()) {
                Some(WifiScanHit::Select(i)) => {
                    if let Some(n) = networks.get(i) {
                        let mut ssid: String<32> = String::new();
                        let _ = ssid.push_str(n.ssid.as_str());
                        return WifiPick::Network {
                            ssid,
                            open: n.open,
                        };
                    }
                }
                Some(WifiScanHit::ScrollUp) => {
                    scroll = scroll.saturating_sub(1);
                    dirty = true;
                }
                Some(WifiScanHit::ScrollDown) => {
                    let max_scroll = networks.len().saturating_sub(1);
                    if scroll < max_scroll {
                        scroll += 1;
                        dirty = true;
                    }
                }
                Some(WifiScanHit::Rescan) => {
                    // Reclaim WIFI after prior scan drop.
                    let wifi = unsafe { WIFI::steal() };
                    status.clear();
                    let _ = status.push_str("scanning…");
                    let _ = display.draw_wifi_scan(&networks, scroll, status.as_str());
                    serial_writeln(usb, "Rescanning WiFi…");
                    match radio::scan_networks(wifi).await {
                        Ok(list) => {
                            networks = list;
                            scroll = 0;
                            status.clear();
                            if networks.is_empty() {
                                let _ = status.push_str("no networks — tap scan/type/skip");
                            }
                            print_scan_list(usb, &networks);
                        }
                        Err(()) => {
                            status.clear();
                            let _ = status.push_str("scan failed — tap type or skip");
                            serial_writeln(usb, "WiFi scan failed.");
                        }
                    }
                    dirty = true;
                }
                Some(WifiScanHit::TypeManual) => {
                    let mut line: String<128> = String::new();
                    serial_write(usb, "wifi_ssid (type): ");
                    let _ = usb.flush();
                    read_field_touch_or_serial(
                        usb,
                        display,
                        touch,
                        touch_delay,
                        SetupField::WifiSsid,
                        &mut line,
                        false,
                        None,
                    )
                    .await;
                    let trimmed = line.as_str().trim();
                    if trimmed.is_empty() || trimmed == "-" || eq_ignore_ascii_case(trimmed, "skip")
                    {
                        return WifiPick::Skip;
                    }
                    let mut ssid: String<32> = String::new();
                    let _ = ssid.push_str(trimmed);
                    return WifiPick::Network {
                        ssid,
                        open: false,
                    };
                }
                Some(WifiScanHit::Skip) => return WifiPick::Skip,
                None => {}
            }
            continue;
        }

        match usb.read(&mut byte) {
            Ok(0) | Err(_) => {
                Timer::after(Duration::from_millis(12)).await;
            }
            Ok(_) => {
                let c = byte[0];
                match c {
                    b'\n' | b'\r' => {
                        let trimmed = serial_buf.as_str().trim();
                        if trimmed.is_empty() {
                            serial_buf.clear();
                            continue;
                        }
                        if trimmed == "-" || eq_ignore_ascii_case(trimmed, "skip") {
                            return WifiPick::Skip;
                        }
                        if let Ok(n) = trimmed.parse::<usize>() {
                            if (1..=networks.len()).contains(&n) {
                                let net = &networks[n - 1];
                                let mut ssid: String<32> = String::new();
                                let _ = ssid.push_str(net.ssid.as_str());
                                return WifiPick::Network {
                                    ssid,
                                    open: net.open,
                                };
                            }
                        }
                        // Treat as typed SSID.
                        let mut ssid: String<32> = String::new();
                        let _ = ssid.push_str(trimmed);
                        serial_buf.clear();
                        return WifiPick::Network {
                            ssid,
                            open: false,
                        };
                    }
                    0x08 | 0x7f => {
                        let _ = serial_buf.pop();
                        let _ = usb.write_all(b"\x08 \x08");
                    }
                    c if (32..127).contains(&c) => {
                        if serial_buf.push(c as char).is_ok() {
                            let _ = usb.write_all(&[c]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn log_touch(usb: &mut Serial<'_>, touch: &Touch, p: esp32_s3_scrypt_miner::keyboard::TouchPoint) {
    let mut m: String<64> = String::new();
    if let Some((rx, ry, z)) = touch.last_raw {
        let _ = core::fmt::Write::write_fmt(
            &mut m,
            format_args!("tap screen=({},{}) raw=({},{},z={})", p.x, p.y, rx, ry, z),
        );
    } else {
        let _ = core::fmt::Write::write_fmt(
            &mut m,
            format_args!("tap screen=({},{})", p.x, p.y),
        );
    }
    serial_writeln(usb, m.as_str());
}

fn print_scan_list(usb: &mut Serial<'_>, networks: &[ScannedNetwork]) {
    if networks.is_empty() {
        serial_writeln(usb, "(no networks found)");
        return;
    }
    serial_writeln(usb, "Networks:");
    for (i, n) in networks.iter().enumerate() {
        let mut line: String<64> = String::new();
        let kind = if n.open { "open" } else { "lock" };
        let _ = core::fmt::Write::write_fmt(
            &mut line,
            format_args!("  {}. {}  {} {}dBm", i + 1, n.ssid, kind, n.rssi),
        );
        serial_writeln(usb, line.as_str());
    }
    serial_writeln(usb, "Enter number, SSID, or '-' to skip.");
}

async fn wait_for_ip_screen<D: embedded_hal::delay::DelayNs>(
    usb: &mut Serial<'_>,
    display: &mut Display<'_, D>,
    touch: &mut Touch,
    touch_delay: &mut Delay,
    pool: &PoolConfig,
) {
    serial_writeln(usb, "Waiting for DHCP / IP…");
    let _ = display.draw_connecting(pool.wifi_ssid.as_str());
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut byte = [0u8; 1];
    while Instant::now() < deadline {
        let st = radio::snapshot().await;
        if let Some(ip) = st.ip {
            serial_write(usb, "IP address: ");
            serial_writeln(usb, st.ip_string().as_str());
            let _ = display.draw_online(pool.wifi_ssid.as_str(), ip);
            let show_until = Instant::now() + Duration::from_secs(6);
            while Instant::now() < show_until {
                if touch.poll_tap(touch_delay).is_some() {
                    break;
                }
                let _ = usb.read(&mut byte);
                Timer::after(Duration::from_millis(40)).await;
            }
            return;
        }
        let _ = usb.read(&mut byte);
        Timer::after(Duration::from_millis(250)).await;
    }
    serial_writeln(usb, "DHCP timeout — check WiFi password / signal.");
    Timer::after(Duration::from_secs(2)).await;
}

/// Collect one field via on-screen keyboard and/or USB serial.
async fn read_field_touch_or_serial<D: embedded_hal::delay::DelayNs>(
    usb: &mut Serial<'_>,
    display: &mut Display<'_, D>,
    touch: &mut Touch,
    touch_delay: &mut Delay,
    field: SetupField,
    line: &mut String<128>,
    secret: bool,
    auth: Option<(u8, u8)>,
) {
    line.clear();
    let mut kb = Keyboard::default();
    let mut dirty = true;
    let mut byte = [0u8; 1];

    loop {
        if dirty {
            if let Some((attempt, max)) = auth {
                let _ = display.draw_auth_keyboard(attempt, max, line.as_str(), &kb);
            } else {
                let _ = display.draw_setup_keyboard(field, line.as_str(), &kb);
            }
            dirty = false;
        }

        if let Some(p) = touch.poll_tap(touch_delay) {
            log_touch(usb, touch, p);
            if let Some(action) = kb.hit_test(p) {
                if kb.apply(action, line) {
                    let _ = usb.write_all(b"\r\n");
                    return;
                }
                dirty = true;
                continue;
            }
            serial_writeln(usb, "  (tap missed key — try again)");
        }

        match usb.read(&mut byte) {
            Ok(0) | Err(_) => {
                Timer::after(Duration::from_millis(12)).await;
            }
            Ok(_) => {
                let c = byte[0];
                match c {
                    b'\n' | b'\r' => {
                        let _ = usb.write_all(b"\r\n");
                        return;
                    }
                    0x08 | 0x7f => {
                        if line.pop().is_some() {
                            let _ = usb.write_all(b"\x08 \x08");
                            dirty = true;
                        }
                    }
                    c if (32..127).contains(&c) => {
                        if line.push(c as char).is_ok() {
                            if secret {
                                let _ = usb.write_all(b"*");
                            } else {
                                let _ = usb.write_all(&[c]);
                            }
                            dirty = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
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

