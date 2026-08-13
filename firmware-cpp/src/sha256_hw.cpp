#include "sha256_hw.hpp"

#if CYD_SHA_HW

#include <Arduino.h>
#include <cstring>
#include <esp_attr.h>
#include <esp_cpu.h>
#include <hal/sha_ll.h>
#include <hal/sha_types.h>
#include <soc/dport_access.h>
#include <soc/dport_reg.h>
#include <soc/hwcrypto_reg.h>
#include <sha/sha_parallel_engine.h>

namespace cyd_sha_hw {
namespace {

constexpr esp_sha_type kSha = SHA2_256;

// SHA-256 block is typically done well under this many CPU cycles @ 240 MHz.
// Pace with ccount so we don't hammer DPORT busy reads.
// Tuned for ~1 MH/s class HW lane — lower pace = less idle wait before busy poll.
constexpr uint32_t kShaPaceCycles = 28;

IRAM_ATTR inline uint32_t ccount() { return esp_cpu_get_ccount(); }

IRAM_ATTR inline void pace(uint32_t start_cc) {
  while ((ccount() - start_cc) < kShaPaceCycles) {
  }
}

IRAM_ATTR inline void wait_busy() {
  while (DPORT_REG_READ(SHA_256_BUSY_REG) != 0) {
  }
}

IRAM_ATTR inline void wait_done(uint32_t start_cc) {
  pace(start_cc);
  wait_busy();
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
  reg[15] = 0x00000280u;
}

IRAM_ATTR inline void pad_second_inplace() {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
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

// ---- IRAM software SHA-256 (second hash only) ----
constexpr uint32_t IV[8] = {
    0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
    0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u,
};

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

#define ROR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (ROR((x), 2) ^ ROR((x), 13) ^ ROR((x), 22))
#define EP1(x) (ROR((x), 6) ^ ROR((x), 11) ^ ROR((x), 25))
#define SIG0(x) (ROR((x), 7) ^ ROR((x), 18) ^ ((x) >> 3))
#define SIG1(x) (ROR((x), 17) ^ ROR((x), 19) ^ ((x) >> 10))

IRAM_ATTR void sha256_one_block(uint32_t state[8], const uint32_t w_in[16]) {
  uint32_t w0 = w_in[0], w1 = w_in[1], w2 = w_in[2], w3 = w_in[3];
  uint32_t w4 = w_in[4], w5 = w_in[5], w6 = w_in[6], w7 = w_in[7];
  uint32_t w8 = w_in[8], w9 = w_in[9], w10 = w_in[10], w11 = w_in[11];
  uint32_t w12 = w_in[12], w13 = w_in[13], w14 = w_in[14], w15 = w_in[15];
  uint32_t a = state[0], b = state[1], c = state[2], d = state[3];
  uint32_t e = state[4], f = state[5], g = state[6], h = state[7];

#define RND(a, b, c, d, e, f, g, h, ki, wi) \
  do {                                      \
    const uint32_t t1 = (h) + EP1(e) + CH(e, f, g) + (ki) + (wi); \
    const uint32_t t2 = EP0(a) + MAJ(a, b, c);                    \
    (d) += t1;                              \
    (h) = t1 + t2;                          \
  } while (0)

  RND(a, b, c, d, e, f, g, h, K[0], w0);
  RND(h, a, b, c, d, e, f, g, K[1], w1);
  RND(g, h, a, b, c, d, e, f, K[2], w2);
  RND(f, g, h, a, b, c, d, e, K[3], w3);
  RND(e, f, g, h, a, b, c, d, K[4], w4);
  RND(d, e, f, g, h, a, b, c, K[5], w5);
  RND(c, d, e, f, g, h, a, b, K[6], w6);
  RND(b, c, d, e, f, g, h, a, K[7], w7);
  RND(a, b, c, d, e, f, g, h, K[8], w8);
  RND(h, a, b, c, d, e, f, g, K[9], w9);
  RND(g, h, a, b, c, d, e, f, K[10], w10);
  RND(f, g, h, a, b, c, d, e, K[11], w11);
  RND(e, f, g, h, a, b, c, d, K[12], w12);
  RND(d, e, f, g, h, a, b, c, K[13], w13);
  RND(c, d, e, f, g, h, a, b, K[14], w14);
  RND(b, c, d, e, f, g, h, a, K[15], w15);

#define SCHED(i0, i1, i2, i3, i4, i5, i6, i7, i8, i9, i10, i11, i12, i13, i14, i15, base) \
  do {                                                                                    \
    i0 += SIG1(i14) + i9 + SIG0(i1);                                                      \
    RND(a, b, c, d, e, f, g, h, K[(base) + 0], i0);                                       \
    i1 += SIG1(i15) + i10 + SIG0(i2);                                                     \
    RND(h, a, b, c, d, e, f, g, K[(base) + 1], i1);                                       \
    i2 += SIG1(i0) + i11 + SIG0(i3);                                                      \
    RND(g, h, a, b, c, d, e, f, K[(base) + 2], i2);                                       \
    i3 += SIG1(i1) + i12 + SIG0(i4);                                                      \
    RND(f, g, h, a, b, c, d, e, K[(base) + 3], i3);                                       \
    i4 += SIG1(i2) + i13 + SIG0(i5);                                                      \
    RND(e, f, g, h, a, b, c, d, K[(base) + 4], i4);                                       \
    i5 += SIG1(i3) + i14 + SIG0(i6);                                                      \
    RND(d, e, f, g, h, a, b, c, K[(base) + 5], i5);                                       \
    i6 += SIG1(i4) + i15 + SIG0(i7);                                                      \
    RND(c, d, e, f, g, h, a, b, K[(base) + 6], i6);                                       \
    i7 += SIG1(i5) + i0 + SIG0(i8);                                                       \
    RND(b, c, d, e, f, g, h, a, K[(base) + 7], i7);                                       \
    i8 += SIG1(i6) + i1 + SIG0(i9);                                                       \
    RND(a, b, c, d, e, f, g, h, K[(base) + 8], i8);                                       \
    i9 += SIG1(i7) + i2 + SIG0(i10);                                                      \
    RND(h, a, b, c, d, e, f, g, K[(base) + 9], i9);                                       \
    i10 += SIG1(i8) + i3 + SIG0(i11);                                                     \
    RND(g, h, a, b, c, d, e, f, K[(base) + 10], i10);                                     \
    i11 += SIG1(i9) + i4 + SIG0(i12);                                                     \
    RND(f, g, h, a, b, c, d, e, K[(base) + 11], i11);                                     \
    i12 += SIG1(i10) + i5 + SIG0(i13);                                                    \
    RND(e, f, g, h, a, b, c, d, K[(base) + 12], i12);                                     \
    i13 += SIG1(i11) + i6 + SIG0(i14);                                                    \
    RND(d, e, f, g, h, a, b, c, K[(base) + 13], i13);                                     \
    i14 += SIG1(i12) + i7 + SIG0(i15);                                                    \
    RND(c, d, e, f, g, h, a, b, K[(base) + 14], i14);                                     \
    i15 += SIG1(i13) + i8 + SIG0(i0);                                                     \
    RND(b, c, d, e, f, g, h, a, K[(base) + 15], i15);                                     \
  } while (0)

  SCHED(w0, w1, w2, w3, w4, w5, w6, w7, w8, w9, w10, w11, w12, w13, w14, w15, 16);
  SCHED(w0, w1, w2, w3, w4, w5, w6, w7, w8, w9, w10, w11, w12, w13, w14, w15, 32);
  SCHED(w0, w1, w2, w3, w4, w5, w6, w7, w8, w9, w10, w11, w12, w13, w14, w15, 48);
#undef SCHED
#undef RND

  state[0] += a;
  state[1] += b;
  state[2] += c;
  state[3] += d;
  state[4] += e;
  state[5] += f;
  state[6] += g;
  state[7] += h;
}

// Second SHA256 of a 32-byte digest (BE words) → out_be. Returns LE MSB word.
IRAM_ATTR uint32_t sha256_second_sw(const uint32_t dig_be[8], uint32_t out_be[8]) {
  uint32_t w[16];
  w[0] = dig_be[0];
  w[1] = dig_be[1];
  w[2] = dig_be[2];
  w[3] = dig_be[3];
  w[4] = dig_be[4];
  w[5] = dig_be[5];
  w[6] = dig_be[6];
  w[7] = dig_be[7];
  w[8] = 0x80000000u;
  w[9] = 0;
  w[10] = 0;
  w[11] = 0;
  w[12] = 0;
  w[13] = 0;
  w[14] = 0;
  w[15] = 256u;
  uint32_t st[8];
  memcpy(st, IV, sizeof(st));
  sha256_one_block(st, w);
  if (out_be) memcpy(out_be, st, sizeof(st));
  return bswap32(st[7]);
}

IRAM_ATTR void hw_first_hash_full(const uint32_t block1[16], uint32_t w16, uint32_t ntime,
                                  uint32_t nbits, uint32_t nonce_be) {
  uint32_t t = ccount();
  write16(block1);
  DPORT_REG_WRITE(SHA_256_START_REG, 1);
  wait_done(t);

  t = ccount();
  fill_block2(w16, ntime, nbits, nonce_be);
  DPORT_REG_WRITE(SHA_256_CONTINUE_REG, 1);
  wait_done(t);

  t = ccount();
  DPORT_REG_WRITE(SHA_256_LOAD_REG, 1);
  wait_done(t);
}

IRAM_ATTR void hw_first_hash_mid(const uint32_t mid_be[8], uint32_t w16, uint32_t ntime,
                                 uint32_t nbits, uint32_t nonce_be) {
  volatile uint32_t* reg = (volatile uint32_t*)SHA_TEXT_BASE;
  uint32_t t = ccount();
  reg[0] = mid_be[0];
  reg[1] = mid_be[1];
  reg[2] = mid_be[2];
  reg[3] = mid_be[3];
  reg[4] = mid_be[4];
  reg[5] = mid_be[5];
  reg[6] = mid_be[6];
  reg[7] = mid_be[7];
  DPORT_REG_WRITE(SHA_256_LOAD_REG, 1);
  wait_done(t);

  t = ccount();
  fill_block2(w16, ntime, nbits, nonce_be);
  DPORT_REG_WRITE(SHA_256_CONTINUE_REG, 1);
  wait_done(t);

  t = ccount();
  DPORT_REG_WRITE(SHA_256_LOAD_REG, 1);
  wait_done(t);
}

IRAM_ATTR void hw_second_sha() {
  uint32_t t = ccount();
  pad_second_inplace();
  DPORT_REG_WRITE(SHA_256_START_REG, 1);
  wait_done(t);

  t = ccount();
  DPORT_REG_WRITE(SHA_256_LOAD_REG, 1);
  wait_done(t);
}

IRAM_ATTR void sha256d_full_hw(const uint32_t block1[16], uint32_t w16, uint32_t ntime,
                               uint32_t nbits, uint32_t nonce_be) {
  hw_first_hash_full(block1, w16, ntime, nbits, nonce_be);
  hw_second_sha();
}

IRAM_ATTR void sha256d_mid_hw(const uint32_t mid_be[8], uint32_t w16, uint32_t ntime,
                              uint32_t nbits, uint32_t nonce_be) {
  hw_first_hash_mid(mid_be, w16, ntime, nbits, nonce_be);
  hw_second_sha();
}

IRAM_ATTR uint32_t sha256d_hw_sw(const uint32_t block1[16], const uint32_t mid_be[8], bool use_mid,
                                 uint32_t w16, uint32_t ntime, uint32_t nbits, uint32_t nonce_be,
                                 uint32_t out_be[8]) {
  if (use_mid) {
    hw_first_hash_mid(mid_be, w16, ntime, nbits, nonce_be);
  } else {
    hw_first_hash_full(block1, w16, ntime, nbits, nonce_be);
  }
  uint32_t dig[8];
  read_digest_be(dig);
  return sha256_second_sw(dig, out_be);
}

bool g_locked = false;
bool g_mid_ok = false;
bool g_hybrid_ok = false;
bool g_calibrated = false;
Mode g_mode = Mode::FullHw;
int8_t g_preferred = -1;  // -1 auto, else Mode ordinal
TuneReport g_last_tune{};
bool g_tune_active = false;

}  // namespace

bool available() { return true; }
bool locked() { return g_locked; }
Mode mode() { return g_mode; }
bool midstate_ok() { return g_mid_ok; }
bool hybrid_ok() { return g_hybrid_ok; }
void disable_midstate() {
  g_mid_ok = false;
  if (g_mode == Mode::MidHw) g_mode = Mode::HwSwSecond;
}

void set_preferred_mode(int8_t mode_or_neg1) { g_preferred = mode_or_neg1; }
int8_t preferred_mode() { return g_preferred; }

void force_mode(Mode m) {
  if (m == Mode::MidHw && !g_mid_ok) m = Mode::FullHw;
  if (m == Mode::HwSwSecond && !g_hybrid_ok) m = Mode::FullHw;
  g_mode = m;
  g_calibrated = true;
}

const char* mode_label_of(Mode m) {
  switch (m) {
    case Mode::MidHw:
      return "HW+";
    case Mode::HwSwSecond:
      return "HW/SW";
    default:
      return "HW";
  }
}

const char* mode_label() { return mode_label_of(g_mode); }

bool acquire() {
  if (g_locked) return true;
  DPORT_SET_PERI_REG_MASK(DPORT_PERI_CLK_EN_REG, DPORT_PERI_EN_SHA);
  DPORT_CLEAR_PERI_REG_MASK(DPORT_PERI_RST_EN_REG, DPORT_PERI_EN_SHA | DPORT_PERI_EN_SECUREBOOT);
  esp_sha_lock_engine(kSha);
  g_locked = true;
  g_mid_ok = false;
  g_hybrid_ok = false;
  g_calibrated = false;
  g_mode = Mode::FullHw;
  return true;
}

void release() {
  if (!g_locked) return;
  esp_sha_unlock_engine(kSha);
  g_locked = false;
  g_mid_ok = false;
  g_hybrid_ok = false;
  g_calibrated = false;
  g_mode = Mode::FullHw;
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

  for (uint32_t n = 0; n < 4; n++) {
    const uint32_t nonce_be = bswap32(n);
    uint32_t full[8], mid[8], hybrid[8];

    sha256d_full_hw(block1, w16, ntime, nbits, nonce_be);
    read_digest_be(full);

    sha256d_mid_hw(mid_be, w16, ntime, nbits, nonce_be);
    read_digest_be(mid);

    (void)sha256d_hw_sw(block1, mid_be, false, w16, ntime, nbits, nonce_be, hybrid);

    if (memcmp(full, hybrid, sizeof(full)) != 0) {
      g_mid_ok = false;
      return false;  // SW second path broken — stay on FullHw
    }
    if (memcmp(full, mid, sizeof(full)) != 0) {
      g_mid_ok = false;
      return false;
    }
  }
  g_mid_ok = true;
  return true;
}

void calibrate(const uint32_t hdr_be[20], const uint32_t mid_be[8]) {
  // Path choice is silicon-stable — do NOT re-bench on every stratum job.
  if (g_calibrated) return;
  if (!hdr_be || !g_locked) {
    g_mode = Mode::FullHw;
    return;
  }
  const uint32_t w16 = hdr_be[16];
  const uint32_t ntime = hdr_be[17];
  const uint32_t nbits = hdr_be[18];
  uint32_t block1[16];
  for (int i = 0; i < 16; i++) block1[i] = hdr_be[i];

  // Correctness gate for mid + hybrid.
  g_mid_ok = false;
  g_hybrid_ok = false;
  bool mid_ok = false;
  bool hybrid_ok = false;
  {
    uint32_t full[8], mid[8], hy[8];
    const uint32_t nonce_be = bswap32(1u);
    sha256d_full_hw(block1, w16, ntime, nbits, nonce_be);
    read_digest_be(full);

    sha256d_mid_hw(mid_be, w16, ntime, nbits, nonce_be);
    read_digest_be(mid);
    mid_ok = (memcmp(full, mid, sizeof(full)) == 0);

    (void)sha256d_hw_sw(block1, mid_be, false, w16, ntime, nbits, nonce_be, hy);
    hybrid_ok = (memcmp(full, hy, sizeof(full)) == 0);

    if (mid_ok) {
      (void)sha256d_hw_sw(block1, mid_be, true, w16, ntime, nbits, nonce_be, hy);
      if (memcmp(full, hy, sizeof(full)) != 0) mid_ok = false;
    }
  }
  g_mid_ok = mid_ok;
  g_hybrid_ok = hybrid_ok;

  // Honoured preferred path from a prior Bench (NVS), if still valid.
  if (g_preferred == (int8_t)Mode::FullHw) {
    g_mode = Mode::FullHw;
    g_calibrated = true;
    return;
  }
  if (g_preferred == (int8_t)Mode::MidHw && mid_ok) {
    g_mode = Mode::MidHw;
    g_calibrated = true;
    return;
  }
  if (g_preferred == (int8_t)Mode::HwSwSecond && hybrid_ok) {
    g_mode = Mode::HwSwSecond;
    g_calibrated = true;
    return;
  }

  auto time_path = [&](Mode m) -> uint32_t {
    // D0 builds use a longer micro-window for a stabler pick.
    const uint32_t N = CYD_D0_BUILD ? 6144u : 2048u;
    uint32_t t0 = micros();
    for (uint32_t i = 0; i < N; i++) {
      const uint32_t nonce_be = bswap32(i);
      switch (m) {
        case Mode::MidHw:
          sha256d_mid_hw(mid_be, w16, ntime, nbits, nonce_be);
          break;
        case Mode::HwSwSecond:
          (void)sha256d_hw_sw(block1, mid_be, mid_ok, w16, ntime, nbits, nonce_be, nullptr);
          break;
        default:
          sha256d_full_hw(block1, w16, ntime, nbits, nonce_be);
          break;
      }
    }
    uint32_t dt = micros() - t0;
    return dt ? dt : 1;
  };

  uint32_t best_dt = time_path(Mode::FullHw);
  Mode best = Mode::FullHw;

  if (hybrid_ok) {
    uint32_t dt = time_path(Mode::HwSwSecond);
    if (dt < best_dt) {
      best_dt = dt;
      best = Mode::HwSwSecond;
    }
  }
  if (mid_ok) {
    uint32_t dt = time_path(Mode::MidHw);
    if (dt < best_dt) {
      best_dt = dt;
      best = Mode::MidHw;
    }
  }

  g_mode = best;
  g_calibrated = true;
}

void force_recalibrate() {
  g_calibrated = false;
  g_mid_ok = false;
  g_hybrid_ok = false;
  g_mode = Mode::FullHw;
}

void begin_tune_session() {
  g_tune_active = true;
  g_last_tune = TuneReport{};
  g_last_tune.mid_ok = g_mid_ok;
  g_last_tune.hybrid_ok = g_hybrid_ok;
  g_last_tune.hw.ok = true;
}

void record_path_hs(Mode m, float hs) {
  if (!g_tune_active) return;
  PathScore* slot = nullptr;
  switch (m) {
    case Mode::MidHw:
      slot = &g_last_tune.hw_plus;
      break;
    case Mode::HwSwSecond:
      slot = &g_last_tune.hw_sw;
      break;
    default:
      slot = &g_last_tune.hw;
      break;
  }
  slot->ok = true;
  slot->hs = hs;
}

TuneReport finish_tune_session() {
  g_tune_active = false;
  g_last_tune.mid_ok = g_mid_ok;
  g_last_tune.hybrid_ok = g_hybrid_ok;

  Mode best = Mode::FullHw;
  float best_hs = g_last_tune.hw.ok ? g_last_tune.hw.hs : 0;
  if (g_last_tune.hw_sw.ok && g_last_tune.hw_sw.hs > best_hs) {
    best = Mode::HwSwSecond;
    best_hs = g_last_tune.hw_sw.hs;
  }
  if (g_last_tune.hw_plus.ok && g_last_tune.hw_plus.hs > best_hs) {
    best = Mode::MidHw;
    best_hs = g_last_tune.hw_plus.hs;
  }
  // Within ~1.5% noise, prefer HW/SW (usually strongest on ESP32-D0).
  if (g_last_tune.hw_sw.ok && best != Mode::HwSwSecond && best_hs > 0 &&
      g_last_tune.hw_sw.hs >= best_hs * 0.985f) {
    best = Mode::HwSwSecond;
    best_hs = g_last_tune.hw_sw.hs;
  }

  force_mode(best);
  g_preferred = (int8_t)best;
  g_last_tune.best = best;
  g_last_tune.best_hs = best_hs;
  return g_last_tune;
}

const TuneReport& last_tune_report() { return g_last_tune; }

IRAM_ATTR bool hash_nonce(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t nonce_le,
                          uint32_t out_be[8], uint32_t msb_limit) {
  const uint32_t w16 = hdr_be[16];
  const uint32_t ntime = hdr_be[17];
  const uint32_t nbits = hdr_be[18];
  const uint32_t nonce_be = bswap32(nonce_le);
  uint32_t block1[16];
  for (int i = 0; i < 16; i++) block1[i] = hdr_be[i];

  uint32_t msb;
  if (g_mode == Mode::HwSwSecond) {
    msb = sha256d_hw_sw(block1, mid_be, g_mid_ok, w16, ntime, nbits, nonce_be, out_be);
  } else if (g_mode == Mode::MidHw && g_mid_ok) {
    sha256d_mid_hw(mid_be, w16, ntime, nbits, nonce_be);
    msb = peek_msb_le();
    if (out_be) read_digest_be(out_be);
  } else {
    sha256d_full_hw(block1, w16, ntime, nbits, nonce_be);
    msb = peek_msb_le();
    if (out_be) read_digest_be(out_be);
  }
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
  for (int i = 0; i < 16; i++) block1[i] = hdr_be[i];
  const bool use_mid = g_mid_ok && mid_be;
  const Mode m = g_mode;

  uint32_t n = *nonce_le;
  size_t done = 0;
  for (; done < count; done++) {
    const uint32_t nonce_be = bswap32(n);
    uint32_t msb;
    uint32_t dig[8];

    if (m == Mode::HwSwSecond) {
      msb = sha256d_hw_sw(block1, mid_be, use_mid, w16, ntime, nbits, nonce_be, dig);
      if (msb <= msb_limit) {
        if (found_hash_be) memcpy(found_hash_be, dig, sizeof(dig));
        if (found_nonce) *found_nonce = n;
        if (hit) *hit = true;
        *nonce_le = n + stride;
        return done + 1;
      }
    } else if (m == Mode::MidHw && use_mid) {
      sha256d_mid_hw(mid_be, w16, ntime, nbits, nonce_be);
      msb = peek_msb_le();
      if (msb <= msb_limit) {
        if (found_hash_be) read_digest_be(found_hash_be);
        if (found_nonce) *found_nonce = n;
        if (hit) *hit = true;
        *nonce_le = n + stride;
        return done + 1;
      }
    } else {
      sha256d_full_hw(block1, w16, ntime, nbits, nonce_be);
      msb = peek_msb_le();
      if (msb <= msb_limit) {
        if (found_hash_be) read_digest_be(found_hash_be);
        if (found_nonce) *found_nonce = n;
        if (hit) *hit = true;
        *nonce_le = n + stride;
        return done + 1;
      }
    }
    n += stride;
  }
  *nonce_le = n;
  return done;
}

}  // namespace cyd_sha_hw

#else  // !CYD_SHA_HW

namespace cyd_sha_hw {
static TuneReport g_last_tune{};

bool available() { return false; }
bool acquire() { return false; }
void release() {}
bool locked() { return false; }
Mode mode() { return Mode::FullHw; }
const char* mode_label_of(Mode) { return "SW"; }
const char* mode_label() { return "SW"; }
bool midstate_ok() { return false; }
bool hybrid_ok() { return false; }
void disable_midstate() {}
void set_preferred_mode(int8_t) {}
int8_t preferred_mode() { return -1; }
void force_mode(Mode) {}
void calibrate(const uint32_t*, const uint32_t*) {}
void force_recalibrate() {}
void begin_tune_session() { g_last_tune = TuneReport{}; }
void record_path_hs(Mode, float) {}
TuneReport finish_tune_session() { return g_last_tune; }
const TuneReport& last_tune_report() { return g_last_tune; }
bool hash_nonce(const uint32_t*, const uint32_t*, uint32_t, uint32_t*, uint32_t) { return false; }
size_t mine(const uint32_t*, const uint32_t*, uint32_t*, size_t, uint32_t, uint32_t, bool*,
            uint32_t*, uint32_t*) {
  return 0;
}
bool self_test(const uint32_t*, const uint32_t*) { return false; }
}  // namespace cyd_sha_hw

#endif
