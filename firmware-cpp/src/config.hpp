#pragma once
#include <Arduino.h>
#include <Preferences.h>

// Board settings — SoftAP Wi‑Fi is on by default; optional STA credentials.
struct AppConfig {
  uint8_t cpuMhz = 240;
  bool hashFocus = true;
  bool wifiEnabled = true;
  String wifiSsid;
  String wifiPass;
  /// Preferred SHA path after Bench auto-tune: -1=auto, 0=HW, 1=HW+, 2=HW/SW.
  int8_t shaPath = -1;

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
