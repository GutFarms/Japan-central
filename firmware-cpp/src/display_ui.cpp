#include "display_ui.hpp"

void DisplayUi::begin() {
  cBg_ = to565(8, 12, 16);
  cPanel_ = to565(18, 26, 36);
  cBubble_ = to565(56, 84, 118);
  cLime_ = to565(180, 240, 90);
  cText_ = to565(228, 238, 248);
  cMuted_ = to565(130, 150, 170);

  tft_.init();
  tft_.setRotation(1);  // landscape 320x240
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);
  tft_.fillScreen(cBg_);
}

void DisplayUi::roundPanel(int x, int y, int w, int h, uint16_t color) {
  tft_.fillRoundRect(x, y, w, h, 10, color);
}

void DisplayUi::showSplash() {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 8, cLime_);
  tft_.fillRect(0, 8, 320, 3, cBubble_);
  roundPanel(40, 70, 240, 90, cPanel_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cPanel_);
  tft_.drawString("SCRYPT", 160, 100, 4);
  tft_.setTextColor(cMuted_, cPanel_);
  tft_.drawString("ESP32-CYD  C++ companion", 160, 130, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("setup via CYD Companion", 160, 220, 2);
}

void DisplayUi::showWaitingCompanion() {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 8, cLime_);
  tft_.fillRect(0, 8, 320, 3, cBubble_);
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 10, 14, 2);
  roundPanel(16, 56, 288, 140, cPanel_);
  tft_.setTextColor(cLime_, cPanel_);
  tft_.drawString("Waiting for companion", 36, 78, 2);
  tft_.setTextColor(cText_, cPanel_);
  tft_.drawString("1. Plug USB (CH340)", 40, 110, 2);
  tft_.drawString("2. Open CYD Companion", 40, 134, 2);
  tft_.drawString("3. Setup -> Save & reboot", 40, 158, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("UART speaks CMP only — no typing here", 16, 220, 1);
}

void DisplayUi::showMining(const AppConfig& cfg, const MinerSnapshot& snap) {
  tft_.fillScreen(cBg_);
  tft_.fillRect(0, 0, 320, 3, cLime_);
  tft_.fillRect(0, 3, 3, 237, cBubble_);
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("SCRYPT", 10, 12, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString(snap.wifi, 200, 14, 2);

  roundPanel(8, 40, 200, 100, cPanel_);
  roundPanel(216, 40, 96, 100, cPanel_);
  roundPanel(8, 150, 304, 56, cPanel_);

  tft_.setTextColor(cMuted_, cPanel_);
  tft_.drawString("H/s active", 16, 50, 2);
  tft_.setTextColor(snap.connected ? cLime_ : cText_, cPanel_);
  char rate[24];
  snprintf(rate, sizeof(rate), "%.2f", snap.hashrateHs);
  tft_.drawString(rate, 16, 78, 4);

  tft_.setTextColor(cText_, cPanel_);
  tft_.drawString(snap.connected ? "ON" : snap.pool, 236, 70, 2);
  char ar[24];
  snprintf(ar, sizeof(ar), "a%u/r%u", snap.accepted, snap.rejected);
  tft_.drawString(ar, 228, 100, 2);

  tft_.setTextColor(cMuted_, cPanel_);
  String line1 = snap.pool + "  " + String(snap.cpuMhz) + "MHz";
  tft_.drawString(line1, 16, 162, 2);
  String line2 = snap.ip;
  if (cfg.worker.length()) line2 += "  " + cfg.worker.substring(0, 16);
  tft_.drawString(line2, 16, 186, 2);

  tft_.fillRect(0, 226, 320, 14, cPanel_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cText_, cPanel_);
  char foot[32];
  snprintf(foot, sizeof(foot), "%.2f H/s", snap.hashrateHs);
  tft_.drawString(foot, 160, 233, 2);
}

void DisplayUi::showMessage(const char* title, const char* detail) {
  tft_.fillScreen(cBg_);
  roundPanel(20, 70, 280, 100, cPanel_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cPanel_);
  tft_.drawString(title, 160, 100, 2);
  tft_.setTextColor(cMuted_, cPanel_);
  tft_.drawString(detail, 160, 130, 2);
}
