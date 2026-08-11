#include "device_config.h"

#include <Preferences.h>
#include <ArduinoJson.h>
#include <cstdio>
#include <cstring>

namespace {
Preferences prefs;
constexpr int BL_PIN = 21;
constexpr int BL_CHANNEL = 0;
constexpr int BL_FREQ = 5000;
constexpr int BL_RES = 8;
}  // namespace

void DeviceSettings::begin() {
  ledcSetup(BL_CHANNEL, BL_FREQ, BL_RES);
  ledcAttachPin(BL_PIN, BL_CHANNEL);
  load();
  applyBrightness();
}

void DeviceSettings::load() {
  if (!prefs.begin("cydmon", true)) {
    return;
  }
  cfg_.rotation = static_cast<uint8_t>(prefs.getUChar("rot", 1));
  cfg_.brightness = static_cast<uint8_t>(prefs.getUChar("bright", 220));
  prefs.end();
  if (cfg_.rotation != 1 && cfg_.rotation != 3) {
    cfg_.rotation = 1;
  }
  cfg_.flip = (cfg_.rotation == 3);
}

void DeviceSettings::save() const {
  if (!prefs.begin("cydmon", false)) {
    return;
  }
  prefs.putUChar("rot", cfg_.rotation);
  prefs.putUChar("bright", cfg_.brightness);
  prefs.end();
}

void DeviceSettings::setRotationValue(uint8_t rot) {
  cfg_.rotation = (rot == 3) ? 3 : 1;
  cfg_.flip = (cfg_.rotation == 3);
}

void DeviceSettings::applyBrightness() const {
  ledcWrite(BL_CHANNEL, cfg_.brightness);
}

void DeviceSettings::apply(TFT_eSPI &tft) {
  tft.setRotation(cfg_.rotation);
  applyBrightness();
}

void DeviceSettings::flipScreen(TFT_eSPI &tft) {
  setRotationValue(cfg_.rotation == 1 ? 3 : 1);
  apply(tft);
  save();
}

void DeviceSettings::toJson(char *out, size_t outLen) const {
  snprintf(out, outLen,
           "{\"ok\":1,\"cfg\":{\"rot\":%u,\"bright\":%u,\"flip\":%s}}",
           static_cast<unsigned>(cfg_.rotation),
           static_cast<unsigned>(cfg_.brightness),
           cfg_.flip ? "true" : "false");
}

bool DeviceSettings::applyCommandJson(const char *json, size_t len, TFT_eSPI &tft) {
  JsonDocument doc;
  if (deserializeJson(doc, json, len)) {
    return false;
  }

  const char *cmd = doc["cmd"] | "";
  if (cmd[0] == '\0') {
    return false;
  }

  bool changed = false;

  if (strcmp(cmd, "get") == 0) {
    return true;  // caller sends cfg JSON
  }

  if (strcmp(cmd, "flip") == 0) {
    flipScreen(tft);
    return true;
  }

  if (strcmp(cmd, "set") != 0) {
    return false;
  }

  if (doc["flip"].is<bool>() || doc["flip"].is<int>()) {
    const bool flip = doc["flip"];
    setRotationValue(flip ? 3 : 1);
    changed = true;
  }
  if (doc["rot"].is<int>()) {
    setRotationValue(static_cast<uint8_t>(doc["rot"].as<int>()));
    changed = true;
  }
  if (doc["bright"].is<int>()) {
    int b = doc["bright"].as<int>();
    if (b < 10) b = 10;
    if (b > 255) b = 255;
    cfg_.brightness = static_cast<uint8_t>(b);
    changed = true;
  }

  if (changed) {
    apply(tft);
    save();
  }
  return true;
}
