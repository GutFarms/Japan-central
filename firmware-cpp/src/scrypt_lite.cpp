#include "scrypt_lite.hpp"

#include <mbedtls/sha256.h>
#include <cstdlib>
#include <cstring>

namespace {

constexpr size_t WBLOCK = 32;  // 128 bytes / 4

inline uint32_t rotl(uint32_t x, int n) { return (x << n) | (x >> (32 - n)); }

#define QR(a, b, c, d)        \
  do {                        \
    b ^= rotl(a + d, 7);      \
    c ^= rotl(b + a, 9);      \
    d ^= rotl(c + b, 13);     \
    a ^= rotl(d + c, 18);     \
  } while (0)

// Salsa20/8 on 16 LE words (in-place).
static inline void salsa20_8_words(uint32_t x[16]) {
  uint32_t z0 = x[0], z1 = x[1], z2 = x[2], z3 = x[3];
  uint32_t z4 = x[4], z5 = x[5], z6 = x[6], z7 = x[7];
  uint32_t z8 = x[8], z9 = x[9], z10 = x[10], z11 = x[11];
  uint32_t z12 = x[12], z13 = x[13], z14 = x[14], z15 = x[15];

  for (int i = 0; i < 4; i++) {
    QR(z0, z4, z8, z12);
    QR(z5, z9, z13, z1);
    QR(z10, z14, z2, z6);
    QR(z15, z3, z7, z11);
    QR(z0, z1, z2, z3);
    QR(z5, z6, z7, z4);
    QR(z10, z11, z8, z9);
    QR(z15, z12, z13, z14);
  }

  x[0] += z0;
  x[1] += z1;
  x[2] += z2;
  x[3] += z3;
  x[4] += z4;
  x[5] += z5;
  x[6] += z6;
  x[7] += z7;
  x[8] += z8;
  x[9] += z9;
  x[10] += z10;
  x[11] += z11;
  x[12] += z12;
  x[13] += z13;
  x[14] += z14;
  x[15] += z15;
}

static inline void blockmix_words(const uint32_t* b, uint32_t* y) {
  uint32_t x[16];
  memcpy(x, b + 16, 64);
  for (int i = 0; i < 2; i++) {
    for (int j = 0; j < 16; j++) x[j] ^= b[i * 16 + j];
    salsa20_8_words(x);
    int dest = (i % 2 == 0) ? (i / 2) * 16 : (1 + (i - 1) / 2) * 16;
    memcpy(y + dest, x, 64);
  }
}

static inline size_t integerify_words(const uint32_t* b) {
  // First 8 bytes of second 64-byte half = words [16], [17] LE.
  uint64_t v = (uint64_t)b[16] | ((uint64_t)b[17] << 32);
  return (size_t)v;
}

static void hmac_sha256(const uint8_t* key, size_t keyLen, const uint8_t* msg, size_t msgLen,
                        uint8_t out[32]) {
  uint8_t k[64];
  memset(k, 0, 64);
  if (keyLen > 64) {
    mbedtls_sha256(key, keyLen, k, 0);
  } else {
    memcpy(k, key, keyLen);
  }
  uint8_t i_pad[64], o_pad[64];
  for (int i = 0; i < 64; i++) {
    i_pad[i] = k[i] ^ 0x36;
    o_pad[i] = k[i] ^ 0x5c;
  }
  uint8_t inner[32];
  mbedtls_sha256_context ctx;
  mbedtls_sha256_init(&ctx);
  mbedtls_sha256_starts(&ctx, 0);
  mbedtls_sha256_update(&ctx, i_pad, 64);
  mbedtls_sha256_update(&ctx, msg, msgLen);
  mbedtls_sha256_finish(&ctx, inner);
  mbedtls_sha256_starts(&ctx, 0);
  mbedtls_sha256_update(&ctx, o_pad, 64);
  mbedtls_sha256_update(&ctx, inner, 32);
  mbedtls_sha256_finish(&ctx, out);
  mbedtls_sha256_free(&ctx);
}

// PBKDF2-HMAC-SHA256 with c=1 (Litecoin scrypt). saltLen ≤ 128.
static void pbkdf2_sha256_c1(const uint8_t* pass, size_t passLen, const uint8_t* salt,
                             size_t saltLen, uint8_t* out, size_t outLen) {
  uint8_t be[4];
  uint8_t tmp[32];
  uint8_t msg[128 + 4];
  size_t produced = 0;
  uint32_t block = 1;
  if (saltLen > 128) saltLen = 128;
  memcpy(msg, salt, saltLen);
  while (produced < outLen) {
    be[0] = (uint8_t)(block >> 24);
    be[1] = (uint8_t)(block >> 16);
    be[2] = (uint8_t)(block >> 8);
    be[3] = (uint8_t)block;
    memcpy(msg + saltLen, be, 4);
    hmac_sha256(pass, passLen, msg, saltLen + 4, tmp);
    size_t n = outLen - produced;
    if (n > 32) n = 32;
    memcpy(out + produced, tmp, n);
    produced += n;
    block++;
  }
}

static void romix_full(uint32_t* x, uint32_t* v, uint32_t* y) {
  constexpr size_t n = ScryptLite::N;
  for (size_t i = 0; i < n; i++) {
    memcpy(v + i * WBLOCK, x, WBLOCK * 4);
    blockmix_words(x, y);
    memcpy(x, y, WBLOCK * 4);
  }
  for (size_t i = 0; i < n; i++) {
    size_t j = integerify_words(x) % n;
    const uint32_t* t = v + j * WBLOCK;
    for (size_t k = 0; k < WBLOCK; k++) x[k] ^= t[k];
    blockmix_words(x, y);
    memcpy(x, y, WBLOCK * 4);
  }
}

static void recover_v(uint32_t* out, size_t j, size_t stride, const uint32_t* v, uint32_t* scratch) {
  size_t base = (j / stride) * stride;
  size_t slot = base / stride;
  memcpy(out, v + slot * WBLOCK, WBLOCK * 4);
  for (size_t i = base; i < j; i++) {
    blockmix_words(out, scratch);
    memcpy(out, scratch, WBLOCK * 4);
  }
}

static void romix_tmto(uint32_t* x, uint32_t* v, uint32_t* y, size_t slots) {
  constexpr size_t n = ScryptLite::N;
  const size_t stride = n / slots;
  for (size_t i = 0; i < n; i++) {
    if (i % stride == 0) {
      memcpy(v + (i / stride) * WBLOCK, x, WBLOCK * 4);
    }
    blockmix_words(x, y);
    memcpy(x, y, WBLOCK * 4);
  }
  uint32_t t[WBLOCK];
  for (size_t i = 0; i < n; i++) {
    size_t j = integerify_words(x) % n;
    recover_v(t, j, stride, v, y);
    for (size_t k = 0; k < WBLOCK; k++) x[k] ^= t[k];
    blockmix_words(x, y);
    memcpy(x, y, WBLOCK * 4);
  }
}

bool hash_lt_target(const uint8_t* hash, const uint8_t* target) {
  for (int i = 31; i >= 0; i--) {
    if (hash[i] < target[i]) return true;
    if (hash[i] > target[i]) return false;
  }
  return false;
}

}  // namespace

ScryptLite::ScryptLite() {
  xy_.assign(2 * WBLOCK, 0);
  // Prefer full V (128 KiB) — ~5× fewer BlockMix vs TMTO-64.
  const size_t fullWords = N * WBLOCK;
  const size_t need = fullWords * sizeof(uint32_t) + 48 * 1024;
  if (ESP.getMaxAllocHeap() >= need) {
    v_.resize(fullWords);
    if (v_.size() == fullWords) {
      fullV_ = true;
      tmtoSlots_ = N;
    }
  }
  if (!fullV_) {
    constexpr size_t slots = 128;  // 16 KiB TMTO
    v_.assign(slots * WBLOCK, 0);
    tmtoSlots_ = slots;
  }
  memset(target_, 0xFF, HASH_LEN);
  target_[31] = 0x00;
  target_[30] = 0x0F;
}

void ScryptLite::setJob(const uint8_t header[HEADER_LEN], const uint8_t target[HASH_LEN],
                        uint32_t startNonce) {
  memcpy(header_, header, HEADER_LEN);
  memcpy(target_, target, HASH_LEN);
  nonce_ = startNonce;
}

void ScryptLite::updateTarget(const uint8_t target[HASH_LEN]) {
  memcpy(target_, target, HASH_LEN);
}

void ScryptLite::hashNonce(uint32_t nonce, uint8_t out[HASH_LEN]) {
  uint8_t hdr[HEADER_LEN];
  memcpy(hdr, header_, HEADER_LEN);
  hdr[76] = (uint8_t)(nonce);
  hdr[77] = (uint8_t)(nonce >> 8);
  hdr[78] = (uint8_t)(nonce >> 16);
  hdr[79] = (uint8_t)(nonce >> 24);

  uint32_t x[WBLOCK];
  pbkdf2_sha256_c1(hdr, HEADER_LEN, hdr, HEADER_LEN, (uint8_t*)x, BLOCK);
  uint32_t* y = xy_.data();
  if (fullV_) {
    romix_full(x, v_.data(), y);
  } else {
    romix_tmto(x, v_.data(), y, tmtoSlots_);
  }
  pbkdf2_sha256_c1(hdr, HEADER_LEN, (uint8_t*)x, BLOCK, out, HASH_LEN);
}

bool ScryptLite::meetsTarget(const uint8_t hash[HASH_LEN]) const {
  return hash_lt_target(hash, target_);
}

bool ScryptLite::mineBatch(size_t count, uint32_t stride) {
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
