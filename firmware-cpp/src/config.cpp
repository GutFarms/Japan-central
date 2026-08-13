#include "config.hpp"

bool ConfigStore::load(AppConfig& cfg) {
  if (!prefs_.begin("cydminer", true)) return false;
  cfg.cpuMhz = cfg.normalizeCpu(prefs_.getUChar("cpu_mhz", 240));
  cfg.hashFocus = prefs_.getBool("hash_focus", true);
  cfg.wifiEnabled = prefs_.getBool("wifi_en", true);
  cfg.wifiSsid = prefs_.getString("wifi_ssid", "");
  cfg.wifiPass = prefs_.getString("wifi_pass", "");
  cfg.shaPath = (int8_t)prefs_.getInt("sha_path", -1);
  prefs_.end();
  return true;
}

bool ConfigStore::save(const AppConfig& cfg) {
  if (!prefs_.begin("cydminer", false)) return false;
  prefs_.putUChar("cpu_mhz", cfg.normalizeCpu(cfg.cpuMhz));
  prefs_.putBool("hash_focus", cfg.hashFocus);
  prefs_.putBool("wifi_en", cfg.wifiEnabled);
  prefs_.putString("wifi_ssid", cfg.wifiSsid);
  prefs_.putString("wifi_pass", cfg.wifiPass);
  prefs_.putInt("sha_path", (int)cfg.shaPath);
  prefs_.end();
  return true;
}
