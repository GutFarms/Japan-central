#include "sha256_hw.hpp"

#if CYD_SHA_HW

#include <Arduino.h>
#include <cstring>
#include <esp_attr.h>
#include <hal/sha_ll.h>
#include <hal/sha_types.h>
#include <soc/dport_access.h>
#include <soc/dport_reg.h>
#include <soc/hwcrypto_reg.h>
#include <sha/sha_parallel_engine.h>

namespace cyd_sha_hw {
namespace {

constexpr esp_sha_type kSha = SHA2_256;

IRAM_ATTR inline void wait_idle() {
  while (DPORT_REG_READ(SHA_256_BUSY_REG) != 0) {
  }
}

IRAM_ATTR inline void wait_before_touch() {
  while (DPORT_REG_READ(SHA_256_BUSY_REG) != 0) {
  }
}

IRAM_ATTR inline uint32_t bswap32(uint32_t x) {
  return ((x & 0x000000ffu) << 24) | ((x & 0x0000ff00u) << 8) | ((x & 0x00ff0000u) >> 8) |
         ((x & 0xff000000u) >> 24);
}

IRAM_ATTR inline void write16(const uint32_t w[16]) {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  reg[0] = w[0];
  reg[1] = w[1];
  reg[2] = w[2];
  reg[3] = w[3];
  reg[4] = w[4];
  reg[5] = w[5];
  reg[6] = w[6];
  reg[7] = w[7];
  reg[8] = w[8];
  reg[9] = w[9];
  reg[10] = w[10];
  reg[11] = w[11];
  reg[12] = w[12];
  reg[13] = w[13];
  reg[14] = w[14];
  reg[15] = w[15];
}

// Block2 = header words 16..19 (merkle-tail, ntime, nbits, nonce) + SHA padding for 80 bytes.
IRAM_ATTR inline void fill_block2(uint32_t w16, uint32_t ntime, uint32_t nbits, uint32_t nonce_be) {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  reg[0] = w16;
  reg[1] = ntime;
  reg[2] = nbits;
  reg[3] = nonce_be;
  reg[4] = 0x80000000u;
  reg[5] = 0;
  reg[6] = 0;
  reg[7] = 0;
  reg[8] = 0;
  reg[9] = 0;
  reg[10] = 0;
  reg[11] = 0;
  reg[12] = 0;
  reg[13] = 0;
  reg[14] = 0;
  reg[15] = 0x00000280u;  // 640 bits
}

IRAM_ATTR inline void pad_second_inplace() {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  // After LOAD, [0..7]=digest; block2 left [8..14]=0 and [15]=0x280.
  reg[8] = 0x80000000u;
  reg[15] = 0x00000100u;
}

IRAM_ATTR inline void read_digest_be(uint32_t out_be[8]) {
  DPORT_INTERRUPT_DISABLE();
  out_be[0] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 0 * 4);
  out_be[1] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 1 * 4);
  out_be[2] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 2 * 4);
  out_be[3] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 3 * 4);
  out_be[4] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 4 * 4);
  out_be[5] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 5 * 4);
  out_be[6] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 6 * 4);
  out_be[7] = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 7 * 4);
  DPORT_INTERRUPT_RESTORE();
}

IRAM_ATTR inline uint32_t peek_msb_le() {
  DPORT_INTERRUPT_DISABLE();
  uint32_t h7 = DPORT_SEQUENCE_REG_READ(SHA_TEXT_BASE + 7 * 4);
  DPORT_INTERRUPT_RESTORE();
  return bswap32(h7);
}

IRAM_ATTR inline void sha256d_full(const uint32_t block1[16], uint32_t w16, uint32_t ntime,
                                   uint32_t nbits, uint32_t nonce_be) {
  wait_before_touch();
  write16(block1);
  sha_ll_start_block(kSha);

  wait_before_touch();
  fill_block2(w16, ntime, nbits, nonce_be);
  sha_ll_continue_block(kSha);

  wait_before_touch();
  sha_ll_load(kSha);
  wait_idle();

  pad_second_inplace();
  sha_ll_start_block(kSha);

  wait_before_touch();
  sha_ll_load(kSha);
  wait_idle();
}

/*
 * Experimental midstate path (Zephyr Apache write-digest pattern).
 * Espressif docs say classic ESP32 cannot restore digest state — only enabled
 * after self_test() proves midstate and full-header digests match on-device.
 */
IRAM_ATTR inline void sha256d_mid(const uint32_t mid_be[8], uint32_t w16, uint32_t ntime,
                                  uint32_t nbits, uint32_t nonce_be) {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  wait_before_touch();
  reg[0] = mid_be[0];
  reg[1] = mid_be[1];
  reg[2] = mid_be[2];
  reg[3] = mid_be[3];
  reg[4] = mid_be[4];
  reg[5] = mid_be[5];
  reg[6] = mid_be[6];
  reg[7] = mid_be[7];
  sha_ll_load(kSha);
  wait_idle();

  fill_block2(w16, ntime, nbits, nonce_be);
  sha_ll_continue_block(kSha);

  wait_before_touch();
  sha_ll_load(kSha);
  wait_idle();

  pad_second_inplace();
  sha_ll_start_block(kSha);

  wait_before_touch();
  sha_ll_load(kSha);
  wait_idle();
}

bool g_locked = false;
bool g_mid_ok = false;

}  // namespace

bool available() { return true; }
bool locked() { return g_locked; }
bool midstate_ok() { return g_mid_ok; }
void disable_midstate() { g_mid_ok = false; }

bool acquire() {
  if (g_locked) return true;
  DPORT_SET_PERI_REG_MASK(DPORT_PERI_CLK_EN_REG, DPORT_PERI_EN_SHA);
  DPORT_CLEAR_PERI_REG_MASK(DPORT_PERI_RST_EN_REG, DPORT_PERI_EN_SHA | DPORT_PERI_EN_SECUREBOOT);
  esp_sha_lock_engine(kSha);
  g_locked = true;
  g_mid_ok = false;
  return true;
}

void release() {
  if (!g_locked) return;
  esp_sha_unlock_engine(kSha);
  g_locked = false;
  g_mid_ok = false;
}

bool self_test(const uint32_t hdr_be[20], const uint32_t mid_be[8]) {
  if (!hdr_be || !mid_be || !g_locked) {
    g_mid_ok = false;
    return false;
  }
  const uint32_t w16 = hdr_be[16];
  const uint32_t ntime = hdr_be[17];
  const uint32_t nbits = hdr_be[18];
  uint32_t block1[16];
  for (int i = 0; i < 16; i++) block1[i] = hdr_be[i];

  for (uint32_t n = 0; n < 8; n++) {
    const uint32_t nonce_be = bswap32(n);
    sha256d_full(block1, w16, ntime, nbits, nonce_be);
    uint32_t full[8];
    read_digest_be(full);

    sha256d_mid(mid_be, w16, ntime, nbits, nonce_be);
    uint32_t mid[8];
    read_digest_be(mid);

    if (memcmp(full, mid, sizeof(full)) != 0) {
      g_mid_ok = false;
      return false;
    }
  }
  g_mid_ok = true;
  return true;
}

IRAM_ATTR bool hash_nonce(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t nonce_le,
                          uint32_t out_be[8], uint32_t msb_limit) {
  const uint32_t w16 = hdr_be[16];
  const uint32_t ntime = hdr_be[17];
  const uint32_t nbits = hdr_be[18];
  const uint32_t nonce_be = bswap32(nonce_le);
  if (g_mid_ok && mid_be) {
    sha256d_mid(mid_be, w16, ntime, nbits, nonce_be);
  } else {
    uint32_t block1[16];
    for (int i = 0; i < 16; i++) block1[i] = hdr_be[i];
    sha256d_full(block1, w16, ntime, nbits, nonce_be);
  }
  const uint32_t msb = peek_msb_le();
  if (out_be) read_digest_be(out_be);
  return msb <= msb_limit;
}

IRAM_ATTR size_t mine(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t* nonce_le,
                      size_t count, uint32_t stride, uint32_t msb_limit, bool* hit,
                      uint32_t* found_nonce, uint32_t found_hash_be[8]) {
  if (hit) *hit = false;
  if (!hdr_be || !nonce_le || stride == 0 || count == 0) return 0;

  const uint32_t w16 = hdr_be[16];
  const uint32_t ntime = hdr_be[17];
  const uint32_t nbits = hdr_be[18];
  uint32_t block1[16];
  const bool use_mid = g_mid_ok && mid_be;
  if (!use_mid) {
    for (int i = 0; i < 16; i++) block1[i] = hdr_be[i];
  }

  uint32_t n = *nonce_le;
  size_t done = 0;
  for (; done < count; done++) {
    const uint32_t nonce_be = bswap32(n);
    if (use_mid) {
      sha256d_mid(mid_be, w16, ntime, nbits, nonce_be);
    } else {
      sha256d_full(block1, w16, ntime, nbits, nonce_be);
    }
    if (peek_msb_le() <= msb_limit) {
      if (found_hash_be) read_digest_be(found_hash_be);
      if (found_nonce) *found_nonce = n;
      if (hit) *hit = true;
      *nonce_le = n + stride;
      return done + 1;
    }
    n += stride;
  }
  *nonce_le = n;
  return done;
}

}  // namespace cyd_sha_hw

#else  // !CYD_SHA_HW

namespace cyd_sha_hw {
bool available() { return false; }
bool acquire() { return false; }
void release() {}
bool locked() { return false; }
bool midstate_ok() { return false; }
void disable_midstate() {}
bool hash_nonce(const uint32_t*, const uint32_t*, uint32_t, uint32_t*, uint32_t) { return false; }
size_t mine(const uint32_t*, const uint32_t*, uint32_t*, size_t, uint32_t, uint32_t, bool*,
            uint32_t*, uint32_t*) {
  return 0;
}
bool self_test(const uint32_t*, const uint32_t*) { return false; }
}  // namespace cyd_sha_hw

#endif
