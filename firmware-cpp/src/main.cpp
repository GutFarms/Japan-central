#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "sha256_hw.hpp"
#include "sha256_miner.hpp"

#include <atomic>
#include <cstdio>
#include <cstring>
#include <esp_system.h>
#include <esp_task_wdt.h>
#include <esp_wifi.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static DisplayUi g_ui;
static Sha256Miner g_minerA;  // core 1 (HW SHA owner, or primary SW lane)
static Sha256Miner g_minerB;  // core 0 SW assist (disjoint nonce range)
static MinerSnapshot g_snap;
static bool g_hwSha = false;
static NetFeed g_net;
static UsbJob g_job;

static portMUX_TYPE g_mux = portMUX_INITIALIZER_UNLOCKED;
static volatile bool g_jobLoaded = false;
static volatile bool g_mining = false;
static std::atomic<uint64_t> g_hashCounter{0};
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
static uint32_t g_lastSnapMs = 0;
static uint32_t g_accepted = 0;
static uint32_t g_rejected = 0;
static TaskHandle_t g_mineTaskA = nullptr;
static TaskHandle_t g_mineTaskB = nullptr;
static TaskHandle_t g_usbTask = nullptr;

// Cached labels — avoid String churn on the USB hot path.
static char g_poolLabel[24] = "WAIT USB";
static char g_shaLabel[12] = "SW";
static bool g_labelsReady = false;

static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  if (mhz < 240) mhz = 240;
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

extern "C" float cyd_last_bench_hs();

static void refreshLabels() {
  if (g_jobLoaded && g_hwSha) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "SHA256-%s", cyd_sha_hw::mode_label());
    snprintf(g_shaLabel, sizeof(g_shaLabel), "%s", cyd_sha_hw::mode_label());
  } else if (g_jobLoaded) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "SHA256");
    snprintf(g_shaLabel, sizeof(g_shaLabel), "SW");
  } else {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "WAIT USB");
    snprintf(g_shaLabel, sizeof(g_shaLabel), g_hwSha ? cyd_sha_hw::mode_label() : "SW");
  }
  g_labelsReady = true;
}

static void updateHashrate() {
  // Core-0 SW assist + USB share a core — short windows swing wildly.
  // ≥1.5s samples + EMA keep LCD/Companion stable.
  if (!g_jobLoaded || !g_mining) {
    if (g_hashrate > 0.0f) {
      g_hashrate *= 0.82f;
      if (g_hashrate < 80.0f) g_hashrate = 0.0f;
    }
    return;
  }
  uint32_t now = millis();
  uint32_t elapsed = now - g_windowStart;
  if (elapsed < 1500) return;
  if (elapsed > 8000) {
    g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
    g_windowStart = now;
    return;
  }
  uint64_t cur = g_hashCounter.load(std::memory_order_relaxed);
  uint64_t delta = cur - g_windowHashesStart;
  float instant = (float)delta * 1000.0f / (float)elapsed;
  if (g_hashrate <= 1.0f) {
    g_hashrate = instant;
  } else {
    g_hashrate = g_hashrate * 0.78f + instant * 0.22f;
  }
  g_windowHashesStart = cur;
  g_windowStart = now;
}

static void fillSnap() {
  updateHashrate();
  if (!g_labelsReady) refreshLabels();
  g_snap.hashrateHs = g_hashrate;
  g_snap.shares = g_shareCounter;
  g_snap.totalHashes = g_hashCounter.load(std::memory_order_relaxed);
  g_snap.accepted = g_accepted;
  g_snap.rejected = g_rejected;
  g_snap.pool = g_poolLabel;
  g_snap.connected = g_jobLoaded;
  g_snap.difficulty = 0;
  g_snap.nonce = g_minerA.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = true;
  g_snap.jobId = g_job.jobId;
  g_snap.shaMode = g_shaLabel;
  g_snap.fullV = true;
  g_snap.benchHs = cyd_last_bench_hs();
  // Ticker disabled while hashing — net pushes are ACK'd but not painted.
  if (!g_mining) g_snap.netTicker = g_net.ticker;
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
  g_job = job;
  uint32_t start = job.startNonce ? job.startNonce : esp_random();
  // HW lane owns the SHA engine on the lower half of the nonce space.
  // Core-0 SW midstate assist searches the upper half — additive, no SHA contention.
  g_minerA.setJob(job.header, job.target, start);
  g_minerB.setJob(job.header, job.target, start ^ 0x80000000u);
  g_jobLoaded = true;
  g_mining = true;
  refreshLabels();
  // Keep last EMA across job switches so LCD doesn't drop.
  g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
  g_windowStart = millis();
}

static void onStop() {
  g_jobLoaded = false;
  g_mining = false;
  g_hashrate = 0;
  refreshLabels();
}

static void onStats(uint32_t accepted, uint32_t rejected) {
  g_accepted = accepted;
  g_rejected = rejected;
}

static void noteShare(uint32_t nonce) {
  char job[48], en2[48], ntime[24];
  strncpy(job, g_job.jobId.c_str(), sizeof(job) - 1);
  job[sizeof(job) - 1] = 0;
  strncpy(en2, g_job.extranonce2.c_str(), sizeof(en2) - 1);
  en2[sizeof(en2) - 1] = 0;
  strncpy(ntime, g_job.ntime.c_str(), sizeof(ntime) - 1);
  ntime[sizeof(ntime) - 1] = 0;
  portENTER_CRITICAL(&g_mux);
  g_shareCounter++;
  g_lastShareNonce = nonce;
  memcpy(g_shareJob, job, sizeof(g_shareJob));
  memcpy(g_shareEn2, en2, sizeof(g_shareEn2));
  memcpy(g_shareNtime, ntime, sizeof(g_shareNtime));
  g_sharePending = true;
  portEXIT_CRITICAL(&g_mux);
}

static void serviceCompanion() {
  // Snapshot ~1.5 Hz — enough for Companion UI, less core-0 churn while hashing.
  uint32_t now = millis();
  if (now - g_lastSnapMs >= 650) {
    fillSnap();
    g_lastSnapMs = now;
  }
  g_cmp.poll(g_cfg, g_snap, applyConfig, &g_net, onJob, onStop, onStats);
  if (g_net.fresh) {
    g_net.fresh = false;  // Accept but do not paint ticker while mining.
    if (!g_mining) g_snap.netTicker = g_net.ticker;
  }
  if (g_sharePending) {
    PendingShare s;
    portENTER_CRITICAL(&g_mux);
    s.nonce = g_lastShareNonce;
    s.jobId = g_shareJob;
    s.extranonce2 = g_shareEn2;
    s.ntime = g_shareNtime;
    s.pending = true;
    g_sharePending = false;
    portEXIT_CRITICAL(&g_mux);
    g_cmp.emitShare(s);
  }
}

static void mineLane(Sha256Miner& m, uint32_t stride, size_t batch) {
  if (m.mineBatch(batch, stride)) {
    noteShare(m.lastShareNonce());
  }
  g_hashCounter.fetch_add(batch, std::memory_order_relaxed);
}

static void mineTaskA(void*) {
  uint32_t loops = 0;
  for (;;) {
    if (!g_mining || !g_jobLoaded) {
      vTaskDelay(pdMS_TO_TICKS(2));
      continue;
    }
    if (g_hwSha) {
      // Big IRAM batches. Delay rarely — TWDT only needs idle every ~few seconds.
      // (0.8.17 delayed every 4 batches and cut peak H/s.)
      mineLane(g_minerA, 1, 32768);
      if ((++loops & 15u) == 0u) {
        vTaskDelay(1);
        esp_task_wdt_reset();
      }
    } else {
      mineLane(g_minerA, 2, 8192);
      if ((++loops & 7u) == 0u) {
        vTaskDelay(1);
        esp_task_wdt_reset();
      }
    }
  }
}

// Core-0 SW assist — dedicated task so LCD/Arduino loop cannot starve hashing.
// vTaskDelay is required (taskYIELD never runs idle / TWDT), but only every
// ~32 batches so assist H/s stays close to early peak speeds.
static void mineTaskB(void*) {
  uint32_t loops = 0;
  for (;;) {
    if (!g_mining || !g_jobLoaded) {
      vTaskDelay(pdMS_TO_TICKS(2));
      continue;
    }
    mineLane(g_minerB, 1, g_hwSha ? 4096 : 2048);
    if ((++loops & 31u) == 0u) {
      vTaskDelay(1);
      esp_task_wdt_reset();
    }
  }
}

// USB — snappy when RX has data; longer quiet delay frees core-0 for mineB.
static void usbTask(void*) {
  for (;;) {
    serviceCompanion();
    vTaskDelay(pdMS_TO_TICKS(Serial.available() > 0 ? 2 : 8));
    esp_task_wdt_reset();
  }
}

static float runBench(uint32_t hashes) {
  if (hashes < 1000) hashes = 1000;
  if (hashes > 400000) hashes = 400000;
  uint8_t hdr[80];
  memset(hdr, 0x11, 80);
  uint8_t tgt[32];
  memset(tgt, 0x00, 32);
  Sha256Miner bench;
  bench.begin();
  if (bench.hardware()) {
    (void)Sha256Miner::acquireHardware();
  }
  bench.setJob(hdr, tgt, 1);
  uint32_t t0 = micros();
  uint32_t done = 0;
  while (done < hashes) {
    uint32_t n = hashes - done;
    if (n > 4096) n = 4096;
    (void)bench.mineBatch(n, 1);
    done += n;
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

  g_minerA.begin();
  g_minerB.begin();
  g_minerB.forceSoftware();
  g_hwSha = g_minerA.hardware();
  if (g_hwSha) {
    (void)Sha256Miner::acquireHardware();
  }
  refreshLabels();

  // Priorities: mineA (core1 max) > USB (3) > mineB (2) > Arduino loop (1).
  xTaskCreatePinnedToCore(usbTask, "usb", 6144, nullptr, 3, &g_usbTask, 0);
  xTaskCreatePinnedToCore(mineTaskB, "shaB", 8192, nullptr, 2, &g_mineTaskB, 0);
  xTaskCreatePinnedToCore(mineTaskA, "shaA", 10240, nullptr, configMAX_PRIORITIES - 1, &g_mineTaskA,
                          1);

  delay(40);
  if (g_hwSha) {
    uint8_t hdr[80];
    memset(hdr, 0xA5, 80);
    uint8_t tgt[32];
    memset(tgt, 0xFF, 32);
    // First setJob runs the one-time HW path calibrate.
    g_minerA.setJob(hdr, tgt, 1);
    refreshLabels();
    char line[28];
    snprintf(line, sizeof(line), "%s · USB link", cyd_sha_hw::mode_label());
    g_ui.showMessage("SHA-256 MAX", line);
  } else {
    g_ui.showMessage("SHA-256", "hash focus · USB");
  }
  delay(280);
  g_ui.showWaitingCompanion();

  g_windowStart = millis();
  g_windowHashesStart = 0;
  g_lastPaint = millis();
  g_lastSnapMs = millis();
  fillSnap();
}

void loop() {
  // Idle: static wait screen (no animated bars).
  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint >= 1000) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(20);
    return;
  }

  // Mining: rare static LCD — hashing lives in mineTaskA/B.
  uint32_t now = millis();
  if (now - g_lastPaint >= 2000) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
  delay(50);
}

extern "C" float cyd_run_bench(uint32_t n) {
  bool was = g_mining;
  g_mining = false;
  delay(8);
  (void)n;
  g_lastBenchHs = runBench(100000);
  g_mining = was;
  if (g_jobLoaded) {
    g_minerA.setJob(g_job.header, g_job.target, g_minerA.nonce());
    g_minerB.setJob(g_job.header, g_job.target, g_minerB.nonce());
  }
  return g_lastBenchHs;
}

extern "C" float cyd_last_bench_hs() { return g_lastBenchHs; }

extern "C" bool cyd_miner_full_v() { return true; }
