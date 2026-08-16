#include "companion.hpp"
#include "mesh_link.hpp"
#include "sha256_hw.hpp"
#include <Update.h>
#include <cstring>
#include <esp_system.h>
#include <esp_task_wdt.h>

extern "C" float cyd_run_bench(uint32_t n, bool tune);

#if CYD_D0_BUILD
static constexpr const char* kFwTag = "0.8.191-sha256-d0";
#else
static constexpr const char* kFwTag = "0.8.191-sha256";
#endif

void CompanionLink::begin(uint32_t baud) {
  Serial.setRxBufferSize(16384);
  Serial.setTxBufferSize(4096);
  Serial.begin(baud);
  Serial.setTimeout(0);
  lineLen_ = 0;
  tcpLineLen_ = 0;
  haveHeader_ = false;
  haveTarget_ = false;
  otaRemain_ = 0;
  otaActive_ = false;
  otaLastRxMs_ = 0;
  out_ = &Serial;
  shareMirror_ = nullptr;
  shareMirror2_ = nullptr;
  activeLineBuf_ = lineBuf_;
  activeLineLen_ = &lineLen_;
}

bool CompanionLink::poll(AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply, NetFeed* net,
                         JobFn onJob, StopFn onStop, StatsFn onStats) {
  activeLineBuf_ = lineBuf_;
  activeLineLen_ = &lineLen_;
  return pollStream(Serial, Serial, cfg, snap, onApply, net, onJob, onStop, onStats);
}

bool CompanionLink::pollTcp(Stream& in, Print& out, AppConfig& cfg, const MinerSnapshot& snap,
                            ApplyFn onApply, NetFeed* net, JobFn onJob, StopFn onStop,
                            StatsFn onStats) {
  activeLineBuf_ = tcpLineBuf_;
  activeLineLen_ = &tcpLineLen_;
  bool ok = pollStream(in, out, cfg, snap, onApply, net, onJob, onStop, onStats);
  activeLineBuf_ = lineBuf_;
  activeLineLen_ = &lineLen_;
  return ok;
}

bool CompanionLink::pollOtaBinary(Stream& in, Print& out) {
  if (!otaActive_) return false;
  // Incomplete transfer / host abort: leave binary mode so the next `cmp ota`
  // can start clean (otherwise retry bytes are eaten as image data).
  if (otaRemain_ > 0 && (millis() - otaLastRxMs_) > 8000u) {
    Update.abort();
    otaActive_ = false;
    otaRemain_ = 0;
    if (onMiningHold_) onMiningHold_(false);
    out.println("CMPERR ota timeout");
    out.flush();
    return true;
  }
  if (otaRemain_ == 0) return false;
  uint8_t buf[512];
  // Drain aggressively — host can outrun Update.write and overflow the 16 KiB RX
  // buffer (looks like a disconnect / fail at ~82% after host upload 100%).
  // Smaller chunks + WDT reset between flash sector writes reduce CMPERR ota write.
  int budget = 1024;
  while (budget-- > 0 && otaRemain_ > 0 && in.available() > 0) {
    size_t want = otaRemain_ < sizeof(buf) ? otaRemain_ : sizeof(buf);
    int n = in.available();
    if (n <= 0) break;
    if ((size_t)n < want) want = (size_t)n;
    size_t got = in.readBytes(buf, want);
    if (got == 0) break;
    otaLastRxMs_ = millis();
    size_t wrote = Update.write(buf, got);
    if (wrote != got) {
      int err = Update.getError();
      Update.abort();
      otaActive_ = false;
      otaRemain_ = 0;
      if (onMiningHold_) onMiningHold_(false);
      char line[48];
      snprintf(line, sizeof(line), "CMPERR ota write err=%d", err);
      out.println(line);
      out.flush();
      return true;
    }
    otaRemain_ -= got;
    esp_task_wdt_reset();
  }
  if (otaRemain_ == 0) {
    otaActive_ = false;
    if (!Update.end(true)) {
      int err = Update.getError();
      if (onMiningHold_) onMiningHold_(false);
      char line[48];
      snprintf(line, sizeof(line), "CMPERR ota end err=%d", err);
      out.println(line);
      out.flush();
      return true;
    }
    // ACK first and hold the USB link long enough for the host to read it
    // before ESP.restart() drops the CH340 (classic fail-at-~82% after 100%).
    out.println("CMPACK ota ok");
    out.flush();
    delay(800);
    ESP.restart();
  }
  return true;
}

bool CompanionLink::pollStream(Stream& in, Print& out, AppConfig& cfg, const MinerSnapshot& snap,
                               ApplyFn onApply, NetFeed* net, JobFn onJob, StopFn onStop,
                               StatsFn onStats) {
  Print* prev = out_;
  out_ = &out;
  if (otaActive_) {
    (void)pollOtaBinary(in, out);
    out_ = prev;
    return false;
  }
  bool applied = false;
  int budget = 8192;
  while (budget-- > 0 && in.available() > 0) {
    char c = (char)in.read();
    if (c == '\n' || c == '\r') {
      if (*activeLineLen_ > 0) {
        activeLineBuf_[*activeLineLen_] = 0;
        String cmd(activeLineBuf_);
        *activeLineLen_ = 0;
        cmd.trim();
        if (cmd.length() == 0) continue;
        handleLine(cmd, cfg, snap,
                   [&](AppConfig& u, bool& reboot) {
                     bool ok = onApply(u, reboot);
                     if (ok) applied = true;
                     return ok;
                   },
                   net, onJob, onStop, onStats);
        // OTA may have switched to binary mode mid-poll.
        if (otaActive_) break;
      }
    } else if (c >= 32 && c < 127) {
      if (*activeLineLen_ + 1 < kLineCap) {
        activeLineBuf_[(*activeLineLen_)++] = c;
      } else {
        *activeLineLen_ = 0;
      }
    }
  }
  if (otaActive_) {
    (void)pollOtaBinary(in, out);
  }
  out_ = prev;
  return applied;
}

void CompanionLink::emitShare(const PendingShare& share) {
  if (!share.pending) return;
  char nonceHex[9];
  snprintf(nonceHex, sizeof(nonceHex), "%08x", (unsigned)share.nonce);
  auto writeShare = [&](Print& p) {
    p.print("CMPSHARE nonce=");
    p.print(nonceHex);
    p.print("&job=");
    p.print(share.jobId);
    p.print("&en2=");
    p.print(share.extranonce2);
    p.print("&ntime=");
    p.println(share.ntime);
    p.flush();
  };
  writeShare(Serial);
  if (shareMirror_) writeShare(*shareMirror_);
  if (shareMirror2_ && shareMirror2_ != shareMirror_) writeShare(*shareMirror2_);
}

void CompanionLink::handleLine(const String& line, AppConfig& cfg, const MinerSnapshot& snap,
                               ApplyFn onApply, NetFeed* net, JobFn onJob, StopFn onStop,
                               StatsFn onStats) {
  String t = line;
  int idx = -1;
  {
    String lower = t;
    lower.toLowerCase();
    for (int i = 0; i + 3 <= (int)lower.length(); i++) {
      if (lower[i] == 'c' && lower[i + 1] == 'm' && lower[i + 2] == 'p' &&
          (i + 3 == (int)lower.length() || lower[i + 3] == ' ' || lower[i + 3] == '\t')) {
        idx = i;
        break;
      }
    }
    if (idx < 0) return;
  }
  t = t.substring(idx);
  t.trim();

  String rest = t.substring(3);
  rest.trim();
  if (rest.length() == 0) rest = "ping";

  int sp = rest.indexOf(' ');
  String verb = (sp < 0) ? rest : rest.substring(0, sp);
  String args = (sp < 0) ? "" : rest.substring(sp + 1);
  verb.toLowerCase();
  args.trim();

  // USB Companion traffic elects this board as the ESP-NOW mesh root/bridge.
  if (out_ == &Serial) g_mesh.noteUsbActivity();

  if (verb == "mesh") {
    g_mesh.replyMeshList(*out_);
    return;
  }
  if (verb == "via") {
    int sp2 = args.indexOf(' ');
    String mac = (sp2 < 0) ? args : args.substring(0, sp2);
    String rest = (sp2 < 0) ? "" : args.substring(sp2 + 1);
    mac.trim();
    rest.trim();
    (void)g_mesh.handleVia(mac, rest);
    return;
  }

  if (verb == "ping") {
    char buf[48];
    snprintf(buf, sizeof(buf), "CMP ok cmp mac=%s",
             snap.mac.length() ? snap.mac.c_str() : "unknown");
    out_->println(buf);
    out_->flush();
    return;
  }
  if (verb == "status") {
    replyStatus(cfg, snap);
    out_->flush();
    return;
  }
  if (verb == "config") {
    replyConfig(cfg, snap);
    out_->flush();
    return;
  }

  // Multi-part job: short lines that survive 115200 USB under load.
  //   cmp jh <160 hex>              — header
  //   cmp jt <64 hex>               — target
  //   cmp ja job=&en2=&ntime=&start= — arm
  // Flush ACKs so Companion never times out waiting under hash load.
  if (verb == "jh" || verb == "jobhdr" || verb == "header") {
    String hex = args;
    int eq = hex.indexOf('=');
    if (eq >= 0) hex = hex.substring(eq + 1);
    hex.trim();
    if (!hexDecodeFixed(hex, stagedHeader_, 80)) {
      out_->println("CMPERR jh need 160 hex");
      out_->flush();
      return;
    }
    haveHeader_ = true;
    out_->println("CMPACK jh");
    out_->flush();
    return;
  }
  if (verb == "jt" || verb == "jobtgt" || verb == "target") {
    String hex = args;
    int eq = hex.indexOf('=');
    if (eq >= 0) hex = hex.substring(eq + 1);
    hex.trim();
    if (!hexDecodeFixed(hex, stagedTarget_, 32)) {
      out_->println("CMPERR jt need 64 hex");
      out_->flush();
      return;
    }
    haveTarget_ = true;
    out_->println("CMPACK jt");
    out_->flush();
    return;
  }
  if (verb == "ja" || verb == "jobarm" || verb == "arm") {
    if (!haveHeader_ || !haveTarget_) {
      out_->println("CMPERR ja need jh+jt first");
      out_->flush();
      return;
    }
    UsbJob job;
    if (!parseJobMeta(args, job)) {
      out_->println("CMPERR ja bad meta");
      out_->flush();
      return;
    }
    memcpy(job.header, stagedHeader_, 80);
    memcpy(job.target, stagedTarget_, 32);
    job.valid = true;
    job.fresh = true;
    out_->println("CMPACK ja");
    out_->flush();
    if (onJob) onJob(job);
    return;
  }

  // Legacy one-shot job (kept for older companions).
  if (verb == "job") {
    UsbJob job;
    if (!parseJob(args, job)) {
      out_->println("CMPERR job header+target required");
      out_->flush();
      return;
    }
    out_->println("CMPACK job");
    out_->flush();
    if (onJob) onJob(job);
    return;
  }
  if (verb == "stop") {
    if (onStop) onStop();
    out_->println("CMPACK stop");
    out_->flush();
    return;
  }
  if (verb == "bench") {
    uint32_t n = 100000;
    bool tune = false;
    if (args.length()) {
      int start = 0;
      while (start < (int)args.length()) {
        int amp = args.indexOf('&', start);
        String pair = (amp < 0) ? args.substring(start) : args.substring(start, amp);
        int eq = pair.indexOf('=');
        String key = (eq < 0) ? pair : pair.substring(0, eq);
        String val = (eq < 0) ? "" : pair.substring(eq + 1);
        key.toLowerCase();
        if (key == "n" || key == "hashes") {
          int v = val.toInt();
          if (v > 0) n = (uint32_t)v;
        } else if (key == "tune" || key == "opt" || key == "best") {
          tune = (val.length() == 0 || val == "1" || val.equalsIgnoreCase("true"));
        }
        if (amp < 0) break;
        start = amp + 1;
      }
      // Legacy: `cmp bench n=8` or bare number
      if (args.indexOf('=') < 0) {
        int v = args.toInt();
        if (v > 0) n = (uint32_t)v;
      }
    }
    if (n < 1000) n = 1000;
    if (n > 400000) n = 400000;
    float hs = cyd_run_bench(n, tune);
    char line[280];
    if (tune) {
      const auto& tr = cyd_sha_hw::last_tune_report();
      snprintf(line, sizeof(line),
               "CMPBENCH hashes=%u hs=%.0f khs=%.3f path=%s tune=1 d0=%u "
               "HW=%.0f HW+=%.0f HW/SW=%.0f best=%s",
               (unsigned)n, hs, hs / 1000.0f, cyd_sha_hw::mode_label(),
               (unsigned)CYD_D0_BUILD, tr.hw.hs, tr.hw_plus.hs, tr.hw_sw.hs,
               cyd_sha_hw::mode_label_of(tr.best));
    } else {
      snprintf(line, sizeof(line),
               "CMPBENCH hashes=%u hs=%.0f khs=%.3f path=%s tune=0 d0=%u", (unsigned)n, hs,
               hs / 1000.0f, cyd_sha_hw::mode_label(), (unsigned)CYD_D0_BUILD);
    }
    out_->println(line);
    out_->flush();
    return;
  }
  if (verb == "stats") {
    uint32_t acc = 0, rej = 0;
    int start = 0;
    while (start < (int)args.length()) {
      int amp = args.indexOf('&', start);
      String pair = (amp < 0) ? args.substring(start) : args.substring(start, amp);
      int eq = pair.indexOf('=');
      String key = (eq < 0) ? pair : pair.substring(0, eq);
      String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
      key.toLowerCase();
      if (key == "accepted" || key == "a") acc = (uint32_t)val.toInt();
      else if (key == "rejected" || key == "r") rej = (uint32_t)val.toInt();
      if (amp < 0) break;
      start = amp + 1;
    }
    if (onStats) onStats(acc, rej);
    out_->println("CMPACK stats");
    return;
  }
  if (verb == "netdata" || verb == "net" || verb == "push") {
    // ACK for Companion compatibility — LCD ticker is disabled in hash-focus builds.
    (void)net;
    (void)args;
    out_->println("CMPACK net");
    return;
  }
  if (verb == "set" || verb == "clock" || verb == "reboot") {
    AppConfig updated = cfg;
    bool reboot = (verb == "reboot");
    parseBody(args, updated, reboot);
    if (verb == "clock" && args.indexOf("cpu_mhz") < 0 && args.indexOf("clock") < 0) {
      out_->println("CMPERR cpu_mhz required");
      return;
    }
    out_->println("CMPACK queued");
    if (reboot) out_->flush();
    (void)onApply(updated, reboot);
    return;
  }
  
  if (verb == "pool") {
    if (args.length() == 0 || args.equalsIgnoreCase("status")) {
      char buf[280];
      snprintf(buf, sizeof(buf),
               "CMPACK pool indep=%u url=%s worker=%s phase=%s ep=%s",
               cfg.mineIndep ? 1u : 0u,
               cfg.poolUrl.length() ? cfg.poolUrl.c_str() : "-",
               cfg.poolWorker.length() ? cfg.poolWorker.c_str() : "-",
               snap.poolPhase.length() ? snap.poolPhase.c_str() : "-",
               snap.poolEndpoint.length() ? snap.poolEndpoint.c_str() : "-");
      out_->println(buf);
      out_->flush();
      return;
    }
    if (args.equalsIgnoreCase("clear") || args.indexOf("clear=1") >= 0) {
      cfg.poolUrl = "";
      cfg.poolWorker = "";
      cfg.poolPass = "x";
      cfg.mineIndep = false;
      if (onWifiPersist_ && !onWifiPersist_()) {
        out_->println("CMPERR pool nvs");
        out_->flush();
        return;
      }
      out_->println("CMPACK pool cleared");
      out_->flush();
      return;
    }
    // cmp pool url=…&worker=…&pass=…&indep=1
    int start = 0;
    while (start < (int)args.length()) {
      int amp = args.indexOf('&', start);
      String pair = (amp < 0) ? args.substring(start) : args.substring(start, amp);
      int eq = pair.indexOf('=');
      String key = (eq < 0) ? pair : pair.substring(0, eq);
      String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
      key.toLowerCase();
      if (key == "url" || key == "pool" || key == "host") {
        if (val.length() > 120) val = val.substring(0, 120);
        cfg.poolUrl = val;
      } else if (key == "worker" || key == "user" || key == "login") {
        if (val.length() > 80) val = val.substring(0, 80);
        cfg.poolWorker = val;
      } else if (key == "pass" || key == "password" || key == "pwd") {
        if (val.length() > 64) val = val.substring(0, 64);
        cfg.poolPass = val;
      } else if (key == "indep" || key == "independent" || key == "en" || key == "enable") {
        cfg.mineIndep =
            !(val == "0" || val.equalsIgnoreCase("false") || val.equalsIgnoreCase("off"));
      }
      if (amp < 0) break;
      start = amp + 1;
    }
    cfg.poolUrl.trim();
    cfg.poolWorker.trim();
    if (cfg.poolUrl.length() == 0 || cfg.poolWorker.length() == 0) {
      out_->println("CMPERR pool need url+worker");
      out_->flush();
      return;
    }
    if (cfg.poolPass.length() == 0) cfg.poolPass = "x";
    cfg.mineIndep = true;
    if (onWifiPersist_ && !onWifiPersist_()) {
      out_->println("CMPERR pool nvs");
      out_->flush();
      return;
    }
    char ack[200];
    snprintf(ack, sizeof(ack), "CMPACK pool saved url=%s worker=%s indep=1",
             cfg.poolUrl.c_str(), cfg.poolWorker.c_str());
    out_->println(ack);
    out_->flush();
    return;
  }

  if (verb == "wifi") {
    if (args.length() == 0 || args.equalsIgnoreCase("status")) {
      char buf[220];
      snprintf(buf, sizeof(buf),
               "CMPACK wifi en=%u ssid=%s mode=%s ip=%s ap=%s",
               cfg.wifiEnabled ? 1u : 0u,
               cfg.wifiSsid.length() ? cfg.wifiSsid.c_str() : "-",
               snap.wifiMode.length() ? snap.wifiMode.c_str() : "-",
               snap.wifiIp.length() ? snap.wifiIp.c_str() : "-",
               snap.wifiAp.length() ? snap.wifiAp.c_str() : "-");
      out_->println(buf);
      out_->flush();
      return;
    }
    if (args.equalsIgnoreCase("clear") || args.indexOf("clear=1") >= 0) {
      cfg.wifiSsid = "";
      cfg.wifiPass = "";
      // Keep SoftAP alive for setup/mesh after clearing home STA.
      cfg.wifiEnabled = true;
      if (onWifiPersist_ && !onWifiPersist_()) {
        out_->println("CMPERR wifi nvs");
        out_->flush();
        return;
      }
      out_->println("CMPACK wifi cleared");
      out_->flush();
      if (onWifiApply_) onWifiApply_();
      return;
    }
    parseWifiBody(args, cfg);
    cfg.wifiSsid.trim();
    if (cfg.wifiSsid.length() == 0) {
      out_->println("CMPERR wifi need ssid");
      out_->flush();
      return;
    }
    // Always enable radio when home credentials are pushed.
    cfg.wifiEnabled = true;
    // Persist to NVS BEFORE ACK so SoftAP TCP teardown cannot strand unsaved creds.
    if (onWifiPersist_ && !onWifiPersist_()) {
      out_->println("CMPERR wifi nvs");
      out_->flush();
      return;
    }
    char ack[160];
    snprintf(ack, sizeof(ack), "CMPACK wifi saved ssid=%s", cfg.wifiSsid.c_str());
    out_->println(ack);
    out_->flush();
    // Apply SoftAP/STA after the host has the ACK (TCP may drop on SoftAP rebuild).
    if (onWifiApply_) onWifiApply_();
    return;
  }

  // Wireless (or USB) firmware push — app image only (not merged @ 0x0).
  //   cmp ota size=NNNN      → CMPACK ota ready → raw NNNN bytes → CMPACK ota ok → reboot
  if (verb == "ota" || verb == "update" || verb == "fwupdate") {
    size_t size = 0;
    int start = 0;
    while (start < (int)args.length()) {
      int amp = args.indexOf('&', start);
      String pair = (amp < 0) ? args.substring(start) : args.substring(start, amp);
      int eq = pair.indexOf('=');
      String key = (eq < 0) ? pair : pair.substring(0, eq);
      String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
      key.toLowerCase();
      if (key == "size" || key == "bytes" || key == "len") {
        long v = val.toInt();
        if (v > 0) size = (size_t)v;
      }
      if (amp < 0) break;
      start = amp + 1;
    }
    // Bare number: `cmp ota 957536`
    if (size == 0 && args.length() && args.indexOf('=') < 0) {
      long v = args.toInt();
      if (v > 0) size = (size_t)v;
    }

    if (size == 0) {
      out_->println("CMPERR ota need size=");
      out_->flush();
      return;
    }
    // App partition is 0x1E0000; reject tiny / oversized payloads.
    if (size < 64 * 1024 || size > 0x1E0000) {
      out_->println("CMPERR ota size");
      out_->flush();
      return;
    }
    if (otaActive_) {
      Update.abort();
      otaActive_ = false;
      otaRemain_ = 0;
    }
    // Force-pause hashing (incl. indep pool) so Update.write is not starved /
    // RX overflowed mid-transfer — host then sees ~82% UI with no ACK.
    if (onMiningHold_) onMiningHold_(true);
    if (onStop) onStop();
    if (!Update.begin(size, U_FLASH)) {
      if (onMiningHold_) onMiningHold_(false);
      out_->println("CMPERR ota begin");
      out_->flush();
      return;
    }
    otaRemain_ = size;
    otaActive_ = true;
    otaLastRxMs_ = millis();
    out_->println("CMPACK ota ready");
    out_->flush();
    // Brief settle so miners are fully suspended before the host streams bytes.
    delay(40);
    return;
  }

  out_->println(
      "CMPERR unknown (ping|status|config|wifi|pool|ota|mesh|via|jh|jt|ja|job|stop|stats|bench|clock|reboot|netdata)");
}

static void copyJsonSafe(char* dst, size_t dstLen, const char* src, size_t maxCopy) {
  if (!dst || dstLen == 0) return;
  size_t n = 0;
  if (src) {
    while (src[n] && n < maxCopy && n + 1 < dstLen) {
      char c = src[n];
      // Keep JSON string-safe (job ids are hex; pool/sha labels are ASCII).
      if (c == '"' || c == '\\' || (unsigned char)c < 32) c = '_';
      dst[n] = c;
      n++;
    }
  }
  dst[n] = 0;
}

void CompanionLink::replyStatus(const AppConfig& cfg, const MinerSnapshot& snap) {
  // Hand-rolled JSON — ArduinoJson alloc on every Companion poll was burning core 0.
  char nonceHex[9];
  snprintf(nonceHex, sizeof(nonceHex), "%08x", (unsigned)snap.nonce);
  char pool[24], job[28], sha[12];
  copyJsonSafe(pool, sizeof(pool), snap.pool.c_str(), 20);
  copyJsonSafe(job, sizeof(job), snap.jobId.c_str(), 24);
  copyJsonSafe(sha, sizeof(sha), snap.shaMode.length() ? snap.shaMode.c_str() : "-", 8);
  char mac[20];
  copyJsonSafe(mac, sizeof(mac), snap.mac.length() ? snap.mac.c_str() : "", 17);
  char pep[48], pph[16];
  copyJsonSafe(pep, sizeof(pep), snap.poolEndpoint.c_str(), 44);
  copyJsonSafe(pph, sizeof(pph), snap.poolPhase.c_str(), 12);
  char buf[640];
  snprintf(
      buf, sizeof(buf),
      "{\"hashrate_hs\":%.0f,\"hashrate_khs\":%.3f,\"shares\":%llu,\"hashes\":%llu,"
      "\"mining\":%s,\"accepted\":%u,\"rejected\":%u,\"pool\":\"%s\",\"connected\":%s,"
      "\"link\":\"cmp\",\"difficulty\":0,\"uptime_secs\":%u,\"cpu_mhz\":%u,"
      "\"hash_focus\":true,\"net_ticker\":\"\",\"job\":\"%s\",\"sha_mode\":\"%s\","
      "\"full_v\":true,\"bench_hs\":%.0f,\"nonce\":\"%s\",\"mac\":\"%s\","
      "\"mesh_root\":%s,\"mesh_bridging\":%s,\"mesh_peers\":%u,"
      "\"mine_indep\":%s,\"pool_ep\":\"%s\",\"pool_phase\":\"%s\"}",
      (double)snap.hashrateHs, (double)(snap.hashrateHs / 1000.0f),
      (unsigned long long)snap.shares, (unsigned long long)snap.totalHashes,
      snap.mining ? "true" : "false", (unsigned)snap.accepted, (unsigned)snap.rejected, pool,
      snap.connected ? "true" : "false", (unsigned)(millis() / 1000),
      (unsigned)(snap.cpuMhz ? snap.cpuMhz : cfg.cpuMhz), job, sha, (double)snap.benchHs, nonceHex,
      mac, snap.meshRoot ? "true" : "false", snap.meshBridging ? "true" : "false",
      (unsigned)snap.meshPeers, snap.mineIndep ? "true" : "false", pep, pph);
  out_->print("CMPSTATUS ");
  out_->println(buf);
}

void CompanionLink::replyConfig(const AppConfig& cfg, const MinerSnapshot& snap) {
  char mac[20];
  copyJsonSafe(mac, sizeof(mac), snap.mac.length() ? snap.mac.c_str() : "", 17);
  char ssid[36];
  copyJsonSafe(ssid, sizeof(ssid), cfg.wifiSsid.c_str(), 32);
  char wmode[12];
  copyJsonSafe(wmode, sizeof(wmode), snap.wifiMode.c_str(), 10);
  char wip[20];
  copyJsonSafe(wip, sizeof(wip), snap.wifiIp.c_str(), 18);
  char wap[36];
  copyJsonSafe(wap, sizeof(wap), snap.wifiAp.c_str(), 32);
  char poolUrl[96], poolWorker[64];
  copyJsonSafe(poolUrl, sizeof(poolUrl), cfg.poolUrl.c_str(), 90);
  copyJsonSafe(poolWorker, sizeof(poolWorker), cfg.poolWorker.c_str(), 60);
  char buf[520];
  snprintf(buf, sizeof(buf),
           "{\"cpu_mhz\":%u,\"hash_focus\":true,\"fw\":\"%s\",\"mode\":\"usb-wifi-sha256\","
           "\"configured\":true,\"mac\":\"%s\",\"wifi_en\":%s,\"wifi_ssid\":\"%s\","
           "\"wifi_mode\":\"%s\",\"wifi_ip\":\"%s\",\"wifi_ap\":\"%s\",\"wifi_tcp\":%u,"
           "\"mine_indep\":%s,\"pool_url\":\"%s\",\"pool_worker\":\"%s\"}",
           (unsigned)cfg.cpuMhz, kFwTag, mac, cfg.wifiEnabled ? "true" : "false", ssid, wmode, wip,
           wap, 19284u, cfg.mineIndep ? "true" : "false", poolUrl, poolWorker);
  out_->print("CMPCONFIG ");
  out_->println(buf);
}

String CompanionLink::urlDecode(const String& in) {
  String out;
  out.reserve(in.length());
  for (unsigned i = 0; i < in.length(); i++) {
    char c = in[i];
    if (c == '+') {
      out += ' ';
    } else if (c == '%' && i + 2 < in.length()) {
      char h[3] = {in[i + 1], in[i + 2], 0};
      out += (char)strtol(h, nullptr, 16);
      i += 2;
    } else {
      out += c;
    }
  }
  return out;
}


void CompanionLink::parseWifiBody(const String& body, AppConfig& cfg) {
  int start = 0;
  while (start < (int)body.length()) {
    int amp = body.indexOf('&', start);
    String pair = (amp < 0) ? body.substring(start) : body.substring(start, amp);
    int eq = pair.indexOf('=');
    String key = (eq < 0) ? pair : pair.substring(0, eq);
    String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
    key.toLowerCase();
    if (key == "ssid") {
      if (val.length() > 32) val = val.substring(0, 32);
      val.trim();
      cfg.wifiSsid = val;
    } else if (key == "pass" || key == "password" || key == "psk") {
      if (val.length() > 63) val = val.substring(0, 63);
      cfg.wifiPass = val;
    } else if (key == "en" || key == "enable" || key == "wifi") {
      cfg.wifiEnabled = !(val == "0" || val.equalsIgnoreCase("false") || val.equalsIgnoreCase("off"));
    }
    if (amp < 0) break;
    start = amp + 1;
  }
}

void CompanionLink::parseBody(const String& body, AppConfig& cfg, bool& reboot) {
  int start = 0;
  while (start < (int)body.length()) {
    int amp = body.indexOf('&', start);
    String pair = (amp < 0) ? body.substring(start) : body.substring(start, amp);
    int eq = pair.indexOf('=');
    String key = (eq < 0) ? pair : pair.substring(0, eq);
    String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
    key.toLowerCase();
    if (key == "cpu_mhz" || key == "clock") {
      cfg.cpuMhz = cfg.normalizeCpu((uint8_t)val.toInt());
      reboot = true;
    } else if (key == "hash_focus" || key == "perf") {
      cfg.hashFocus = (val == "1" || val.equalsIgnoreCase("true"));
    } else if (key == "reboot") {
      reboot = (val == "1" || val.equalsIgnoreCase("true"));
    }
    if (amp < 0) break;
    start = amp + 1;
  }
}

void CompanionLink::parseNetData(const String& body, NetFeed& net) {
  int start = 0;
  while (start < (int)body.length()) {
    int amp = body.indexOf('&', start);
    String pair = (amp < 0) ? body.substring(start) : body.substring(start, amp);
    int eq = pair.indexOf('=');
    String key = (eq < 0) ? pair : pair.substring(0, eq);
    String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
    key.toLowerCase();
    if (key == "text" || key == "ticker" || key == "line") {
      if (val.length() > 96) val = val.substring(0, 96);
      net.ticker = val;
    } else if (key == "source" || key == "src") {
      if (val.length() > 24) val = val.substring(0, 24);
      net.source = val;
    }
    if (amp < 0) break;
    start = amp + 1;
  }
}

bool CompanionLink::hexDecodeFixed(const String& hex, uint8_t* out, size_t n) {
  if (hex.length() != n * 2) return false;
  auto nib = [](char c) -> int {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
  };
  for (size_t i = 0; i < n; i++) {
    int hi = nib(hex[i * 2]), lo = nib(hex[i * 2 + 1]);
    if (hi < 0 || lo < 0) return false;
    out[i] = (uint8_t)((hi << 4) | lo);
  }
  return true;
}

bool CompanionLink::parseJobMeta(const String& body, UsbJob& job) {
  job = UsbJob{};
  String startStr;
  int start = 0;
  while (start < (int)body.length()) {
    int amp = body.indexOf('&', start);
    String pair = (amp < 0) ? body.substring(start) : body.substring(start, amp);
    int eq = pair.indexOf('=');
    String key = (eq < 0) ? pair : pair.substring(0, eq);
    String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
    key.toLowerCase();
    if (key == "job" || key == "job_id" || key == "id") job.jobId = val;
    else if (key == "en2" || key == "extranonce2") job.extranonce2 = val;
    else if (key == "ntime" || key == "time") job.ntime = val;
    else if (key == "start" || key == "nonce") startStr = val;
    if (amp < 0) break;
    start = amp + 1;
  }
  if (startStr.length()) {
    job.startNonce = (uint32_t)strtoul(startStr.c_str(), nullptr, 16);
  } else {
    job.startNonce = esp_random();
  }
  if (job.jobId.length() == 0) job.jobId = "0";
  return true;
}

bool CompanionLink::parseJob(const String& body, UsbJob& job) {
  String headerHex, targetHex, startStr;
  job = UsbJob{};
  int start = 0;
  while (start < (int)body.length()) {
    int amp = body.indexOf('&', start);
    String pair = (amp < 0) ? body.substring(start) : body.substring(start, amp);
    int eq = pair.indexOf('=');
    String key = (eq < 0) ? pair : pair.substring(0, eq);
    String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
    key.toLowerCase();
    if (key == "header" || key == "hdr") headerHex = val;
    else if (key == "target" || key == "tgt") targetHex = val;
    else if (key == "job" || key == "job_id" || key == "id") job.jobId = val;
    else if (key == "en2" || key == "extranonce2") job.extranonce2 = val;
    else if (key == "ntime" || key == "time") job.ntime = val;
    else if (key == "start" || key == "nonce") startStr = val;
    if (amp < 0) break;
    start = amp + 1;
  }
  if (!hexDecodeFixed(headerHex, job.header, 80)) return false;
  if (!hexDecodeFixed(targetHex, job.target, 32)) return false;
  if (startStr.length()) {
    job.startNonce = (uint32_t)strtoul(startStr.c_str(), nullptr, 16);
  } else {
    job.startNonce = esp_random();
  }
  if (job.jobId.length() == 0) job.jobId = "0";
  job.valid = true;
  job.fresh = true;
  return true;
}
