#include "display_ui.hpp"

void DisplayUi::begin() {
  cBg_ = to565(8, 12, 16);
  cPanel_ = to565(18, 26, 36);
  cLime_ = to565(180, 240, 90);
  cText_ = to565(228, 238, 248);
  cMuted_ = to565(130, 150, 170);

  tft_.init();
  tft_.setRotation(1);  // landscape 320x240
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);
  tft_.fillScreen(cBg_);
}

void DisplayUi::showSplash() {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 160, 100, 4);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("mining", 160, 140, 2);
}

void DisplayUi::showWaitingCompanion() {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 12, 16, 4);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString("Waiting for app", 12, 70, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("Plug USB and open CYD Companion", 12, 110, 2);
  tft_.drawString("Setup is done in the app only", 12, 140, 2);
}

void DisplayUi::showMining(const AppConfig& cfg, const MinerSnapshot& snap) {
  (void)cfg;
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 12, 14, 2);

  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("H/s", 12, 48, 2);
  tft_.setTextColor(cLime_, cBg_);
  char rate[24];
  snprintf(rate, sizeof(rate), "%.2f", snap.hashrateHs);
  tft_.drawString(rate, 12, 72, 4);

  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("pool", 12, 130, 2);
  tft_.setTextColor(snap.connected ? cLime_ : cText_, cBg_);
  tft_.drawString(snap.connected ? "CONNECTED" : snap.pool, 12, 152, 2);

  char line[48];
  snprintf(line, sizeof(line), "a%u  r%u  %u MHz", snap.accepted, snap.rejected, snap.cpuMhz);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString(line, 12, 190, 2);

  tft_.setTextColor(cMuted_, cBg_);
  String wifiIp = snap.wifi + "  " + snap.ip;
  tft_.drawString(wifiIp, 12, 218, 2);
}

void DisplayUi::showMessage(const char* title, const char* detail) {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 4, cLime_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString(title, 160, 100, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString(detail, 160, 140, 2);
}
