#include "display_ui.hpp"
#include <cstdio>
#include <cstring>

void DisplayUi::begin() {
  // Match Companion palette (basic).
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
  tft_.drawString("USB miner · pool on PC", 24, 188, 2);
}

void DisplayUi::drawWaitAnim(uint8_t frame) {
  const int y = 196;
  tft_.fillRect(24, y, 272, 18, cBg_);
  // Simple Companion-style activity bars.
  for (int i = 0; i < 16; i++) {
    int h = 4 + ((frame + i * 3) % 8);
    int x = 24 + i * 17;
    uint16_t c = ((frame + i) % 5 < 3) ? cLime_ : cPanel_;
    tft_.fillRect(x, y + 14 - h, 10, h, c);
  }
}

void DisplayUi::showWaitingCompanion() {
  uint8_t frame = (uint8_t)((millis() / 160) % 40);
  const bool chromeDirty = miningDrawn_ || lastAnim_ == 0xFF;
  miningDrawn_ = false;

  if (chromeDirty) {
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

  if (frame != lastAnim_) {
    drawWaitAnim(frame);
    lastAnim_ = frame;
  }
}

void DisplayUi::drawActivityBar(float khs, bool hashing, uint8_t frame) {
  // Companion-like hash activity bars (not a filled pill).
  const int x0 = 24, y = 112, w = 272, h = 16;
  tft_.fillRect(x0, y, w, h, cBg_);
  const int n = 18;
  const int gap = 3;
  const int bar_w = (w - gap * (n - 1)) / n;
  float level = hashing ? (khs / 80.0f) : 0.08f;
  if (level < 0.12f && hashing) level = 0.12f;
  if (level > 1.0f) level = 1.0f;
  for (int i = 0; i < n; i++) {
    float breathe = 0.45f + 0.55f * (0.5f + 0.5f * ((float)((frame + i * 2) % 16) / 15.0f));
    float lh = hashing ? (level * breathe) : (0.08f + 0.04f * ((i + frame) % 5) / 4.0f);
    if (lh < 0.1f) lh = 0.1f;
    int bh = (int)(h * lh);
    int x = x0 + i * (bar_w + gap);
    tft_.fillRect(x, y + h - bh, bar_w, bh, hashing ? cLime_ : cPanel_);
  }
}

void DisplayUi::drawMiningChrome() {
  tft_.fillScreen(cBg_);
  drawTopRule(true);
  tft_.fillRect(0, 24, 6, 188, cLime_);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 24, 12, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("Companion · SHA-256", 70, 16, 1);

  // Flat metric row — no card wells (Companion aesthetic).
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("ACCEPT", 24, 150, 1);
  tft_.drawString("REJECT", 120, 150, 1);
  tft_.drawString("CLOCK", 220, 150, 1);

  tft_.fillRect(0, 220, 320, 20, cPanel_);
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

  uint8_t frame = (uint8_t)((millis() / 120) % 32);
  float khs = snap.hashrateHs / 1000.0f;
  bool hashing = snap.connected || snap.hashrateHs > 0.0f;
  const bool rateDirty = forceFull || snap.hashrateHs != lastRate_;
  const bool animDirty = forceFull || frame != lastAnim_;

  tft_.setTextDatum(TL_DATUM);

  if (animDirty) {
    uint16_t pip = hashing ? ((frame & 1) ? cLime_ : cLimeDim_) : cMuted_;
    tft_.fillCircle(300, 20, 4, pip);
  }

  if (rateDirty) {
    tft_.fillRect(24, 40, 260, 56, cBg_);
    tft_.setTextColor(cText_, cBg_);
    char rate[24];
    char unit[8];
    // Auto-scale H/s → kH/s → MH/s (same idea as Companion).
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
  }

  if (animDirty || rateDirty) {
    drawActivityBar(khs, hashing, frame);
  }

  if (forceFull || snap.connected != lastConnected_ || snap.pool != lastPool_ ||
      snap.jobId != lastJob_) {
    tft_.fillRect(24, 134, 280, 12, cBg_);
    tft_.setTextColor(hashing ? cLime_ : cWarn_, cBg_);
    tft_.drawString(hashing ? "HASHING" : "WAIT JOB", 24, 134, 1);
    tft_.setTextColor(cMuted_, cBg_);
    String job = snap.jobId.length() ? snap.jobId : String("—");
    if (job.length() > 16) job = job.substring(0, 16);
    tft_.drawString(job, 100, 134, 1);
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

  if (forceFull || snap.netTicker != lastTicker_ || animDirty) {
    tft_.fillRect(0, 220, 320, 20, cPanel_);
    tft_.setTextColor(cLime_, cPanel_);
    String tick = snap.netTicker.length() ? snap.netTicker : String("usb · companion linked");
    if (tick.length() > 40) {
      int off = (frame / 2) % (tick.length() - 39);
      tick = tick.substring(off, off + 40);
    }
    tft_.drawString(tick, 8, 224, 1);
    lastTicker_ = snap.netTicker;
  }

  lastRate_ = snap.hashrateHs;
  lastAnim_ = frame;
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
