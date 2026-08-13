#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "sha256_hw.hpp"
#include "sha256_miner.hpp"
#include "wifi_link.hpp"

#include <atomic>
#include <cstdio>
#include <cstring>
#include <esp_system.h>
#include <esp_task_wdt.h>
#include <esp_wifi.h>
#include <esp_mac.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>
#include <WiFi.h>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static WifiLink g_wifi;
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
static char g_macStr[18] = "";
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

static void syncMinePriorities();

// Classic ESP32 SHA-256 mining ceiling is ~0.5–1 MH/s in ideal conditions.
// Anything far above that is a measurement bug (never a real sustained rate).
static constexpr float kMaxPlausibleHs = 2000000.0f;

static void updateHashrate() {
  // Core-0 SW assist + USB share a core — short windows swing wildly.
  // ~1.5s samples + heavy EMA keep LCD/Companion stable.
  if (!g_jobLoaded || !g_mining) {
    if (g_hashrate > 0.0f) {
      g_hashrate *= 0.92f;
      if (g_hashrate < 40.0f) g_hashrate = 0.0f;
    }
    return;
  }
  uint32_t now = millis();
  uint32_t elapsed = now - g_windowStart;
  if (elapsed < 1500) return;
  if (elapsed > 10000) {
    // Stale window (e.g. long stall) — resync baseline without zeroing EMA.
    g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
    g_windowStart = now;
    return;
  }
  uint64_t cur = g_hashCounter.load(std::memory_order_relaxed);
  uint64_t delta = cur - g_windowHashesStart;
  float instant = (float)delta * 1000.0f / (float)elapsed;
  if (instant > kMaxPlausibleHs) {
    // Discard impossible samples (e.g. old phantom counter bugs).
    g_windowHashesStart = cur;
    g_windowStart = now;
    return;
  }
  // Ignore empty windows (job switch / USB stall) so EMA doesn't collapse to 0.
  if (delta == 0) {
    g_windowStart = now;
    return;
  }
  if (g_hashrate <= 1.0f) {
    g_hashrate = instant;
  } else {
    // Heavy EMA — USB yield / mineB scheduling makes 1s instant rates noisy.
    g_hashrate = g_hashrate * 0.88f + instant * 0.12f;
  }
  if (g_hashrate > kMaxPlausibleHs) g_hashrate = kMaxPlausibleHs;
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
  g_snap.mining = g_mining && g_jobLoaded;
  g_snap.difficulty = 0;
  g_snap.nonce = g_minerA.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = true;
  g_snap.jobId = g_job.jobId;
  g_snap.shaMode = g_shaLabel;
  g_snap.fullV = true;
  g_snap.benchHs = cyd_last_bench_hs();
  g_snap.mac = g_macStr;
  g_snap.wifiMode = g_wifi.modeLabel();
  g_snap.wifiAp = g_wifi.softApSsid();
  if (WiFi.status() == WL_CONNECTED) {
    g_snap.wifiIp = WiFi.localIP().toString();
  } else {
    g_snap.wifiIp = g_wifi.softApIp().toString();
  }
  // Ticker disabled while hashing — net pushes are ACK'd but not painted.
  if (!g_mining) g_snap.netTicker = g_net.ticker;
}

static bool applyConfig(AppConfig& updated, bool& reboot) {
  updated.cpuMhz = 240;
  updated.hashFocus = true;
  g_cfg = updated;
  g_store.save(g_cfg);
  g_wifi.applyConfig(g_cfg);
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
  syncMinePriorities();
  refreshLabels();
  // Do NOT reset the hashrate sample window here. Faster pool notifies (0.8.26+)
  // were restarting the ≥1–2s window every job so the rate never matured and
  // Companion showed 0 H/s. Keep EMA; the counter keeps rising across jobs.
  if (g_windowStart == 0) {
    g_windowStart = millis();
    g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
  }
}

static void onStop() {
  g_jobLoaded = false;
  g_mining = false;
  g_hashrate = 0;
  syncMinePriorities();
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
  // Snapshot often enough for live H/s without starving USB replies.
  uint32_t now = millis();
  const uint32_t snapMs = g_mining ? 320u : 220u;
  if (now - g_lastSnapMs >= snapMs) {
    fillSnap();
    g_lastSnapMs = now;
  }
  auto onApply = applyConfig;
  auto job = onJob;
  auto stop = onStop;
  auto stats = onStats;
  g_cmp.poll(g_cfg, g_snap, onApply, &g_net, job, stop, stats);
  g_wifi.poll(g_cmp, g_cfg, g_snap, onApply, &g_net, job, stop, stats);
  if (g_net.fresh) {
    g_net.fresh = false;
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

static void syncMinePriorities() {
  if (!g_mineTaskB || !g_usbTask) return;
  if (g_mining && g_jobLoaded) {
    // Same priority: FreeRTOS time-slices core-0 so USB can ACK status/jobs
    // while SW assist still runs large batches (see mineTaskB Serial yield).
    vTaskPrioritySet(g_mineTaskB, 3);
    vTaskPrioritySet(g_usbTask, 3);
  } else {
    vTaskPrioritySet(g_usbTask, 3);
    vTaskPrioritySet(g_mineTaskB, 2);
  }
}

static void mineLane(Sha256Miner& m, uint32_t stride, size_t batch) {
  // Count only hashes the miner actually performed. Adding `batch` blindly
  // inflated the rate to tens of MH/s whenever mineBatch returned early
  // (not ready / midstate not set) — ESP32 cannot sustain that.
  const uint64_t before = m.hashes();
  if (m.mineBatch(batch, stride)) {
    noteShare(m.lastShareNonce());
  }
  const uint64_t after = m.hashes();
  if (after > before) {
    g_hashCounter.fetch_add(after - before, std::memory_order_relaxed);
  }
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
      mineLane(g_minerA, 1, 65536);
      if ((++loops & 127u) == 0u) {
        vTaskDelay(1);
        esp_task_wdt_reset();
      }
    } else {
      mineLane(g_minerA, 2, 12288);
      if ((++loops & 31u) == 0u) {
        vTaskDelay(1);
        esp_task_wdt_reset();
      }
    }
  }
}

// Core-0 SW assist — dedicated task so LCD/Arduino loop cannot starve hashing.
// vTaskDelay is required (taskYIELD never runs idle / TWDT). Yield promptly when
// Companion has RX pending so `cmp status` / job ACKs are not starved.
static void mineTaskB(void*) {
  uint32_t loops = 0;
  for (;;) {
    if (!g_mining || !g_jobLoaded) {
      vTaskDelay(pdMS_TO_TICKS(2));
      continue;
    }
    mineLane(g_minerB, 1, g_hwSha ? 12288 : 4096);
    // Pending USB bytes: step aside so usbTask can drain + reply.
    if (Serial.available() > 0) {
      vTaskDelay(1);
      esp_task_wdt_reset();
      continue;
    }
    if ((++loops & 31u) == 0u) {
      vTaskDelay(1);
      esp_task_wdt_reset();
    }
  }
}

// USB — stay responsive under hash load; mineB yields when RX is pending.
static void usbTask(void*) {
  for (;;) {
    serviceCompanion();
    const bool talk = Serial.available() > 0;
    const uint32_t ms = talk ? 1u : (g_mining ? 4u : 3u);
    vTaskDelay(pdMS_TO_TICKS(ms));
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
  uint8_t mac[6] = {0};
  if (esp_read_mac(mac, ESP_MAC_WIFI_STA) == ESP_OK) {
    snprintf(g_macStr, sizeof(g_macStr), "%02x:%02x:%02x:%02x:%02x:%02x", mac[0], mac[1], mac[2],
             mac[3], mac[4], mac[5]);
  } else {
    snprintf(g_macStr, sizeof(g_macStr), "unknown");
  }

  g_cmp.begin(460800);
  // Start USB cmp early — SoftAP / splash can take >1s; Companion probes must get
  // `CMP ok` even while Wi‑Fi is still coming up (second board after UART reset).
  xTaskCreatePinnedToCore(usbTask, "usb", 6144, nullptr, 3, &g_usbTask, 0);

  g_ui.begin();
  g_ui.showSplash();

  g_store.load(g_cfg);
  g_cfg.cpuMhz = 240;
  g_cfg.hashFocus = true;
  g_cfg.wifiEnabled = true;
  applyCpu(240);
  g_wifi.begin(g_macStr, g_cfg);
  g_cmp.setWifiApply([]() {
    g_store.save(g_cfg);
    g_wifi.applyConfig(g_cfg);
  });

  g_minerA.begin();
  g_minerB.begin();
  g_minerB.forceSoftware();
  g_hwSha = g_minerA.hardware();
  if (g_hwSha) {
    (void)Sha256Miner::acquireHardware();
  }
  refreshLabels();

  // Priorities: mineA (core1 max) > USB (3) > mineB (2) > Arduino loop (1).
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
    snprintf(line, sizeof(line), "%s · USB/WiFi", cyd_sha_hw::mode_label());
    g_ui.showMessage("SHA-256 MAX", line);
  } else {
    g_ui.showMessage("SHA-256", "USB + WiFi link");
  }
  delay(280);
  g_ui.showWaitingCompanion(g_snap);

  g_windowStart = millis();
  g_windowHashesStart = 0;
  g_lastPaint = millis();
  g_lastSnapMs = millis();
  fillSnap();
}

void loop() {
  syncMinePriorities();

  // Idle: logo + link / rate / Wi‑Fi IP (no animated bars).
  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint >= 1000) {
      fillSnap();
      g_ui.showWaitingCompanion(g_snap);
      g_lastPaint = now;
    }
    delay(20);
    return;
  }

  // Mining: keep logo static; refresh status strip every ~2s (light SPI only).
  if (g_mining) {
    uint32_t now = millis();
    if (!g_ui.miningChromeDrawn()) {
      fillSnap();
      g_ui.showMining(g_cfg, g_snap, true);
      g_lastPaint = now;
    } else if (now - g_lastPaint >= 2000) {
      fillSnap();
      g_ui.showMining(g_cfg, g_snap, false);
      g_lastPaint = now;
    }
    delay(100);
    return;
  }

  uint32_t now = millis();
  if (now - g_lastPaint >= 2000) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
  delay(50);
}

extern "C" float cyd_run_bench(uint32_t n, bool tune) {
  bool was = g_mining;
  g_mining = false;
  delay(12);
  if (tune) {
    cyd_sha_hw::force_recalibrate();
  }
  // Warm job so calibrate() can pick the fastest correct HW path.
  uint8_t hdr[80];
  memset(hdr, 0xA5, 80);
  uint8_t tgt[32];
  memset(tgt, 0xFF, 32);
  g_minerA.setJob(hdr, tgt, 1);
  if (tune) {
    // Second setJob after force_recalibrate still skips if calibrated mid-setJob —
    // calibrate runs inside setJob; force again then setJob once more for a clean timing.
    cyd_sha_hw::force_recalibrate();
    g_minerA.setJob(hdr, tgt, 2);
  }
  uint32_t hashes = n;
  if (hashes < 20000) hashes = 20000;
  if (hashes > 400000) hashes = 400000;
  g_lastBenchHs = runBench(hashes);
  g_mining = was;
  if (g_jobLoaded) {
    g_minerA.setJob(g_job.header, g_job.target, g_minerA.nonce());
    g_minerB.setJob(g_job.header, g_job.target, g_minerB.nonce());
  }
  return g_lastBenchHs;
}

extern "C" float cyd_last_bench_hs() { return g_lastBenchHs; }

extern "C" bool cyd_miner_full_v() { return true; }
