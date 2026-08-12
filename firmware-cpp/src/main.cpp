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
static Sha256Miner g_minerB;  // core 0 SW assist (disabled when HW SHA is active)
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
static uint32_t g_accepted = 0;
static uint32_t g_rejected = 0;
static TaskHandle_t g_mineTaskA = nullptr;
static TaskHandle_t g_usbTask = nullptr;

static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  if (mhz < 240) mhz = 240;
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

extern "C" float cyd_last_bench_hs();

static void updateHashrate() {
  // Must run from usbTask — Arduino loop() is starved by a high-prio USB task.
  // Core-0 SW assist + USB/LCD share a core, so short windows swing ±100+ kH/s.
  // Use ≥1.5s samples + EMA so LCD/Companion show a stable rate.
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
    // Window went stale (long pause) — restart without slamming the display to 0.
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
  g_snap.hashrateHs = g_hashrate;
  g_snap.shares = g_shareCounter;
  g_snap.totalHashes = g_hashCounter.load(std::memory_order_relaxed);
  g_snap.accepted = g_accepted;
  g_snap.rejected = g_rejected;
  if (g_jobLoaded && g_hwSha) {
    char poolBuf[24];
    snprintf(poolBuf, sizeof(poolBuf), "SHA256-%s", cyd_sha_hw::mode_label());
    g_snap.pool = poolBuf;
  } else if (g_jobLoaded) {
    g_snap.pool = "SHA256";
  } else {
    g_snap.pool = "WAIT USB";
  }
  g_snap.connected = g_jobLoaded;
  g_snap.difficulty = 0;
  g_snap.nonce = g_minerA.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = true;
  g_snap.netTicker = g_net.ticker;
  g_snap.jobId = g_job.jobId;
  g_snap.shaMode = g_hwSha ? String(cyd_sha_hw::mode_label()) : String("SW");
  g_snap.fullV = true;
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
  g_job = job;
  uint32_t start = job.startNonce ? job.startNonce : esp_random();
  // HW lane owns the SHA engine on the lower half of the nonce space.
  // Core-0 SW midstate assist searches the upper half — additive, no SHA contention.
  g_minerA.setJob(job.header, job.target, start);
  g_minerB.setJob(job.header, job.target, start ^ 0x80000000u);
  g_jobLoaded = true;
  g_mining = true;
  // New job: restart the sample window but keep the last EMA so the LCD
  // does not drop by 100+ kH/s on every stratum job switch.
  g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
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
  fillSnap();
  g_cmp.poll(g_cfg, g_snap, applyConfig, &g_net, onJob, onStop, onStats);
  if (g_net.fresh) {
    g_snap.netTicker = g_net.ticker;
    g_net.fresh = false;
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
      // Big IRAM batches; yield only every few batches to keep the WDT happy.
      mineLane(g_minerA, 1, 32768);
      if ((++loops & 3u) == 0u) {
        taskYIELD();
        esp_task_wdt_reset();
      }
    } else {
      mineLane(g_minerA, 2, 4096);
      taskYIELD();
      esp_task_wdt_reset();
    }
  }
}

// USB + hashrate accounting on core 0. Leave headroom for Arduino loop (LCD).
static void usbTask(void*) {
  for (;;) {
    serviceCompanion();
    // 5ms keeps Serial snappy without permanently preempting loopTask.
    vTaskDelay(pdMS_TO_TICKS(5));
    esp_task_wdt_reset();
  }
}

static float runBench(uint32_t hashes) {
  if (hashes < 1000) hashes = 1000;
  if (hashes > 400000) hashes = 400000;
  uint8_t hdr[80];
  memset(hdr, 0x11, 80);
  // Impossible target so the msb gate almost never trips (full-speed loop).
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
  // Core-0 assist must stay on software midstate — never touch the SHA peripheral.
  g_minerB.forceSoftware();
  g_hwSha = g_minerA.hardware();
  if (g_hwSha) {
    (void)Sha256Miner::acquireHardware();
  }

  // USB below miner, above default loop — enough to answer cmp without freezing LCD.
  xTaskCreatePinnedToCore(usbTask, "usb", 8192, nullptr, 3, &g_usbTask, 0);
  xTaskCreatePinnedToCore(mineTaskA, "shaA", 10240, nullptr, configMAX_PRIORITIES - 1, &g_mineTaskA,
                          1);

  delay(80);
  if (g_hwSha) {
    uint8_t hdr[80];
    memset(hdr, 0xA5, 80);
    uint8_t tgt[32];
    memset(tgt, 0xFF, 32);
    g_minerA.setJob(hdr, tgt, 1);
    char line[28];
    snprintf(line, sizeof(line), "%s · USB link", cyd_sha_hw::mode_label());
    g_ui.showMessage("SHA-256 MAX", line);
  } else {
    g_ui.showMessage("SHA-256", "live kH/s · USB link");
  }
  delay(420);
  g_ui.showWaitingCompanion();

  g_windowStart = millis();
  g_windowHashesStart = 0;
  g_lastPaint = millis();
  fillSnap();
}

void loop() {
  // LCD + light assist. Hashrate is updated in usbTask/fillSnap.
  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint >= 160) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(5);
    return;
  }

  // Core-0 software midstate assist on a disjoint nonce range (additive H/s).
  // Bigger batches while mining — LCD paints less often below.
  if (g_mining) {
    mineLane(g_minerB, 1, g_hwSha ? 512 : 128);
  }

  uint32_t now = millis();
  // ~4 fps UI while hashing — less contention with the mine task.
  if (now - g_lastPaint >= 250) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
  delay(1);
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
