#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "mesh_link.hpp"
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
// Small ring so dual-lane hits are not overwritten before USB/ESP-NOW emit (NerdMiner queues work).
static constexpr size_t kShareQ = 4;
struct ShareSlot {
  uint32_t nonce = 0;
  char job[48]{};
  char en2[48]{};
  char ntime[24]{};
  bool used = false;
};
static ShareSlot g_shareQ[kShareQ];
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
  // ~0.6s first sample, then ~1.2s + EMA keep LCD/Companion stable.
  if (!g_jobLoaded || !g_mining) {
    if (g_hashrate > 0.0f) {
      g_hashrate *= 0.92f;
      if (g_hashrate < 40.0f) g_hashrate = 0.0f;
    }
    return;
  }
  uint32_t now = millis();
  if (g_windowStart == 0) {
    g_windowStart = now;
    g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
    return;
  }
  uint32_t elapsed = now - g_windowStart;
  // First reading ASAP so the LCD is not stuck on 0 H/s after each job.
  const uint32_t needMs = (g_hashrate <= 1.0f) ? 500u : 1200u;
  if (elapsed < needMs) return;
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
  g_snap.meshRoot = g_mesh.isRoot();
  g_snap.meshBridging = g_mesh.isBridging();
  g_snap.meshPeers = (uint8_t)g_mesh.leafCount();
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
  // Drop shares from the previous header — NerdMiner invalidates on new work.
  portENTER_CRITICAL(&g_mux);
  for (size_t i = 0; i < kShareQ; i++) g_shareQ[i].used = false;
  portEXIT_CRITICAL(&g_mux);
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
  // Keep EMA across job switches so the LCD does not flash 0 H/s on every notify.
  // Only resync the sample window to the live counter.
  g_windowStart = millis();
  g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
}

static void onStop() {
  portENTER_CRITICAL(&g_mux);
  for (size_t i = 0; i < kShareQ; i++) g_shareQ[i].used = false;
  portEXIT_CRITICAL(&g_mux);
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
  size_t slot = kShareQ;
  for (size_t i = 0; i < kShareQ; i++) {
    if (!g_shareQ[i].used) {
      slot = i;
      break;
    }
  }
  if (slot == kShareQ) {
    // Queue full — keep older shares; drop this hit rather than overwrite.
    portEXIT_CRITICAL(&g_mux);
    return;
  }
  g_shareQ[slot].nonce = nonce;
  memcpy(g_shareQ[slot].job, job, sizeof(job));
  memcpy(g_shareQ[slot].en2, en2, sizeof(en2));
  memcpy(g_shareQ[slot].ntime, ntime, sizeof(ntime));
  g_shareQ[slot].used = true;
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
  g_mesh.poll(g_cmp, g_cfg, g_snap, onApply, &g_net, job, stop, stats);
  // Connectivity mesh: leaf boards without a SoftAP TCP Companion mirror shares
  // to the USB-linked root over ESP-NOW (does not multiply hashrate).
  if (!g_mesh.isRoot() && g_mesh.hasRootPeer() && !g_wifi.tcpConnected()) {
    g_cmp.setShareMirror(&g_mesh.leafOut());
  }
  // Re-balance core-0 when leaf peers appear/disappear on a hashing root.
  syncMinePriorities();
  if (g_net.fresh) {
    g_net.fresh = false;
    if (!g_mining) g_snap.netTicker = g_net.ticker;
  }
  for (;;) {
    PendingShare s;
    bool got = false;
    portENTER_CRITICAL(&g_mux);
    for (size_t i = 0; i < kShareQ; i++) {
      if (g_shareQ[i].used) {
        s.nonce = g_shareQ[i].nonce;
        s.jobId = g_shareQ[i].job;
        s.extranonce2 = g_shareQ[i].en2;
        s.ntime = g_shareQ[i].ntime;
        s.pending = true;
        g_shareQ[i].used = false;
        got = true;
        break;
      }
    }
    portEXIT_CRITICAL(&g_mux);
    if (!got) break;
    g_cmp.emitShare(s);
  }
}

static void syncMinePriorities() {
  if (!g_mineTaskB || !g_usbTask) return;
  const bool bridging = g_mesh.isBridging();
  if (g_mining && g_jobLoaded) {
    // Bridging root: USB/mesh slightly above SW assist; keep B live for H/s.
    // Solo root: equal slice so SW assist still adds H/s.
    vTaskPrioritySet(g_usbTask, bridging ? 4 : 3);
    vTaskPrioritySet(g_mineTaskB, bridging ? 2 : 3);
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
    // USB mesh root still hashes while bridging — mild batch cut + yields leave
    // headroom for ESP-NOW / cmp via without cratering solo-class H/s.
    const bool bridging = g_mesh.isBridging();
    if (g_hwSha) {
      mineLane(g_minerA, 1, bridging ? 49152 : 65536);
      const uint32_t mask = bridging ? 63u : 127u;
      if ((++loops & mask) == 0u) {
        vTaskDelay(1);
        esp_task_wdt_reset();
      }
    } else {
      mineLane(g_minerA, 2, bridging ? 8192 : 12288);
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
    // Bridging root: keep SW assist alive at a smaller batch so fleet H/s stays
    // high; yield more often so USB + ESP-NOW via still get core-0 time.
    const bool bridging = g_mesh.isBridging();
    mineLane(g_minerB, 1, bridging ? (g_hwSha ? 4096 : 2048) : (g_hwSha ? 12288 : 4096));
    // Pending USB or SoftAP TCP bytes: step aside so cmp RX isn't starved.
    if (Serial.available() > 0 || g_wifi.tcpConnected()) {
      vTaskDelay(1);
      esp_task_wdt_reset();
      continue;
    }
    if (bridging || (++loops & 31u) == 0u) {
      vTaskDelay(bridging ? 2 : 1);
      esp_task_wdt_reset();
    }
  }
}

// USB — stay responsive under hash load; mineB yields when RX is pending.
static void usbTask(void*) {
  for (;;) {
    serviceCompanion();
    const bool talk = Serial.available() > 0;
    const bool bridging = g_mesh.isBridging();
    const uint32_t ms = talk ? 1u : (bridging ? 2u : (g_mining ? 4u : 3u));
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
  cyd_sha_hw::set_preferred_mode(g_cfg.shaPath);
  g_wifi.begin(g_macStr, g_cfg);
  g_mesh.begin(mac);
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
    // First setJob runs the one-time HW path calibrate (honours NVS preferred path).
    g_minerA.setJob(hdr, tgt, 1);
    refreshLabels();
    char line[36];
#if CYD_D0_BUILD
    snprintf(line, sizeof(line), "D0 %s · USB/WiFi", cyd_sha_hw::mode_label());
    g_ui.showMessage("SHA-256 D0", line);
#else
    snprintf(line, sizeof(line), "%s · USB/WiFi", cyd_sha_hw::mode_label());
    g_ui.showMessage("SHA-256 MAX", line);
#endif
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

  // Mining: keep logo static; refresh status strip every ~1s so H/s stays live.
  if (g_mining) {
    uint32_t now = millis();
    if (!g_ui.miningChromeDrawn()) {
      fillSnap();
      g_ui.showMining(g_cfg, g_snap, true);
      g_lastPaint = now;
    } else if (now - g_lastPaint >= 1000) {
      fillSnap();
      g_ui.showMining(g_cfg, g_snap, false);
      g_lastPaint = now;
    }
    delay(50);
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

  uint8_t hdr[80];
  memset(hdr, 0xA5, 80);
  uint8_t tgt[32];
  memset(tgt, 0xFF, 32);

  uint32_t hashes = n;
  if (hashes < 12000) hashes = 12000;
  if (hashes > 400000) hashes = 400000;

  if (tune && g_hwSha) {
    // Full D0 auto-tune: time each correct SHA path with a real hash window,
    // then lock the winner into NVS so boot keeps the best option.
    cyd_sha_hw::set_preferred_mode(-1);
    cyd_sha_hw::force_recalibrate();
    g_minerA.setJob(hdr, tgt, 1);  // correctness gate + micro calibrate
    cyd_sha_hw::begin_tune_session();

    const cyd_sha_hw::Mode candidates[] = {
        cyd_sha_hw::Mode::FullHw,
        cyd_sha_hw::Mode::HwSwSecond,
        cyd_sha_hw::Mode::MidHw,
    };
    // Per-path sample — enough to rank stably; keep short so USB wait never
    // looks like a dead "Bench boards" click (Companion ~120s budget).
    uint32_t per = hashes / 3;
    if (per < 12000) per = 12000;
    if (per > 80000) per = 80000;

    for (cyd_sha_hw::Mode m : candidates) {
      if (m == cyd_sha_hw::Mode::MidHw && !cyd_sha_hw::midstate_ok()) continue;
      if (m == cyd_sha_hw::Mode::HwSwSecond && !cyd_sha_hw::hybrid_ok()) continue;
      cyd_sha_hw::force_mode(m);
      g_minerA.setJob(hdr, tgt, (uint32_t)m + 10);
      char msg[40];
      snprintf(msg, sizeof(msg), "bench %s…", cyd_sha_hw::mode_label_of(m));
      g_ui.showMessage("D0 AUTO-TUNE", msg);
      float hs = runBench(per);
      cyd_sha_hw::record_path_hs(m, hs);
      esp_task_wdt_reset();
    }

    auto report = cyd_sha_hw::finish_tune_session();
    g_cfg.shaPath = (int8_t)report.best;
    cyd_sha_hw::set_preferred_mode(g_cfg.shaPath);
    g_store.save(g_cfg);
    g_lastBenchHs = report.best_hs > 0 ? report.best_hs : runBench(hashes);
    refreshLabels();
    char done[40];
    snprintf(done, sizeof(done), "best %s · %.0f kH/s", cyd_sha_hw::mode_label(),
             g_lastBenchHs / 1000.0f);
    g_ui.showMessage("D0 AUTO-TUNE", done);
  } else {
    if (tune) {
      cyd_sha_hw::force_recalibrate();
    }
    g_minerA.setJob(hdr, tgt, 1);
    if (tune) {
      cyd_sha_hw::force_recalibrate();
      g_minerA.setJob(hdr, tgt, 2);
    }
    g_lastBenchHs = runBench(hashes);
  }

  g_mining = was;
  if (g_jobLoaded) {
    g_minerA.setJob(g_job.header, g_job.target, g_minerA.nonce());
    g_minerB.setJob(g_job.header, g_job.target, g_minerB.nonce());
  }
  return g_lastBenchHs;
}

extern "C" float cyd_last_bench_hs() { return g_lastBenchHs; }

extern "C" bool cyd_miner_full_v() { return true; }
