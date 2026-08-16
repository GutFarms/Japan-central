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
  /// Preferred SHA path after Bench: -1=legacy auto, 0=Full HW, 1=HW+, 2=HW/SW.
  /// Default Full HW — always correct; Bench can lock a faster path into NVS.
  int8_t shaPath = 0;

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
