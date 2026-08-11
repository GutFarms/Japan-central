#pragma once
#include <Arduino.h>

// Bitcoin double-SHA256 hasher. On classic ESP32 uses the hardware SHA engine;
// otherwise falls back to unrolled IRAM software midstate.
class Sha256Miner {
 public:
  static constexpr size_t HEADER_LEN = 80;
  static constexpr size_t HASH_LEN = 32;

  Sha256Miner() = default;
  bool begin();
  bool ready() const { return ready_; }
  bool hardware() const { return hw_; }

  void setJob(const uint8_t header[HEADER_LEN], const uint8_t target[HASH_LEN], uint32_t startNonce);
  void updateTarget(const uint8_t target[HASH_LEN]);

  // Hash `count` nonces stepping by `stride`. Returns true if a share was found.
  bool mineBatch(size_t count, uint32_t stride = 1);
  void hashNonce(uint32_t nonce, uint8_t out[HASH_LEN]);

  void setNonce(uint32_t n) { nonce_ = n; }
  uint32_t nonce() const { return nonce_; }
  uint64_t hashes() const { return hashes_; }
  uint64_t shares() const { return shares_; }
  uint32_t lastShareNonce() const { return lastShareNonce_; }
  const uint8_t* lastHash() const { return lastHash_; }

  // One miner owns the SHA peripheral (classic ESP32 has a single SHA2-256 engine).
  static bool acquireHardware();
  static void releaseHardware();

 private:
  bool meetsTargetWords(const uint32_t hash_be[8]) const;
  void prepareMidstate();
  void packTarget(const uint8_t target[HASH_LEN]);
  bool mineBatchHw(size_t count, uint32_t stride);
  bool mineBatchSw(size_t count, uint32_t stride);

  uint8_t header_[HEADER_LEN]{};
  uint8_t target_[HASH_LEN]{};
  uint32_t targetLe_[8]{};
  uint32_t midstate_[8]{};
  uint32_t chunk2_[16]{};
  uint32_t hdrBe_[20]{};  // BE message words for HW SHA
  bool midReady_ = false;
  bool ready_ = false;
  bool hw_ = false;
  uint32_t nonce_ = 0;
  uint64_t hashes_ = 0;
  uint64_t shares_ = 0;
  uint32_t lastShareNonce_ = 0;
  uint8_t lastHash_[HASH_LEN]{};
};
