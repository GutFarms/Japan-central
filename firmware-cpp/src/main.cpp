#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "sha256_miner.hpp"

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
static Sha256Miner g_minerA;  // core 1
static Sha256Miner g_minerB;  // core 0 assist
static MinerSnapshot g_snap;
static NetFeed g_net;
static UsbJob g_job;

static portMUX_TYPE g_mux = portMUX_INITIALIZER_UNLOCKED;
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
  g_snap.pool = g_jobLoaded ? "SHA256" : "WAIT USB";
  g_snap.connected = g_jobLoaded;
  g_snap.difficulty = 0;
  g_snap.nonce = g_minerA.nonce();
  g_snap.cpuMhz = (uint8_t)getCpuFrequencyMhz();
  g_snap.hashFocus = true;
  g_snap.netTicker = g_net.ticker;
  g_snap.jobId = g_job.jobId;
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
  g_minerA.setJob(job.header, job.target, start);
  g_minerB.setJob(job.header, job.target, start + 1);
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

static void mineLane(Sha256Miner& m, uint32_t stride) {
  // Custom midstate SHA256d: ~2–4k hashes per slice keeps USB/WDT healthy.
  constexpr size_t BATCH = 2048;
  if (m.mineBatch(BATCH, stride)) {
    noteShare(m.lastShareNonce());
  }
  portENTER_CRITICAL(&g_mux);
  g_hashCounter += BATCH;
  portEXIT_CRITICAL(&g_mux);
}

static void mineTask(void*) {
  for (;;) {
    if (!g_mining || !g_jobLoaded) {
      vTaskDelay(pdMS_TO_TICKS(2));
      continue;
    }
    mineLane(g_minerA, 2);
    // Brief yield so core-0 USB / LCD keep up under dual-core load.
    taskYIELD();
    esp_task_wdt_reset();
  }
}

static float runBench(uint32_t hashes) {
  if (hashes < 1000) hashes = 1000;
  if (hashes > 200000) hashes = 200000;
  uint8_t hdr[80];
  memset(hdr, 0x11, 80);
  uint8_t tgt[32];
  memset(tgt, 0xFF, 32);
  Sha256Miner bench;
  bench.begin();
  bench.setJob(hdr, tgt, 1);
  uint32_t t0 = micros();
  uint8_t out[32];
  for (uint32_t i = 0; i < hashes; i++) {
    bench.hashNonce(i, out);
    if ((i & 0x3ff) == 0) {
      yield();
      esp_task_wdt_reset();
    }
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

  xTaskCreatePinnedToCore(mineTask, "sha256", 12288, nullptr, configMAX_PRIORITIES - 2, &g_mineTask,
                          1);

  delay(80);
  g_ui.showMessage("SHA-256", "midstate · 240MHz dual-core");
  delay(350);
  g_ui.showWaitingCompanion();

  g_windowStart = millis();
  g_windowHashesStart = 0;
  g_lastPaint = millis();
  fillSnap();
}

void loop() {
  serviceCompanion();

  if (g_mining && g_jobLoaded) {
    mineLane(g_minerB, 2);
  }

  if (!g_jobLoaded) {
    uint32_t now = millis();
    if (now - g_lastPaint > 2000) {
      g_ui.showWaitingCompanion();
      g_lastPaint = now;
    }
    delay(2);
    return;
  }

  static uint32_t lastUsb = 0;
  uint32_t now = millis();
  if (now - lastUsb >= 20) {
    serviceCompanion();
    lastUsb = now;
  }

  uint32_t elapsed = now - g_windowStart;
  if (elapsed >= 1000) {
    uint64_t cur = g_hashCounter;
    uint64_t delta = cur - g_windowHashesStart;
    g_hashrate = (float)delta * 1000.0f / (float)elapsed;
    g_windowHashesStart = cur;
    g_windowStart = now;
  }

  if (now - g_lastPaint >= 1500) {
    fillSnap();
    g_ui.showMining(g_cfg, g_snap, false);
    g_lastPaint = now;
  }
}

extern "C" float cyd_run_bench(uint32_t n) {
  bool was = g_mining;
  g_mining = false;
  delay(5);
  // n is unused small count from cmp — run a real KH/s bench.
  (void)n;
  g_lastBenchHs = runBench(50000);
  g_mining = was;
  if (g_jobLoaded) {
    g_minerA.setJob(g_job.header, g_job.target, g_minerA.nonce());
    g_minerB.setJob(g_job.header, g_job.target, g_minerB.nonce());
  }
  return g_lastBenchHs;
}

extern "C" float cyd_last_bench_hs() { return g_lastBenchHs; }

extern "C" bool cyd_miner_full_v() { return true; }
