#include "config.hpp"

bool ConfigStore::load(AppConfig& cfg) {
  if (!prefs_.begin("cydminer", true)) return false;
  cfg.cpuMhz = cfg.normalizeCpu(prefs_.getUChar("cpu_mhz", 240));
  cfg.hashFocus = prefs_.getBool("hash_focus", true);
  cfg.wifiEnabled = prefs_.getBool("wifi_en", true);
  cfg.wifiSsid = prefs_.getString("wifi_ssid", "");
  cfg.wifiPass = prefs_.getString("wifi_pass", "");
  const int raw_path = prefs_.getInt("sha_path", 0);
  const bool path_migrated = prefs_.getBool("path_v87", false);
  cfg.shaPath = (int8_t)((raw_path < 0) ? 0 : raw_path);
  cfg.mineIndep = prefs_.getBool("mine_indep", true);
  cfg.poolUrl = prefs_.getString("pool_url", "");
  cfg.poolWorker = prefs_.getString("pool_user", "");
  cfg.poolPass = prefs_.getString("pool_pass", "x");
  cfg.pathTuned = prefs_.getBool("path_tuned", false);
  prefs_.end();
  // 0.8.87: one-shot reset to Full HW. Prior auto-bench could lock Mid/HW-SW
  // paths that look fast but yield few valid pool shares; Bench can re-lock.
  if (!path_migrated || raw_path < 0) {
    cfg.shaPath = 0;
    if (!prefs_.begin("cydminer", false)) return true;
    prefs_.putInt("sha_path", 0);
    prefs_.putBool("path_v87", true);
    prefs_.end();
  }
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
  prefs_.putBool("mine_indep", cfg.mineIndep);
  prefs_.putString("pool_url", cfg.poolUrl);
  prefs_.putString("pool_user", cfg.poolWorker);
  prefs_.putString("pool_pass", cfg.poolPass);
  prefs_.putBool("path_tuned", cfg.pathTuned);
  prefs_.end();
  return true;
}
