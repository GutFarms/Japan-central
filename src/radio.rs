//! Onboard WiFi (STA + DHCP) and Bluetooth LE advertising status / tasks.
//!
//! Status types are available on host builds; the radio stack only runs with `esp`.
//!
//! Uses `esp-radio` + `embassy-net` for WiFi and `trouble-host` 0.6 (bt-hci 0.8)
//! for BLE so versions stay aligned with `esp-radio` 1.0.0-beta.0.

use core::fmt;
use heapless::String;

use crate::config::{BleNameString, WIFI_SSID_MAX};

/// WiFi connection phase shown on the Radio tab / serial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WifiPhase {
    Disabled,
    Starting,
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

impl WifiPhase {
    pub fn label(self) -> &'static str {
        match self {
            WifiPhase::Disabled => "off",
            WifiPhase::Starting => "start",
            WifiPhase::Connecting => "assoc",
            WifiPhase::Connected => "up",
            WifiPhase::Disconnected => "down",
            WifiPhase::Failed => "fail",
        }
    }
}

/// Snapshot of onboard radio state for GUI / serial.
#[derive(Clone, Debug)]
pub struct RadioStatus {
    pub wifi: WifiPhase,
    pub ssid: String<WIFI_SSID_MAX>,
    pub ip: Option<[u8; 4]>,
    pub ble_advertising: bool,
    pub ble_connected: bool,
    pub ble_name: BleNameString,
}

impl Default for RadioStatus {
    fn default() -> Self {
        Self {
            wifi: WifiPhase::Disabled,
            ssid: String::new(),
            ip: None,
            ble_advertising: false,
            ble_connected: false,
            ble_name: BleNameString::new(),
        }
    }
}

impl RadioStatus {
    pub fn ip_string(&self) -> String<16> {
        let mut s = String::new();
        match self.ip {
            Some([a, b, c, d]) => {
                let _ = fmt::Write::write_fmt(&mut s, format_args!("{a}.{b}.{c}.{d}"));
            }
            None => {
                let _ = s.push_str("---");
            }
        }
        s
    }
}

#[cfg(feature = "esp")]
mod stack {
    use alloc::string::String as AllocString;

    use embassy_executor::Spawner;
    use embassy_futures::join::join;
    use embassy_net::{Runner, Stack, StackResources};
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::mutex::Mutex;
    use embassy_time::{Duration, Timer};
    use esp_hal::peripherals::{BT, WIFI};
    use esp_hal::rng::Rng;
    use esp_radio::ble::controller::BleConnector;
    use esp_radio::wifi::{
        AuthenticationMethod, Config, ControllerConfig, Interface, WifiController,
        sta::StationConfig,
    };
    use log::info;
    use static_cell::StaticCell;
    use trouble_host::prelude::*;

    use super::{RadioStatus, WifiPhase};
    use crate::config::PoolConfig;

    static STATUS: Mutex<CriticalSectionRawMutex, RadioStatus> = Mutex::new(RadioStatus {
        wifi: WifiPhase::Disabled,
        ssid: heapless::String::new(),
        ip: None,
        ble_advertising: false,
        ble_connected: false,
        ble_name: heapless::String::new(),
    });

    // DNS + stratum TCP + HTTP server (+ spare). Keep small — classic ESP32 RAM.
    static STACK_RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    static BLE_NAME_BUF: StaticCell<[u8; 24]> = StaticCell::new();

    /// Advertise-only BLE (no GATT) — avoids embassy-sync version skew with trouble-host.
    const CONNECTIONS_MAX: usize = 1;
    const L2CAP_CHANNELS_MAX: usize = 2;

    fn spawn_task<S>(
        spawner: &Spawner,
        token: Result<embassy_executor::SpawnToken<S>, embassy_executor::SpawnError>,
        what: &str,
    ) {
        match token {
            Ok(t) => spawner.spawn(t),
            Err(_) => info!("failed to spawn {what}"),
        }
    }

    pub async fn snapshot() -> RadioStatus {
        STATUS.lock().await.clone()
    }

    async fn set_wifi_phase(phase: WifiPhase) {
        let mut s = STATUS.lock().await;
        s.wifi = phase;
        if phase != WifiPhase::Connected {
            s.ip = None;
        }
    }

    async fn set_ip(ip: Option<[u8; 4]>) {
        let mut s = STATUS.lock().await;
        s.ip = ip;
        if ip.is_some() {
            s.wifi = WifiPhase::Connected;
        }
    }

    async fn set_ble(advertising: bool, connected: bool) {
        let mut s = STATUS.lock().await;
        s.ble_advertising = advertising;
        s.ble_connected = connected;
    }

    fn seed_status(cfg: &PoolConfig) {
        if let Ok(mut s) = STATUS.try_lock() {
            s.wifi = if cfg.wifi_enabled() {
                WifiPhase::Starting
            } else {
                WifiPhase::Disabled
            };
            s.ssid.clear();
            let _ = s.ssid.push_str(cfg.wifi_ssid.as_str());
            s.ip = None;
            s.ble_advertising = false;
            s.ble_connected = false;
            s.ble_name.clear();
            if cfg.ble_enabled() {
                let _ = s.ble_name.push_str(cfg.ble_name.as_str());
            }
        }
    }

    fn start_ble(spawner: &Spawner, bt: BT<'static>, ble_name: &str) {
        let name_buf = BLE_NAME_BUF.init([0u8; 24]);
        let ble_bytes = ble_name.as_bytes();
        let n = core::cmp::min(ble_bytes.len(), name_buf.len());
        name_buf[..n].copy_from_slice(&ble_bytes[..n]);
        let ble_name_bytes: &'static [u8] = &name_buf[..n];

        let connector = match BleConnector::new(bt, Default::default()) {
            Ok(c) => c,
            Err(e) => {
                info!("BLE init failed: {e:?}");
                return;
            }
        };
        let ble_controller: ExternalController<_, 1> = ExternalController::new(connector);
        spawn_task(spawner, ble_task(ble_controller, ble_name_bytes), "BLE");
    }

    fn start_wifi(
        spawner: &Spawner,
        wifi: WIFI<'static>,
        cfg: &PoolConfig,
    ) -> Option<Stack<'static>> {
        let ssid = cfg.wifi_ssid.as_str();
        let password = cfg.wifi_password.as_str();

        let mut station = StationConfig::default().with_ssid(ssid);
        if password.is_empty() {
            station = station.with_auth_method(AuthenticationMethod::None);
        } else {
            // StationConfig::password is alloc::String; builder takes it by value.
            station = station.with_password(AllocString::from(password));
        }

        let wifi_interface = Interface::station();
        let controller = match WifiController::new(
            wifi,
            ControllerConfig::default().with_initial_config(Config::Station(station)),
        ) {
            Ok(c) => c,
            Err(e) => {
                info!("WiFi init failed: {e:?}");
                if let Ok(mut s) = STATUS.try_lock() {
                    s.wifi = WifiPhase::Failed;
                }
                return None;
            }
        };

        let net_config = embassy_net::Config::dhcpv4(Default::default());
        let rng = Rng::new();
        let seed = (rng.random() as u64) << 32 | rng.random() as u64;
        let (stack, runner) = embassy_net::new(
            wifi_interface,
            net_config,
            STACK_RESOURCES.init(StackResources::<5>::new()),
            seed,
        );

        info!("WiFi starting for SSID={ssid}");
        spawn_task(spawner, connection(controller), "WiFi connection");
        spawn_task(spawner, net_task(runner), "net runner");
        spawn_task(spawner, dhcp_watch(stack), "DHCP watch");
        Some(stack)
    }

    /// Start optional BLE advertising and, when configured, WiFi STA + DHCP.
    ///
    /// Returns the embassy-net [`Stack`] when WiFi was started so callers can
    /// open TCP (stratum) sockets. BLE is opt-in (`ble_name`) so WiFi/stratum
    /// mining can keep more RAM free.
    pub fn start(
        spawner: &Spawner,
        wifi: WIFI<'static>,
        bt: BT<'static>,
        cfg: &PoolConfig,
    ) -> Option<Stack<'static>> {
        seed_status(cfg);

        // Prefer WiFi when both are set — classic ESP32 can't comfortably run
        // WiFi+BLE without coexistence + a lot more RAM.
        let stack = if cfg.wifi_enabled() {
            if cfg.ble_enabled() {
                info!("BLE requested but WiFi active — skipping BLE (no coex)");
            }
            let _ = bt;
            start_wifi(spawner, wifi, cfg)
        } else if cfg.ble_enabled() {
            start_ble(spawner, bt, cfg.ble_name.as_str());
            let _ = wifi;
            None
        } else {
            info!("WiFi skipped (no SSID); BLE skipped (no ble_name)");
            let _ = wifi;
            let _ = bt;
            None
        };
        stack
    }

    #[embassy_executor::task]
    async fn connection(mut controller: WifiController<'static>) {
        info!("WiFi connection task");
        loop {
            set_wifi_phase(WifiPhase::Connecting).await;
            match controller.connect_async().await {
                Ok(info) => {
                    info!("WiFi connected: {info:?}");
                    set_wifi_phase(WifiPhase::Connected).await;
                    let _ = controller.wait_for_disconnect_async().await;
                    info!("WiFi disconnected");
                    set_wifi_phase(WifiPhase::Disconnected).await;
                }
                Err(e) => {
                    info!("WiFi connect failed: {e:?}");
                    set_wifi_phase(WifiPhase::Failed).await;
                }
            }
            Timer::after(Duration::from_secs(5)).await;
        }
    }

    #[embassy_executor::task]
    async fn net_task(mut runner: Runner<'static, Interface>) {
        runner.run().await
    }

    #[embassy_executor::task]
    async fn dhcp_watch(stack: embassy_net::Stack<'static>) {
        loop {
            stack.wait_config_up().await;
            if let Some(cfg) = stack.config_v4() {
                let octets = cfg.address.address().octets();
                info!(
                    "DHCP got IP {}.{}.{}.{}",
                    octets[0], octets[1], octets[2], octets[3]
                );
                set_ip(Some(octets)).await;
            }
            loop {
                Timer::after(Duration::from_secs(2)).await;
                match stack.config_v4() {
                    Some(cfg) => {
                        set_ip(Some(cfg.address.address().octets())).await;
                    }
                    None => {
                        set_ip(None).await;
                        set_wifi_phase(WifiPhase::Disconnected).await;
                        break;
                    }
                }
            }
        }
    }

    #[embassy_executor::task]
    async fn ble_task(
        controller: ExternalController<BleConnector<'static>, 1>,
        ble_name: &'static [u8],
    ) {
        let address = Address::random([0x42, 0x53, 0x43, 0x52, 0x59, 0x50]);
        let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
            HostResources::new();
        let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
        let Host {
            mut peripheral,
            mut runner,
            ..
        } = stack.build();

        let mut adv_data = [0; 31];
        let adv_len = match AdStructure::encode_slice(
            &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::CompleteLocalName(ble_name),
            ],
            &mut adv_data[..],
        ) {
            Ok(len) => len,
            Err(e) => {
                info!("BLE adv encode failed: {e:?}");
                return;
            }
        };

        info!(
            "BLE advertising (non-connectable) as {:?}",
            core::str::from_utf8(ble_name)
        );
        let _ = join(runner.run(), async {
            let mut params = AdvertisementParameters::default();
            params.interval_min = Duration::from_millis(200);
            params.interval_max = Duration::from_millis(200);

            loop {
                set_ble(true, false).await;
                match peripheral
                    .advertise(
                        &params,
                        Advertisement::NonconnectableScannableUndirected {
                            adv_data: &adv_data[..adv_len],
                            scan_data: &[],
                        },
                    )
                    .await
                {
                    Ok(_advertiser) => {
                        // Keep advertising until error; non-connectable has no accept loop.
                        Timer::after(Duration::from_secs(30)).await;
                    }
                    Err(e) => {
                        info!("BLE advertise error: {e:?}");
                        set_ble(false, false).await;
                        Timer::after(Duration::from_secs(2)).await;
                    }
                }
            }
        })
        .await;
    }
}

#[cfg(feature = "esp")]
pub use stack::{snapshot, start};
