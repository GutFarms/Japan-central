#include "stratum_client.hpp"

#include <ArduinoJson.h>
#include <WiFi.h>
#include <mbedtls/sha256.h>
#include <cstdio>
#include <cstring>

void StratumClient::begin(const AppConfig& cfg) {
  cfg_ = cfg;
  phase_ = "wait";
  wantReconnect_ = true;
  lastConnectAttempt_ = 0;
  targetFromDifficulty(1, target_);
}

void StratumClient::updateConfig(const AppConfig& cfg) {
  cfg_ = cfg;
  wantReconnect_ = true;
}

bool StratumClient::parseEndpoint(const String& raw, String& host, uint16_t& port) {
  String s = raw;
  s.trim();
  s.toLowerCase();
  if (s.startsWith("stratum+ssl://")) return false;
  const char* prefixes[] = {"stratum+tcp://", "stratum://", "tcp://"};
  String orig = raw;
  orig.trim();
  for (const char* p : prefixes) {
    if (orig.startsWith(p) || String(orig).startsWith(String(p))) {
      // case-insensitive strip
      break;
    }
  }
  // Strip known prefixes from original (case-insensitive)
  String work = orig;
  auto stripCi = [&](const char* pref) {
    String w = work;
    String p = pref;
    if (w.length() >= p.length()) {
      String head = w.substring(0, p.length());
      head.toLowerCase();
      p.toLowerCase();
      if (head == p) {
        work = w.substring(p.length());
        return true;
      }
    }
    return false;
  };
  if (stripCi("stratum+ssl://")) return false;
  (void)stripCi("stratum+tcp://");
  (void)stripCi("stratum://");
  (void)stripCi("tcp://");
  int slash = work.indexOf('/');
  if (slash >= 0) work = work.substring(0, slash);

  int colon = work.lastIndexOf(':');
  if (colon > 0) {
    host = work.substring(0, colon);
    int p = work.substring(colon + 1).toInt();
    if (p <= 0 || p > 65535) return false;
    port = (uint16_t)p;
  } else {
    host = work;
    port = 3333;
  }
  host.trim();
  return host.length() > 0 && host.length() <= 96;
}

void StratumClient::targetFromDifficulty(uint32_t difficulty, uint8_t out[32]) {
  uint32_t diff = difficulty < 1 ? 1 : difficulty;
  uint8_t num[32]{};
  num[2] = 0xff;
  num[3] = 0xff;
  uint8_t outBe[32]{};
  uint64_t rem = 0;
  for (int i = 0; i < 32; i++) {
    uint64_t cur = (rem << 8) | num[i];
    outBe[i] = (uint8_t)(cur / diff);
    rem = cur % diff;
  }
  for (int i = 0; i < 32; i++) out[i] = outBe[31 - i];
  bool allZero = true;
  for (int i = 0; i < 32; i++) {
    if (out[i]) {
      allZero = false;
      break;
    }
  }
  if (allZero) out[0] = 1;
}

void StratumClient::sendLine(const String& s) {
  if (!client_.connected()) return;
  client_.print(s);
  if (!s.endsWith("\n")) client_.print('\n');
}

void StratumClient::sendSubscribe() {
  subscribeId_ = msgId_++;
  JsonDocument doc;
  doc["id"] = subscribeId_;
  doc["method"] = "mining.subscribe";
  JsonArray params = doc["params"].to<JsonArray>();
  params.add("cyd-cpp/0.2.0");
  String out;
  serializeJson(doc, out);
  sendLine(out);
  phase_ = "sub";
}

void StratumClient::sendAuthorize() {
  authorizeId_ = msgId_++;
  JsonDocument doc;
  doc["id"] = authorizeId_;
  doc["method"] = "mining.authorize";
  JsonArray params = doc["params"].to<JsonArray>();
  params.add(cfg_.worker);
  params.add(cfg_.password);
  String out;
  serializeJson(doc, out);
  sendLine(out);
  phase_ = "auth";
}

bool StratumClient::hexDecode(const String& hex, std::vector<uint8_t>& out) {
  if (hex.length() % 2 != 0) return false;
  out.resize(hex.length() / 2);
  auto nib = [](char c) -> int {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
  };
  for (size_t i = 0; i < out.size(); i++) {
    int hi = nib(hex[i * 2]), lo = nib(hex[i * 2 + 1]);
    if (hi < 0 || lo < 0) return false;
    out[i] = (uint8_t)((hi << 4) | lo);
  }
  return true;
}

bool StratumClient::hexDecodeFixed(const String& hex, uint8_t* out, size_t n) {
  std::vector<uint8_t> tmp;
  if (!hexDecode(hex, tmp) || tmp.size() != n) return false;
  memcpy(out, tmp.data(), n);
  return true;
}

String StratumClient::hexEncode(const uint8_t* data, size_t n) {
  static const char* hexd = "0123456789abcdef";
  String out;
  out.reserve(n * 2);
  for (size_t i = 0; i < n; i++) {
    out += hexd[data[i] >> 4];
    out += hexd[data[i] & 0x0f];
  }
  return out;
}

void StratumClient::dsha256(const uint8_t* data, size_t len, uint8_t out[32]) {
  uint8_t tmp[32];
  mbedtls_sha256(data, len, tmp, 0);
  mbedtls_sha256(tmp, 32, out, 0);
}

void StratumClient::swab256(uint8_t hash[32]) {
  for (int i = 0; i < 8; i++) {
    uint8_t* c = hash + i * 4;
    uint8_t t0 = c[0], t1 = c[1];
    c[0] = c[3];
    c[1] = c[2];
    c[2] = t1;
    c[3] = t0;
  }
}

bool StratumClient::buildJobFromNotify() {
  std::vector<uint8_t> coinb1, coinb2;
  if (!hexDecode(coinb1Hex_, coinb1) || !hexDecode(coinb2Hex_, coinb2)) return false;
  if (extranonce1_.empty() && extranonce1Hex_.length()) {
    if (!hexDecode(extranonce1Hex_, extranonce1_)) return false;
  }
  size_t en2Size = extranonce2Size_;
  if (en2Size < 1) en2Size = 1;
  if (en2Size > 16) en2Size = 16;
  uint8_t en2[16]{};
  for (size_t i = 0; i < en2Size; i++) {
    size_t shift = (en2Size - 1 - i) * 8;
    en2[i] = (uint8_t)((en2Counter_ >> shift) & 0xff);
  }
  en2Counter_++;
  activeEn2Hex_ = hexEncode(en2, en2Size);
  activeNtimeHex_ = ntimeHex_;

  std::vector<uint8_t> coinbase;
  coinbase.reserve(coinb1.size() + extranonce1_.size() + en2Size + coinb2.size());
  coinbase.insert(coinbase.end(), coinb1.begin(), coinb1.end());
  coinbase.insert(coinbase.end(), extranonce1_.begin(), extranonce1_.end());
  coinbase.insert(coinbase.end(), en2, en2 + en2Size);
  coinbase.insert(coinbase.end(), coinb2.begin(), coinb2.end());

  uint8_t merkle[32];
  dsha256(coinbase.data(), coinbase.size(), merkle);
  for (const String& branchHex : merkleHex_) {
    uint8_t branch[32];
    if (!hexDecodeFixed(branchHex, branch, 32)) return false;
    uint8_t cat[64];
    memcpy(cat, merkle, 32);
    memcpy(cat + 32, branch, 32);
    dsha256(cat, 64, merkle);
  }

  uint8_t version[4], prev[32], nbits[4], ntime[4];
  if (!hexDecodeFixed(versionHex_, version, 4)) return false;
  if (!hexDecodeFixed(prevhashHex_, prev, 32)) return false;
  if (!hexDecodeFixed(nbitsHex_, nbits, 4)) return false;
  if (!hexDecodeFixed(ntimeHex_, ntime, 4)) return false;
  swab256(prev);

  memset(header_, 0, 80);
  memcpy(header_ + 0, version, 4);
  memcpy(header_ + 4, prev, 32);
  memcpy(header_ + 36, merkle, 32);
  memcpy(header_ + 68, ntime, 4);
  memcpy(header_ + 72, nbits, 4);

  targetFromDifficulty(difficulty_, target_);
  hasJob_ = true;
  phase_ = "mine";
  return true;
}

void StratumClient::handleLine(const String& line) {
  JsonDocument doc;
  DeserializationError err = deserializeJson(doc, line);
  if (err) return;

  if (doc["method"].is<const char*>()) {
    const char* method = doc["method"];
    JsonArrayConst params = doc["params"].as<JsonArrayConst>();
    if (!strcmp(method, "mining.set_difficulty") && params.size() > 0) {
      double d = params[0].as<double>();
      difficulty_ = d < 1 ? 1 : (uint32_t)d;
      if (hasJob_) targetFromDifficulty(difficulty_, target_);
      return;
    }
    if (!strcmp(method, "mining.notify") && params.size() >= 8) {
      jobId_ = params[0].as<const char*>();
      prevhashHex_ = params[1].as<const char*>();
      coinb1Hex_ = params[2].as<const char*>();
      coinb2Hex_ = params[3].as<const char*>();
      merkleHex_.clear();
      if (params[4].is<JsonArrayConst>()) {
        for (JsonVariantConst v : params[4].as<JsonArrayConst>()) {
          merkleHex_.push_back(String(v.as<const char*>()));
        }
      }
      versionHex_ = params[5].as<const char*>();
      nbitsHex_ = params[6].as<const char*>();
      ntimeHex_ = params[7].as<const char*>();
      buildJobFromNotify();
      return;
    }
    return;
  }

  if (doc["id"].isNull()) return;
  uint32_t id = doc["id"].as<uint32_t>();
  // Missing or JSON null → no error; otherwise reject/fail.
  bool hasError = !doc["error"].isNull();

  if (id == subscribeId_ && !subscribed_) {
    if (hasError) {
      phase_ = "err";
      return;
    }
    // result: [[...], "en1", size]
    JsonArrayConst res = doc["result"].as<JsonArrayConst>();
    if (res.size() >= 3) {
      extranonce1Hex_ = res[1].as<const char*>();
      extranonce2Size_ = res[2].as<size_t>();
      hexDecode(extranonce1Hex_, extranonce1_);
      if (extranonce2Size_ == 0) extranonce2Size_ = 4;
      subscribed_ = true;
      sendAuthorize();
    }
    return;
  }

  if (doc["result"].is<bool>()) {
    bool ok = doc["result"].as<bool>() && !hasError;
    if (id == authorizeId_) {
      authorized_ = ok;
      phase_ = ok ? "idle" : "err";
      return;
    }
    // share submit result
    if (ok) accepted_++;
    else rejected_++;
    return;
  }

  if (hasError) {
    rejected_++;
    return;
  }

  // authorize with result:null
  if (id == authorizeId_ && !authorized_) {
    authorized_ = true;
    phase_ = "idle";
  }
}

void StratumClient::loop() {
  if (wantReconnect_ || (!client_.connected() && phase_ != "off")) {
    uint32_t now = millis();
    if (wantReconnect_ || now - lastConnectAttempt_ > 5000) {
      lastConnectAttempt_ = now;
      wantReconnect_ = false;
      if (client_.connected()) client_.stop();
      hasJob_ = false;
      subscribed_ = false;
      authorized_ = false;
      rxBuf_ = "";

      if (WiFi.status() != WL_CONNECTED) {
        phase_ = "wait";
        return;
      }
      String host;
      uint16_t port = 3333;
      if (!parseEndpoint(cfg_.stratum, host, port)) {
        phase_ = "err";
        return;
      }
      phase_ = "tcp";
      if (!client_.connect(host.c_str(), port, 8000)) {
        phase_ = "err";
        return;
      }
      client_.setNoDelay(true);
      sendSubscribe();
    }
  }

  if (!client_.connected()) return;
  while (client_.available()) {
    char c = (char)client_.read();
    if (c == '\n') {
      if (rxBuf_.length()) {
        handleLine(rxBuf_);
        rxBuf_ = "";
      }
    } else if (c != '\r') {
      if (rxBuf_.length() < 8192) rxBuf_ += c;
      else rxBuf_ = "";
    }
  }
}

bool StratumClient::peekJob(uint8_t header[80], uint8_t target[32]) const {
  if (!hasJob_) return false;
  memcpy(header, header_, 80);
  memcpy(target, target_, 32);
  return true;
}

void StratumClient::submitShare(uint32_t nonce) {
  if (!client_.connected() || !authorized_ || jobId_.isEmpty()) {
    dropped_++;
    return;
  }
  char nonceHex[9];
  snprintf(nonceHex, sizeof(nonceHex), "%08x", nonce);
  JsonDocument doc;
  doc["id"] = msgId_++;
  doc["method"] = "mining.submit";
  JsonArray params = doc["params"].to<JsonArray>();
  params.add(cfg_.worker);
  params.add(jobId_);
  params.add(activeEn2Hex_);
  params.add(activeNtimeHex_);
  params.add(nonceHex);
  String out;
  serializeJson(doc, out);
  sendLine(out);
}
