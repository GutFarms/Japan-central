#pragma once
#include "config.hpp"
#include "companion.hpp"
#include <TFT_eSPI.h>

// Mining-only screen: brand + live stats. All control is via CYD Companion.
class DisplayUi {
 public:
  void begin();
  void showSplash();
  void showWaitingCompanion();
  void showMining(const AppConfig& cfg, const MinerSnapshot& snap, bool forceFull = false);
  void showMessage(const char* title, const char* detail);

 private:
  TFT_eSPI tft_;
  uint16_t to565(uint8_t r, uint8_t g, uint8_t b) const {
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
  }
  uint16_t cBg_ = 0, cLime_ = 0, cText_ = 0, cMuted_ = 0;
  // Cached paint fields — skip full redraw when unchanged.
  float lastRate_ = -1;
  uint32_t lastAccepted_ = 0xFFFFFFFFu;
  uint32_t lastRejected_ = 0xFFFFFFFFu;
  uint32_t lastMhz_ = 0xFFFFFFFFu;
  bool lastConnected_ = false;
  String lastPool_;
  String lastWifiIp_;
  bool miningDrawn_ = false;

  void drawMiningChrome();
};
