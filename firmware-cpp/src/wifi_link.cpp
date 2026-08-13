#include "wifi_link.hpp"
#include <WiFiUdp.h>

void WifiLink::begin(const char* macStr, const AppConfig& cfg) {
  mac_ = macStr ? macStr : "unknown";
  // SoftAP SSID: Njordr-XXXX from last MAC nibbles
  String hex;
  for (size_t i = 0; i < mac_.length(); i++) {
    char c = mac_[i];
    if ((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) hex += c;
  }
  String tail = hex.length() >= 4 ? hex.substring(hex.length() - 4) : String("cyd");
  tail.toUpperCase();
  apSsid_ = String("Njordr-") + tail;
  ensureWifi(cfg);
  started_ = true;
}

void WifiLink::applyConfig(const AppConfig& cfg) {
  ensureWifi(cfg);
}

const char* WifiLink::modeLabel() const {
  if (!started_) return "off";
  wifi_mode_t m = WiFi.getMode();
  if (m == WIFI_OFF) return "off";
  if (m == WIFI_AP_STA) return "apsta";
  if (m == WIFI_AP) return "ap";
  if (m == WIFI_STA) return "sta";
  return "off";
}

void WifiLink::ensureWifi(const AppConfig& cfg) {
  if (!cfg.wifiEnabled) {
    if (client_ && client_.connected()) client_.stop();
    server_.end();
    WiFi.mode(WIFI_OFF);
    staWanted_ = false;
    return;
  }

  staWanted_ = cfg.wifiSsid.length() > 0;
  if (staWanted_) {
    WiFi.mode(WIFI_AP_STA);
    WiFi.begin(cfg.wifiSsid.c_str(), cfg.wifiPass.c_str());
  } else {
    WiFi.mode(WIFI_AP);
  }

  // Channel 1 SoftAP — Companion can join or hear UDP on the LAN when STA is up.
  bool ok = WiFi.softAP(apSsid_.c_str(), CYD_SOFTAP_PASS, 1, 0, 4);
  (void)ok;
  delay(40);
  server_.begin();
  server_.setNoDelay(true);
  udp_.begin(CYD_WIFI_UDP_PORT);
  lastBeaconMs_ = 0;
}

void WifiLink::beacon() {
  uint32_t now = millis();
  if (now - lastBeaconMs_ < 2500) return;
  lastBeaconMs_ = now;

  IPAddress ap = WiFi.softAPIP();
  IPAddress sta = WiFi.localIP();
  IPAddress advertise = (WiFi.status() == WL_CONNECTED) ? sta : ap;
  char msg[220];
#if CYD_D0_BUILD
  static constexpr const char* kFwTag = "0.8.66-sha256-d0";
  static constexpr const char* kFwShort = "0.8.66-d0";
#else
  static constexpr const char* kFwTag = "0.8.66-sha256";
  static constexpr const char* kFwShort = "0.8.66";
#endif
  snprintf(msg, sizeof(msg),
           "%s|v=%s|mac=%s|fw=%s|tcp=%u|ip=%u.%u.%u.%u|ap=%s|mode=%s",
           CYD_WIFI_MAGIC, kFwShort, mac_.c_str(), kFwTag, (unsigned)CYD_WIFI_TCP_PORT,
           advertise[0], advertise[1], advertise[2], advertise[3], apSsid_.c_str(), modeLabel());

  // Broadcast on SoftAP subnet and STA subnet when available.
  IPAddress bcast(255, 255, 255, 255);
  udp_.beginPacket(bcast, CYD_WIFI_UDP_PORT);
  udp_.write((const uint8_t*)msg, strlen(msg));
  udp_.endPacket();

  if (WiFi.status() == WL_CONNECTED) {
    IPAddress gw = WiFi.gatewayIP();
    IPAddress sb = WiFi.subnetMask();
    IPAddress local = WiFi.localIP();
    IPAddress directed((local[0] & sb[0]) | (~sb[0] & 255), (local[1] & sb[1]) | (~sb[1] & 255),
                       (local[2] & sb[2]) | (~sb[2] & 255), (local[3] & sb[3]) | (~sb[3] & 255));
    (void)gw;
    udp_.beginPacket(directed, CYD_WIFI_UDP_PORT);
    udp_.write((const uint8_t*)msg, strlen(msg));
    udp_.endPacket();
  }
}

void WifiLink::acceptClient() {
  if (client_ && client_.connected()) return;
  if (client_) client_.stop();
  WiFiClient incoming = server_.available();
  if (incoming) {
    client_ = incoming;
    client_.setNoDelay(true);
    client_.setTimeout(50);
  }
}

void WifiLink::poll(CompanionLink& cmp, AppConfig& cfg, const MinerSnapshot& snap,
                    CompanionLink::ApplyFn onApply, NetFeed* net, CompanionLink::JobFn onJob,
                    CompanionLink::StopFn onStop, CompanionLink::StatsFn onStats) {
  if (!cfg.wifiEnabled) {
    cmp.setShareMirror(nullptr);
    return;
  }
  if (!started_) begin(mac_.c_str(), cfg);

  uint32_t now = millis();
  if (now - lastWifiCheckMs_ > 8000) {
    lastWifiCheckMs_ = now;
    // SoftAP can drop after mode flaps — refresh lightly.
    if (WiFi.getMode() == WIFI_OFF) ensureWifi(cfg);
  }

  acceptClient();
  if (client_ && client_.connected()) {
    cmp.setShareMirror(&client_);
    // Dedicated line buffer for TCP so USB and Wi‑Fi don't corrupt each other.
    // pollStream uses activeLineBuf set by caller — CompanionLink::poll uses USB buf;
    // we need a TCP-specific entry. Use pollStream with tcp buffers via a small hack:
    // CompanionLink exposes pollStream which uses activeLineBuf_; set via poll() only.
    // So call a method that switches buffers — added as pollStream which expects
    // active buffers. We'll set them by calling through a TCP-specific poll on cmp.
    // Simpler: use pollStream and accept that TCP uses tcpLineBuf if we add pollTcp.
    cmp.pollTcp(client_, client_, cfg, snap, onApply, net, onJob, onStop, onStats);
  } else {
    cmp.setShareMirror(nullptr);
  }
  beacon();
}
