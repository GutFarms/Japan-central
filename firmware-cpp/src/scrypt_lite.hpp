#pragma once
#include <Arduino.h>
#include <vector>

// Litecoin-style scrypt N=1024,r=1,p=1 with TMTO checkpoints (V_SLOTS=64).
class ScryptLite {
 public:
  static constexpr size_t N = 1024;
  static constexpr size_t V_SLOTS = 64;
  static constexpr size_t HEADER_LEN = 80;
  static constexpr size_t HASH_LEN = 32;

  ScryptLite();
  void setJob(const uint8_t header[HEADER_LEN], const uint8_t target[HASH_LEN], uint32_t startNonce);
  void updateTarget(const uint8_t target[HASH_LEN]);
  // Hash `count` nonces. Returns true if a share was found (lastShareNonce set).
  bool mineBatch(size_t count);
  uint32_t nonce() const { return nonce_; }
  uint64_t hashes() const { return hashes_; }
  uint64_t shares() const { return shares_; }
  uint32_t lastShareNonce() const { return lastShareNonce_; }
  const uint8_t* lastHash() const { return lastHash_; }

 private:
  void hashNonce(uint32_t nonce, uint8_t out[HASH_LEN]);
  bool meetsTarget(const uint8_t hash[HASH_LEN]) const;

  uint8_t header_[HEADER_LEN]{};
  uint8_t target_[HASH_LEN]{};
  uint32_t nonce_ = 0;
  uint64_t hashes_ = 0;
  uint64_t shares_ = 0;
  uint32_t lastShareNonce_ = 0;
  uint8_t lastHash_[HASH_LEN]{};
  std::vector<uint8_t> v_;
  std::vector<uint8_t> xy_;
};
