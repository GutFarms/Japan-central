#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "scrypt_lite.hpp"

#include <cstring>
#include <esp_system.h>
#include <esp_wifi.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>
#include <freertos/semphr.h>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static DisplayUi g_ui;
static ScryptLite g_miner;       // core 1 — full-V when possible
static ScryptLite g_minerB;      // core 0 assist (TMTO / second lane)
static MinerSnapshot g_snap;
static NetFeed g_net;
static UsbJob g_job;

static portMUX_TYPE g_mineMux = portMUX_INITIALIZER_UNLOCKED;
static volatile bool g_jobLoaded = false;
static volatile bool g_mining = false;
static volatile uint64_t g_hashCounter = 0;
static volatile uint64_t g_shareCounter = 0;
static volatile uint32_t g_lastShareNonce = 0;
static volatile bool g_sharePending = false;
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
static TaskHandle_t g_mineTask = nullptr;

static void applyCpu(uint8_t mhz) {
  // Always prefer max clock for hashrate.
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
  g_snap.pool = g_jobLoaded ? (g_miner.fullV() ? "USB MAX" : "USB TMTO") : "WAIT USB";
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
  portENTER_CRITICAL(&g_mineMux);
  g_job = job;
  // Split nonce space across cores for dual-lane hashing.
  g_miner.setJob(job.header, job.target, job.startNonce);
  g_minerB.setJob(job.header, job.target, job.startNonce + 1);
  g_jobLoaded = true;
  g_mining = true;
  g_windowHashesStart = g_hashCounter;
  g_windowStart = millis();
  portEXIT_CRITICAL(&g_mineMux);
}

static void onStop() {
  portENTER_CRITICAL(&g_mineMux);
  g_jobLoaded = false;
  g_mining = false;
  g_hashrate = 0;
  portEXIT_CRITICAL(&g_mineMux);
}

static void onStats(uint32_t accepted, uint32_t rejected) {
  g_accepted = accepted;
  g_rejected = rejected;
}

static void noteShare(uint32_t nonce) {
  portENTER_CRITICAL(&g_mineMux);
  g_shareCounter++;
  g_lastShareNonce = nonce;
  strncpy(g_shareJob, g_job.jobId.c_str(), sizeof(g_shareJob) - 1);
  g_shareJob[sizeof(g_shareJob) - 1] = 0;
  strncpy(g_shareEn2, g_job.extranonce2.c_str(), sizeof(g_shareEn2) - 1);
  g_shareEn2[sizeof(g_shareEn2) - 1] = 0;
  strncpy(g_shareNtime, g_job.ntime.c_str(), sizeof(g_shareNtime) - 1);
  g_shareNtime[sizeof(g_shareNtime) - 1] = 0;
  g_sharePending = true;
  portEXIT_CRITICAL(&g_mineMux);
}

static void mineLane(ScryptLite& m, uint32_t stride) {
  // Large batches — USB is serviced on core 0 between bursts.
  const size_t batch = 8;
  if (m.mineBatch(batch, stride)) {
    noteShare(m.lastShareNonce());
  }
  portENTER_CRITICAL(&g_mineMux);
  g_hashCounter += batch;
  portEXIT_CRITICAL(&g_mineMux);
}

static void mineTask(void*) {
  for (;;) {
    if (!g_mining || !g_jobLoaded) {
      vTaskDelay(pdMS_TO_TICKS(2));
      continue;
    }
    mineLane(g_miner, 2);  // even nonces: start, start+2, …
  }
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
    portENTER_CRITICAL(&g_mineMux);
    s.nonce = g_lastShareNonce;
    s.jobId = g_shareJob;
    s.extranonce2 = g_shareEn2;
    s.ntime = g_shareNtime;
    s.pending = true;
    g_sharePending = false;
    portEXIT_CRITICAL(&g_mineMux);
    g_cmp.emitShare(s);
  }
}

static float runBench(uint32_t hashes) {
  if (hashes < 1) hashes = 4;
  if (hashes > 64) hashes = 64;
  uint8_t hdr[80];
  memset(hdr, 0xA5, 80);
  uint8_t tgt[32];
  memset(tgt, 0xFF, 32);
  tgt[31] = 0;
  ScryptLite bench;
  bench.setJob(hdr, tgt, 1);
  uint32_t t0 = micros();
  for (uint32_t i = 0; i < hashes; i++) {
    uint8_t out[32];
    bench.hashNonce(i, out);
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

  // g_miner / g_minerB already constructed: first grabs full 128KB V when heap allows.

  // Core 1 = dedicated hasher (highest throughput).
  xTaskCreatePinnedToCore(mineTask, "scrypt", 8192, nullptr, configMAX_PRIORITIES - 1, &g_mineTask,
                          1);

  delay(120);
  {
    char line[96];
    snprintf(line, sizeof(line), "v=%uKB %s · dual-core · 240MHz",
             (unsigned)(g_miner.vBytes() * 4 / 1024), g_miner.fullV() ? "FULL" : "TMTO");
    g_ui.showMessage("MAX HASH", line);
    delay(400);
  }
  g_ui.showWaitingCompanion();

  g_windowStart = millis();
  g_windowHashesStart = 0;
  g_lastPaint = millis();
  fillSnap();
}

void loop() {
  serviceCompanion();

  // Core-0 assist lane (odd nonces) when a job is live — squeezes both cores.
  if (g_mining && g_jobLoaded) {
    mineLane(g_minerB, 2);
  }

  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint > 2500) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(1);
    return;
  }

  // Light USB poll; hashing dominates both cores.
  static uint32_t lastUsb = 0;
  uint32_t now = millis();
  if (now - lastUsb >= 15) {
    serviceCompanion();
    lastUsb = now;
  }

  uint32_t elapsed = now - g_windowStart;
  if (elapsed >= 1500) {
    uint64_t cur = g_hashCounter;
    uint64_t delta = cur - g_windowHashesStart;
    g_hashrate = (float)delta * 1000.0f / (float)elapsed;
    g_windowHashesStart = cur;
    g_windowStart = now;
  }

  // Rare LCD paints — display kH/s.
  if (now - g_lastPaint >= 5000) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
}

// Hook bench into companion: parse is in companion.cpp via netdata-style; we add status fields.
// Provide C linkage helper used from companion.cpp
extern "C" float cyd_run_bench(uint32_t n) {
  bool was = g_mining;
  g_mining = false;
  delay(5);
  g_lastBenchHs = runBench(n);
  g_mining = was;
  return g_lastBenchHs;
}

extern "C" float cyd_last_bench_hs() { return g_lastBenchHs; }

extern "C" bool cyd_miner_full_v() { return g_miner.fullV(); }
