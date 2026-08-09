#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "scrypt_lite.hpp"
#include "stratum_client.hpp"

#include <WiFi.h>
#include <cstring>
#include <esp_system.h>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static DisplayUi g_ui;
static ScryptLite g_miner;
static StratumClient g_stratum;
static MinerSnapshot g_snap;

static uint32_t g_windowStart = 0;
static uint64_t g_windowHashes = 0;
static float g_hashrate = 0;
static uint32_t g_lastPaint = 0;
static uint32_t g_lastWifiAttempt = 0;
static bool g_wifiStarted = false;
static bool g_jobLoaded = false;
static bool g_poolMode = false;

static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

static const char* wifiLabel() {
  switch (WiFi.status()) {
    case WL_CONNECTED:
      return "ok";
    case WL_NO_SSID_AVAIL:
      return "ssid";
    case WL_CONNECT_FAILED:
      return "fail";
    case WL_IDLE_STATUS:
      return "idle";
    case WL_DISCONNECTED:
      return "off";
    default:
      return "...";
  }
}

static void startWifi() {
  if (!g_cfg.wifiSsid.length()) return;
  WiFi.mode(WIFI_STA);
  WiFi.setSleep(false);
  WiFi.setAutoReconnect(true);
  WiFi.begin(g_cfg.wifiSsid.c_str(), g_cfg.wifiPassword.c_str());
  g_wifiStarted = true;
  g_lastWifiAttempt = millis();
}

static void serviceWifi() {
  if (!g_cfg.wifiSsid.length()) return;
  if (!g_wifiStarted) {
    startWifi();
    g_stratum.begin(g_cfg);
    return;
  }
  if (WiFi.status() == WL_CONNECTED) return;
  uint32_t now = millis();
  if (now - g_lastWifiAttempt < 15000) return;
  g_lastWifiAttempt = now;
  WiFi.disconnect();
  WiFi.begin(g_cfg.wifiSsid.c_str(), g_cfg.wifiPassword.c_str());
}

static void fillSnap() {
  g_snap.hashrateHs = g_hashrate;
  g_snap.shares = g_miner.shares();
  g_snap.accepted = g_stratum.accepted();
  g_snap.rejected = g_stratum.rejected();
  g_snap.dropped = g_stratum.dropped();
  g_snap.pool = g_stratum.phase();
  if (g_stratum.connected()) g_snap.pool = "CONNECTED";
  g_snap.connected = g_stratum.connected();
  g_snap.wifi = wifiLabel();
  if (WiFi.status() == WL_CONNECTED) {
    g_snap.ip = WiFi.localIP().toString();
  } else {
    g_snap.ip = "---";
  }
  g_snap.difficulty = g_stratum.difficulty();
  g_snap.nonce = g_miner.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = g_cfg.hashFocus;
}

static bool applyConfig(AppConfig& updated, bool& reboot, bool reconnect) {
  if (updated.wifiPassword.length() == 0 && g_cfg.wifiPassword.length()) {
    updated.wifiPassword = g_cfg.wifiPassword;
  }
  if (updated.password.length() == 0 && g_cfg.password.length()) {
    updated.password = g_cfg.password;
  }
  updated.cpuMhz = updated.normalizeCpu(updated.cpuMhz);

  bool wifiChanged = updated.wifiSsid != g_cfg.wifiSsid ||
                     updated.wifiPassword != g_cfg.wifiPassword;
  bool poolChanged = updated.stratum != g_cfg.stratum || updated.worker != g_cfg.worker ||
                     updated.password != g_cfg.password;

  g_cfg = updated;

  if (g_cfg.isComplete()) {
    g_store.save(g_cfg);
  }

  if (wifiChanged && g_cfg.wifiSsid.length()) {
    WiFi.disconnect(true);
    delay(40);
    startWifi();
  }
  if (poolChanged || wifiChanged || reconnect) {
    g_stratum.updateConfig(g_cfg);
    g_stratum.requestReconnect();
    g_jobLoaded = false;
    g_poolMode = false;
  }

  if (reboot) {
    Serial.flush();
    delay(60);
    ESP.restart();
  }
  if (updated.cpuMhz != (uint8_t)getCpuFrequencyMhz()) {
    applyCpu(updated.cpuMhz);
  }
  return g_cfg.isComplete();
}

static void serviceCompanion() {
  fillSnap();
  g_cmp.poll(g_cfg, g_snap, applyConfig);
}

static void mineBurst() {
  // Larger bursts when hash-focus; still break for USB between sub-batches.
  const size_t total = g_cfg.hashFocus ? 12 : 4;
  const size_t slice = g_cfg.hashFocus ? 3 : 2;
  size_t done = 0;
  while (done < total) {
    size_t n = total - done;
    if (n > slice) n = slice;
    bool share = g_miner.mineBatch(n);
    g_windowHashes += n;
    done += n;
    if (share && g_poolMode) {
      g_stratum.submitShare(g_miner.lastShareNonce());
    }
    // Keep CMP responsive during hashing.
    g_cmp.poll(g_cfg, g_snap, applyConfig);
  }
}

void setup() {
  g_cmp.begin(115200);
  g_ui.begin();
  g_ui.showSplash();

  bool fromFlash = g_store.load(g_cfg);
  applyCpu(g_cfg.cpuMhz);

  // Allocate miner buffers before WiFi eats heap.
  g_miner = ScryptLite();

  delay(250);

  if (!fromFlash || !g_cfg.isComplete()) {
    g_ui.showWaitingCompanion();
  } else {
    g_ui.showMessage("Connecting", g_cfg.wifiSsid.c_str());
    startWifi();
    g_stratum.begin(g_cfg);
  }

  g_windowStart = millis();
  g_lastPaint = millis();
  fillSnap();
}

void loop() {
  serviceCompanion();

  if (!g_cfg.isComplete()) {
    uint32_t now = millis();
    if (now - g_lastPaint > 3000) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(2);
    return;
  }

  serviceWifi();
  g_stratum.loop();

  uint8_t header[80], target[32];
  if (g_stratum.peekJob(header, target)) {
    static uint8_t lastHdr[80];
    static bool haveLast = false;
    if (!haveLast || memcmp(header, lastHdr, 76) != 0) {
      g_miner.setJob(header, target, esp_random());
      memcpy(lastHdr, header, 80);
      haveLast = true;
      g_jobLoaded = true;
      g_poolMode = true;
    } else {
      g_miner.updateTarget(target);
      g_jobLoaded = true;
      g_poolMode = true;
    }
  }

  if (g_jobLoaded && g_poolMode) {
    mineBurst();
  }

  serviceCompanion();
  g_stratum.loop();

  uint32_t now = millis();
  uint32_t elapsed = now - g_windowStart;
  if (elapsed >= 2000) {
    if (g_poolMode) {
      g_hashrate = (float)g_windowHashes * 1000.0f / (float)elapsed;
    } else {
      g_hashrate = 0;
    }
    g_windowHashes = 0;
    g_windowStart = now;
  }

  // Rare paints in hash-focus; partial updates keep cost low.
  uint32_t paintMs = g_cfg.hashFocus ? 4000 : 1000;
  if (now - g_lastPaint >= paintMs) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
}
