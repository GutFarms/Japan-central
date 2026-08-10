#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "scrypt_lite.hpp"

#include <cstring>
#include <esp_system.h>
#include <esp_wifi.h>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static DisplayUi g_ui;
static ScryptLite g_miner;
static MinerSnapshot g_snap;
static NetFeed g_net;
static UsbJob g_job;
static PendingShare g_shareOut;

static uint32_t g_windowStart = 0;
static uint64_t g_windowHashes = 0;
static float g_hashrate = 0;
static uint32_t g_lastPaint = 0;
static bool g_jobLoaded = false;
static uint32_t g_accepted = 0;
static uint32_t g_rejected = 0;

static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

static void fillSnap() {
  g_snap.hashrateHs = g_hashrate;
  g_snap.shares = g_miner.shares();
  g_snap.accepted = g_accepted;
  g_snap.rejected = g_rejected;
  g_snap.pool = g_jobLoaded ? "USB HASH" : "WAIT USB";
  g_snap.connected = g_jobLoaded;
  g_snap.difficulty = 0;
  g_snap.nonce = g_miner.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = g_cfg.hashFocus;
  g_snap.netTicker = g_net.ticker;
  g_snap.jobId = g_job.jobId;
}

static bool applyConfig(AppConfig& updated, bool& reboot) {
  updated.cpuMhz = updated.normalizeCpu(updated.cpuMhz);
  g_cfg = updated;
  g_store.save(g_cfg);

  if (reboot) {
    Serial.flush();
    delay(60);
    ESP.restart();
  }
  if (updated.cpuMhz != (uint8_t)getCpuFrequencyMhz()) {
    applyCpu(updated.cpuMhz);
  }
  return true;
}

static void onJob(const UsbJob& job) {
  g_job = job;
  g_miner.setJob(job.header, job.target, job.startNonce);
  g_jobLoaded = true;
  g_windowHashes = 0;
  g_windowStart = millis();
}

static void onStop() {
  g_jobLoaded = false;
  g_job.valid = false;
  g_hashrate = 0;
}

static void onStats(uint32_t accepted, uint32_t rejected) {
  g_accepted = accepted;
  g_rejected = rejected;
}

static void serviceCompanion() {
  fillSnap();
  g_cmp.poll(g_cfg, g_snap, applyConfig, &g_net, onJob, onStop, onStats);
  if (g_net.fresh) {
    g_snap.netTicker = g_net.ticker;
    g_net.fresh = false;
  }
  if (g_shareOut.pending) {
    g_cmp.emitShare(g_shareOut);
    g_shareOut.pending = false;
  }
}

static void mineBurst() {
  const size_t total = g_cfg.hashFocus ? 12 : 4;
  const size_t slice = g_cfg.hashFocus ? 3 : 2;
  size_t done = 0;
  while (done < total && g_jobLoaded) {
    size_t n = total - done;
    if (n > slice) n = slice;
    bool share = g_miner.mineBatch(n);
    g_windowHashes += n;
    done += n;
    if (share) {
      g_shareOut.nonce = g_miner.lastShareNonce();
      g_shareOut.jobId = g_job.jobId;
      g_shareOut.extranonce2 = g_job.extranonce2;
      g_shareOut.ntime = g_job.ntime;
      g_shareOut.pending = true;
      g_cmp.emitShare(g_shareOut);
      g_shareOut.pending = false;
    }
    // Keep USB responsive while hashing.
    g_cmp.poll(g_cfg, g_snap, applyConfig, &g_net, onJob, onStop, onStats);
  }
}

void setup() {
  // Ensure Wi‑Fi stays down — pool traffic is USB-C companion only.
  // Safe if the driver was never started.
  (void)esp_wifi_stop();
  (void)esp_wifi_deinit();

  g_cmp.begin(115200);
  g_ui.begin();
  g_ui.showSplash();

  g_store.load(g_cfg);
  applyCpu(g_cfg.cpuMhz);
  g_miner = ScryptLite();

  delay(200);
  g_ui.showWaitingCompanion();

  g_windowStart = millis();
  g_lastPaint = millis();
  fillSnap();
}

void loop() {
  serviceCompanion();

  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint > 2500) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(2);
    return;
  }

  mineBurst();
  serviceCompanion();

  uint32_t now = millis();
  uint32_t elapsed = now - g_windowStart;
  if (elapsed >= 2000) {
    g_hashrate = (float)g_windowHashes * 1000.0f / (float)elapsed;
    g_windowHashes = 0;
    g_windowStart = now;
  }

  uint32_t paintMs = g_cfg.hashFocus ? 4000 : 1000;
  if (now - g_lastPaint >= paintMs) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
}
