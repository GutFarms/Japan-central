#pragma once
#include "config.hpp"
#include "companion.hpp"
#include <TFT_eSPI.h>

// Mining-only screen: brand + live stats. Control is via USB companion.
class DisplayUi {
 public:
  void begin();
  void showSplash();
  void showWaitingCompanion();
  void showMining(const AppConfig& cfg, const MinerSnapshot& snap, bool forceFull = false);
  void showMessage(const char* title, const char* detail);
  bool miningChromeDrawn() const { return miningDrawn_; }

 private:
  TFT_eSPI tft_;
  uint16_t to565(uint8_t r, uint8_t g, uint8_t b) const {
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3);
  }
  uint16_t cBg_ = 0, cPanel_ = 0, cLime_ = 0, cLimeDim_ = 0, cText_ = 0, cMuted_ = 0, cWarn_ = 0,
           cErr_ = 0, cInk_ = 0;
  float lastRate_ = -1;
  uint32_t lastAccepted_ = 0xFFFFFFFFu;
  uint32_t lastRejected_ = 0xFFFFFFFFu;
  uint32_t lastMhz_ = 0xFFFFFFFFu;
  uint32_t lastNonce_ = 0xFFFFFFFFu;
  uint64_t lastHashes_ = 0;
  bool lastConnected_ = false;
  String lastPool_;
  String lastTicker_;
  String lastJob_;
  bool miningDrawn_ = false;
  uint8_t lastAnim_ = 0xFF;

  void drawTopRule(bool live);
  void drawMiningChrome();
  void drawActivityBar(float khs, bool hashing, uint8_t frame);
  void drawWaitAnim(uint8_t frame);
};
