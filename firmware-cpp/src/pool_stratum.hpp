#pragma once
#include "companion.hpp"
#include "config.hpp"
#include <Arduino.h>
#include <WiFiClient.h>
#include <functional>

/// Onboard stratum (cleartext TCP) so each board mines after STA internet —
/// independent of Companion and of other boards. Companion still polls H/s.
class PoolStratum {
 public:
  using JobFn = std::function<void(const UsbJob& job)>;
  using StopFn = std::function<void()>;
  using StatsFn = std::function<void(uint32_t accepted, uint32_t rejected)>;
  using TuneFn = std::function<void()>;

  void setCallbacks(JobFn onJob, StopFn onStop, StatsFn onStats, TuneFn onTune) {
    onJob_ = std::move(onJob);
    onStop_ = std::move(onStop);
    onStats_ = std::move(onStats);
    onTune_ = std::move(onTune);
  }

  /// Call from USB/loop task — non-blocking poll of pool socket + reconnect.
  void poll(const AppConfig& cfg);

  /// True when this board owns mining (authorized or connecting with pool cfg).
  bool active(const AppConfig& cfg) const;
  bool authorized() const { return authorized_; }
  bool connected() { return client_.connected(); }
  const char* phase() const { return phase_; }
  uint32_t accepted() const { return accepted_; }
  uint32_t rejected() const { return rejected_; }
  float difficulty() const { return difficulty_; }
  const String& endpoint() const { return endpoint_; }

  /// Submit a share found by the dual-lane miners.
  bool submitShare(const PendingShare& share);

  void disconnect();

 private:
  WiFiClient client_;
  JobFn onJob_;
  StopFn onStop_;
  StatsFn onStats_;
  TuneFn onTune_;

  String endpoint_;
  String worker_;
  String password_;
  String lineBuf_;
  uint32_t msgId_ = 1;
  uint32_t subscribeId_ = 0;
  uint32_t authorizeId_ = 0;
  bool subscribed_ = false;
  bool authorized_ = false;
  bool haveDifficulty_ = false;
  bool wantReconnect_ = false;
  bool tunedThisSession_ = false;
  uint8_t en1_[32]{};
  size_t en1Len_ = 0;
  size_t en2Size_ = 4;
  uint64_t en2Counter_ = 1;
  float difficulty_ = 0.001f;
  char phase_[16] = "off";

  String jobId_;
  String prevhashHex_;
  String coinb1Hex_;
  String coinb2Hex_;
  String merkleHex_[16];
  size_t merkleCount_ = 0;
  String versionHex_;
  String nbitsHex_;
  String ntimeHex_;
  String activeEn2Hex_;
  String activeNtimeHex_;

  uint32_t accepted_ = 0;
  uint32_t rejected_ = 0;
  uint32_t pendingShareId_ = 0;
  uint32_t lastConnectAttemptMs_ = 0;
  uint32_t reconnectBackoffMs_ = 2000;
  uint32_t lastSuggestMs_ = 0;

  bool parseEndpoint(const String& raw, String& host, uint16_t& port) const;
  bool connectPool(const AppConfig& cfg);
  bool sendLine(const String& json);
  bool sendSubscribe();
  bool sendAuthorize();
  bool sendSuggestDifficulty();
  void handleLine(const String& line);
  void onAuthorized();
  void emitJob();
  bool buildHeader(uint8_t outHeader[80], uint8_t outTarget[32]);
  void targetFromDifficulty(float diff, uint8_t out[32]) const;
  static bool hexDecode(const String& hex, uint8_t* out, size_t need);
  static void swab32(uint8_t* b);
  static void swab256(uint8_t* b);
  static void dsha256(const uint8_t* data, size_t len, uint8_t out[32]);
  static String bytesToHex(const uint8_t* data, size_t len);
};
