#pragma once
#include <cstdint>
#include <cstddef>

// Classic ESP32 hardware SHA-256 mining helpers (ESP-IDF Apache register API).

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

// True when midstate→CONTINUE path passed the on-device self-test (faster).
bool midstate_ok();
void disable_midstate();  // force full-header path (e.g. after a verify miss)

// hdr_be: 20 big-endian SHA message words from the 80-byte Bitcoin wire header.
// mid_be: 8 midstate words after hashing hdr_be[0..15] (same format as SW midstate).
bool hash_nonce(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t nonce_le,
                uint32_t out_be[8], uint32_t msb_limit);

size_t mine(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t* nonce_le, size_t count,
            uint32_t stride, uint32_t msb_limit, bool* hit, uint32_t* found_nonce,
            uint32_t found_hash_be[8]);

// Compare midstate path vs full-header path for a few nonces. Enables midstate_ok on pass.
bool self_test(const uint32_t hdr_be[20], const uint32_t mid_be[8]);

}  // namespace cyd_sha_hw
