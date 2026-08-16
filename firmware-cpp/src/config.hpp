#pragma once
#include <Arduino.h>
#include <Preferences.h>

// Board settings — SoftAP Wi‑Fi is on by default; optional STA + pool for independent mining.
struct AppConfig {
  uint8_t cpuMhz = 240;
  bool hashFocus = true;
  bool wifiEnabled = true;
  String wifiSsid;
  String wifiPass;
  /// Preferred SHA path after Bench: -1=legacy auto, 0=Full HW, 1=HW+, 2=HW/SW.
  /// Default Full HW — always correct; Bench can lock a faster path into NVS.
  int8_t shaPath = 0;
  /// When true and STA is up with pool_url set, board mines to the pool itself.
  bool mineIndep = true;
  /// stratum+tcp://host:port or host:port (cleartext only).
  String poolUrl;
  String poolWorker;
  String poolPass;
  /// True after a D0 Bench auto-tune has locked shaPath (skip re-tune every boot).
  bool pathTuned = false;

  uint8_t normalizeCpu(uint8_t mhz) const {
    if (mhz == 0 || mhz >= 200) return 240;
    if (mhz <= 100) return 80;
    return 160;
  }

  bool poolConfigured() const {
    return poolUrl.length() > 0 && poolWorker.length() > 0;
  }
};

class ConfigStore {
 public:
  bool load(AppConfig& cfg);
  bool save(const AppConfig& cfg);

 private:
  Preferences prefs_;
};
