#include <Arduino.h>
#include <ArduinoJson.h>
#include <TFT_eSPI.h>
#include <WiFi.h>
#include <WiFiUdp.h>

#include <cstring>

#include "device_config.h"
#include "gui.h"
#include "metrics.h"
#include "serial_link.h"
#include "wifi_config.h"

namespace {
TFT_eSPI tft;
MonitorGui gui;
WiFiUDP udp;
SerialLink serialLink;
DeviceSettings deviceSettings;
SystemMetrics metrics;
LinkStats linkStats;

char packetBuf[512];
char cfgBuf[160];
uint32_t lastUiMs = 0;
uint32_t lastWifiCheckMs = 0;
uint32_t bootHoldUntilMs = 0;
bool bootDone = false;
bool wifiReady = false;
bool wifiAttempted = false;

constexpr uint32_t BOOT_HOLD_MS = 4000;

enum class LinkSource : uint8_t { None, Usb, Udp };
LinkSource lastSource = LinkSource::None;

bool wifiConfigured() {
  return strcmp(WIFI_SSID, "YOUR_WIFI_SSID") != 0;
}

void beginWifi() {
  if (!wifiConfigured() || wifiAttempted) {
    return;
  }
  WiFi.mode(WIFI_STA);
  WiFi.persistent(true);
  WiFi.setAutoReconnect(true);
  WiFi.setSleep(false);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  wifiAttempted = true;
}

void ensureWifi() {
  if (!wifiConfigured()) {
    return;
  }
  if (!wifiAttempted) {
    beginWifi();
    return;
  }
  if (WiFi.status() == WL_CONNECTED) {
    if (!wifiReady) {
      wifiReady = true;
      udp.begin(METRICS_UDP_PORT);
    }
    return;
  }
  wifiReady = false;
  static uint32_t lastAttempt = 0;
  if (millis() - lastAttempt < 5000) {
    return;
  }
  lastAttempt = millis();
  WiFi.disconnect();
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
}

void sendCfgReply(bool viaUsb) {
  deviceSettings.toJson(cfgBuf, sizeof(cfgBuf));
  if (viaUsb) {
    serialLink.sendRaw(cfgBuf);
  } else if (wifiReady) {
    udp.beginPacket(udp.remoteIP(), udp.remotePort());
    udp.write(reinterpret_cast<const uint8_t *>(cfgBuf), strlen(cfgBuf));
    udp.endPacket();
  }
}

void handleHostCommand(const char *json, size_t len, bool viaUsb) {
  JsonDocument doc;
  if (deserializeJson(doc, json, len)) {
    return;
  }
  const char *cmd = doc["cmd"] | "";
  if (cmd[0] == '\0') {
    return;
  }

  if (strcmp(cmd, "get") == 0) {
    sendCfgReply(viaUsb);
    return;
  }

  const uint8_t oldRot = deviceSettings.cfg().rotation;
  if (!deviceSettings.applyCommandJson(json, len, tft)) {
    return;
  }
  if (deviceSettings.cfg().rotation != oldRot) {
    if (bootDone) {
      gui.setRotation(tft, deviceSettings.cfg().rotation);
    } else {
      // Stay on the loading splash until the 4s hold completes.
      tft.setRotation(deviceSettings.cfg().rotation == 3 ? 3 : 1);
      gui.showBoot(tft, wifiConfigured() ? "USB + WiFi ready" : "USB 115200 ready");
    }
  }
  sendCfgReply(viaUsb);
}

void finishBoot() {
  if (bootDone) {
    return;
  }
  bootDone = true;
  gui.drawChrome(tft);
  sendCfgReply(true);
  beginWifi();
}

void onMetricsPacket(LinkSource source) {
  lastSource = source;
  notePacketReceived(linkStats, metrics.seq, millis());
}

void pollUdp() {
  if (!wifiReady) {
    return;
  }
  const int packetSize = udp.parsePacket();
  if (packetSize <= 0) {
    return;
  }
  const int len = udp.read(packetBuf, sizeof(packetBuf) - 1);
  if (len <= 0) {
    return;
  }
  packetBuf[len] = '\0';

  // Config commands over UDP.
  if (strstr(packetBuf, "\"cmd\"") != nullptr) {
    handleHostCommand(packetBuf, static_cast<size_t>(len), false);
    return;
  }
  if (strstr(packetBuf, "\"hello\"") != nullptr) {
    deviceSettings.toJson(cfgBuf, sizeof(cfgBuf));
    udp.beginPacket(udp.remoteIP(), udp.remotePort());
    udp.write(reinterpret_cast<const uint8_t *>(cfgBuf), strlen(cfgBuf));
    udp.endPacket();
    return;
  }

  if (parseMetricsJson(packetBuf, static_cast<size_t>(len), metrics)) {
    onMetricsPacket(LinkSource::Udp);
    udp.beginPacket(udp.remoteIP(), udp.remotePort());
    char ack[48];
    snprintf(ack, sizeof(ack), "{\"ok\":1,\"seq\":%lu}", static_cast<unsigned long>(metrics.seq));
    udp.write(reinterpret_cast<const uint8_t *>(ack), strlen(ack));
    udp.endPacket();
  }
}

void pollSerial() {
  const HostMessageKind kind = serialLink.poll(metrics);
  if (kind == HostMessageKind::Hello) {
    sendCfgReply(true);
  } else if (kind == HostMessageKind::Command) {
    handleHostCommand(serialLink.lastLine(), serialLink.lastLineLen(), true);
  } else if (kind == HostMessageKind::Metrics) {
    onMetricsPacket(LinkSource::Usb);
  }
}
}  // namespace

void setup() {
  serialLink.begin(115200);
  Serial.println();
  Serial.println(F("ESP32-CYD PC/GPU Monitor"));

  deviceSettings.begin();

  tft.init();
  // Backlight is owned by DeviceSettings PWM now.
  gui.begin(tft, deviceSettings.cfg().rotation);
  deviceSettings.apply(tft);

  gui.showBoot(tft, wifiConfigured() ? "USB + WiFi ready" : "USB 115200 ready");
  bootHoldUntilMs = millis() + BOOT_HOLD_MS;
  bootDone = false;
  // Keep the loading screen up for 4s — do not draw chrome / start Wi‑Fi yet.
}

void loop() {
  const uint32_t now = millis();

  // Hold splash for a full 4 seconds so it does not flash into the monitor UI.
  if (!bootDone) {
    pollSerial();  // USB can settle, but keep splash until hold ends.
    if (static_cast<int32_t>(now - bootHoldUntilMs) >= 0) {
      finishBoot();
    } else {
      delay(10);
      return;
    }
  }

  pollSerial();
  pollUdp();

  if (now - lastWifiCheckMs >= 2000) {
    lastWifiCheckMs = now;
    ensureWifi();
  }

  if (now - lastUiMs >= 50) {
    lastUiMs = now;

    const bool stale = metricsAreStale(metrics, now, METRICS_STALE_MS);
    const bool linked = !stale && metrics.valid;

    char status[40];
    if (linked) {
      if (lastSource == LinkSource::Usb) {
        snprintf(status, sizeof(status), "USB");
      } else if (lastSource == LinkSource::Udp) {
        snprintf(status, sizeof(status), "WiFi");
      } else {
        snprintf(status, sizeof(status), "Live");
      }
    } else if (wifiReady) {
      const IPAddress ip = WiFi.localIP();
      snprintf(status, sizeof(status), "%d.%d.%d.%d", ip[0], ip[1], ip[2], ip[3]);
    } else {
      snprintf(status, sizeof(status), "Waiting");
    }

    gui.render(tft, metrics, linkStats, linked, status);
  }
}
