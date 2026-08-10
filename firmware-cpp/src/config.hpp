#pragma once
#include <Arduino.h>
#include <Preferences.h>

// Board settings only — pool/WiFi live on the PC companion.
struct AppConfig {
  uint8_t cpuMhz = 240;
  bool hashFocus = true;

  uint8_t normalizeCpu(uint8_t mhz) const {
    if (mhz == 0 || mhz >= 200) return 240;
    if (mhz <= 100) return 80;
    return 160;
  }
};

class ConfigStore {
 public:
  bool load(AppConfig& cfg);
  bool save(const AppConfig& cfg);

 private:
  Preferences prefs_;
};
