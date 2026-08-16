#include "display_ui.hpp"
#include "logo_rgb565.h"
#include <cstdio>
#include <cstring>

void DisplayUi::begin() {
  cBg_ = to565(5, 8, 10);
  cPanel_ = to565(6, 14, 22);
  cLime_ = to565(126, 220, 255);
  cLimeDim_ = to565(64, 160, 220);
  cText_ = to565(230, 242, 252);
  cMuted_ = to565(120, 158, 186);
  cWarn_ = to565(255, 196, 91);
  cErr_ = to565(255, 108, 91);

  tft_.init();
  tft_.setRotation(1);  // landscape 320x240
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);
  tft_.fillScreen(cBg_);
  miningDrawn_ = false;
}

bool DisplayUi::wifiIpVisible(const MinerSnapshot& snap) {
  if (snap.wifiIp.isEmpty() || snap.wifiIp == "0.0.0.0") return false;
  // Show IP whenever SoftAP / STA is up (companion may link over Wi‑Fi).
  return snap.wifiMode == "ap" || snap.wifiMode == "sta" || snap.wifiMode == "apsta";
}

void DisplayUi::drawLogoFullscreen() {
  // Logo fills the upper display; status strip owns the bottom band.
  tft_.fillScreen(cBg_);
  const int x = (320 - LOGO_W) / 2;
  const int y = (188 - LOGO_H) / 2;
  tft_.setSwapBytes(true);
  tft_.pushImage(x, y < 0 ? 0 : y, LOGO_W, LOGO_H, LOGO_RGB565);
  tft_.setSwapBytes(false);
}

void DisplayUi::drawStatusStrip() {
  // Opaque bottom strip so rate/link text stays readable over the logo.
  tft_.fillRect(0, 188, 320, 52, cPanel_);
  tft_.fillRect(0, 187, 320, 1, cLimeDim_);
}

void DisplayUi::showSplash() {
  miningDrawn_ = false;
  lastAnim_ = 0xFF;
  drawLogoFullscreen();
  drawStatusStrip();
  tft_.setTextDatum(TC_DATUM);
  tft_.setTextColor(cLime_, cPanel_);
  tft_.drawString("Njordr Seas'", 160, 196, 2);
  tft_.setTextColor(cMuted_, cPanel_);
  tft_.drawString("CYD miner", 160, 218, 1);
}

void DisplayUi::showWaitingCompanion(const MinerSnapshot& snap) {
  const bool chromeDirty = miningDrawn_ || lastAnim_ == 0xFF;
  miningDrawn_ = false;
  if (chromeDirty) {
    drawLogoFullscreen();
    drawStatusStrip();
    lastAnim_ = 0xFE;
    lastRate_ = -1;
    lastConnected_ = false;
    lastMining_ = false;
    lastWifiIp_ = "";
    lastWifiMode_ = "";
  }
  // Still refresh link / IP while idle (SoftAP comes up during wait).
  paintLinkRateIp(snap, chromeDirty);
}

void DisplayUi::paintLinkRateIp(const MinerSnapshot& snap, bool forceFull) {
  const bool hashing = snap.mining || snap.hashrateHs > 0.0f;
  const bool linked = snap.connected || hashing;
  const bool showIp = wifiIpVisible(snap);

  char link[28];
  if (hashing) {
    if (showIp) {
      snprintf(link, sizeof(link), "LINK  HASH · WiFi");
    } else {
      snprintf(link, sizeof(link), "LINK  HASH · USB");
    }
  } else if (linked) {
    snprintf(link, sizeof(link), "LINK  OK");
  } else if (showIp) {
    // SoftAP-only: nudge phone users to the captive HTTP setup portal.
    if (snap.wifiMode == "ap") {
      snprintf(link, sizeof(link), "PHONE SETUP");
    } else {
      snprintf(link, sizeof(link), "LINK  WiFi");
    }
  } else {
    snprintf(link, sizeof(link), "LINK  WAIT");
  }

  const bool linkDirty =
      forceFull || linked != lastConnected_ || hashing != lastMining_ || snap.wifiMode != lastWifiMode_;
  if (linkDirty) {
    tft_.fillRect(8, 192, 304, 20, cPanel_);
    tft_.setTextDatum(TL_DATUM);
    tft_.setTextColor(hashing || linked ? cLime_ : cWarn_, cPanel_);
    tft_.drawString(link, 12, 194, 2);
    lastConnected_ = linked;
    lastMining_ = hashing;
    lastWifiMode_ = snap.wifiMode;
  }

  const float rateDelta = snap.hashrateHs > lastRate_ ? snap.hashrateHs - lastRate_
                                                      : lastRate_ - snap.hashrateHs;
  // Refresh often — 800 H/s was so coarse the strip looked stuck/blank at CYD rates.
  const float rateThresh =
      (lastRate_ < 1000.0f) ? 25.0f : (lastRate_ * 0.02f < 200.0f ? 200.0f : lastRate_ * 0.02f);
  const bool rateDirty = forceFull || lastRate_ < 0.0f || rateDelta >= rateThresh ||
                         (hashing && lastRate_ <= 0.0f && snap.hashrateHs > 0.0f);
  if (rateDirty) {
    tft_.fillRect(8, 214, 150, 18, cPanel_);
    char rate[28];
    float hs = snap.hashrateHs;
    if (!hashing && hs <= 0.0f) {
      snprintf(rate, sizeof(rate), "—");
    } else if (hs < 1000.0f) {
      snprintf(rate, sizeof(rate), "%.0f H/s", hs);
    } else if (hs < 1000000.0f) {
      float khs = hs / 1000.0f;
      if (khs >= 100.0f)
        snprintf(rate, sizeof(rate), "%.0f kH/s", khs);
      else if (khs >= 10.0f)
        snprintf(rate, sizeof(rate), "%.1f kH/s", khs);
      else
        snprintf(rate, sizeof(rate), "%.2f kH/s", khs);
    } else {
      snprintf(rate, sizeof(rate), "%.2f MH/s", hs / 1000000.0f);
    }
    tft_.setTextDatum(TL_DATUM);
    tft_.setTextColor(cText_, cPanel_);
    tft_.drawString(rate, 12, 216, 2);
    lastRate_ = snap.hashrateHs;
  }

  if (forceFull || snap.wifiIp != lastWifiIp_ || showIp != !lastWifiIp_.isEmpty()) {
    tft_.fillRect(160, 214, 152, 18, cPanel_);
    if (showIp) {
      tft_.setTextDatum(TR_DATUM);
      tft_.setTextColor(cLime_, cPanel_);
      tft_.drawString(snap.wifiIp, 308, 216, 2);
      lastWifiIp_ = snap.wifiIp;
    } else {
      lastWifiIp_ = "";
    }
  }
}

void DisplayUi::showMining(const AppConfig& cfg, const MinerSnapshot& snap, bool forceFull) {
  (void)cfg;
  if (!miningDrawn_ || forceFull) {
    drawLogoFullscreen();
    drawStatusStrip();
    miningDrawn_ = true;
    lastAnim_ = 1;
    lastRate_ = -1;
    lastConnected_ = false;
    lastMining_ = false;
    lastWifiIp_ = "";
    lastWifiMode_ = "";
    forceFull = true;
  }
  paintLinkRateIp(snap, forceFull);
}

void DisplayUi::showMessage(const char* title, const char* detail) {
  miningDrawn_ = false;
  lastAnim_ = 0xFF;
  drawLogoFullscreen();
  drawStatusStrip();
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cPanel_);
  tft_.drawString(title, 12, 194, 2);
  tft_.setTextColor(cMuted_, cPanel_);
  tft_.drawString(detail, 12, 218, 1);
}
