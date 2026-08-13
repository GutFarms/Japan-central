#pragma once
#include "config.hpp"
#include "companion.hpp"
#include <TFT_eSPI.h>

// Full-screen logo chrome with link status, hashrate, and Wi‑Fi IP only.
class DisplayUi {
 public:
  void begin();
  void showSplash();
  void showWaitingCompanion(const MinerSnapshot& snap);
  void showMining(const AppConfig& cfg, const MinerSnapshot& snap, bool forceFull = false);
  void showMessage(const char* title, const char* detail);
  bool miningChromeDrawn() const { return miningDrawn_; }

 private:
  TFT_eSPI tft_;
  uint16_t to565(uint8_t r, uint8_t g, uint8_t b) const {
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
  }
  uint16_t cBg_ = 0, cPanel_ = 0, cLime_ = 0, cLimeDim_ = 0, cText_ = 0, cMuted_ = 0, cWarn_ = 0,
           cErr_ = 0;
  float lastRate_ = -1;
  bool lastConnected_ = false;
  bool lastMining_ = false;
  String lastWifiIp_;
  String lastWifiMode_;
  bool miningDrawn_ = false;
  uint8_t lastAnim_ = 0xFF;

  void drawLogoFullscreen();
  void drawStatusStrip();
  void paintLinkRateIp(const MinerSnapshot& snap, bool forceFull);
  static bool wifiIpVisible(const MinerSnapshot& snap);
};
