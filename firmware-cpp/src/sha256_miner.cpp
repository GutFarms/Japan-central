#include "sha256_miner.hpp"
#include <cstring>

#if defined(ESP_PLATFORM)
#include <esp_attr.h>
#define SHA_HOT IRAM_ATTR
#else
#define SHA_HOT
#endif

namespace {

constexpr uint32_t K[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u,
    0xab1c5ed5u, 0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu,
    0x9bdc06a7u, 0xc19bf174u, 0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu,
    0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau, 0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u,
    0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u, 0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu,
    0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u, 0xa2bfe8a1u, 0xa81a664bu,
    0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u, 0x19a4c116u,
    0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u,
    0xc67178f2u,
};

constexpr uint32_t IV[8] = {
    0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
    0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u,
};

static inline uint32_t rotr(uint32_t x, uint32_t n) {
  return (x >> n) | (x << (32 - n));
}

static inline uint32_t be32(const uint8_t* p) {
  return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | (uint32_t)p[3];
}

static inline void store_be32(uint8_t* p, uint32_t v) {
  p[0] = (uint8_t)(v >> 24);
  p[1] = (uint8_t)(v >> 16);
  p[2] = (uint8_t)(v >> 8);
  p[3] = (uint8_t)v;
}

static inline uint32_t nonce_be(uint32_t nonce_le) {
  // Header stores nonce little-endian; SHA-256 consumes big-endian words.
  return ((nonce_le & 0xffu) << 24) | (((nonce_le >> 8) & 0xffu) << 16) |
         (((nonce_le >> 16) & 0xffu) << 8) | ((nonce_le >> 24) & 0xffu);
}

SHA_HOT static void sha256_transform(uint32_t state[8], const uint32_t w_in[16]) {
  uint32_t w[64];
  for (int i = 0; i < 16; i++) w[i] = w_in[i];
  for (int i = 16; i < 64; i++) {
    const uint32_t s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
    const uint32_t s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
    w[i] = w[i - 16] + s0 + w[i - 7] + s1;
  }

  uint32_t a = state[0], b = state[1], c = state[2], d = state[3];
  uint32_t e = state[4], f = state[5], g = state[6], h = state[7];

  for (int i = 0; i < 64; i++) {
    const uint32_t S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
    const uint32_t ch = (e & f) ^ ((~e) & g);
    const uint32_t t1 = h + S1 + ch + K[i] + w[i];
    const uint32_t S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
    const uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
    const uint32_t t2 = S0 + maj;
    h = g;
    g = f;
    f = e;
    e = d + t1;
    d = c;
    c = b;
    b = a;
    a = t1 + t2;
  }

  state[0] += a;
  state[1] += b;
  state[2] += c;
  state[3] += d;
  state[4] += e;
  state[5] += f;
  state[6] += g;
  state[7] += h;
}

SHA_HOT static void sha256_mid_finish(const uint32_t mid[8], const uint32_t chunk2[16],
                                      uint8_t out[32]) {
  uint32_t st[8];
  memcpy(st, mid, sizeof(st));
  sha256_transform(st, chunk2);
  for (int i = 0; i < 8; i++) store_be32(out + i * 4, st[i]);
}

SHA_HOT static void sha256_32(const uint8_t in[32], uint8_t out[32]) {
  uint32_t w[16];
  for (int i = 0; i < 8; i++) w[i] = be32(in + i * 4);
  w[8] = 0x80000000u;
  for (int i = 9; i < 15; i++) w[i] = 0;
  w[15] = 256u;
  uint32_t st[8];
  memcpy(st, IV, sizeof(st));
  sha256_transform(st, w);
  for (int i = 0; i < 8; i++) store_be32(out + i * 4, st[i]);
}

}  // namespace

bool Sha256Miner::begin() {
  memset(target_, 0xFF, HASH_LEN);
  target_[31] = 0x00;
  target_[30] = 0xFF;
  ready_ = true;
  return true;
}

void Sha256Miner::prepareMidstate() {
  // Block 1: header[0..63] → midstate.
  uint32_t w[16];
  for (int i = 0; i < 16; i++) w[i] = be32(header_ + i * 4);
  memcpy(midstate_, IV, sizeof(midstate_));
  sha256_transform(midstate_, w);

  // Block 2: header[64..79] + SHA-256 padding for an 80-byte message.
  // [64..67]=merkle tail, [68..71]=ntime, [72..75]=nbits, [76..79]=nonce.
  memset(chunk2_, 0, sizeof(chunk2_));
  chunk2_[0] = be32(header_ + 64);
  chunk2_[1] = be32(header_ + 68);
  chunk2_[2] = be32(header_ + 72);
  chunk2_[3] = 0;  // nonce, filled per hash
  chunk2_[4] = 0x80000000u;
  chunk2_[15] = 640u;  // 80 * 8 bits
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

  uint32_t w2[16];
  memcpy(w2, chunk2_, sizeof(w2));
  w2[3] = nonce_be(nonce);

  uint8_t hash1[32];
  sha256_mid_finish(midstate_, w2, hash1);
  sha256_32(hash1, out);
}

bool Sha256Miner::meetsTarget(const uint8_t hash[HASH_LEN]) const {
  for (int i = 31; i >= 0; i--) {
    if (hash[i] < target_[i]) return true;
    if (hash[i] > target_[i]) return false;
  }
  return true;
}

bool Sha256Miner::mineBatch(size_t count, uint32_t stride) {
  if (!ready_ || !midReady_) return false;
  if (stride == 0) stride = 1;

  uint32_t w2[16];
  memcpy(w2, chunk2_, sizeof(w2));

  bool found = false;
  uint32_t n = nonce_;
  for (size_t i = 0; i < count; i++) {
    w2[3] = nonce_be(n);

    uint8_t hash1[32];
    sha256_mid_finish(midstate_, w2, hash1);
    sha256_32(hash1, lastHash_);
    hashes_++;

    // Fast reject on most-significant byte (Bitcoin LE hash).
    if (lastHash_[31] <= target_[31] && meetsTarget(lastHash_)) {
      shares_++;
      lastShareNonce_ = n;
      found = true;
    }
    n += stride;
  }
  nonce_ = n;
  return found;
}
