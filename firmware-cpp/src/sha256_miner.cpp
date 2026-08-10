#include "sha256_miner.hpp"
#include <cstring>

bool Sha256Miner::begin() {
  mbedtls_sha256_init(&midCtx_);
  memset(target_, 0xFF, HASH_LEN);
  // Easy default target for warmup.
  target_[31] = 0x00;
  target_[30] = 0xFF;
  ready_ = true;
  return true;
}

void Sha256Miner::prepareMidstate() {
  mbedtls_sha256_free(&midCtx_);
  mbedtls_sha256_init(&midCtx_);
  mbedtls_sha256_starts(&midCtx_, 0);
  mbedtls_sha256_update(&midCtx_, header_, 64);
  midReady_ = true;
}

void Sha256Miner::setJob(const uint8_t header[HEADER_LEN], const uint8_t target[HASH_LEN],
                         uint32_t startNonce) {
  memcpy(header_, header, HEADER_LEN);
  memcpy(target_, target, HASH_LEN);
  nonce_ = startNonce;
  prepareMidstate();
}

void Sha256Miner::updateTarget(const uint8_t target[HASH_LEN]) {
  memcpy(target_, target, HASH_LEN);
}

void Sha256Miner::hashNonce(uint32_t nonce, uint8_t out[HASH_LEN]) {
  header_[76] = (uint8_t)(nonce);
  header_[77] = (uint8_t)(nonce >> 8);
  header_[78] = (uint8_t)(nonce >> 16);
  header_[79] = (uint8_t)(nonce >> 24);

  uint8_t hash1[32];
  if (midReady_) {
    mbedtls_sha256_context ctx;
    mbedtls_sha256_init(&ctx);
    mbedtls_sha256_clone(&ctx, &midCtx_);
    mbedtls_sha256_update(&ctx, header_ + 64, 16);
    mbedtls_sha256_finish(&ctx, hash1);
    mbedtls_sha256_free(&ctx);
  } else {
    mbedtls_sha256(header_, HEADER_LEN, hash1, 0);
  }
  mbedtls_sha256(hash1, 32, out, 0);
}

bool Sha256Miner::meetsTarget(const uint8_t hash[HASH_LEN]) const {
  // LE compare: hash[31] is most significant (Bitcoin / stratum convention).
  for (int i = 31; i >= 0; i--) {
    if (hash[i] < target_[i]) return true;
    if (hash[i] > target_[i]) return false;
  }
  return true;
}

bool Sha256Miner::mineBatch(size_t count, uint32_t stride) {
  if (!ready_ || !midReady_) return false;
  if (stride == 0) stride = 1;
  bool found = false;
  for (size_t i = 0; i < count; i++) {
    hashNonce(nonce_, lastHash_);
    hashes_++;
    if (meetsTarget(lastHash_)) {
      shares_++;
      lastShareNonce_ = nonce_;
      found = true;
    }
    nonce_ += stride;
  }
  return found;
}
