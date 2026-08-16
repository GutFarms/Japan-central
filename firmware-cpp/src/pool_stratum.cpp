#include "pool_stratum.hpp"

#include <ArduinoJson.h>
#include <cstring>
#include <mbedtls/sha256.h>
#include <WiFi.h>

bool PoolStratum::active(const AppConfig& cfg) const {
  return cfg.mineIndep && cfg.poolConfigured() && WiFi.status() == WL_CONNECTED;
}

void PoolStratum::disconnect() {
  client_.stop();
  subscribed_ = false;
  authorized_ = false;
  haveDifficulty_ = false;
  wantReconnect_ = false;
  pendingShareId_ = 0;
  lineBuf_ = "";
  snprintf(phase_, sizeof(phase_), "off");
}

bool PoolStratum::parseEndpoint(const String& raw, String& host, uint16_t& port) const {
  String s = raw;
  s.trim();
  s.toLowerCase();
  const char* prefs[] = {"stratum+ssl://", "stratum+tcp://", "stratum://", "tcp://"};
  for (const char* p : prefs) {
    if (s.startsWith(p)) {
      if (strstr(p, "ssl")) return false;
      s = raw.substring(strlen(p));
      s.trim();
      break;
    }
  }
  // Restore original case for host (DNS is usually case-insensitive but keep raw host).
  String rest = raw;
  rest.trim();
  for (const char* p : prefs) {
    String pl = p;
    if (rest.substring(0, pl.length()).equalsIgnoreCase(pl)) {
      rest = rest.substring(pl.length());
      break;
    }
  }
  int slash = rest.indexOf('/');
  if (slash >= 0) rest = rest.substring(0, slash);
  int colon = rest.lastIndexOf(':');
  if (colon > 0) {
    host = rest.substring(0, colon);
    port = (uint16_t)rest.substring(colon + 1).toInt();
    if (port == 0) port = 3333;
  } else {
    host = rest;
    port = 3333;
  }
  host.trim();
  return host.length() > 0;
}

bool PoolStratum::connectPool(const AppConfig& cfg) {
  String host;
  uint16_t port = 3333;
  if (!parseEndpoint(cfg.poolUrl, host, port)) {
    snprintf(phase_, sizeof(phase_), "bad-url");
    return false;
  }
  endpoint_ = host + ":" + String(port);
  worker_ = cfg.poolWorker;
  password_ = cfg.poolPass.length() ? cfg.poolPass : String("x");
  client_.setTimeout(8);
  if (!client_.connect(host.c_str(), port)) {
    snprintf(phase_, sizeof(phase_), "tcp-fail");
    return false;
  }
  client_.setNoDelay(true);
  subscribed_ = false;
  authorized_ = false;
  haveDifficulty_ = false;
  difficulty_ = 0.001f;
  wantReconnect_ = false;
  pendingShareId_ = 0;
  lineBuf_ = "";
  msgId_ = 1;
  en2Counter_ = 1;
  snprintf(phase_, sizeof(phase_), "tcp");
  return sendSubscribe();
}

bool PoolStratum::sendLine(const String& json) {
  if (!client_.connected()) return false;
  String line = json;
  line += '\n';
  size_t n = client_.print(line);
  client_.flush();
  return n == line.length();
}

bool PoolStratum::sendSubscribe() {
  subscribeId_ = msgId_++;
  char buf[128];
  snprintf(buf, sizeof(buf),
           "{\"id\":%u,\"method\":\"mining.subscribe\",\"params\":[\"cgminer/4.12.0\"]}",
           (unsigned)subscribeId_);
  snprintf(phase_, sizeof(phase_), "sub");
  return sendLine(buf);
}

bool PoolStratum::sendAuthorize() {
  authorizeId_ = msgId_++;
  // Manual JSON — worker/pass may contain quotes; escape minimally.
  String w = worker_;
  w.replace("\\", "\\\\");
  w.replace("\"", "\\\"");
  String p = password_;
  p.replace("\\", "\\\\");
  p.replace("\"", "\\\"");
  String msg = "{\"id\":";
  msg += String(authorizeId_);
  msg += ",\"method\":\"mining.authorize\",\"params\":[\"";
  msg += w;
  msg += "\",\"";
  msg += p;
  msg += "\"]}";
  snprintf(phase_, sizeof(phase_), "auth");
  return sendLine(msg);
}

bool PoolStratum::sendSuggestDifficulty() {
  uint32_t id = msgId_++;
  char buf[128];
  snprintf(buf, sizeof(buf),
           "{\"id\":%u,\"method\":\"mining.suggest_difficulty\",\"params\":[0.001]}",
           (unsigned)id);
  lastSuggestMs_ = millis();
  return sendLine(buf);
}

void PoolStratum::onAuthorized() {
  authorized_ = true;
  snprintf(phase_, sizeof(phase_), "ok");
  accepted_ = 0;
  rejected_ = 0;
  if (onStats_) onStats_(accepted_, rejected_);
  if (!tunedThisSession_ && onTune_) {
    tunedThisSession_ = true;
    onTune_();
  }
  (void)sendSuggestDifficulty();
  if (!jobId_.isEmpty() && haveDifficulty_) emitJob();
}

void PoolStratum::poll(const AppConfig& cfg) {
  if (!active(cfg)) {
    if (client_.connected()) {
      disconnect();
      if (onStop_) onStop_();
    }
    return;
  }

  if (wantReconnect_ || !client_.connected()) {
    if (client_.connected()) client_.stop();
    subscribed_ = false;
    authorized_ = false;
    uint32_t now = millis();
    if (now - lastConnectAttemptMs_ < reconnectBackoffMs_) return;
    lastConnectAttemptMs_ = now;
    if (!connectPool(cfg)) {
      reconnectBackoffMs_ = min(reconnectBackoffMs_ * 2, 60000u);
      return;
    }
    reconnectBackoffMs_ = 2000;
  }

  // Keepalive suggest every ~45s when authorized.
  if (authorized_ && millis() - lastSuggestMs_ > 45000) {
    (void)sendSuggestDifficulty();
  }

  while (client_.available() > 0) {
    char c = (char)client_.read();
    if (c == '\r') continue;
    if (c == '\n') {
      if (lineBuf_.length()) {
        handleLine(lineBuf_);
        lineBuf_ = "";
      }
      continue;
    }
    if (lineBuf_.length() < 2048) lineBuf_ += c;
    else lineBuf_ = "";  // overrun — drop
  }
}

bool PoolStratum::submitShare(const PendingShare& share) {
  if (!authorized_ || !client_.connected()) return false;
  if (share.jobId.isEmpty()) return false;
  uint32_t id = msgId_++;
  pendingShareId_ = id;
  char nonceHex[9];
  snprintf(nonceHex, sizeof(nonceHex), "%08x", (unsigned)share.nonce);
  String w = worker_;
  w.replace("\\", "\\\\");
  w.replace("\"", "\\\"");
  String msg = "{\"id\":";
  msg += String(id);
  msg += ",\"method\":\"mining.submit\",\"params\":[\"";
  msg += w;
  msg += "\",\"";
  msg += share.jobId;
  msg += "\",\"";
  msg += share.extranonce2;
  msg += "\",\"";
  msg += share.ntime;
  msg += "\",\"";
  msg += nonceHex;
  msg += "\"]}";
  return sendLine(msg);
}

void PoolStratum::handleLine(const String& line) {
  JsonDocument doc;
  DeserializationError err = deserializeJson(doc, line);
  if (err) return;

  const char* method = doc["method"];
  if (method) {
    if (strcmp(method, "mining.set_difficulty") == 0) {
      float d = 0;
      JsonVariantConst p = doc["params"];
      if (p.is<JsonArrayConst>()) {
        d = p[0].as<float>();
      } else {
        d = p.as<float>();
      }
      if (d > 0) {
        difficulty_ = d;
        haveDifficulty_ = true;
        if (authorized_ && !jobId_.isEmpty()) emitJob();
      }
      return;
    }
    if (strcmp(method, "mining.notify") == 0) {
      JsonArrayConst arr = doc["params"].as<JsonArrayConst>();
      if (arr.size() < 8) return;
      String prev = jobId_;
      jobId_ = arr[0].as<const char*>();
      prevhashHex_ = arr[1].as<const char*>();
      coinb1Hex_ = arr[2].as<const char*>();
      coinb2Hex_ = arr[3].as<const char*>();
      merkleCount_ = 0;
      JsonArrayConst merkle = arr[4].as<JsonArrayConst>();
      for (JsonVariantConst b : merkle) {
        if (merkleCount_ >= 16) break;
        merkleHex_[merkleCount_++] = b.as<const char*>();
      }
      versionHex_ = arr[5].as<const char*>();
      nbitsHex_ = arr[6].as<const char*>();
      ntimeHex_ = arr[7].as<const char*>();
      bool clean = arr.size() > 8 ? arr[8].as<bool>() : false;
      (void)prev;
      (void)clean;
      if (!authorized_ || !haveDifficulty_) return;
      emitJob();
      return;
    }
    if (strcmp(method, "mining.set_extranonce") == 0) {
      JsonArrayConst arr = doc["params"].as<JsonArrayConst>();
      if (arr.size() < 1) return;
      String en1 = arr[0].as<const char*>();
      size_t n = en1.length() / 2;
      if (n == 0 || n > sizeof(en1_)) return;
      if (!hexDecode(en1, en1_, n)) return;
      en1Len_ = n;
      if (arr.size() > 1) {
        size_t sz = arr[1].as<unsigned>();
        if (sz < 1) sz = 1;
        if (sz > 16) sz = 16;
        en2Size_ = sz;
      }
      if (authorized_ && !jobId_.isEmpty() && haveDifficulty_) emitJob();
      return;
    }
    if (strcmp(method, "client.reconnect") == 0) {
      wantReconnect_ = true;
      return;
    }
    return;
  }

  uint32_t id = doc["id"] | 0u;
  bool hasError = !doc["error"].isNull();

  if (id == subscribeId_ && !subscribed_) {
    if (hasError) {
      snprintf(phase_, sizeof(phase_), "sub-fail");
      wantReconnect_ = true;
      return;
    }
    JsonArrayConst res = doc["result"].as<JsonArrayConst>();
    if (res.size() >= 3) {
      String en1 = res[1].as<const char*>();
      size_t n = en1.length() / 2;
      if (n == 0 || n > sizeof(en1_)) {
        snprintf(phase_, sizeof(phase_), "sub-fail");
        return;
      }
      if (!hexDecode(en1, en1_, n)) {
        snprintf(phase_, sizeof(phase_), "sub-fail");
        return;
      }
      en1Len_ = n;
      en2Size_ = res[2].as<unsigned>();
      if (en2Size_ < 1) en2Size_ = 4;
      if (en2Size_ > 16) en2Size_ = 16;
      subscribed_ = true;
      (void)sendAuthorize();
    }
    return;
  }

  if (id == authorizeId_) {
    bool ok = false;
    if (!hasError) {
      if (doc["result"].is<bool>()) ok = doc["result"].as<bool>();
      else if (doc["result"].isNull()) ok = true;
      else if (doc["result"].is<int>()) ok = doc["result"].as<int>() != 0;
    }
    if (ok) onAuthorized();
    else {
      snprintf(phase_, sizeof(phase_), "auth-fail");
      authorized_ = false;
    }
    return;
  }

  if (id == pendingShareId_ && pendingShareId_ != 0) {
    bool ok = false;
    if (!hasError) {
      if (doc["result"].is<bool>()) ok = doc["result"].as<bool>();
      else if (doc["result"].isNull()) ok = true;
      else if (doc["result"].is<int>()) ok = doc["result"].as<int>() != 0;
    }
    if (ok) accepted_++;
    else rejected_++;
    pendingShareId_ = 0;
    if (onStats_) onStats_(accepted_, rejected_);
  }
}

void PoolStratum::emitJob() {
  uint8_t header[80];
  uint8_t target[32];
  if (!buildHeader(header, target)) return;
  UsbJob job;
  memcpy(job.header, header, 80);
  memcpy(job.target, target, 32);
  job.jobId = jobId_;
  job.extranonce2 = activeEn2Hex_;
  job.ntime = activeNtimeHex_;
  job.startNonce = esp_random();
  job.valid = true;
  job.fresh = true;
  if (onJob_) onJob_(job);
}

bool PoolStratum::buildHeader(uint8_t outHeader[80], uint8_t outTarget[32]) {
  // Decode coinbase parts
  size_t c1n = coinb1Hex_.length() / 2;
  size_t c2n = coinb2Hex_.length() / 2;
  if (c1n > 512 || c2n > 512 || en1Len_ == 0 || en2Size_ == 0) return false;
  uint8_t coinb1[512], coinb2[512];
  if (!hexDecode(coinb1Hex_, coinb1, c1n)) return false;
  if (!hexDecode(coinb2Hex_, coinb2, c2n)) return false;

  uint8_t en2[16]{};
  uint64_t ctr = en2Counter_++;
  size_t n = en2Size_ < 8 ? en2Size_ : 8;
  for (size_t i = 0; i < n; i++) en2[i] = (uint8_t)((ctr >> (8 * i)) & 0xff);
  activeEn2Hex_ = bytesToHex(en2, en2Size_);
  activeNtimeHex_ = ntimeHex_;

  // coinbase = c1 || en1 || en2 || c2
  size_t total = c1n + en1Len_ + en2Size_ + c2n;
  if (total > 1024) return false;
  uint8_t coinbase[1024];
  size_t off = 0;
  memcpy(coinbase + off, coinb1, c1n);
  off += c1n;
  memcpy(coinbase + off, en1_, en1Len_);
  off += en1Len_;
  memcpy(coinbase + off, en2, en2Size_);
  off += en2Size_;
  memcpy(coinbase + off, coinb2, c2n);
  off += c2n;

  uint8_t merkle[32];
  dsha256(coinbase, off, merkle);
  for (size_t i = 0; i < merkleCount_; i++) {
    uint8_t branch[32];
    if (!hexDecode(merkleHex_[i], branch, 32)) return false;
    uint8_t cat[64];
    memcpy(cat, merkle, 32);
    memcpy(cat + 32, branch, 32);
    dsha256(cat, 64, merkle);
  }

  uint8_t version[4], prev[32], nbits[4], ntime[4];
  if (!hexDecode(versionHex_, version, 4)) return false;
  if (!hexDecode(prevhashHex_, prev, 32)) return false;
  if (!hexDecode(nbitsHex_, nbits, 4)) return false;
  if (!hexDecode(ntimeHex_, ntime, 4)) return false;
  swab32(version);
  swab256(prev);
  swab32(ntime);
  swab32(nbits);

  memcpy(outHeader + 0, version, 4);
  memcpy(outHeader + 4, prev, 32);
  memcpy(outHeader + 36, merkle, 32);
  memcpy(outHeader + 68, ntime, 4);
  memcpy(outHeader + 72, nbits, 4);
  memset(outHeader + 76, 0, 4);
  targetFromDifficulty(difficulty_, outTarget);
  return true;
}

void PoolStratum::targetFromDifficulty(float diff, uint8_t out[32]) const {
  double d = diff;
  if (d <= 0.0) d = 1e-12;
  int k = 6;
  while (k > 0 && d > 1.0) {
    d /= 4294967296.0;
    k -= 1;
  }
  uint64_t m = (uint64_t)(4294901760.0 / d);
  memset(out, 0, 32);
  size_t idx = (size_t)k * 4;
  if (idx + 4 <= 32) {
    uint32_t lo = (uint32_t)m;
    memcpy(out + idx, &lo, 4);
  }
  if (idx + 8 <= 32) {
    uint32_t hi = (uint32_t)(m >> 32);
    memcpy(out + idx + 4, &hi, 4);
  }
  bool allZero = true;
  for (int i = 0; i < 32; i++)
    if (out[i]) allZero = false;
  if (allZero) out[0] = 1;
}

bool PoolStratum::hexDecode(const String& hex, uint8_t* out, size_t need) {
  if (need == 0 || hex.length() < need * 2) return false;
  for (size_t i = 0; i < need; i++) {
    char h[3] = {hex.charAt(i * 2), hex.charAt(i * 2 + 1), 0};
    if (!isxdigit((unsigned char)h[0]) || !isxdigit((unsigned char)h[1])) return false;
    out[i] = (uint8_t)strtoul(h, nullptr, 16);
  }
  return true;
}

void PoolStratum::swab32(uint8_t* b) {
  uint8_t t = b[0];
  b[0] = b[3];
  b[3] = t;
  t = b[1];
  b[1] = b[2];
  b[2] = t;
}

void PoolStratum::swab256(uint8_t* b) {
  for (int i = 0; i < 8; i++) swab32(b + i * 4);
}

void PoolStratum::dsha256(const uint8_t* data, size_t len, uint8_t out[32]) {
  uint8_t first[32];
  mbedtls_sha256(data, len, first, 0);
  mbedtls_sha256(first, 32, out, 0);
}

String PoolStratum::bytesToHex(const uint8_t* data, size_t len) {
  static const char* hexd = "0123456789abcdef";
  String s;
  s.reserve(len * 2);
  for (size_t i = 0; i < len; i++) {
    s += hexd[data[i] >> 4];
    s += hexd[data[i] & 0xf];
  }
  return s;
}
