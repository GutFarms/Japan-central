#include <Arduino.h>
#include <TFT_eSPI.h>
#include <WiFi.h>
#include <WiFiUdp.h>

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
LinkStats linkStats;

char packetBuf[512];
uint32_t lastUiMs = 0;
uint32_t lastWifiCheckMs = 0;
bool wifiReady = false;
bool wifiAttempted = false;

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
  if (parseMetricsJson(packetBuf, static_cast<size_t>(len), metrics)) {
    onMetricsPacket(LinkSource::Udp);
    // UDP ACK back to sender when possible.
    udp.beginPacket(udp.remoteIP(), udp.remotePort());
    char ack[48];
    snprintf(ack, sizeof(ack), "{\"ok\":1,\"seq\":%lu}", static_cast<unsigned long>(metrics.seq));
    udp.write(reinterpret_cast<const uint8_t *>(ack), strlen(ack));
    udp.endPacket();
  }
}

void pollSerial() {
  if (serialLink.poll(metrics)) {
    onMetricsPacket(LinkSource::Usb);
  }
}
}  // namespace

void setup() {
  serialLink.begin(115200);
  Serial.println();
  Serial.println(F("ESP32-CYD PC/GPU Monitor"));
  serialLink.sendHelloAck();

  tft.init();
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);

  gui.begin(tft);
  gui.showBoot(tft, wifiConfigured() ? "USB + WiFi ready" : "USB 115200 ready");
  delay(400);
  gui.drawChrome(tft);

  beginWifi();
}

void loop() {
  const uint32_t now = millis();

  pollSerial();
  pollUdp();

  if (now - lastWifiCheckMs >= 2000) {
    lastWifiCheckMs = now;
    ensureWifi();
  }

  // Higher UI rate for smoother needle animation.
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
