#include "companion.hpp"
#include "config.hpp"
#include "display_ui.hpp"
#include "pool_stratum.hpp"
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
#include <cmath>

static ConfigStore g_store;
static AppConfig g_cfg;
static CompanionLink g_cmp;
static WifiLink g_wifi;
static PoolStratum g_pool;
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
static volatile bool g_otaHoldMining = false;
static std::atomic<uint64_t> g_hashCounter{0};
static volatile uint64_t g_shareCounter = 0;
// Ring so dual-lane hits survive USB/ESP-NOW latency and mid-job flushes.
static constexpr size_t kShareQ = 32;
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

static void flushShareQueue();
static void applyCpu(uint8_t mhz) {
  mhz = g_cfg.normalizeCpu(mhz);
  setCpuFrequencyMhz(mhz);
  g_cfg.cpuMhz = mhz;
}

extern "C" float cyd_last_bench_hs();
extern "C" float cyd_run_bench(uint32_t n, bool tune);

static void refreshLabels() {
  if (g_pool.authorized() && g_jobLoaded && g_hwSha) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "POOL-%s", cyd_sha_hw::mode_label());
    snprintf(g_shaLabel, sizeof(g_shaLabel), "%s", cyd_sha_hw::mode_label());
  } else if (g_pool.authorized() && g_jobLoaded) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "POOL");
    snprintf(g_shaLabel, sizeof(g_shaLabel), "SW");
  } else if (g_jobLoaded && g_hwSha) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "SHA256-%s", cyd_sha_hw::mode_label());
    snprintf(g_shaLabel, sizeof(g_shaLabel), "%s", cyd_sha_hw::mode_label());
  } else if (g_jobLoaded) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "SHA256");
    snprintf(g_shaLabel, sizeof(g_shaLabel), "SW");
  } else if (g_pool.active(g_cfg)) {
    snprintf(g_poolLabel, sizeof(g_poolLabel), "POOL %s", g_pool.phase());
    snprintf(g_shaLabel, sizeof(g_shaLabel), g_hwSha ? cyd_sha_hw::mode_label() : "SW");
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
  if (g_pool.authorized()) {
    g_snap.accepted = g_pool.accepted();
    g_snap.rejected = g_pool.rejected();
  } else {
    g_snap.accepted = g_accepted;
    g_snap.rejected = g_rejected;
  }
  g_snap.pool = g_poolLabel;
  g_snap.connected = g_jobLoaded || g_pool.authorized();
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
  // Companion only hands off job ownership once the board pool is authorized
  // (phase "ok"). Configured-but-offline must keep accepting USB/Wi‑Fi jobs.
  g_snap.mineIndep =
      g_cfg.mineIndep && g_cfg.poolConfigured() && g_pool.authorized();
  g_snap.poolEndpoint = g_pool.endpoint().length() ? g_pool.endpoint() : g_cfg.poolUrl;
  g_snap.poolPhase = g_pool.phase();
  if (WiFi.status() == WL_CONNECTED) {
    g_snap.wifiIp = WiFi.localIP().toString();
  } else {
    g_snap.wifiIp = g_wifi.softApIp().toString();
  }
  // Ticker disabled while hashing — net pushes are ACK'd but not painted.
  if (!g_mining) g_snap.netTicker = g_net.ticker;
}

static bool applyConfig(AppConfig& updated, bool& reboot) {
  // Honor caller cpu_mhz (cmp clock); default / corrupt → 240.
  updated.cpuMhz = updated.normalizeCpu(updated.cpuMhz ? updated.cpuMhz : 240);
  updated.hashFocus = true;
  g_cfg = updated;
  g_store.save(g_cfg);
  g_wifi.applyConfig(g_cfg);
  if (reboot) {
    Serial.flush();
    delay(60);
    ESP.restart();
  }
  applyCpu(g_cfg.cpuMhz);
  return true;
}

static void onJob(const UsbJob& job) {
  // Pause lanes first so a mid-hash hit cannot be tagged with the new job/en2
  // while still using the old midstate (invalid / Low-difficulty rejects).
  g_mining = false;
  // Emit queued hits for the previous header BEFORE invalidating the ring.
  // serviceCompanion used to poll (apply job → clear Q) then emit — so every
  // notify wiped in-flight CMPSHAREs and pool hashrate lagged the LCD (e.g. 71
  // vs 206 kH/s) while boards kept hashing.
  flushShareQueue();
  // Core-1 HW lane may still be inside mineBatch — give it a slice to exit and
  // noteShare against the *old* g_job, then flush again before we swap midstate.
  // Skip the settle during OTA hold so pool notify cannot stall binary RX.
  if (!g_otaHoldMining) {
    delay(3);
    flushShareQueue();
  }
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
  g_mining = !g_otaHoldMining;
  syncMinePriorities();
  refreshLabels();
  // Keep EMA across job switches so the LCD does not flash 0 H/s on every notify.
  // Only resync the sample window to the live counter.
  g_windowStart = millis();
  g_windowHashesStart = g_hashCounter.load(std::memory_order_relaxed);
}

static volatile bool g_needIndepTune = false;

/// Companion-fed jobs are ignored only once onboard pool is authorized.
/// Gating on active() (Wi‑Fi up + pool URL) left a dead gap: STA joins, Companion
/// jobs dropped, onboard stratum still connecting → pool sees no workers/shares.
static void onJobFromCompanion(const UsbJob& job) {
  if (g_pool.authorized()) return;
  onJob(job);
  // First Companion-fed job: lock D0 high-rate path once (same as indep).
  // FullHw == shaPath 0 is a valid lock — do NOT require shaPath > 0 or every
  // job re-runs a 60s bench and share finds go to zero at ESP diffs.
  if (g_hwSha && !g_cfg.pathTuned) {
    g_needIndepTune = true;
  }
}

static void onStop() {
  flushShareQueue();
  delay(2);
  flushShareQueue();
  portENTER_CRITICAL(&g_mux);
  for (size_t i = 0; i < kShareQ; i++) g_shareQ[i].used = false;
  portEXIT_CRITICAL(&g_mux);
  g_jobLoaded = false;
  g_mining = false;
  // Keep EMA visible across brief Companion stop→re-arm windows so LCD/Companion
  // don't flash 0 H/s while the pool is between unique-en2 jobs.
  syncMinePriorities();
  refreshLabels();
}

static void onStopFromCompanion() {
  // Indep-authorized boards own their work — ignore routine Companion stop/re-arm
  // (unique-en2 job push). OTA/Push calls setMiningHold(true) first so
  // g_otaHoldMining forces a real pause even while authorized.
  if (g_pool.authorized() && !g_otaHoldMining) return;
  onStop();
}

static void onStats(uint32_t accepted, uint32_t rejected) {
  // Independent pool owns accept/reject counters while authorized.
  if (g_pool.authorized()) return;
  g_accepted = accepted;
  g_rejected = rejected;
}

static void onPoolStats(uint32_t accepted, uint32_t rejected) {
  g_accepted = accepted;
  g_rejected = rejected;
}

static void onIndepTune() {
  // Defer heavy D0 Bench off the USB/pool poll path (blocks tens of seconds).
  if (!g_hwSha) return;
  if (g_cfg.pathTuned) {
    // shaPath 0 (FullHw) is a valid tuned lock.
    cyd_sha_hw::set_preferred_mode(g_cfg.shaPath);
    return;
  }
  g_needIndepTune = true;
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

static void flushShareQueue() {
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
    if (g_pool.authorized()) {
      (void)g_pool.submitShare(s);
    } else {
      g_cmp.emitShare(s);
    }
  }
}

static void serviceCompanion() {
  // During USB/Wi‑Fi OTA binary receive, only drain companion streams.
  // Pool connect / snap work can stall RX long enough to fail right
  // after "Board ready" (UI ~15%) before the first upload tick.
  if (g_cmp.otaBusy()) {
    // USB/TCP OTA binary only — skip Wi‑Fi/pool (NVS + radio work races
    // Update.write → CMPERR ota write mid-upload after Board ready).
    auto onApply = applyConfig;
    auto job = onJobFromCompanion;
    auto stop = onStopFromCompanion;
    auto stats = onStats;
    for (int i = 0; i < 16; i++) {
      g_cmp.poll(g_cfg, g_snap, onApply, &g_net, job, stop, stats);
      // SoftAP/TCP OTA still needs wifi drain; USB OTA stays on Serial.poll.
      if (g_wifi.tcpRxPending()) {
        g_wifi.poll(g_cmp, g_cfg, g_snap, onApply, &g_net, job, stop, stats);
      }
      if (!g_cmp.otaBusy()) break;
    }
    return;
  }
  // Snapshot often enough for live H/s without starving USB replies.
  uint32_t now = millis();
  const uint32_t snapMs = g_mining ? 320u : 220u;
  if (now - g_lastSnapMs >= snapMs) {
    fillSnap();
    g_lastSnapMs = now;
  }
  auto onApply = applyConfig;
  auto job = onJobFromCompanion;
  auto stop = onStopFromCompanion;
  auto stats = onStats;
  // Drain hits before poll so a cmp ja/job cannot wipe them inside onJob
  // without a prior emit (onJob also flushes; this covers the common path).
  flushShareQueue();
  // Independent pool mining (STA + pool URL) — each board owns its own stratum.
  g_pool.poll(g_cfg);
  refreshLabels();
  // Primary mirror: Companion TCP when linked; else cleared (USB Serial still emits).
  if (!g_wifi.tcpConnected()) {
    g_cmp.setShareMirror(nullptr);
  }
  g_cmp.poll(g_cfg, g_snap, onApply, &g_net, job, stop, stats);
  g_wifi.poll(g_cmp, g_cfg, g_snap, onApply, &g_net, job, stop, stats);
  syncMinePriorities();
  if (g_net.fresh) {
    g_net.fresh = false;
    if (!g_mining) g_snap.netTicker = g_net.ticker;
  }
  flushShareQueue();
}

static void syncMinePriorities() {
  if (!g_mineTaskB || !g_usbTask) return;
  if (g_mining && g_jobLoaded) {
    // USB slightly above SW assist; equal slice when idle.
    vTaskPrioritySet(g_usbTask, 3);
    vTaskPrioritySet(g_mineTaskB, 3);
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
    // Pending USB or SoftAP TCP bytes only — idle TCP must not starve SW assist.
    if (Serial.available() > 0 || g_wifi.tcpRxPending()) {
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
    const bool ota = g_cmp.otaBusy();
    const bool talk = ota || Serial.available() > 0;
    // During OTA binary, spin tight so Update.write keeps up with the host.
    const uint32_t ms = ota ? 0u : (talk ? 1u : (g_mining ? 4u : 3u));
    if (ms == 0) {
      esp_task_wdt_reset();
    } else {
      vTaskDelay(pdMS_TO_TICKS(ms));
      esp_task_wdt_reset();
    }
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

/// Live progress for Companion — keeps UI kH updating while USB waits on CMPBENCH.
static void emitBenchProg(const char* step, const char* path, float hs, uint8_t mhz) {
  if (hs > 0) g_lastBenchHs = hs;
  char line[192];
  snprintf(line, sizeof(line),
           "CMPBENCHPROG step=%s path=%s mhz=%u hs=%.0f khs=%.2f", step, path ? path : "?",
           (unsigned)mhz, hs, hs / 1000.0f);
  Serial.println(line);
  Serial.flush();
}

/// Two timed windows; reject if they disagree by >12% (unstable). Returns mean H/s or 0.
static float runBenchStable(uint32_t hashes, const char* path, uint8_t mhz) {
  uint32_t half = hashes / 2;
  if (half < 8000) half = 8000;
  float a = runBench(half);
  emitBenchProg("pass1", path, a, mhz);
  esp_task_wdt_reset();
  float b = runBench(half);
  emitBenchProg("pass2", path, b, mhz);
  esp_task_wdt_reset();
  float avg = (a + b) * 0.5f;
  if (avg < 1000.0f) return 0;
  float spread = fabsf(a - b) / avg;
  if (spread > 0.12f) {
    // One more settling pass — take the better of mean vs third if still close.
    float c = runBench(half);
    emitBenchProg("pass3", path, c, mhz);
    float avg2 = (avg + c) / 2.0f;
    float spread2 = fabsf(avg - c) / (avg2 > 1 ? avg2 : 1);
    if (spread2 > 0.15f) {
      emitBenchProg("unstable", path, avg2, mhz);
      return 0;
    }
    return avg2;
  }
  return avg;
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
  xTaskCreatePinnedToCore(usbTask, "usb", 8192, nullptr, 3, &g_usbTask, 0);

  g_ui.begin();
  g_ui.showSplash();

  g_store.load(g_cfg);
  g_cfg.cpuMhz = g_cfg.normalizeCpu(g_cfg.cpuMhz ? g_cfg.cpuMhz : 240);
  g_cfg.hashFocus = true;
  g_cfg.wifiEnabled = true;
  applyCpu(g_cfg.cpuMhz);
  cyd_sha_hw::set_preferred_mode(g_cfg.shaPath);
  g_wifi.begin(g_macStr, g_cfg);
  g_cmp.setWifiPersist([]() -> bool { return g_store.save(g_cfg); });
  g_cmp.setWifiApply([]() { g_wifi.applyConfig(g_cfg); });
  g_cmp.setMiningHold([](bool hold) {
    g_otaHoldMining = hold;
    if (hold) {
      onStop();
      // Park hash lanes so flash erase/write is not preempted mid-sector.
      if (g_mineTaskA) vTaskSuspend(g_mineTaskA);
      if (g_mineTaskB) vTaskSuspend(g_mineTaskB);
      if (g_usbTask) vTaskPrioritySet(g_usbTask, 5);
    } else {
      if (g_mineTaskA) vTaskResume(g_mineTaskA);
      if (g_mineTaskB) vTaskResume(g_mineTaskB);
      syncMinePriorities();
    }
  });
  // Phone SoftAP portal uses the same NVS + SoftAP/STA apply path.
  g_wifi.setPersist([]() {
    g_store.save(g_cfg);
    g_wifi.applyConfig(g_cfg);
  });
  g_pool.setCallbacks(onJob, onStop, onPoolStats, onIndepTune);

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
  if (g_needIndepTune) {
    g_needIndepTune = false;
    // First independent authorize: D0 auto-tune once so boards follow the high
    // hash-rate path without needing a Companion Bench click.
    (void)cyd_run_bench(60000, true);
    g_cfg.pathTuned = true;
    g_store.save(g_cfg);
    refreshLabels();
  }
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
    // Climb paths low→high throughput: MidHw → FullHw → HW/SW.
    // Dual-window timing rejects unstable scores; lock the highest stable path.
    cyd_sha_hw::set_preferred_mode(-1);
    cyd_sha_hw::force_recalibrate();
    g_minerA.setJob(hdr, tgt, 1);
    cyd_sha_hw::begin_tune_session();

    const cyd_sha_hw::Mode candidates[] = {
        cyd_sha_hw::Mode::MidHw,       // start lower
        cyd_sha_hw::Mode::FullHw,      // climb
        cyd_sha_hw::Mode::HwSwSecond,  // usually peak on D0
    };
    uint32_t per = hashes / 3;
    if (per < 16000) per = 16000;
    if (per > 90000) per = 90000;
    uint8_t mhz = (uint8_t)getCpuFrequencyMhz();
    if (mhz < 80) mhz = 240;

    emitBenchProg("start", "—", 0, mhz);
    for (cyd_sha_hw::Mode m : candidates) {
      if (m == cyd_sha_hw::Mode::MidHw && !cyd_sha_hw::midstate_ok()) continue;
      if (m == cyd_sha_hw::Mode::HwSwSecond && !cyd_sha_hw::hybrid_ok()) continue;
      cyd_sha_hw::force_mode(m);
      g_minerA.setJob(hdr, tgt, (uint32_t)m + 10);
      const char* label = cyd_sha_hw::mode_label_of(m);
      char msg[40];
      snprintf(msg, sizeof(msg), "bench %s…", label);
      g_ui.showMessage("D0 AUTO-TUNE", msg);
      emitBenchProg("path", label, g_lastBenchHs, mhz);
      float hs = runBenchStable(per, label, mhz);
      if (hs <= 0) {
        // Unstable — one longer single window as fallback score.
        hs = runBench(per);
        emitBenchProg("fallback", label, hs, mhz);
      }
      cyd_sha_hw::record_path_hs(m, hs);
      emitBenchProg("scored", label, hs, mhz);
      esp_task_wdt_reset();
    }

    auto report = cyd_sha_hw::finish_tune_session();
    g_cfg.shaPath = (int8_t)report.best;
    g_cfg.pathTuned = true;
    cyd_sha_hw::set_preferred_mode(g_cfg.shaPath);
    g_store.save(g_cfg);
    g_lastBenchHs = report.best_hs > 0 ? report.best_hs : runBench(hashes);
    emitBenchProg("best", cyd_sha_hw::mode_label(), g_lastBenchHs, mhz);
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
    uint8_t mhz = (uint8_t)getCpuFrequencyMhz();
    emitBenchProg("run", cyd_sha_hw::mode_label(), 0, mhz);
    g_lastBenchHs = runBench(hashes);
    emitBenchProg("done", cyd_sha_hw::mode_label(), g_lastBenchHs, mhz);
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
