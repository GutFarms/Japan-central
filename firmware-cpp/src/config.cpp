#include "config.hpp"

bool ConfigStore::load(AppConfig& cfg) {
  if (!prefs_.begin("cydminer", true)) return false;
  cfg.wifiSsid = prefs_.getString("wifi_ssid", "");
  cfg.wifiPassword = prefs_.getString("wifi_pass", "");
  cfg.stratum = prefs_.getString("stratum", "");
  cfg.worker = prefs_.getString("worker", "");
  cfg.password = prefs_.getString("password", "");
  cfg.cpuMhz = cfg.normalizeCpu(prefs_.getUChar("cpu_mhz", 240));
  cfg.hashFocus = prefs_.getBool("hash_focus", true);
  cfg.touchMap = prefs_.getUChar("touch_map", 1);
  prefs_.end();
  return cfg.isComplete();
}

bool ConfigStore::save(const AppConfig& cfg) {
  if (!cfg.isComplete()) return false;
  if (!prefs_.begin("cydminer", false)) return false;
  prefs_.putString("wifi_ssid", cfg.wifiSsid);
  prefs_.putString("wifi_pass", cfg.wifiPassword);
  prefs_.putString("stratum", cfg.stratum);
  prefs_.putString("worker", cfg.worker);
  prefs_.putString("password", cfg.password);
  prefs_.putUChar("cpu_mhz", cfg.normalizeCpu(cfg.cpuMhz));
  prefs_.putBool("hash_focus", cfg.hashFocus);
  prefs_.putUChar("touch_map", cfg.touchMap);
  prefs_.end();
  return true;
}

void ConfigStore::clear() {
  if (prefs_.begin("cydminer", false)) {
    prefs_.clear();
    prefs_.end();
  }
}
