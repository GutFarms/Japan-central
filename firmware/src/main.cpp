#include <Arduino.h>
#include <TFT_eSPI.h>
#include <WiFi.h>
#include <WiFiUdp.h>
#include <esp_wifi.h>

#include <cstring>

#include "gui.h"
#include "metrics.h"
#include "serial_link.h"
#include "wifi_config.h"

namespace {
TFT_eSPI tft;
MonitorGui gui;
WiFiUDP udp;
SerialLink serialLink;
SystemMetrics metrics;

char packetBuf[512];
uint32_t lastUiMs = 0;
uint32_t lastWifiCheckMs = 0;
uint32_t wifiEarliestMs = 0;
bool wifiReady = false;
bool wifiAttempted = false;

enum class LinkSource : uint8_t { None, Usb, Udp };
LinkSource lastSource = LinkSource::None;

bool wifiConfigured() {
  return strcmp(WIFI_SSID, "YOUR_WIFI_SSID") != 0;
}

bool ssidInScan(const char *ssid, int16_t networkCount) {
  if (ssid == nullptr || ssid[0] == '\0' || networkCount <= 0) {
    return false;
  }
  for (int16_t i = 0; i < networkCount; ++i) {
    if (WiFi.SSID(i) == ssid) {
      return true;
    }
  }
  return false;
}

void markWifiConnected() {
  if (!wifiReady) {
    wifiReady = true;
    udp.begin(METRICS_UDP_PORT);
    Serial.print(F("WiFi connected: "));
    Serial.print(WiFi.SSID());
    Serial.print(F("  IP "));
    Serial.println(WiFi.localIP());
  }
}

// Sniff nearby APs and reconnect to the configured / previously stored network.
void sniffAndReconnectWifi() {
  if (!wifiConfigured()) {
    return;
  }

  WiFi.mode(WIFI_STA);
  WiFi.persistent(true);
  WiFi.setAutoReconnect(true);
  WiFi.setSleep(false);

  // Pull previously saved station credentials from NVS (if any).
  wifi_config_t conf{};
  char storedSsid[33] = {0};
  if (esp_wifi_get_config(WIFI_IF_STA, &conf) == ESP_OK) {
    memcpy(storedSsid, conf.sta.ssid, sizeof(conf.sta.ssid));
    storedSsid[32] = '\0';
  }

  Serial.println(F("WiFi sniff: scanning for previous network…"));
  const int16_t n = WiFi.scanNetworks(/*async=*/false, /*hidden=*/true);
  const bool foundConfigured = ssidInScan(WIFI_SSID, n);
  const bool foundStored =
      storedSsid[0] != '\0' && strcmp(storedSsid, WIFI_SSID) != 0 && ssidInScan(storedSsid, n);
  WiFi.scanDelete();

  if (foundConfigured) {
    Serial.print(F("WiFi sniff: found configured SSID "));
    Serial.println(WIFI_SSID);
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  } else if (foundStored) {
    Serial.print(F("WiFi sniff: found previous SSID "));
    Serial.println(storedSsid);
    WiFi.begin();  // reconnect using NVS-stored credentials
  } else {
    // Hidden / missed in scan — still try configured, then fall back to NVS begin().
    Serial.println(F("WiFi sniff: trying configured + stored credentials"));
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  }

  wifiAttempted = true;
}

void ensureWifi() {
  if (!wifiConfigured()) {
    return;
  }

  const uint32_t now = millis();
  if (now < wifiEarliestMs) {
    return;  // honor post-boot delay
  }

  if (WiFi.status() == WL_CONNECTED) {
    markWifiConnected();
    return;
  }

  wifiReady = false;

  if (!wifiAttempted) {
    sniffAndReconnectWifi();
    return;
  }

  static uint32_t lastAttempt = 0;
  if (now - lastAttempt < WIFI_RETRY_MS) {
    return;
  }
  lastAttempt = now;
  WiFi.disconnect(false, false);
  delay(50);
  sniffAndReconnectWifi();
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
  if (parseMetricsJson(packetBuf, static_cast<size_t>(len), metrics)) {
    lastSource = LinkSource::Udp;
  }
}

void pollSerial() {
  if (serialLink.poll(metrics)) {
    lastSource = LinkSource::Usb;
  }
}
}  // namespace

void setup() {
  serialLink.begin(115200);
  Serial.println();
  Serial.println(F("ESP32-CYD PC/GPU Monitor"));
  Serial.println(F("USB NDJSON @115200 or UDP :4210"));

  tft.init();
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);

  gui.begin(tft);
  if (wifiConfigured()) {
    gui.showBoot(tft, "WiFi sniff in 5s");
  } else {
    gui.showBoot(tft, "USB 115200 ready");
  }
  delay(450);
  gui.drawChrome(tft);

  // USB works immediately; Wi-Fi sniff / auto-reconnect starts after the delay.
  wifiEarliestMs = millis() + WIFI_BOOT_DELAY_MS;
  wifiAttempted = false;
  WiFi.persistent(true);
  WiFi.setAutoReconnect(true);
}

void loop() {
  const uint32_t now = millis();

  pollSerial();
  pollUdp();

  if (now - lastWifiCheckMs >= 500) {
    lastWifiCheckMs = now;
    ensureWifi();
  }

  if (now - lastUiMs >= 200) {
    lastUiMs = now;

    const bool stale = metricsAreStale(metrics, now, METRICS_STALE_MS);
    const bool linked = !stale && metrics.valid;

    char status[48];
    if (linked) {
      if (lastSource == LinkSource::Usb) {
        snprintf(status, sizeof(status), "USB serial");
      } else if (lastSource == LinkSource::Udp) {
        snprintf(status, sizeof(status), "WiFi UDP");
      } else {
        snprintf(status, sizeof(status), "Live metrics");
      }
    } else if (wifiReady) {
      const IPAddress ip = WiFi.localIP();
      snprintf(status, sizeof(status), "USB / %d.%d.%d.%d", ip[0], ip[1], ip[2], ip[3]);
    } else if (wifiConfigured() && now < wifiEarliestMs) {
      const uint32_t left = (wifiEarliestMs - now + 999) / 1000;
      snprintf(status, sizeof(status), "WiFi sniff %us", static_cast<unsigned>(left));
    } else if (wifiConfigured()) {
      snprintf(status, sizeof(status), "WiFi scanning…");
    } else {
      snprintf(status, sizeof(status), "USB 115200");
    }

    gui.render(tft, metrics, linked, status);
  }
}
