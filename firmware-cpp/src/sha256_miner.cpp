#include "sha256_miner.hpp"
#include "sha256_hw.hpp"
#include <cstring>

#if defined(ESP_PLATFORM)
#include <esp_attr.h>
#define SHA_HOT IRAM_ATTR
#else
#define SHA_HOT
#endif

#define SHA_INLINE __attribute__((always_inline)) inline

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

SHA_INLINE uint32_t rotr(uint32_t x, uint32_t n) {
  return (x >> n) | (x << (32u - n));
}

SHA_INLINE uint32_t be32(const uint8_t* p) {
  return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | (uint32_t)p[3];
}

SHA_INLINE void store_be32(uint8_t* p, uint32_t v) {
  p[0] = (uint8_t)(v >> 24);
  p[1] = (uint8_t)(v >> 16);
  p[2] = (uint8_t)(v >> 8);
  p[3] = (uint8_t)v;
}

SHA_INLINE uint32_t nonce_be(uint32_t nonce_le) {
  return ((nonce_le & 0xffu) << 24) | (((nonce_le >> 8) & 0xffu) << 16) |
         (((nonce_le >> 16) & 0xffu) << 8) | ((nonce_le >> 24) & 0xffu);
}

SHA_INLINE uint32_t bswap32(uint32_t x) {
  return ((x & 0x000000ffu) << 24) | ((x & 0x0000ff00u) << 8) | ((x & 0x00ff0000u) >> 8) |
         ((x & 0xff000000u) >> 24);
}

#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (rotr((x), 2) ^ rotr((x), 13) ^ rotr((x), 22))
#define EP1(x) (rotr((x), 6) ^ rotr((x), 11) ^ rotr((x), 25))
#define SIG0(x) (rotr((x), 7) ^ rotr((x), 18) ^ ((x) >> 3))
#define SIG1(x) (rotr((x), 17) ^ rotr((x), 19) ^ ((x) >> 10))

#define RND(a, b, c, d, e, f, g, h, ki, wi)                         \
  do {                                                              \
    const uint32_t t1 = (h) + EP1(e) + CH(e, f, g) + (ki) + (wi); \
    const uint32_t t2 = EP0(a) + MAJ(a, b, c);                      \
    (d) += t1;                                                      \
    (h) = t1 + t2;                                                  \
  } while (0)

SHA_HOT static void sha256_transform(uint32_t state[8], const uint32_t w_in[16]) {
  uint32_t w0 = w_in[0], w1 = w_in[1], w2 = w_in[2], w3 = w_in[3];
  uint32_t w4 = w_in[4], w5 = w_in[5], w6 = w_in[6], w7 = w_in[7];
  uint32_t w8 = w_in[8], w9 = w_in[9], w10 = w_in[10], w11 = w_in[11];
  uint32_t w12 = w_in[12], w13 = w_in[13], w14 = w_in[14], w15 = w_in[15];

  uint32_t a = state[0], b = state[1], c = state[2], d = state[3];
  uint32_t e = state[4], f = state[5], g = state[6], h = state[7];

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

  state[0] += a;
  state[1] += b;
  state[2] += c;
  state[3] += d;
  state[4] += e;
  state[5] += f;
  state[6] += g;
  state[7] += h;
}

SHA_HOT static void sha256d_mid(const uint32_t mid[8], uint32_t w2[16], uint32_t out_be[8]) {
  uint32_t st[8];
  memcpy(st, mid, sizeof(st));
  sha256_transform(st, w2);

  uint32_t w3[16];
  w3[0] = st[0];
  w3[1] = st[1];
  w3[2] = st[2];
  w3[3] = st[3];
  w3[4] = st[4];
  w3[5] = st[5];
  w3[6] = st[6];
  w3[7] = st[7];
  w3[8] = 0x80000000u;
  w3[9] = 0;
  w3[10] = 0;
  w3[11] = 0;
  w3[12] = 0;
  w3[13] = 0;
  w3[14] = 0;
  w3[15] = 256u;
  memcpy(out_be, IV, sizeof(IV));
  sha256_transform(out_be, w3);
}

}  // namespace

bool Sha256Miner::acquireHardware() { return cyd_sha_hw::acquire(); }
void Sha256Miner::releaseHardware() { cyd_sha_hw::release(); }

bool Sha256Miner::begin() {
  memset(target_, 0xFF, HASH_LEN);
  target_[31] = 0x00;
  target_[30] = 0xFF;
  packTarget(target_);
  hw_ = cyd_sha_hw::available();
  ready_ = true;
  return true;
}

void Sha256Miner::packTarget(const uint8_t target[HASH_LEN]) {
  memcpy(target_, target, HASH_LEN);
  for (int i = 0; i < 8; i++) {
    targetLe_[i] = (uint32_t)target[i * 4] | ((uint32_t)target[i * 4 + 1] << 8) |
                   ((uint32_t)target[i * 4 + 2] << 16) | ((uint32_t)target[i * 4 + 3] << 24);
  }
}

void Sha256Miner::prepareMidstate() {
  for (int i = 0; i < 20; i++) hdrBe_[i] = be32(header_ + i * 4);

  uint32_t w[16];
  for (int i = 0; i < 16; i++) w[i] = hdrBe_[i];
  memcpy(midstate_, IV, sizeof(midstate_));
  sha256_transform(midstate_, w);

  memset(chunk2_, 0, sizeof(chunk2_));
  chunk2_[0] = hdrBe_[16];
  chunk2_[1] = hdrBe_[17];
  chunk2_[2] = hdrBe_[18];
  chunk2_[3] = 0;
  chunk2_[4] = 0x80000000u;
  chunk2_[15] = 640u;
  midReady_ = true;
}

void Sha256Miner::setJob(const uint8_t header[HEADER_LEN], const uint8_t target[HASH_LEN],
                         uint32_t startNonce) {
  memcpy(header_, header, HEADER_LEN);
  packTarget(target);
  nonce_ = startNonce;
  prepareMidstate();
}

void Sha256Miner::updateTarget(const uint8_t target[HASH_LEN]) { packTarget(target); }

bool Sha256Miner::meetsTargetWords(const uint32_t hash_be[8]) const {
  for (int i = 7; i >= 0; i--) {
    const uint32_t hv = bswap32(hash_be[i]);
    const uint32_t tv = targetLe_[i];
    if (hv < tv) return true;
    if (hv > tv) return false;
  }
  return true;
}

void Sha256Miner::hashNonce(uint32_t nonce, uint8_t out[HASH_LEN]) {
  header_[76] = (uint8_t)(nonce);
  header_[77] = (uint8_t)(nonce >> 8);
  header_[78] = (uint8_t)(nonce >> 16);
  header_[79] = (uint8_t)(nonce >> 24);

  uint32_t digest[8];
  if (hw_ && cyd_sha_hw::locked()) {
    (void)cyd_sha_hw::hash_nonce(hdrBe_, nonce, digest, 0xFFFFFFFFu);
  } else {
    uint32_t w2[16];
    memcpy(w2, chunk2_, sizeof(w2));
    w2[3] = nonce_be(nonce);
    sha256d_mid(midstate_, w2, digest);
  }
  for (int i = 0; i < 8; i++) store_be32(out + i * 4, digest[i]);
}

bool Sha256Miner::mineBatchHw(size_t count, uint32_t stride) {
  if (!cyd_sha_hw::locked()) return mineBatchSw(count, stride);

  size_t remaining = count;
  bool anyShare = false;
  while (remaining > 0) {
    const uint32_t msb_target = targetLe_[7];
    uint32_t found = 0;
    uint32_t digest[8]{};
    bool hit = false;
    size_t stepped =
        cyd_sha_hw::mine(hdrBe_, &nonce_, remaining, stride, msb_target, &hit, &found, digest);
    hashes_ += stepped;
    if (stepped == 0) break;
    remaining = remaining > stepped ? remaining - stepped : 0;
    if (!hit) break;

    // Re-check with software midstate so a bad HW digest can never become a share.
    uint32_t w2[16];
    memcpy(w2, chunk2_, sizeof(w2));
    w2[3] = nonce_be(found);
    uint32_t sw[8];
    sha256d_mid(midstate_, w2, sw);
    if (memcmp(sw, digest, sizeof(sw)) != 0) {
      continue;  // HW/SW mismatch — skip, keep mining
    }
    if (meetsTargetWords(sw)) {
      for (int j = 0; j < 8; j++) store_be32(lastHash_ + j * 4, sw[j]);
      shares_++;
      lastShareNonce_ = found;
      anyShare = true;
      break;
    }
  }
  return anyShare;
}

bool Sha256Miner::mineBatchSw(size_t count, uint32_t stride) {
  uint32_t w2[16];
  memcpy(w2, chunk2_, sizeof(w2));

  bool found = false;
  uint32_t n = nonce_;
  const uint32_t msb_target = targetLe_[7];

  for (size_t i = 0; i < count; i++) {
    w2[3] = nonce_be(n);
    uint32_t digest[8];
    sha256d_mid(midstate_, w2, digest);
    hashes_++;

    const uint32_t msb = bswap32(digest[7]);
    if (msb <= msb_target && meetsTargetWords(digest)) {
      for (int j = 0; j < 8; j++) store_be32(lastHash_ + j * 4, digest[j]);
      shares_++;
      lastShareNonce_ = n;
      found = true;
    }
    n += stride;
  }
  nonce_ = n;
  return found;
}

bool Sha256Miner::mineBatch(size_t count, uint32_t stride) {
  if (!ready_ || !midReady_) return false;
  if (stride == 0) stride = 1;
  if (hw_) return mineBatchHw(count, stride);
  return mineBatchSw(count, stride);
}
