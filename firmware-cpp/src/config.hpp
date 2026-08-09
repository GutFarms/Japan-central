#pragma once
#include <Arduino.h>
#include <Preferences.h>

struct AppConfig {
  String wifiSsid;
  String wifiPassword;
  String stratum;
  String worker;
  String password;  // pool password / companion auth
  uint8_t cpuMhz = 240;
  bool hashFocus = true;
  uint8_t touchMap = 1;

  bool isComplete() const {
    return wifiSsid.length() > 0 && stratum.length() > 0 && worker.length() > 0 &&
           password.length() > 0;
  }

  bool authorizeOrSetup(const String& auth) const {
    if (!isComplete()) return true;  // first setup: any auth ok
    return auth == password;
  }

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
  void clear();

 private:
  Preferences prefs_;
};
