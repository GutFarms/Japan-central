#pragma once
#include "companion.hpp"
#include <Arduino.h>
#include <DNSServer.h>
#include <WebServer.h>
#include <WiFi.h>
#include <WiFiUdp.h>
#include <functional>

// SoftAP (+ optional STA) so Companion can find boards on Wi‑Fi and speak `cmp` over TCP.
// SoftAP hosts a phone captive portal (DNS + HTTP :80) that auto-opens Board Setup.
static constexpr uint16_t CYD_WIFI_TCP_PORT = 19284;
static constexpr uint16_t CYD_WIFI_UDP_PORT = 19284;
static constexpr uint16_t CYD_WIFI_HTTP_PORT = 80;
static constexpr uint16_t CYD_WIFI_DNS_PORT = 53;
static constexpr const char* CYD_WIFI_MAGIC = "CYDBOARD";
// Initial SoftAP is open (no password) so first-time Setup is one tap in Windows / phone Wi‑Fi.
static constexpr const char* CYD_SOFTAP_PASS = nullptr;
// SoftAP setup portal address — NOT on 192.168.1.0/24 (avoids home LAN / pool routing clash).
// Phone Board Setup: http://10.88.88.1/
static constexpr uint8_t CYD_SOFTAP_IP0 = 10;
static constexpr uint8_t CYD_SOFTAP_IP1 = 88;
static constexpr uint8_t CYD_SOFTAP_IP2 = 88;
static constexpr uint8_t CYD_SOFTAP_IP3 = 1;
// Preferred STA address on home LAN after setup (falls back to DHCP if unavailable).
static constexpr uint8_t CYD_STA_PREF_IP0 = 192;
static constexpr uint8_t CYD_STA_PREF_IP1 = 168;
static constexpr uint8_t CYD_STA_PREF_IP2 = 1;
static constexpr uint8_t CYD_STA_PREF_IP3 = 88;

class WifiLink {
 public:
  using PersistFn = std::function<void()>;  // save NVS + re-apply SoftAP/STA

  void begin(const char* macStr, const AppConfig& cfg);
  void applyConfig(const AppConfig& cfg);
  void setPersist(PersistFn fn) { persist_ = std::move(fn); }
  // Drive SoftAP/STA, UDP beacon, TCP cmp, and phone HTTP setup portal.
  void poll(CompanionLink& cmp, AppConfig& cfg, const MinerSnapshot& snap,
            CompanionLink::ApplyFn onApply, NetFeed* net, CompanionLink::JobFn onJob,
            CompanionLink::StopFn onStop, CompanionLink::StatsFn onStats);

  bool tcpConnected() { return client_.connected() != 0; }
  IPAddress softApIp() const { return WiFi.softAPIP(); }
  IPAddress staIp() const { return WiFi.localIP(); }
  String softApSsid() const { return apSsid_; }
  const char* modeLabel() const;

 private:
  WiFiServer server_{CYD_WIFI_TCP_PORT};
  WiFiClient client_;
  WiFiUDP udp_;
  WebServer http_{CYD_WIFI_HTTP_PORT};
  DNSServer dns_;
  PersistFn persist_;
  AppConfig* portalCfg_ = nullptr;
  const MinerSnapshot* portalSnap_ = nullptr;
  String apSsid_;
  String mac_;
  String lastStaSsid_;
  String lastStaPass_;
  IPAddress lastApIp_{0, 0, 0, 0};
  uint32_t lastBeaconMs_ = 0;
  uint32_t lastWifiCheckMs_ = 0;
  uint32_t staStaticTryAt_ = 0;
  bool started_ = false;
  bool staWanted_ = false;
  bool softApUp_ = false;
  bool portalUp_ = false;
  /// After home Wi‑Fi save: try 192.168.1.88 once DHCP shows a matching subnet.
  bool staPrefer88Pending_ = false;
  bool staStaticTried_ = false;
  uint8_t lastStaChannel_ = 0;

  void ensureWifi(const AppConfig& cfg);
  void beacon();
  void acceptClient();
  void startPortal();
  void stopPortal();
  void pollPortal(AppConfig& cfg, const MinerSnapshot& snap);
  void configureSoftApDns(const IPAddress& apIp);
  void realignSoftApToStaChannel();
  void tickStaAddressPolicy();
  void beginStaDhcp(const AppConfig& cfg);
  void beginStaPrefer88(const IPAddress& gateway, const IPAddress& mask);
  IPAddress softApIpForMode(bool wantSta) const;
  void handlePortalRoot();
  void handlePortalSave();
  void handlePortalClear();
  void handlePortalReboot();
  void handlePortalCaptive();
  String portalPageHtml(bool saved, const char* flash) const;
};
