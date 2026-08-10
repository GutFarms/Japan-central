#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "scrypt_lite.hpp"

#include <cstring>
#include <esp_system.h>
#include <esp_task_wdt.h>
#include <esp_wifi.h>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static DisplayUi g_ui;
static ScryptLite g_miner;
static MinerSnapshot g_snap;
static NetFeed g_net;
static UsbJob g_job;

static volatile bool g_jobLoaded = false;
static volatile bool g_mining = false;
static uint64_t g_hashCounter = 0;
static uint64_t g_shareCounter = 0;
static uint32_t g_lastShareNonce = 0;
static bool g_sharePending = false;
static char g_shareJob[48];
static char g_shareEn2[48];
static char g_shareNtime[24];
static float g_lastBenchHs = 0;

static uint32_t g_windowStart = 0;
static uint64_t g_windowHashesStart = 0;
static float g_hashrate = 0;
static uint32_t g_lastPaint = 0;
static uint32_t g_accepted = 0;
static uint32_t g_rejected = 0;

static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  if (mhz < 240) mhz = 240;
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

extern "C" float cyd_last_bench_hs();

static void fillSnap() {
  g_snap.hashrateHs = g_hashrate;
  g_snap.shares = g_shareCounter;
  g_snap.accepted = g_accepted;
  g_snap.rejected = g_rejected;
  g_snap.pool = g_jobLoaded ? (g_miner.fullV() ? "USB HASH" : "USB TMTO") : "WAIT USB";
  g_snap.connected = g_jobLoaded;
  g_snap.difficulty = 0;
  g_snap.nonce = g_miner.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = true;
  g_snap.netTicker = g_net.ticker;
  g_snap.jobId = g_job.jobId;
  g_snap.fullV = g_miner.fullV();
  g_snap.benchHs = cyd_last_bench_hs();
}

static bool applyConfig(AppConfig& updated, bool& reboot) {
  updated.cpuMhz = 240;
  updated.hashFocus = true;
  g_cfg = updated;
  g_store.save(g_cfg);
  if (reboot) {
    Serial.flush();
    delay(60);
    ESP.restart();
  }
  applyCpu(240);
  return true;
}

static void onJob(const UsbJob& job) {
  // No spinlocks / no heap-in-critical — String copies are fine here.
  g_job = job;
  g_miner.setJob(job.header, job.target, job.startNonce ? job.startNonce : 1);
  g_jobLoaded = true;
  g_mining = true;
  g_windowHashesStart = g_hashCounter;
  g_windowStart = millis();
}

static void onStop() {
  g_jobLoaded = false;
  g_mining = false;
  g_hashrate = 0;
}

static void onStats(uint32_t accepted, uint32_t rejected) {
  g_accepted = accepted;
  g_rejected = rejected;
}

static void noteShare(uint32_t nonce) {
  g_shareCounter++;
  g_lastShareNonce = nonce;
  strncpy(g_shareJob, g_job.jobId.c_str(), sizeof(g_shareJob) - 1);
  g_shareJob[sizeof(g_shareJob) - 1] = 0;
  strncpy(g_shareEn2, g_job.extranonce2.c_str(), sizeof(g_shareEn2) - 1);
  g_shareEn2[sizeof(g_shareEn2) - 1] = 0;
  strncpy(g_shareNtime, g_job.ntime.c_str(), sizeof(g_shareNtime) - 1);
  g_shareNtime[sizeof(g_shareNtime) - 1] = 0;
  g_sharePending = true;
}

static void serviceCompanion() {
  fillSnap();
  g_cmp.poll(g_cfg, g_snap, applyConfig, &g_net, onJob, onStop, onStats);
  if (g_net.fresh) {
    g_snap.netTicker = g_net.ticker;
    g_net.fresh = false;
  }
  if (g_sharePending) {
    PendingShare s;
    s.nonce = g_lastShareNonce;
    s.jobId = g_shareJob;
    s.extranonce2 = g_shareEn2;
    s.ntime = g_shareNtime;
    s.pending = true;
    g_sharePending = false;
    g_cmp.emitShare(s);
  }
}

static void hashOnce() {
  if (!g_mining || !g_jobLoaded || !g_miner.ready()) return;
  // One hash, then return so USB/WDT stay alive (each scrypt hash can take ~0.5–2s).
  if (g_miner.mineOne(1)) {
    noteShare(g_miner.lastShareNonce());
  }
  g_hashCounter++;
  yield();
  esp_task_wdt_reset();
}

static float runBench(uint32_t hashes) {
  if (hashes < 1) hashes = 2;
  if (hashes > 16) hashes = 16;
  uint8_t hdr[80];
  memset(hdr, 0xA5, 80);
  uint8_t tgt[32];
  memset(tgt, 0xFF, 32);
  tgt[31] = 0;
  g_miner.setJob(hdr, tgt, 1);
  uint32_t t0 = micros();
  for (uint32_t i = 0; i < hashes; i++) {
    uint8_t out[32];
    g_miner.hashNonce(i + 1, out);
    yield();
    esp_task_wdt_reset();
  }
  uint32_t dt = micros() - t0;
  if (dt < 1) dt = 1;
  return (float)hashes * 1000000.0f / (float)dt;
}

void setup() {
  (void)esp_wifi_stop();
  (void)esp_wifi_deinit();

  g_cmp.begin(115200);
  g_ui.begin();
  g_ui.showSplash();

  g_store.load(g_cfg);
  g_cfg.cpuMhz = 240;
  g_cfg.hashFocus = true;
  applyCpu(240);

  // Allocate scrypt buffers now (heap ready). Prefer full V; fall back to TMTO.
  if (!g_miner.begin(true)) {
    g_ui.showMessage("HASH ERR", "scrypt alloc failed");
    delay(2000);
  }

  delay(80);
  {
    char line[96];
    snprintf(line, sizeof(line), "v=%uKB %s · 240MHz", (unsigned)(g_miner.vBytes() / 1024),
             g_miner.fullV() ? "FULL" : "TMTO");
    g_ui.showMessage("SCRYPT", line);
    delay(350);
  }
  g_ui.showWaitingCompanion();

  g_windowStart = millis();
  g_windowHashesStart = 0;
  g_lastPaint = millis();
  fillSnap();
}

void loop() {
  // Always service USB first so jobs/status never starve.
  serviceCompanion();

  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint > 2000) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(2);
    return;
  }

  // Hash one nonce, then USB again — keeps companion status/jobs alive.
  hashOnce();
  serviceCompanion();

  uint32_t now = millis();
  uint32_t elapsed = now - g_windowStart;
  if (elapsed >= 2000) {
    uint64_t cur = g_hashCounter;
    uint64_t delta = cur - g_windowHashesStart;
    g_hashrate = (float)delta * 1000.0f / (float)elapsed;
    g_windowHashesStart = cur;
    g_windowStart = now;
  }

  if (now - g_lastPaint >= 2000) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
}

extern "C" float cyd_run_bench(uint32_t n) {
  bool was = g_mining;
  g_mining = false;
  delay(2);
  g_lastBenchHs = runBench(n);
  g_mining = was;
  // Restore active job header if we interrupted mining.
  if (g_jobLoaded) {
    g_miner.setJob(g_job.header, g_job.target, g_miner.nonce());
  }
  return g_lastBenchHs;
}

extern "C" float cyd_last_bench_hs() { return g_lastBenchHs; }

extern "C" bool cyd_miner_full_v() { return g_miner.fullV(); }
