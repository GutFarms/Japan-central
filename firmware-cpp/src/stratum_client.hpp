#pragma once
#include "config.hpp"
#include <Arduino.h>
#include <WiFiClient.h>
#include <vector>

class StratumClient {
 public:
  void begin(const AppConfig& cfg);
  void updateConfig(const AppConfig& cfg);
  void loop();  // non-blocking poll / reconnect
  bool hasJob() const { return hasJob_; }
  // Copies current header/target; keeps job active for more nonces.
  bool peekJob(uint8_t header[80], uint8_t target[32]) const;
  void submitShare(uint32_t nonce);
  String phase() const { return phase_; }
  bool connected() const {
    return phase_ == "idle" || phase_ == "mine" || phase_ == "CONNECTED";
  }
  uint32_t accepted() const { return accepted_; }
  uint32_t rejected() const { return rejected_; }
  uint32_t dropped() const { return dropped_; }
  uint32_t difficulty() const { return difficulty_; }
  void requestReconnect() { wantReconnect_ = true; }

  static bool parseEndpoint(const String& raw, String& host, uint16_t& port);
  static void targetFromDifficulty(uint32_t difficulty, uint8_t out[32]);

 private:
  AppConfig cfg_;
  WiFiClient client_;
  String phase_ = "off";
  String rxBuf_;
  bool hasJob_ = false;
  uint8_t header_[80]{};
  uint8_t target_[32]{};
  uint32_t difficulty_ = 1;
  uint32_t accepted_ = 0;
  uint32_t rejected_ = 0;
  uint32_t dropped_ = 0;
  uint32_t msgId_ = 1;
  uint32_t subscribeId_ = 0;
  uint32_t authorizeId_ = 0;
  bool subscribed_ = false;
  bool authorized_ = false;
  bool wantReconnect_ = false;
  uint32_t lastConnectAttempt_ = 0;
  uint64_t en2Counter_ = 1;

  String extranonce1Hex_;
  std::vector<uint8_t> extranonce1_;
  size_t extranonce2Size_ = 4;
  String jobId_;
  String prevhashHex_;
  String coinb1Hex_;
  String coinb2Hex_;
  String versionHex_;
  String nbitsHex_;
  String ntimeHex_;
  std::vector<String> merkleHex_;
  String activeEn2Hex_;
  String activeNtimeHex_;

  void sendLine(const String& s);
  void sendSubscribe();
  void sendAuthorize();
  void handleLine(const String& line);
  bool buildJobFromNotify();
  static bool hexDecode(const String& hex, std::vector<uint8_t>& out);
  static bool hexDecodeFixed(const String& hex, uint8_t* out, size_t n);
  static String hexEncode(const uint8_t* data, size_t n);
  static void dsha256(const uint8_t* data, size_t len, uint8_t out[32]);
  static void swab256(uint8_t hash[32]);
};
