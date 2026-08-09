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
static bool g_wifiStarted = false;
static bool g_jobLoaded = false;
static String g_jobIdSeen;

static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

static String wifiLabel() {
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
      return "…";
  }
}

static void startWifi() {
  if (!g_cfg.wifiSsid.length()) return;
  WiFi.mode(WIFI_STA);
  WiFi.setSleep(false);
  WiFi.begin(g_cfg.wifiSsid.c_str(), g_cfg.wifiPassword.c_str());
  g_wifiStarted = true;
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
  // Merge: empty password fields keep previous when already configured
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
    delay(50);
    startWifi();
  }
  if (poolChanged || wifiChanged || reconnect) {
    g_stratum.updateConfig(g_cfg);
    g_stratum.requestReconnect();
    g_jobLoaded = false;
  }

  if (reboot) {
    delay(80);
    ESP.restart();
  }
  if (updated.cpuMhz != (uint8_t)getCpuFrequencyMhz()) {
    applyCpu(updated.cpuMhz);
  }
  return g_cfg.isComplete();
}

void setup() {
  g_cmp.begin(115200);
  g_ui.begin();
  g_ui.showSplash();

  bool fromFlash = g_store.load(g_cfg);
  applyCpu(g_cfg.cpuMhz);

  // Allocate miner buffers before WiFi.
  g_miner = ScryptLite();

  delay(400);

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
  // Companion first — never block USB behind a long scrypt batch.
  fillSnap();
  g_cmp.poll(g_cfg, g_snap, applyConfig);

  if (!g_cfg.isComplete()) {
    // Stay in waiting UI; still answer CMP.
    uint32_t now = millis();
    if (now - g_lastPaint > 2000) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(2);
    return;
  }

  if (!g_wifiStarted) {
    startWifi();
    g_stratum.begin(g_cfg);
  }

  g_stratum.loop();

  uint8_t header[80], target[32];
  if (g_stratum.peekJob(header, target)) {
    // Reload job when header identity changes (prevhash/merkle/ntime region).
    static uint8_t lastHdr[80];
    static bool haveLast = false;
    if (!haveLast || memcmp(header, lastHdr, 76) != 0) {
      g_miner.setJob(header, target, esp_random());
      memcpy(lastHdr, header, 80);
      haveLast = true;
      g_jobLoaded = true;
    } else {
      g_miner.updateTarget(target);
    }
  }

  // Mine a short batch so USB stays responsive.
  size_t batch = g_cfg.hashFocus ? 4 : 2;
  if (g_jobLoaded) {
    bool share = g_miner.mineBatch(batch);
    if (share) {
      g_stratum.submitShare(g_miner.lastShareNonce());
    }
    g_windowHashes += batch;
  } else {
    // Demo hashing so H/s is visible offline (easy target already in ScryptLite ctor).
    static bool demoSet = false;
    if (!demoSet) {
      uint8_t hdr[80]{};
      hdr[0] = 1;
      for (int i = 4; i < 76; i++) hdr[i] = (uint8_t)(i * 17 + 0xA5);
      uint8_t tgt[32];
      memset(tgt, 0xff, 32);
      tgt[31] = 0x00;
      tgt[30] = 0x0f;
      g_miner.setJob(hdr, tgt, 0);
      demoSet = true;
    }
    g_miner.mineBatch(batch);
    g_windowHashes += batch;
  }

  // Drain companion again after batch.
  fillSnap();
  g_cmp.poll(g_cfg, g_snap, applyConfig);
  g_stratum.loop();

  uint32_t now = millis();
  uint32_t elapsed = now - g_windowStart;
  if (elapsed >= 2000) {
    g_hashrate = (float)g_windowHashes * 1000.0f / (float)elapsed;
    g_windowHashes = 0;
    g_windowStart = now;
  }

  uint32_t paintMs = g_cfg.hashFocus ? 5000 : 1200;
  if (now - g_lastPaint >= paintMs) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap);
    g_lastPaint = now;
  }
}
