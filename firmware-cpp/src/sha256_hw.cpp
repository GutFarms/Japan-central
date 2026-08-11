#include "sha256_hw.hpp"

#if CYD_SHA_HW

#include <Arduino.h>
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

// ESP-IDF sha_ll_fill_text_block() HAL_SWAP32s host words into BE register words.
// We write BE message words directly (same final register contents).

IRAM_ATTR inline void wait_idle() {
  while (DPORT_REG_READ(SHA_256_BUSY_REG) != 0) {
  }
}

IRAM_ATTR inline void write_block_be(const uint32_t be_words[16]) {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  reg[0] = be_words[0];
  reg[1] = be_words[1];
  reg[2] = be_words[2];
  reg[3] = be_words[3];
  reg[4] = be_words[4];
  reg[5] = be_words[5];
  reg[6] = be_words[6];
  reg[7] = be_words[7];
  reg[8] = be_words[8];
  reg[9] = be_words[9];
  reg[10] = be_words[10];
  reg[11] = be_words[11];
  reg[12] = be_words[12];
  reg[13] = be_words[13];
  reg[14] = be_words[14];
  reg[15] = be_words[15];
}

IRAM_ATTR inline void write_block2(const uint32_t hdr_be[20], uint32_t nonce_le) {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  const uint32_t nonce_be =
      ((nonce_le & 0xffu) << 24) | (((nonce_le >> 8) & 0xffu) << 16) |
      (((nonce_le >> 16) & 0xffu) << 8) | ((nonce_le >> 24) & 0xffu);
  reg[0] = hdr_be[16];
  reg[1] = hdr_be[17];
  reg[2] = hdr_be[18];
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

IRAM_ATTR inline void pad_second_sha() {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  // After LOAD, TEXT[0..7] hold the first digest (BE words). Pad a 32-byte message.
  reg[8] = 0x80000000u;
  reg[9] = 0;
  reg[10] = 0;
  reg[11] = 0;
  reg[12] = 0;
  reg[13] = 0;
  reg[14] = 0;
  reg[15] = 0x00000100u;  // 256 bits
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
  // BE digest word 7 → Bitcoin LE hash bytes 28..31 as uint32.
  return ((h7 & 0xffu) << 24) | (((h7 >> 8) & 0xffu) << 16) | (((h7 >> 16) & 0xffu) << 8) |
         ((h7 >> 24) & 0xffu);
}

IRAM_ATTR inline void sha256d_once(const uint32_t hdr_be[20], uint32_t nonce_le) {
  write_block_be(hdr_be);
  sha_ll_start_block(kSha);
  wait_idle();

  write_block2(hdr_be, nonce_le);
  sha_ll_continue_block(kSha);
  wait_idle();
  sha_ll_load(kSha);
  wait_idle();

  pad_second_sha();
  sha_ll_start_block(kSha);
  wait_idle();
  sha_ll_load(kSha);
  wait_idle();
}

bool g_locked = false;

}  // namespace

bool available() { return true; }
bool locked() { return g_locked; }

bool acquire() {
  if (g_locked) return true;
  DPORT_SET_PERI_REG_MASK(DPORT_PERI_CLK_EN_REG, DPORT_PERI_EN_SHA);
  DPORT_CLEAR_PERI_REG_MASK(DPORT_PERI_RST_EN_REG, DPORT_PERI_EN_SHA | DPORT_PERI_EN_SECUREBOOT);
  esp_sha_lock_engine(kSha);
  g_locked = true;
  return true;
}

void release() {
  if (!g_locked) return;
  esp_sha_unlock_engine(kSha);
  g_locked = false;
}

IRAM_ATTR bool hash_nonce(const uint32_t hdr_be[20], uint32_t nonce_le, uint32_t out_be[8],
                          uint32_t msb_limit) {
  sha256d_once(hdr_be, nonce_le);
  const uint32_t msb = peek_msb_le();
  if (out_be) read_digest_be(out_be);
  return msb <= msb_limit;
}

IRAM_ATTR size_t mine(const uint32_t hdr_be[20], uint32_t* nonce_le, size_t count, uint32_t stride,
                      uint32_t msb_limit, bool* hit, uint32_t* found_nonce,
                      uint32_t found_hash_be[8]) {
  if (hit) *hit = false;
  if (!hdr_be || !nonce_le || stride == 0 || count == 0) return 0;

  uint32_t n = *nonce_le;
  size_t done = 0;
  for (; done < count; done++) {
    sha256d_once(hdr_be, n);
    const uint32_t msb = peek_msb_le();
    if (msb <= msb_limit) {
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
bool hash_nonce(const uint32_t*, uint32_t, uint32_t*, uint32_t) { return false; }
size_t mine(const uint32_t*, uint32_t*, size_t, uint32_t, uint32_t, bool*, uint32_t*, uint32_t*) {
  return 0;
}
}  // namespace cyd_sha_hw

#endif
