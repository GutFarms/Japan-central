#include <Arduino.h>
#include <ArduinoJson.h>
#include <TFT_eSPI.h>
#include <WiFi.h>
#include <WiFiUdp.h>
#include <esp_system.h>

#include <cstring>

// CYD USB power is marginal when Wi‑Fi TX kicks in — disable brownout resets.
#include "soc/rtc_cntl_reg.h"
#include "soc/soc.h"

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
uint32_t wifiStartAtMs = 0;
bool bootDone = false;
bool wifiReady = false;
bool wifiAttempted = false;

constexpr uint32_t BOOT_HOLD_MS = 4000;
constexpr uint32_t WIFI_DEFER_MS = 3000;  // after splash, wait before Wi‑Fi radio

enum class LinkSource : uint8_t { None, Usb, Udp };
LinkSource lastSource = LinkSource::None;

bool wifiConfigured() {
  return strcmp(WIFI_SSID, "YOUR_WIFI_SSID") != 0;
}

const char *resetReasonText(esp_reset_reason_t reason) {
  switch (reason) {
    case ESP_RST_POWERON:
      return "POWERON";
    case ESP_RST_EXT:
      return "EXT";
    case ESP_RST_SW:
      return "SW";
    case ESP_RST_PANIC:
      return "PANIC";
    case ESP_RST_INT_WDT:
      return "INT_WDT";
    case ESP_RST_TASK_WDT:
      return "TASK_WDT";
    case ESP_RST_WDT:
      return "WDT";
    case ESP_RST_DEEPSLEEP:
      return "DEEPSLEEP";
    case ESP_RST_BROWNOUT:
      return "BROWNOUT";
    case ESP_RST_SDIO:
      return "SDIO";
    default:
      return "OTHER";
  }
}

void beginWifi() {
  if (!wifiConfigured() || wifiAttempted) {
    return;
  }
  wifiAttempted = true;

  Serial.println(F("WiFi: starting (low TX power)"));
  WiFi.persistent(false);
  WiFi.mode(WIFI_STA);
  WiFi.setSleep(true);  // lower average current on CYD
  WiFi.setAutoReconnect(true);
  // Reduce peak current — primary fix for CYD reboot loops on USB power.
  WiFi.setTxPower(WIFI_POWER_8_5dBm);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
}

void ensureWifi() {
  if (!wifiConfigured()) {
    return;
  }
  if (!wifiAttempted) {
    if (wifiStartAtMs != 0 && static_cast<int32_t>(millis() - wifiStartAtMs) >= 0) {
      beginWifi();
    }
    return;
  }
  if (WiFi.status() == WL_CONNECTED) {
    if (!wifiReady) {
      wifiReady = true;
      udp.begin(METRICS_UDP_PORT);
      Serial.printf("WiFi: connected %s\n", WiFi.localIP().toString().c_str());
    }
    return;
  }
  wifiReady = false;
  static uint32_t lastAttempt = 0;
  if (millis() - lastAttempt < 15000) {
    return;
  }
  lastAttempt = millis();
  Serial.println(F("WiFi: retry begin"));
  WiFi.setTxPower(WIFI_POWER_8_5dBm);
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
      tft.setRotation(deviceSettings.cfg().rotation == 3 ? 3 : 1);
      gui.showBoot(tft, "USB ready");
    }
  }
  sendCfgReply(viaUsb);
}

void finishBoot() {
  if (bootDone) {
    return;
  }
  bootDone = true;
  gui.ensureSprite();
  gui.drawChrome(tft);
  sendCfgReply(true);
  // Do not start Wi‑Fi immediately after drawing — stagger the power spike.
  wifiStartAtMs = millis() + WIFI_DEFER_MS;
  Serial.printf("Boot done. WiFi in %lu ms. Free heap %u\n",
                static_cast<unsigned long>(WIFI_DEFER_MS),
                static_cast<unsigned>(ESP.getFreeHeap()));
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
  WRITE_PERI_REG(RTC_CNTL_BROWN_OUT_REG, 0);

  serialLink.begin(115200);
  delay(50);
  Serial.println();
  Serial.println(F("ESP32-CYD PC/GPU Monitor"));
  Serial.printf("Reset reason: %s\n", resetReasonText(esp_reset_reason()));
  Serial.printf("Free heap: %u\n", static_cast<unsigned>(ESP.getFreeHeap()));

  deviceSettings.begin();

  tft.init();
  gui.begin(tft, deviceSettings.cfg().rotation);
  deviceSettings.apply(tft);

  gui.showBoot(tft, wifiConfigured() ? "Starting…" : "USB 115200 ready");
  bootHoldUntilMs = millis() + BOOT_HOLD_MS;
  bootDone = false;
  wifiStartAtMs = 0;
}

void loop() {
  const uint32_t now = millis();

  if (!bootDone) {
    // Keep splash steady; avoid heavy work / Wi‑Fi during the 4s hold.
    if (static_cast<int32_t>(now - bootHoldUntilMs) >= 0) {
      finishBoot();
    } else {
      delay(20);
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
    } else if (wifiAttempted) {
      snprintf(status, sizeof(status), "WiFi…");
    } else {
      snprintf(status, sizeof(status), "Waiting");
    }

    gui.render(tft, metrics, linkStats, linked, status);
  }
}
