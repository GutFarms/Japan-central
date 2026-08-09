#pragma once
#include "config.hpp"
#include "companion.hpp"
#include <TFT_eSPI.h>

// Match CYD Companion: deep blue-black + lime.
class DisplayUi {
 public:
  void begin();
  void showSplash();
  void showWaitingCompanion();
  void showMining(const AppConfig& cfg, const MinerSnapshot& snap);
  void showMessage(const char* title, const char* detail);

 private:
  TFT_eSPI tft_;
  uint16_t to565(uint8_t r, uint8_t g, uint8_t b) const {
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
  }
  uint16_t cBg_ = 0, cPanel_ = 0, cBubble_ = 0, cLime_ = 0, cText_ = 0, cMuted_ = 0;
  void roundPanel(int x, int y, int w, int h, uint16_t color);
};
