#include "config.hpp"

bool ConfigStore::load(AppConfig& cfg) {
  if (!prefs_.begin("cydminer", true)) return false;
  cfg.cpuMhz = cfg.normalizeCpu(prefs_.getUChar("cpu_mhz", 240));
  cfg.hashFocus = prefs_.getBool("hash_focus", true);
  prefs_.end();
  return true;
}

bool ConfigStore::save(const AppConfig& cfg) {
  if (!prefs_.begin("cydminer", false)) return false;
  prefs_.putUChar("cpu_mhz", cfg.normalizeCpu(cfg.cpuMhz));
  prefs_.putBool("hash_focus", cfg.hashFocus);
  prefs_.end();
  return true;
}
