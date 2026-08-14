#pragma once
#include "companion.hpp"
#include <Arduino.h>
#include <WiFi.h>
#include <WiFiUdp.h>

// SoftAP (+ optional STA) so Companion can find boards on Wi‑Fi and speak `cmp` over TCP.
static constexpr uint16_t CYD_WIFI_TCP_PORT = 19284;
static constexpr uint16_t CYD_WIFI_UDP_PORT = 19284;
static constexpr const char* CYD_WIFI_MAGIC = "CYDBOARD";
static constexpr const char* CYD_SOFTAP_PASS = "njordrseas";

class WifiLink {
 public:
  void begin(const char* macStr, const AppConfig& cfg);
  void applyConfig(const AppConfig& cfg);
  // Drive SoftAP/STA, UDP beacon, and one TCP cmp client.
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
  String apSsid_;
  String mac_;
  String lastStaSsid_;
  String lastStaPass_;
  uint32_t lastBeaconMs_ = 0;
  uint32_t lastWifiCheckMs_ = 0;
  bool started_ = false;
  bool staWanted_ = false;
  bool softApUp_ = false;

  void ensureWifi(const AppConfig& cfg);
  void beacon();
  void acceptClient();
};
