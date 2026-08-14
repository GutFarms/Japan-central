#include "wifi_link.hpp"
#include <esp_wifi.h>
#include <cstdio>
#include <cstring>

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
    stopPortal();
    WiFi.mode(WIFI_OFF);
    staWanted_ = false;
    softApUp_ = false;
    lastStaSsid_ = "";
    lastStaPass_ = "";
    return;
  }

  const bool wantSta = cfg.wifiSsid.length() > 0;
  const bool staCredsChanged =
      wantSta && (cfg.wifiSsid != lastStaSsid_ || cfg.wifiPass != lastStaPass_);
  const bool modeChanged = wantSta != staWanted_;
  staWanted_ = wantSta;

  if (staWanted_) {
    WiFi.mode(WIFI_AP_STA);
    // Only call begin when SSID/pass change — avoid SoftAP flaps on every applyConfig.
    if (staCredsChanged) {
      WiFi.begin(cfg.wifiSsid.c_str(), cfg.wifiPass.c_str());
      lastStaSsid_ = cfg.wifiSsid;
      lastStaPass_ = cfg.wifiPass;
    }
  } else {
    WiFi.mode(WIFI_AP);
    lastStaSsid_ = "";
    lastStaPass_ = "";
  }

  // Unique SoftAP subnet from MAC so multiple boards aren't all 192.168.4.1.
  // 10.<b4>.<b5>.1 /24 — PC/phone joins one Njordr-XXXX AP at a time, but beacons
  // and TCP endpoints stay distinct when STA is used on a shared LAN.
  uint8_t b4 = 1, b5 = 1;
  {
    String hex;
    for (size_t i = 0; i < mac_.length(); i++) {
      char c = mac_[i];
      if ((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) hex += c;
    }
    hex.toLowerCase();
    if (hex.length() >= 12) {
      b4 = (uint8_t)strtoul(hex.substring(8, 10).c_str(), nullptr, 16);
      b5 = (uint8_t)strtoul(hex.substring(10, 12).c_str(), nullptr, 16);
      if (b4 == 0) b4 = 1;
      if (b5 == 0) b5 = 1;
    }
  }
  IPAddress apIp(10, b4, b5, 1);
  IPAddress apGw(10, b4, b5, 1);
  IPAddress apMask(255, 255, 255, 0);
  WiFi.softAPConfig(apIp, apGw, apMask);

  // Channel 1 SoftAP — Companion can join or hear UDP on the LAN when STA is up.
  // Re-create SoftAP only when bringing Wi‑Fi up or subnet/SSID may have changed.
  if (!softApUp_ || modeChanged || staCredsChanged) {
    bool ok = WiFi.softAP(apSsid_.c_str(), CYD_SOFTAP_PASS, 1, 0, 4);
    softApUp_ = ok;
    (void)ok;
    delay(40);
    // SoftAP recreate can leave the radio off ch1 — re-pin for ESP-NOW mesh.
    esp_wifi_set_channel(1, WIFI_SECOND_CHAN_NONE);
    server_.begin();
    server_.setNoDelay(true);
    udp_.begin(CYD_WIFI_UDP_PORT);
    // Phone captive portal + HTTP setup for home Wi‑Fi credentials.
    startPortal();
  }
  lastBeaconMs_ = 0;
}

void WifiLink::startPortal() {
  if (portalUp_) {
    // SoftAP IP may have changed after recreate — restart DNS bind.
    stopPortal();
  }
  IPAddress ap = WiFi.softAPIP();
  dns_.setErrorReplyCode(DNSReplyCode::NoError);
  dns_.start(CYD_WIFI_DNS_PORT, "*", ap);

  http_.on("/", HTTP_GET, [this]() { handlePortalRoot(); });
  http_.on("/setup", HTTP_GET, [this]() { handlePortalRoot(); });
  http_.on("/save", HTTP_GET, [this]() { handlePortalSave(); });
  http_.on("/save", HTTP_POST, [this]() { handlePortalSave(); });
  // Captive-portal probes — open the phone "Sign in" browser onto Setup.
  http_.on("/generate_204", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/gen_204", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/hotspot-detect.html", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/library/test/success.html", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/connecttest.txt", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/ncsi.txt", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/fwlink/", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.on("/fwlink", HTTP_GET, [this]() { handlePortalCaptive(); });
  http_.onNotFound([this]() { handlePortalCaptive(); });
  http_.begin();
  portalUp_ = true;
}

void WifiLink::stopPortal() {
  if (!portalUp_) return;
  http_.stop();
  dns_.stop();
  portalUp_ = false;
}

void WifiLink::pollPortal(AppConfig& cfg) {
  if (!portalUp_) return;
  portalCfg_ = &cfg;
  dns_.processNextRequest();
  http_.handleClient();
}

String WifiLink::portalPageHtml(bool saved) const {
  IPAddress ap = WiFi.softAPIP();
  char ip[20];
  snprintf(ip, sizeof(ip), "%u.%u.%u.%u", ap[0], ap[1], ap[2], ap[3]);
  String page;
  page.reserve(1600);
  page += F("<!DOCTYPE html><html><head><meta charset=utf-8>"
            "<meta name=viewport content=\"width=device-width,initial-scale=1\">"
            "<title>Njörðr Setup</title><style>"
            "body{font-family:system-ui,-apple-system,sans-serif;background:#04141f;"
            "color:#e6f2fc;margin:0;padding:22px;line-height:1.35}"
            "h1{color:#7edcff;font-size:1.55rem;margin:0 0 6px}"
            "p{color:#9bb8cc;margin:0 0 14px;font-size:.95rem}"
            "label{display:block;margin:12px 0 6px;color:#c5dceb;font-size:.9rem}"
            "input{width:100%;box-sizing:border-box;padding:14px;border:0;border-radius:10px;"
            "font-size:16px;background:#0b2433;color:#e6f2fc}"
            "button{width:100%;margin-top:18px;padding:15px;border:0;border-radius:10px;"
            "background:#7edcff;color:#04141f;font-size:1.05rem;font-weight:700}"
            ".ok{background:#12301f;color:#9dffb0;padding:12px;border-radius:10px;margin-bottom:14px}"
            ".meta{margin-top:18px;font-size:.8rem;color:#6f8fa8}"
            "</style></head><body><h1>Njörðr</h1>");
  if (saved) {
    page += F("<div class=ok>Saved — board is joining home Wi‑Fi. "
              "Reconnect this phone to your home network.</div>");
  }
  page += F("<p>Enter your home Wi‑Fi so this board can mine on your LAN.</p>"
            "<form method=POST action=/save>"
            "<label>Home Wi‑Fi name (SSID)</label>"
            "<input name=ssid maxlength=32 required autocomplete=username "
            "placeholder=\"Router Wi‑Fi name\">"
            "<label>Password</label>"
            "<input name=pass type=password maxlength=63 autocomplete=current-password "
            "placeholder=\"Leave blank if open\">"
            "<button type=submit>Save &amp; connect</button></form><p class=meta>");
  page += apSsid_;
  page += F(" · open SoftAP · http://");
  page += ip;
  page += F("</p></body></html>");
  return page;
}

void WifiLink::handlePortalRoot() {
  http_.send(200, "text/html", portalPageHtml(false));
}

void WifiLink::handlePortalCaptive() {
  // Force the phone captive browser onto our setup page.
  String loc = String("http://") + WiFi.softAPIP().toString() + "/";
  http_.sendHeader("Location", loc, true);
  http_.sendHeader("Cache-Control", "no-cache");
  http_.send(302, "text/plain", "Redirecting to Njörðr Setup");
}

void WifiLink::handlePortalSave() {
  if (!portalCfg_) {
    http_.send(503, "text/plain", "Portal busy — retry");
    return;
  }
  String ssid = http_.hasArg("ssid") ? http_.arg("ssid") : "";
  String pass = http_.hasArg("pass") ? http_.arg("pass") : "";
  if (http_.hasArg("password") && pass.isEmpty()) pass = http_.arg("password");
  ssid.trim();
  if (ssid.isEmpty()) {
    http_.send(400, "text/html",
               F("<!DOCTYPE html><html><body><p>SSID required.</p>"
                 "<a href=/>Back</a></body></html>"));
    return;
  }
  if (ssid.length() > 32) ssid = ssid.substring(0, 32);
  if (pass.length() > 63) pass = pass.substring(0, 63);

  portalCfg_->wifiEnabled = true;
  portalCfg_->wifiSsid = ssid;
  portalCfg_->wifiPass = pass;

  http_.send(200, "text/html", portalPageHtml(true));
  http_.client().flush();
  delay(30);

  if (persist_) {
    persist_();
  } else {
    applyConfig(*portalCfg_);
  }
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
  static constexpr const char* kFwTag = "0.8.139-sha256-d0";
  static constexpr const char* kFwShort = "0.8.139-d0";
#else
  static constexpr const char* kFwTag = "0.8.139-sha256";
  static constexpr const char* kFwShort = "0.8.139";
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

  // Phone SoftAP setup portal (DNS + HTTP) — keep ahead of cmp so captive probes stay snappy.
  pollPortal(cfg);

  acceptClient();
  if (client_ && client_.connected()) {
    cmp.setShareMirror(&client_);
    cmp.pollTcp(client_, client_, cfg, snap, onApply, net, onJob, onStop, onStats);
  } else {
    cmp.setShareMirror(nullptr);
  }
  beacon();
}
