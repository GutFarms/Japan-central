#include "scrypt_lite.hpp"
#include <mbedtls/md.h>
#include <mbedtls/pkcs5.h>
#include <cstring>

namespace {

constexpr size_t BLOCK = 128;  // r=1

void pbkdf2_sha256(const uint8_t* pass, size_t passLen, const uint8_t* salt, size_t saltLen,
                   uint8_t* out, size_t outLen) {
  mbedtls_md_context_t ctx;
  mbedtls_md_init(&ctx);
  const mbedtls_md_info_t* info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA256);
  mbedtls_md_setup(&ctx, info, 1);
  mbedtls_pkcs5_pbkdf2_hmac(&ctx, pass, passLen, salt, saltLen, 1, outLen, out);
  mbedtls_md_free(&ctx);
}

inline uint32_t rotl(uint32_t x, int n) { return (x << n) | (x >> (32 - n)); }

inline void qr(uint32_t& a, uint32_t& b, uint32_t& c, uint32_t& d) {
  b ^= rotl(a + d, 7);
  c ^= rotl(b + a, 9);
  d ^= rotl(c + b, 13);
  a ^= rotl(d + c, 18);
}

// RFC 7914 Salsa20/8 on a 64-byte block (LE words).
void salsa20_8(uint8_t block[64]) {
  uint32_t x[16];
  for (int i = 0; i < 16; i++) {
    memcpy(&x[i], block + i * 4, 4);
  }
  uint32_t orig[16];
  memcpy(orig, x, sizeof(x));

  for (int i = 0; i < 4; i++) {
    qr(x[0], x[4], x[8], x[12]);
    qr(x[5], x[9], x[13], x[1]);
    qr(x[10], x[14], x[2], x[6]);
    qr(x[15], x[3], x[7], x[11]);
    qr(x[0], x[1], x[2], x[3]);
    qr(x[5], x[6], x[7], x[4]);
    qr(x[10], x[11], x[8], x[9]);
    qr(x[15], x[12], x[13], x[14]);
  }
  for (int i = 0; i < 16; i++) {
    uint32_t v = x[i] + orig[i];
    memcpy(block + i * 4, &v, 4);
  }
}

void blockmix(const uint8_t* b, uint8_t* y) {
  // RFC 7914 BlockMix, r=1
  uint8_t x[64];
  memcpy(x, b + 64, 64);
  for (int i = 0; i < 2; i++) {
    for (int j = 0; j < 64; j++) x[j] ^= b[i * 64 + j];
    salsa20_8(x);
    int dest = (i % 2 == 0) ? (i / 2) * 64 : (1 + (i - 1) / 2) * 64;
    memcpy(y + dest, x, 64);
  }
}

void xor_block(uint8_t* a, const uint8_t* b) {
  for (size_t i = 0; i < BLOCK; i++) a[i] ^= b[i];
}

size_t integerify(const uint8_t* b) {
  uint64_t v;
  memcpy(&v, b + 64, 8);
  return (size_t)v;
}

void recover_v(uint8_t* out, size_t j, size_t stride, const uint8_t* v, uint8_t* scratch) {
  size_t base = (j / stride) * stride;
  size_t slot = base / stride;
  memcpy(out, v + slot * BLOCK, BLOCK);
  for (size_t i = base; i < j; i++) {
    blockmix(out, scratch);
    memcpy(out, scratch, BLOCK);
  }
}

void romix_tmto(uint8_t* b, uint8_t* v, uint8_t* xy) {
  constexpr size_t n = ScryptLite::N;
  constexpr size_t slots = ScryptLite::V_SLOTS;
  constexpr size_t stride = n / slots;
  uint8_t* x = xy;
  uint8_t* y = xy + BLOCK;
  memcpy(x, b, BLOCK);
  for (size_t i = 0; i < n; i++) {
    if (i % stride == 0) {
      memcpy(v + (i / stride) * BLOCK, x, BLOCK);
    }
    blockmix(x, y);
    memcpy(x, y, BLOCK);
  }
  uint8_t t[BLOCK];
  for (size_t i = 0; i < n; i++) {
    size_t j = integerify(x) % n;
    recover_v(t, j, stride, v, y);
    xor_block(x, t);
    blockmix(x, y);
    memcpy(x, y, BLOCK);
  }
  memcpy(b, x, BLOCK);
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
  v_.assign(V_SLOTS * BLOCK, 0);
  xy_.assign(2 * BLOCK, 0);
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

  uint8_t b[BLOCK];
  pbkdf2_sha256(hdr, HEADER_LEN, hdr, HEADER_LEN, b, BLOCK);
  romix_tmto(b, v_.data(), xy_.data());
  pbkdf2_sha256(hdr, HEADER_LEN, b, BLOCK, out, HASH_LEN);
}

bool ScryptLite::meetsTarget(const uint8_t hash[HASH_LEN]) const {
  return hash_lt_target(hash, target_);
}

bool ScryptLite::mineBatch(size_t count) {
  bool found = false;
  for (size_t i = 0; i < count; i++) {
    hashNonce(nonce_, lastHash_);
    hashes_++;
    if (meetsTarget(lastHash_)) {
      shares_++;
      lastShareNonce_ = nonce_;
      found = true;
    }
    nonce_++;
  }
  return found;
}
