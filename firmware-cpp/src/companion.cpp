#include "companion.hpp"
#include <ArduinoJson.h>
#include <cstring>
#include <esp_system.h>

extern "C" float cyd_run_bench(uint32_t n);

void CompanionLink::begin(uint32_t baud) {
  Serial.setRxBufferSize(8192);
  Serial.setTxBufferSize(2048);
  Serial.begin(baud);
  Serial.setTimeout(0);
  lineLen_ = 0;
  haveHeader_ = false;
  haveTarget_ = false;
}

bool CompanionLink::poll(AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply, NetFeed* net,
                         JobFn onJob, StopFn onStop, StatsFn onStats) {
  bool applied = false;
  // Drain aggressively — long job traffic must not wait on mining.
  int budget = 4096;
  while (budget-- > 0 && Serial.available() > 0) {
    char c = (char)Serial.read();
    if (c == '\n' || c == '\r') {
      if (lineLen_ > 0) {
        lineBuf_[lineLen_] = 0;
        String cmd(lineBuf_);
        lineLen_ = 0;
        cmd.trim();
        if (cmd.length() == 0) continue;
        handleLine(cmd, cfg, snap,
                   [&](AppConfig& u, bool& reboot) {
                     bool ok = onApply(u, reboot);
                     if (ok) applied = true;
                     return ok;
                   },
                   net, onJob, onStop, onStats);
      }
    } else if (c >= 32 && c < 127) {
      if (lineLen_ + 1 < kLineCap) {
        lineBuf_[lineLen_++] = c;
      } else {
        // Overflow — drop line so a bad frame can't wedge the parser.
        lineLen_ = 0;
      }
    }
  }
  return applied;
}

void CompanionLink::emitShare(const PendingShare& share) {
  if (!share.pending) return;
  char nonceHex[9];
  // Stratum mining.submit nonce matches cgminer: printf("%08x", nonce_uint32)
  // where nonce_uint32 is the LE reading of the wire header bytes (getwork order).
  // Do NOT emit the raw wire byte hex (that rejects on every pool).
  snprintf(nonceHex, sizeof(nonceHex), "%08x", (unsigned)share.nonce);
  Serial.print("CMPSHARE nonce=");
  Serial.print(nonceHex);
  Serial.print("&job=");
  Serial.print(share.jobId);
  Serial.print("&en2=");
  Serial.print(share.extranonce2);
  Serial.print("&ntime=");
  Serial.println(share.ntime);
  Serial.flush();
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

  if (verb == "ping") {
    Serial.println("CMP ok usb");
    Serial.flush();
    return;
  }
  if (verb == "status") {
    replyStatus(cfg, snap);
    Serial.flush();
    return;
  }
  if (verb == "config") {
    replyConfig(cfg);
    Serial.flush();
    return;
  }

  // Multi-part job: short lines that survive 115200 USB under load.
  //   cmp jh <160 hex>              — header
  //   cmp jt <64 hex>               — target
  //   cmp ja job=&en2=&ntime=&start= — arm
  if (verb == "jh" || verb == "jobhdr" || verb == "header") {
    String hex = args;
    int eq = hex.indexOf('=');
    if (eq >= 0) hex = hex.substring(eq + 1);
    hex.trim();
    if (!hexDecodeFixed(hex, stagedHeader_, 80)) {
      Serial.println("CMPERR jh need 160 hex");
      Serial.flush();
      return;
    }
    haveHeader_ = true;
    Serial.println("CMPACK jh");
    Serial.flush();
    return;
  }
  if (verb == "jt" || verb == "jobtgt" || verb == "target") {
    String hex = args;
    int eq = hex.indexOf('=');
    if (eq >= 0) hex = hex.substring(eq + 1);
    hex.trim();
    if (!hexDecodeFixed(hex, stagedTarget_, 32)) {
      Serial.println("CMPERR jt need 64 hex");
      Serial.flush();
      return;
    }
    haveTarget_ = true;
    Serial.println("CMPACK jt");
    Serial.flush();
    return;
  }
  if (verb == "ja" || verb == "jobarm" || verb == "arm") {
    if (!haveHeader_ || !haveTarget_) {
      Serial.println("CMPERR ja need jh+jt first");
      Serial.flush();
      return;
    }
    UsbJob job;
    if (!parseJobMeta(args, job)) {
      Serial.println("CMPERR ja bad meta");
      Serial.flush();
      return;
    }
    memcpy(job.header, stagedHeader_, 80);
    memcpy(job.target, stagedTarget_, 32);
    job.valid = true;
    job.fresh = true;
    Serial.println("CMPACK ja");
    Serial.flush();
    if (onJob) onJob(job);
    return;
  }

  // Legacy one-shot job (kept for older companions).
  if (verb == "job") {
    UsbJob job;
    if (!parseJob(args, job)) {
      Serial.println("CMPERR job header+target required");
      Serial.flush();
      return;
    }
    Serial.println("CMPACK job");
    Serial.flush();
    if (onJob) onJob(job);
    return;
  }
  if (verb == "stop") {
    if (onStop) onStop();
    Serial.println("CMPACK stop");
    Serial.flush();
    return;
  }
  if (verb == "bench") {
    uint32_t n = 8;
    if (args.length()) {
      int eq = args.indexOf('=');
      String val = (eq < 0) ? args : args.substring(eq + 1);
      int v = val.toInt();
      if (v > 0) n = (uint32_t)v;
    }
    float hs = cyd_run_bench(n);
    char line[96];
    snprintf(line, sizeof(line), "CMPBENCH hashes=%u hs=%.4f khs=%.6f", (unsigned)n, hs,
             hs / 1000.0f);
    Serial.println(line);
    Serial.flush();
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
    Serial.println("CMPACK stats");
    Serial.flush();
    return;
  }
  if (verb == "netdata" || verb == "net" || verb == "push") {
    if (!net) {
      Serial.println("CMPERR net feed unavailable");
      Serial.flush();
      return;
    }
    parseNetData(args, *net);
    if (net->ticker.length() == 0) {
      Serial.println("CMPERR netdata text required");
      Serial.flush();
      return;
    }
    net->updatedMs = millis();
    net->fresh = true;
    Serial.println("CMPACK net");
    Serial.flush();
    return;
  }
  if (verb == "set" || verb == "clock" || verb == "reboot") {
    AppConfig updated = cfg;
    bool reboot = (verb == "reboot");
    parseBody(args, updated, reboot);
    if (verb == "clock" && args.indexOf("cpu_mhz") < 0 && args.indexOf("clock") < 0) {
      Serial.println("CMPERR cpu_mhz required");
      Serial.flush();
      return;
    }
    Serial.println("CMPACK queued");
    Serial.flush();
    (void)onApply(updated, reboot);
    return;
  }
  Serial.println("CMPERR unknown (ping|status|config|jh|jt|ja|job|stop|stats|bench|clock|reboot|netdata)");
  Serial.flush();
}

void CompanionLink::replyStatus(const AppConfig& cfg, const MinerSnapshot& snap) {
  JsonDocument doc;
  doc["hashrate_hs"] = snap.hashrateHs;
  doc["hashrate_khs"] = snap.hashrateHs / 1000.0f;
  doc["shares"] = snap.shares;
  doc["hashes"] = snap.totalHashes;
  doc["mining"] = snap.connected;
  doc["accepted"] = snap.accepted;
  doc["rejected"] = snap.rejected;
  doc["pool"] = snap.pool;
  doc["connected"] = snap.connected;
  doc["link"] = "usb";
  doc["difficulty"] = snap.difficulty;
  doc["uptime_secs"] = (uint32_t)(millis() / 1000);
  doc["cpu_mhz"] = snap.cpuMhz ? snap.cpuMhz : cfg.cpuMhz;
  doc["hash_focus"] = snap.hashFocus;
  doc["net_ticker"] = snap.netTicker;
  doc["job"] = snap.jobId;
  doc["full_v"] = snap.fullV;
  doc["bench_hs"] = snap.benchHs;
  char nonceHex[9];
  snprintf(nonceHex, sizeof(nonceHex), "%08x", snap.nonce);
  doc["nonce"] = nonceHex;
  Serial.print("CMPSTATUS ");
  serializeJson(doc, Serial);
  Serial.println();
}

void CompanionLink::replyConfig(const AppConfig& cfg) {
  JsonDocument doc;
  doc["cpu_mhz"] = cfg.cpuMhz;
  doc["hash_focus"] = cfg.hashFocus;
  doc["fw"] = "0.8.1-sha256";
  doc["mode"] = "usb-sha256";
  doc["configured"] = true;
  Serial.print("CMPCONFIG ");
  serializeJson(doc, Serial);
  Serial.println();
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
