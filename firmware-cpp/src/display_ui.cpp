#include "display_ui.hpp"
#include <cstdio>

void DisplayUi::begin() {
  cBg_ = to565(8, 12, 16);
  cLime_ = to565(180, 240, 90);
  cText_ = to565(228, 238, 248);
  cMuted_ = to565(130, 150, 170);

  tft_.init();
  tft_.setRotation(1);  // landscape 320x240
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);
  tft_.fillScreen(cBg_);
  miningDrawn_ = false;
}

void DisplayUi::showSplash() {
  miningDrawn_ = false;
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 160, 100, 4);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("usb hash", 160, 140, 2);
}

void DisplayUi::showWaitingCompanion() {
  miningDrawn_ = false;
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 12, 16, 4);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString("Waiting for USB", 12, 70, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("Plug USB-C · open CYD Companion", 12, 110, 2);
  tft_.drawString("Pool traffic stays on the PC", 12, 140, 2);
}

void DisplayUi::drawMiningChrome() {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 12, 14, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("H/s", 12, 48, 2);
  tft_.drawString("usb", 12, 130, 2);
  miningDrawn_ = true;
  lastRate_ = -1;
  lastAccepted_ = 0xFFFFFFFFu;
  lastRejected_ = 0xFFFFFFFFu;
  lastMhz_ = 0xFFFFFFFFu;
  lastConnected_ = false;
  lastPool_ = "";
  lastTicker_ = "";
}

void DisplayUi::showMining(const AppConfig& cfg, const MinerSnapshot& snap, bool forceFull) {
  (void)cfg;
  if (!miningDrawn_ || forceFull) drawMiningChrome();

  tft_.setTextDatum(TL_DATUM);

  if (forceFull || snap.hashrateHs != lastRate_) {
    tft_.fillRect(12, 72, 300, 40, cBg_);
    tft_.setTextColor(cLime_, cBg_);
    char rate[24];
    // Scrypt on ESP32 is ~H/s — show H/s so activity is obvious.
    snprintf(rate, sizeof(rate), "%.2f", snap.hashrateHs);
    tft_.drawString(rate, 12, 72, 4);
    lastRate_ = snap.hashrateHs;
  }

  if (forceFull || snap.connected != lastConnected_ || snap.pool != lastPool_) {
    tft_.fillRect(12, 152, 300, 24, cBg_);
    tft_.setTextColor(snap.connected ? cLime_ : cText_, cBg_);
    tft_.drawString(snap.connected ? "USB HASHING" : snap.pool, 12, 152, 2);
    lastConnected_ = snap.connected;
    lastPool_ = snap.pool;
  }

  if (forceFull || snap.accepted != lastAccepted_ || snap.rejected != lastRejected_ ||
      snap.cpuMhz != lastMhz_) {
    tft_.fillRect(12, 190, 300, 22, cBg_);
    char line[48];
    snprintf(line, sizeof(line), "a%u  r%u  %u MHz", snap.accepted, snap.rejected, snap.cpuMhz);
    tft_.setTextColor(cText_, cBg_);
    tft_.drawString(line, 12, 190, 2);
    lastAccepted_ = snap.accepted;
    lastRejected_ = snap.rejected;
    lastMhz_ = snap.cpuMhz;
  }

  tft_.fillRect(12, 214, 300, 12, cBg_);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("link: USB-C only · no WiFi", 12, 212, 1);

  if (forceFull || snap.netTicker != lastTicker_) {
    tft_.fillRect(0, 226, 320, 14, cBg_);
    tft_.fillRect(0, 226, 320, 14, to565(14, 22, 32));
    tft_.setTextDatum(TL_DATUM);
    tft_.setTextColor(cLime_, to565(14, 22, 32));
    String tick = snap.netTicker.length() ? snap.netTicker : String("usb: companion linked");
    if (tick.length() > 40) tick = tick.substring(0, 40);
    tft_.drawString(tick, 6, 228, 1);
    lastTicker_ = snap.netTicker;
  }
}

void DisplayUi::showMessage(const char* title, const char* detail) {
  miningDrawn_ = false;
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString(title, 160, 100, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString(detail, 160, 140, 2);
}
