#pragma once
#include <cstdint>
#include <cstddef>

// Classic ESP32-D0 hardware SHA-256 mining (ESP-IDF Apache register API).

#if defined(ARDUINO_ARCH_ESP32) && !defined(CONFIG_IDF_TARGET_ESP32S2) && \
    !defined(CONFIG_IDF_TARGET_ESP32S3) && !defined(CONFIG_IDF_TARGET_ESP32C3) && \
    !defined(CONFIG_IDF_TARGET_ESP32C6) && !defined(CONFIG_IDF_TARGET_ESP32H2)
#define CYD_SHA_HW 1
#else
#define CYD_SHA_HW 0
#endif

#ifndef CYD_D0_BUILD
#define CYD_D0_BUILD 0
#endif

namespace cyd_sha_hw {

enum class Mode : uint8_t {
  FullHw = 0,     // 3 HW blocks / nonce (always correct)
  MidHw = 1,      // midstate CONTINUE + 2nd HW SHA (if self-test OK)
  HwSwSecond = 2  // 2 HW blocks + IRAM SW second SHA (often fastest on classic ESP32-D0)
};

struct PathScore {
  bool ok = false;
  float hs = 0;  // measured H/s for this path (0 if not timed)
};

struct TuneReport {
  Mode best = Mode::FullHw;
  float best_hs = 0;
  PathScore hw;       // FullHw
  PathScore hw_plus;  // MidHw
  PathScore hw_sw;    // HwSwSecond
  bool mid_ok = false;
  bool hybrid_ok = false;
};

bool available();
bool acquire();
void release();
bool locked();

Mode mode();
const char* mode_label();  // "HW" / "HW+" / "HW/SW"
const char* mode_label_of(Mode m);
bool midstate_ok();
bool hybrid_ok();
void disable_midstate();

// Preferred path from NVS (-1 = auto). Applied after correctness gate.
void set_preferred_mode(int8_t mode_or_neg1);
int8_t preferred_mode();

// Force a validated path (used while auto-tuning candidates).
void force_mode(Mode m);

// Tune path on-device (correctness + timed micro-bench). Call once after acquire.
void calibrate(const uint32_t hdr_be[20], const uint32_t mid_be[8]);
// Clear calibration so the next setJob/bench re-times HW/HW+/HW-SW paths.
void force_recalibrate();

// Record measured H/s for a path during board bench; pick/lock the winner.
void begin_tune_session();
void record_path_hs(Mode m, float hs);
TuneReport finish_tune_session();  // selects best, locks mode, updates preferred
const TuneReport& last_tune_report();

bool hash_nonce(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t nonce_le,
                uint32_t out_be[8], uint32_t msb_limit);

size_t mine(const uint32_t hdr_be[20], const uint32_t mid_be[8], uint32_t* nonce_le, size_t count,
            uint32_t stride, uint32_t msb_limit, bool* hit, uint32_t* found_nonce,
            uint32_t found_hash_be[8]);

bool self_test(const uint32_t hdr_be[20], const uint32_t mid_be[8]);

}  // namespace cyd_sha_hw
