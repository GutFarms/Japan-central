#include <Arduino.h>
#include <TFT_eSPI.h>
#include <WiFi.h>
#include <WiFiUdp.h>

#include <cstring>

#include "gui.h"
#include "metrics.h"
#include "wifi_config.h"

namespace {
  TFT_eSPI tft;
  MonitorGui gui;
  WiFiUDP udp;
  SystemMetrics metrics;

  char packetBuf[512];
  uint32_t lastUiMs = 0;
  uint32_t lastWifiCheckMs = 0;
  bool wifiReady = false;

  void connectWifi() {
    gui.showBoot(tft, "Connecting WiFi...");
    WiFi.mode(WIFI_STA);
    WiFi.setSleep(false);
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    const uint32_t start = millis();
    while (WiFi.status() != WL_CONNECTED && (millis() - start) < 20000) {
      delay(250);
    }

    wifiReady = WiFi.status() == WL_CONNECTED;
    if (wifiReady) {
      udp.begin(METRICS_UDP_PORT);
      char msg[48];
      snprintf(msg, sizeof(msg), "UDP :%u", static_cast<unsigned>(METRICS_UDP_PORT));
      gui.showBoot(tft, msg);
      delay(600);
      gui.drawChrome(tft);
    } else {
      gui.showBoot(tft, "WiFi failed — retrying");
    }
  }

  void ensureWifi() {
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
    parseMetricsJson(packetBuf, static_cast<size_t>(len), metrics);
  }
}  // namespace

void setup() {
  Serial.begin(115200);
  delay(100);
  Serial.println();
  Serial.println(F("ESP32-CYD PC/GPU Monitor"));

  tft.init();
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);

  gui.begin(tft);

  if (strcmp(WIFI_SSID, "YOUR_WIFI_SSID") == 0) {
    gui.showBoot(tft, "Edit secrets.h / wifi_config");
    Serial.println(F("Configure WIFI_SSID and WIFI_PASSWORD before flashing."));
  }

  connectWifi();
}

void loop() {
  const uint32_t now = millis();

  if (now - lastWifiCheckMs >= 2000) {
    lastWifiCheckMs = now;
    ensureWifi();
  }

  pollUdp();

  if (now - lastUiMs >= 200) {
    lastUiMs = now;

    const bool stale = metricsAreStale(metrics, now, METRICS_STALE_MS);
    const bool linked = wifiReady && !stale && metrics.valid;

    char status[48];
    if (!wifiReady) {
      snprintf(status, sizeof(status), "WiFi connecting...");
    } else if (!metrics.valid || stale) {
      IPAddress ip = WiFi.localIP();
      snprintf(status, sizeof(status), "%d.%d.%d.%d:%u",
               ip[0], ip[1], ip[2], ip[3],
               static_cast<unsigned>(METRICS_UDP_PORT));
    } else {
      snprintf(status, sizeof(status), "Live metrics");
    }

    gui.render(tft, metrics, linked, status);
  }
}
