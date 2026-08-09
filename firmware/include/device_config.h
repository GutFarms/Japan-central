#pragma once

#include <Arduino.h>
#include <TFT_eSPI.h>

// Runtime settings controllable from the host app (persisted in NVS).
struct DeviceConfig {
  uint8_t rotation = 1;     // TFT_eSPI rotation: 1 = landscape, 3 = flipped landscape
  uint8_t brightness = 220; // 0-255 backlight PWM
  bool flip = false;        // convenience mirror of rotation==3
};

class DeviceSettings {
 public:
  void begin();
  void load();
  void save() const;
  void apply(TFT_eSPI &tft);

  bool applyCommandJson(const char *json, size_t len, TFT_eSPI &tft);
  void toJson(char *out, size_t outLen) const;

  DeviceConfig &cfg() { return cfg_; }
  const DeviceConfig &cfg() const { return cfg_; }

  // Toggle landscape ↔ flipped landscape.
  void flipScreen(TFT_eSPI &tft);

 private:
  DeviceConfig cfg_;
  void applyBrightness() const;
  void setRotationValue(uint8_t rot);
};
