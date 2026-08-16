#include "wifi_link.hpp"
#include <esp_netif.h>
#include <esp_wifi.h>
#include "dhcpserver/dhcpserver.h"
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

void WifiLink::configureSoftApDns(const IPAddress& apIp) {
  // Phones only hit our DNSServer when DHCP advertises SoftAP as DNS.
  esp_netif_t* netif = esp_netif_get_handle_from_ifkey("WIFI_AP_DEF");
  if (!netif) return;

  esp_err_t err = esp_netif_dhcps_stop(netif);
  if (err != ESP_OK && err != ESP_ERR_ESP_NETIF_DHCP_ALREADY_STOPPED) {
    return;
  }

  dhcps_offer_t offer = OFFER_DNS;
  (void)esp_netif_dhcps_option(netif, ESP_NETIF_OP_SET, ESP_NETIF_DOMAIN_NAME_SERVER, &offer,
                               sizeof(offer));

  esp_netif_dns_info_t dns{};
  dns.ip.u_addr.ip4.addr = static_cast<uint32_t>(apIp);
  dns.ip.type = ESP_IPADDR_TYPE_V4;
  (void)esp_netif_set_dns_info(netif, ESP_NETIF_DNS_MAIN, &dns);

  (void)esp_netif_dhcps_start(netif);
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

  // Fixed SoftAP address 192.168.1.88 — phone Board Setup is always http://192.168.1.88/
  IPAddress apIp(CYD_SOFTAP_IP0, CYD_SOFTAP_IP1, CYD_SOFTAP_IP2, CYD_SOFTAP_IP3);
  IPAddress apGw(CYD_SOFTAP_IP0, CYD_SOFTAP_IP1, CYD_SOFTAP_IP2, CYD_SOFTAP_IP3);
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
    // Advertise SoftAP IP as DNS so captive probes hit our DNSServer → phone Sign-in UI.
    configureSoftApDns(WiFi.softAPIP());
    server_.begin();
    server_.setNoDelay(true);
    udp_.begin(CYD_WIFI_UDP_PORT);
    startPortal();
  }
  lastBeaconMs_ = 0;
}

void WifiLink::startPortal() {
  if (portalUp_) {
    stopPortal();
  }
  IPAddress ap = WiFi.softAPIP();
  dns_.setErrorReplyCode(DNSReplyCode::NoError);
  dns_.start(CYD_WIFI_DNS_PORT, "*", ap);

  http_.on("/", HTTP_GET, [this]() { handlePortalRoot(); });
  http_.on("/setup", HTTP_GET, [this]() { handlePortalRoot(); });
  http_.on("/control", HTTP_GET, [this]() { handlePortalRoot(); });
  http_.on("/save", HTTP_GET, [this]() { handlePortalSave(); });
  http_.on("/save", HTTP_POST, [this]() { handlePortalSave(); });
  http_.on("/clear", HTTP_GET, [this]() { handlePortalClear(); });
  http_.on("/clear", HTTP_POST, [this]() { handlePortalClear(); });
  http_.on("/reboot", HTTP_GET, [this]() { handlePortalReboot(); });
  http_.on("/reboot", HTTP_POST, [this]() { handlePortalReboot(); });
  // Captive probes — serve the control page directly so the phone Sign-in browser
  // lands on Board Setup immediately (302 alone often never opens on modern Android).
  http_.on("/generate_204", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/gen_204", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/hotspot-detect.html", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/library/test/success.html", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/connecttest.txt", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/ncsi.txt", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/fwlink/", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/fwlink", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/canonical.html", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/success.txt", HTTP_ANY, [this]() { handlePortalCaptive(); });
  http_.on("/mobile/status.php", HTTP_ANY, [this]() { handlePortalCaptive(); });
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

void WifiLink::pollPortal(AppConfig& cfg, const MinerSnapshot& snap) {
  if (!portalUp_) return;
  portalCfg_ = &cfg;
  portalSnap_ = &snap;
  // Drain DNS quickly — phones spam captive lookups on join.
  for (int i = 0; i < 12; i++) {
    dns_.processNextRequest();
  }
  http_.handleClient();
}

String WifiLink::portalPageHtml(bool saved, const char* flash) const {
  IPAddress ap = WiFi.softAPIP();
  char ip[20];
  snprintf(ip, sizeof(ip), "%u.%u.%u.%u", ap[0], ap[1], ap[2], ap[3]);

  const char* mode = modeLabel();
  String home = portalCfg_ && portalCfg_->wifiSsid.length() ? portalCfg_->wifiSsid : String("—");
  String boardMac = mac_.length() ? mac_ : String("—");
  String fw = "—";
  String rate = "—";
  if (portalSnap_) {
    if (portalSnap_->shaMode.length()) fw = portalSnap_->shaMode;
    if (portalSnap_->hashrateHs > 0.0f) {
      char rbuf[24];
      if (portalSnap_->hashrateHs >= 1000.0f) {
        snprintf(rbuf, sizeof(rbuf), "%.1f kH/s", portalSnap_->hashrateHs / 1000.0f);
      } else {
        snprintf(rbuf, sizeof(rbuf), "%.0f H/s", portalSnap_->hashrateHs);
      }
      rate = rbuf;
    }
  }

  String page;
  page.reserve(2800);
  page += F("<!DOCTYPE html><html><head><meta charset=utf-8>"
            "<meta name=viewport content=\"width=device-width,initial-scale=1\">"
            "<meta http-equiv=\"Cache-Control\" content=\"no-cache\">"
            "<title>Njörðr Board Setup</title><style>"
            "body{font-family:system-ui,-apple-system,sans-serif;background:#04141f;"
            "color:#e6f2fc;margin:0;padding:20px;line-height:1.35}"
            "h1{color:#7edcff;font-size:1.55rem;margin:0 0 4px}"
            "h2{color:#c5dceb;font-size:1.05rem;margin:18px 0 8px}"
            "p{color:#9bb8cc;margin:0 0 12px;font-size:.95rem}"
            ".card{background:#0b2433;border-radius:12px;padding:14px;margin:12px 0}"
            ".row{display:flex;justify-content:space-between;gap:10px;margin:6px 0;"
            "font-size:.9rem}.k{color:#789eba}.v{color:#e6f2fc;text-align:right}"
            "label{display:block;margin:12px 0 6px;color:#c5dceb;font-size:.9rem}"
            "input{width:100%;box-sizing:border-box;padding:14px;border:0;border-radius:10px;"
            "font-size:16px;background:#071a26;color:#e6f2fc}"
            "button,.btn{display:block;width:100%;margin-top:12px;padding:15px;border:0;"
            "border-radius:10px;background:#7edcff;color:#04141f;font-size:1.05rem;"
            "font-weight:700;text-align:center;text-decoration:none}"
            ".btn2{background:#1a3a52;color:#e6f2fc;font-weight:600}"
            ".ok{background:#12301f;color:#9dffb0;padding:12px;border-radius:10px;margin:10px 0}"
            ".warn{background:#3a2a10;color:#ffd27a;padding:12px;border-radius:10px;margin:10px 0}"
            ".meta{margin-top:16px;font-size:.8rem;color:#6f8fa8}"
            "</style></head><body>"
            "<h1>Njörðr</h1><p>Board Setup — control this miner over SoftAP</p>");

  if (flash && flash[0]) {
    page += F("<div class=ok>");
    page += flash;
    page += F("</div>");
  } else if (saved) {
    page += F("<div class=ok>Saved — board is joining home Wi‑Fi. "
              "Reconnect this phone to your home network.</div>");
  }

  page += F("<div class=card><h2>Board</h2>");
  auto row = [&](const char* k, const String& v) {
    page += F("<div class=row><span class=k>");
    page += k;
    page += F("</span><span class=v>");
    page += v;
    page += F("</span></div>");
  };
  row("SoftAP", apSsid_);
  row("Setup URL", String("http://") + ip + "/");
  row("Mode", String(mode));
  row("MAC", boardMac);
  row("Home Wi‑Fi", home);
  row("Hashrate", rate);
  page += F("</div>");

  page += F("<div class=card><h2>Home Wi‑Fi</h2>"
            "<p>Enter your router credentials so the board can mine on your LAN.</p>"
            "<form method=POST action=/save>"
            "<label>Home Wi‑Fi name (SSID)</label>"
            "<input name=ssid maxlength=32 required autocomplete=username "
            "placeholder=\"Router Wi‑Fi name\">"
            "<label>Password</label>"
            "<input name=pass type=password maxlength=63 autocomplete=current-password "
            "placeholder=\"Leave blank if open\">"
            "<button type=submit>Save &amp; connect</button></form>"
            "<form method=POST action=/clear style=\"margin-top:8px\">"
            "<button class=btn2 type=submit>Clear home Wi‑Fi</button></form>"
            "</div>");

  page += F("<div class=card>"
            "<form method=POST action=/reboot>"
            "<button class=btn2 type=submit>Reboot board</button></form>"
            "</div>");

  page += F("<p class=meta>Open SoftAP · phone Sign-in opens this page automatically · ");
  page += ip;
  page += F("</p></body></html>");
  return page;
}

void WifiLink::handlePortalRoot() {
  http_.sendHeader("Cache-Control", "no-cache, no-store, must-revalidate");
  http_.send(200, "text/html", portalPageHtml(false, nullptr));
}

void WifiLink::handlePortalCaptive() {
  // Serve the control page as 200 so the OS captive browser shows Board Setup
  // immediately. Avoid the word "Success" (iOS treats that as online).
  http_.sendHeader("Cache-Control", "no-cache, no-store, must-revalidate");
  http_.sendHeader("Connection", "close");
  http_.send(200, "text/html", portalPageHtml(false, nullptr));
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
               portalPageHtml(false, "SSID required — enter your home Wi‑Fi name."));
    return;
  }
  if (ssid.length() > 32) ssid = ssid.substring(0, 32);
  if (pass.length() > 63) pass = pass.substring(0, 63);

  portalCfg_->wifiEnabled = true;
  portalCfg_->wifiSsid = ssid;
  portalCfg_->wifiPass = pass;

  http_.sendHeader("Cache-Control", "no-cache");
  http_.send(200, "text/html", portalPageHtml(true, nullptr));
  http_.client().flush();
  delay(40);

  if (persist_) {
    persist_();
  } else {
    applyConfig(*portalCfg_);
  }
}

void WifiLink::handlePortalClear() {
  if (!portalCfg_) {
    http_.send(503, "text/plain", "Portal busy — retry");
    return;
  }
  portalCfg_->wifiSsid = "";
  portalCfg_->wifiPass = "";
  portalCfg_->wifiEnabled = true;
  http_.send(200, "text/html",
             portalPageHtml(false, "Home Wi‑Fi cleared — SoftAP setup mode."));
  http_.client().flush();
  delay(40);
  if (persist_) {
    persist_();
  } else {
    applyConfig(*portalCfg_);
  }
}

void WifiLink::handlePortalReboot() {
  http_.send(200, "text/html",
             F("<!DOCTYPE html><html><body style=\"background:#04141f;color:#7edcff;"
               "font-family:system-ui;padding:24px\"><h1>Rebooting…</h1>"
               "<p>Reconnect to Njordr SoftAP in a few seconds.</p></body></html>"));
  http_.client().flush();
  delay(120);
  ESP.restart();
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
  static constexpr const char* kFwTag = "0.8.141-sha256-d0";
  static constexpr const char* kFwShort = "0.8.141-d0";
#else
  static constexpr const char* kFwTag = "0.8.141-sha256";
  static constexpr const char* kFwShort = "0.8.141";
#endif
  snprintf(msg, sizeof(msg),
           "%s|v=%s|mac=%s|fw=%s|tcp=%u|ip=%u.%u.%u.%u|ap=%s|mode=%s",
           CYD_WIFI_MAGIC, kFwShort, mac_.c_str(), kFwTag, (unsigned)CYD_WIFI_TCP_PORT,
           advertise[0], advertise[1], advertise[2], advertise[3], apSsid_.c_str(), modeLabel());

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
    if (WiFi.getMode() == WIFI_OFF) ensureWifi(cfg);
  }

  // Captive portal first so Sign-in probes stay snappy when a phone joins SoftAP.
  pollPortal(cfg, snap);

  acceptClient();
  if (client_ && client_.connected()) {
    cmp.setShareMirror(&client_);
    cmp.pollTcp(client_, client_, cfg, snap, onApply, net, onJob, onStop, onStats);
  } else {
    cmp.setShareMirror(nullptr);
  }
  beacon();
}
