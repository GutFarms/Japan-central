#include "display_ui.hpp"
#include <cstdio>
#include <cstring>

void DisplayUi::begin() {
  cBg_ = to565(5, 8, 10);
  cPanel_ = to565(14, 22, 24);
  cLime_ = to565(198, 255, 64);
  cLimeDim_ = to565(130, 210, 52);
  cText_ = to565(235, 244, 238);
  cMuted_ = to565(134, 154, 148);
  cWarn_ = to565(255, 196, 91);
  cErr_ = to565(255, 108, 91);
  cInk_ = to565(8, 14, 12);

  tft_.init();
  tft_.setRotation(1);  // landscape 320x240
  pinMode(TFT_BL, OUTPUT);
  digitalWrite(TFT_BL, HIGH);
  tft_.fillScreen(cBg_);
  miningDrawn_ = false;
}

void DisplayUi::drawTopRule(bool live) {
  tft_.fillRect(0, 0, 320, 3, cLime_);
  tft_.fillRect(0, 3, 320, 1, live ? cLimeDim_ : cBg_);
}

void DisplayUi::showSplash() {
  miningDrawn_ = false;
  tft_.fillScreen(cBg_);
  drawTopRule(false);

  tft_.fillRect(0, 24, 6, 192, cLime_);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 24, 48, 4);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("Companion", 24, 100, 2);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString("SHA-256", 24, 132, 4);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("USB miner · hash focus", 24, 188, 2);
}

void DisplayUi::drawWaitAnim(uint8_t /*frame*/) {
  // Animations removed — SPI bars stole core-0 hash time.
}

void DisplayUi::showWaitingCompanion() {
  const bool chromeDirty = miningDrawn_ || lastAnim_ == 0xFF;
  miningDrawn_ = false;

  if (!chromeDirty) return;

  tft_.fillScreen(cBg_);
  drawTopRule(false);
  tft_.fillRect(0, 24, 6, 192, cLime_);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 24, 20, 4);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("Companion", 24, 68, 2);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString("Waiting for USB", 24, 100, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("Open Companion · Connect", 24, 132, 2);
  tft_.drawString("Board hashes only", 24, 156, 2);
  lastAnim_ = 0xFE;
}

void DisplayUi::drawActivityBar(float /*khs*/, bool /*hashing*/, uint8_t /*frame*/) {
  // Disabled — 18 SPI rects per frame crushed SW assist H/s.
}

void DisplayUi::drawMiningChrome() {
  tft_.fillScreen(cBg_);
  drawTopRule(true);
  tft_.fillRect(0, 24, 6, 188, cLime_);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 24, 12, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("hash focus · SHA-256", 70, 16, 1);

  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("ACCEPT", 24, 150, 1);
  tft_.drawString("REJECT", 120, 150, 1);
  tft_.drawString("CLOCK", 220, 150, 1);

  // Static footer — no scrolling ticker strip.
  tft_.fillRect(0, 220, 320, 20, cPanel_);
  tft_.setTextColor(cLime_, cPanel_);
  tft_.drawString("usb · hashing", 8, 224, 1);

  miningDrawn_ = true;
  lastRate_ = -1;
  lastAccepted_ = 0xFFFFFFFFu;
  lastRejected_ = 0xFFFFFFFFu;
  lastMhz_ = 0xFFFFFFFFu;
  lastNonce_ = 0xFFFFFFFFu;
  lastHashes_ = 0;
  lastConnected_ = false;
  lastPool_ = "";
  lastTicker_ = "";
  lastJob_ = "";
  lastAnim_ = 0xFF;
}

void DisplayUi::showMining(const AppConfig& cfg, const MinerSnapshot& snap, bool forceFull) {
  (void)cfg;
  if (!miningDrawn_ || forceFull) drawMiningChrome();

  bool hashing = snap.connected || snap.hashrateHs > 0.0f;
  const float rateDelta = snap.hashrateHs > lastRate_ ? snap.hashrateHs - lastRate_
                                                      : lastRate_ - snap.hashrateHs;
  // Redraw rate only on meaningful change — avoid SPI digit flicker.
  const bool rateDirty = forceFull || rateDelta >= 1500.0f;

  tft_.setTextDatum(TL_DATUM);

  // Static live pip (no blink).
  if (forceFull || lastAnim_ != 1) {
    tft_.fillCircle(300, 20, 4, hashing ? cLime_ : cMuted_);
    lastAnim_ = 1;
  }

  if (rateDirty) {
    tft_.fillRect(24, 40, 260, 56, cBg_);
    tft_.setTextColor(cText_, cBg_);
    char rate[24];
    char unit[8];
    float hs = snap.hashrateHs;
    if (hs < 1000.0f) {
      snprintf(rate, sizeof(rate), "%.0f", hs);
      snprintf(unit, sizeof(unit), "H/s");
    } else if (hs < 1000000.0f) {
      float khs = hs / 1000.0f;
      if (khs >= 100.0f)
        snprintf(rate, sizeof(rate), "%.0f", khs);
      else if (khs >= 10.0f)
        snprintf(rate, sizeof(rate), "%.1f", khs);
      else
        snprintf(rate, sizeof(rate), "%.2f", khs);
      snprintf(unit, sizeof(unit), "kH/s");
    } else {
      float mhs = hs / 1000000.0f;
      if (mhs >= 100.0f)
        snprintf(rate, sizeof(rate), "%.0f", mhs);
      else if (mhs >= 10.0f)
        snprintf(rate, sizeof(rate), "%.1f", mhs);
      else
        snprintf(rate, sizeof(rate), "%.2f", mhs);
      snprintf(unit, sizeof(unit), "MH/s");
    }
    tft_.drawString(rate, 24, 40, 4);
    tft_.setTextColor(cLime_, cBg_);
    tft_.drawString(unit, 180, 62, 2);
    lastRate_ = snap.hashrateHs;
  }

  if (forceFull || snap.connected != lastConnected_ || snap.pool != lastPool_ ||
      snap.jobId != lastJob_) {
    tft_.fillRect(24, 112, 280, 24, cBg_);
    tft_.setTextColor(hashing ? cLime_ : cWarn_, cBg_);
    tft_.drawString(hashing ? "HASHING" : "WAIT JOB", 24, 112, 1);
    tft_.setTextColor(cMuted_, cBg_);
    String job = snap.jobId.length() ? snap.jobId : String("—");
    if (job.length() > 16) job = job.substring(0, 16);
    tft_.drawString(job, 100, 112, 1);
    lastConnected_ = snap.connected;
    lastPool_ = snap.pool;
    lastJob_ = snap.jobId;
  }

  if (forceFull || snap.accepted != lastAccepted_ || snap.rejected != lastRejected_ ||
      snap.cpuMhz != lastMhz_) {
    tft_.fillRect(24, 164, 280, 28, cBg_);
    char a[12], r[12], m[16];
    snprintf(a, sizeof(a), "%u", snap.accepted);
    snprintf(r, sizeof(r), "%u", snap.rejected);
    snprintf(m, sizeof(m), "%u MHz", snap.cpuMhz);
    tft_.setTextColor(cLime_, cBg_);
    tft_.drawString(a, 24, 164, 2);
    tft_.setTextColor(snap.rejected ? cErr_ : cText_, cBg_);
    tft_.drawString(r, 120, 164, 2);
    tft_.setTextColor(cText_, cBg_);
    tft_.drawString(m, 220, 164, 2);
    lastAccepted_ = snap.accepted;
    lastRejected_ = snap.rejected;
    lastMhz_ = snap.cpuMhz;
  }

  if (forceFull || snap.nonce != lastNonce_ || snap.totalHashes != lastHashes_) {
    tft_.fillRect(24, 196, 280, 16, cBg_);
    char hashes[24];
    uint64_t n = snap.totalHashes;
    if (n < 1000ull) {
      snprintf(hashes, sizeof(hashes), "%llu H", (unsigned long long)n);
    } else if (n < 1000000ull) {
      snprintf(hashes, sizeof(hashes), "%.1f kH", (double)n / 1000.0);
    } else if (n < 1000000000ull) {
      snprintf(hashes, sizeof(hashes), "%.2f MH", (double)n / 1000000.0);
    } else {
      snprintf(hashes, sizeof(hashes), "%.2f GH", (double)n / 1000000000.0);
    }
    char line[48];
    snprintf(line, sizeof(line), "%s · %08lx", hashes, (unsigned long)snap.nonce);
    tft_.setTextColor(cMuted_, cBg_);
    tft_.drawString(line, 24, 198, 1);
    lastNonce_ = snap.nonce;
    lastHashes_ = snap.totalHashes;
  }
}

void DisplayUi::showMessage(const char* title, const char* detail) {
  miningDrawn_ = false;
  lastAnim_ = 0xFF;
  tft_.fillScreen(cBg_);
  drawTopRule(false);
  tft_.fillRect(0, 24, 6, 192, cLime_);
  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 24, 48, 4);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString(title, 24, 110, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString(detail, 24, 146, 2);
}
