#pragma once
#include <Arduino.h>
#include <vector>

// Litecoin-style scrypt N=1024,r=1,p=1.
// Prefers full 128 KiB V (fast); falls back to TMTO if allocation fails.
class ScryptLite {
 public:
  static constexpr size_t N = 1024;
  static constexpr size_t BLOCK = 128;
  static constexpr size_t HEADER_LEN = 80;
  static constexpr size_t HASH_LEN = 32;

  ScryptLite();
  bool ready() const { return !v_.empty(); }
  bool fullV() const { return fullV_; }
  size_t vBytes() const { return v_.size(); }

  void setJob(const uint8_t header[HEADER_LEN], const uint8_t target[HASH_LEN], uint32_t startNonce);
  void updateTarget(const uint8_t target[HASH_LEN]);
  // Hash `count` nonces starting at nonce_, stepping by `stride` (dual-core).
  bool mineBatch(size_t count, uint32_t stride = 1);
  void setNonce(uint32_t n) { nonce_ = n; }
  uint32_t nonce() const { return nonce_; }
  uint64_t hashes() const { return hashes_; }
  uint64_t shares() const { return shares_; }
  uint32_t lastShareNonce() const { return lastShareNonce_; }
  const uint8_t* lastHash() const { return lastHash_; }
  const uint8_t* header() const { return header_; }
  const uint8_t* target() const { return target_; }

  // One hash — used by bench.
  void hashNonce(uint32_t nonce, uint8_t out[HASH_LEN]);

 private:
  bool meetsTarget(const uint8_t hash[HASH_LEN]) const;

  uint8_t header_[HEADER_LEN]{};
  uint8_t target_[HASH_LEN]{};
  uint32_t nonce_ = 0;
  uint64_t hashes_ = 0;
  uint64_t shares_ = 0;
  uint32_t lastShareNonce_ = 0;
  uint8_t lastHash_[HASH_LEN]{};
  std::vector<uint32_t> v_;   // words
  std::vector<uint32_t> xy_;  // 2 * 32 words
  bool fullV_ = false;
  size_t tmtoSlots_ = 0;
};
