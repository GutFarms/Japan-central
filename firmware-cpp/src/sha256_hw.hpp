#pragma once
#include <cstdint>
#include <cstddef>

// Classic ESP32 hardware SHA-256 mining helpers (ESP-IDF Apache register API).
// The ESP32 SHA engine has no public midstate-restore in the HAL, so each nonce
// re-feeds both 64-byte header blocks, then a fresh second SHA-256. Still much
// faster than software transforms when the loop stays in IRAM.

#if defined(ARDUINO_ARCH_ESP32) && !defined(CONFIG_IDF_TARGET_ESP32S2) && \
    !defined(CONFIG_IDF_TARGET_ESP32S3) && !defined(CONFIG_IDF_TARGET_ESP32C3) && \
    !defined(CONFIG_IDF_TARGET_ESP32C6) && !defined(CONFIG_IDF_TARGET_ESP32H2)
#define CYD_SHA_HW 1
#else
#define CYD_SHA_HW 0
#endif

namespace cyd_sha_hw {

bool available();
bool acquire();
void release();
bool locked();

// hdr_be: 20 big-endian SHA message words from the 80-byte Bitcoin wire header.
// nonce_le: header nonce as the uint32 stored in bytes 76..79 (host-endian value).
// out_be: always filled with SHA256d digest words (BE), when non-null.
// Returns true if the Bitcoin LE-MSB word (hash bytes 28..31) is <= msb_limit.
bool hash_nonce(const uint32_t hdr_be[20], uint32_t nonce_le, uint32_t out_be[8],
                uint32_t msb_limit);

// Mine up to `count` nonces starting at *nonce_le, stepping by `stride`.
// Advances *nonce_le past the last tried nonce.
// If a candidate passes msb_limit, sets *found_nonce / found_hash_be and returns
// hashes performed including that candidate. Sets *hit=true in that case.
// If no candidate, *hit=false and return value == count (or 0 on bad args).
size_t mine(const uint32_t hdr_be[20], uint32_t* nonce_le, size_t count, uint32_t stride,
            uint32_t msb_limit, bool* hit, uint32_t* found_nonce, uint32_t found_hash_be[8]);

}  // namespace cyd_sha_hw
