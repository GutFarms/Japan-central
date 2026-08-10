#include "display_ui.hpp"
#include <cstdio>
#include <cstring>

void DisplayUi::begin() {
  cBg_ = to565(6, 10, 12);
  cPanel_ = to565(12, 20, 22);
  cLime_ = to565(198, 255, 64);
  cLimeDim_ = to565(110, 160, 48);
  cText_ = to565(232, 242, 236);
  cMuted_ = to565(120, 142, 136);
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
  if (live) {
    // Soft secondary glow strip under the rule.
    tft_.fillRect(0, 3, 320, 2, cLimeDim_);
  } else {
    tft_.fillRect(0, 3, 320, 2, cBg_);
  }
}

void DisplayUi::showSplash() {
  miningDrawn_ = false;
  tft_.fillScreen(cBg_);
  drawTopRule(false);

  // Left accent plane — brand presence, not a card.
  tft_.fillRect(0, 28, 8, 184, cLime_);
  tft_.fillRect(8, 28, 4, 184, cLimeDim_);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 28, 56, 4);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString("SHA-256", 28, 108, 4);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("USB Bitcoin miner", 28, 156, 2);
  tft_.drawString("pool lives on the PC", 28, 180, 2);
}

void DisplayUi::drawWaitAnim(uint8_t frame) {
  // Breathing USB activity dots + sweep bar.
  const int y = 188;
  tft_.fillRect(28, y, 260, 22, cBg_);
  for (int i = 0; i < 5; i++) {
    bool on = ((frame + i) % 5) < 3;
    uint16_t c = on ? cLime_ : cPanel_;
    tft_.fillRoundRect(28 + i * 22, y + 4, 14, 14, 3, c);
  }
  int sweep = (int)((frame % 20) * 12);
  tft_.fillRect(28, y + 20, 240, 2, cPanel_);
  tft_.fillRect(28 + sweep, y + 20, 36, 2, cLime_);
}

void DisplayUi::showWaitingCompanion() {
  uint8_t frame = (uint8_t)((millis() / 160) % 40);
  // Leaving mining mode requires a full redraw of the wait chrome.
  const bool chromeDirty = miningDrawn_ || lastAnim_ == 0xFF;
  miningDrawn_ = false;

  if (chromeDirty) {
    tft_.fillScreen(cBg_);
    drawTopRule(false);
    tft_.fillRect(0, 28, 8, 184, cLime_);
    tft_.fillRect(8, 28, 4, 184, cLimeDim_);

    tft_.setTextDatum(TL_DATUM);
    tft_.setTextColor(cLime_, cBg_);
    tft_.drawString("CYD", 28, 24, 4);
    tft_.setTextColor(cText_, cBg_);
    tft_.drawString("Waiting for USB", 28, 78, 2);
    tft_.setTextColor(cMuted_, cBg_);
    tft_.drawString("Plug USB-C · open Companion", 28, 112, 2);
    tft_.drawString("Bitcoin SHA-256 · pool on PC", 28, 136, 2);
    tft_.drawString("Board hashes only · no WiFi", 28, 160, 2);
    lastAnim_ = 0xFE;  // force anim paint below
  }

  if (frame != lastAnim_) {
    drawWaitAnim(frame);
    lastAnim_ = frame;
  }
}

void DisplayUi::drawActivityBar(float khs, bool hashing, uint8_t frame) {
  const int x = 12, y = 118, w = 296, h = 10;
  tft_.fillRoundRect(x, y, w, h, 3, cPanel_);
  float fill = hashing ? (khs / 80.0f) : 0.0f;  // ~80 kH/s full-ish for ESP32
  if (fill < 0.08f && hashing) fill = 0.08f + 0.04f * ((frame % 8) / 7.0f);
  if (fill > 1.0f) fill = 1.0f;
  int fw = (int)(fill * (w - 4));
  if (fw > 0) {
    // Sweep highlight inside the fill.
    tft_.fillRoundRect(x + 2, y + 2, fw, h - 4, 2, cLime_);
    int hi = (frame % 16) * (fw > 16 ? (fw - 16) / 15 : 0);
    tft_.fillRect(x + 2 + hi, y + 2, 10, h - 4, cText_);
  }
}

void DisplayUi::drawMiningChrome() {
  tft_.fillScreen(cBg_);
  drawTopRule(true);

  tft_.setTextDatum(TL_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 12, 10, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString("SHA-256", 68, 14, 1);
  tft_.drawString("kH/s", 12, 42, 1);

  // Metric wells (interaction-free layout blocks, not card chrome).
  tft_.fillRoundRect(12, 148, 94, 44, 4, cPanel_);
  tft_.fillRoundRect(113, 148, 94, 44, 4, cPanel_);
  tft_.fillRoundRect(214, 148, 94, 44, 4, cPanel_);
  tft_.setTextColor(cMuted_, cPanel_);
  tft_.drawString("ACCEPT", 20, 154, 1);
  tft_.drawString("REJECT", 121, 154, 1);
  tft_.drawString("CLOCK", 222, 154, 1);

  tft_.fillRect(0, 226, 320, 14, cPanel_);
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

  // Live pulse pip next to brand.
  if (animDirty) {
    uint16_t pip = hashing ? ((frame & 1) ? cLime_ : cLimeDim_) : cMuted_;
    tft_.fillCircle(300, 18, 5, pip);
    tft_.fillCircle(300, 18, 2, hashing ? cInk_ : cBg_);
  }

  if (rateDirty) {
    tft_.fillRect(12, 56, 250, 56, cBg_);
    tft_.setTextColor(cLime_, cBg_);
    char rate[24];
    snprintf(rate, sizeof(rate), "%.2f", khs);
    tft_.drawString(rate, 12, 56, 4);
    tft_.setTextColor(cMuted_, cBg_);
    tft_.drawString("kH/s", 168, 78, 2);
  }

  if (animDirty || rateDirty) {
    drawActivityBar(khs, hashing, frame);
  }

  if (forceFull || snap.connected != lastConnected_ || snap.pool != lastPool_ ||
      snap.jobId != lastJob_) {
    tft_.fillRect(12, 134, 296, 12, cBg_);
    tft_.setTextColor(hashing ? cLime_ : cWarn_, cBg_);
    char state[48];
    if (hashing) {
      snprintf(state, sizeof(state), "HASHING");
    } else {
      snprintf(state, sizeof(state), "WAIT JOB");
    }
    tft_.drawString(state, 12, 134, 1);
    tft_.setTextColor(cMuted_, cBg_);
    String job = snap.jobId.length() ? snap.jobId : String("—");
    if (job.length() > 18) job = job.substring(0, 18);
    tft_.drawString(job, 90, 134, 1);
    lastConnected_ = snap.connected;
    lastPool_ = snap.pool;
    lastJob_ = snap.jobId;
  }

  if (forceFull || snap.accepted != lastAccepted_ || snap.rejected != lastRejected_ ||
      snap.cpuMhz != lastMhz_) {
    tft_.fillRect(20, 168, 78, 20, cPanel_);
    tft_.fillRect(121, 168, 78, 20, cPanel_);
    tft_.fillRect(222, 168, 78, 20, cPanel_);
    char a[12], r[12], m[12];
    snprintf(a, sizeof(a), "%u", snap.accepted);
    snprintf(r, sizeof(r), "%u", snap.rejected);
    snprintf(m, sizeof(m), "%u", snap.cpuMhz);
    tft_.setTextColor(cLime_, cPanel_);
    tft_.drawString(a, 20, 168, 2);
    tft_.setTextColor(snap.rejected ? cErr_ : cText_, cPanel_);
    tft_.drawString(r, 121, 168, 2);
    tft_.setTextColor(cText_, cPanel_);
    tft_.drawString(m, 222, 168, 2);
    tft_.setTextColor(cMuted_, cPanel_);
    tft_.drawString("MHz", 250, 176, 1);
    lastAccepted_ = snap.accepted;
    lastRejected_ = snap.rejected;
    lastMhz_ = snap.cpuMhz;
  }

  if (forceFull || snap.nonce != lastNonce_ || snap.totalHashes != lastHashes_) {
    tft_.fillRect(12, 198, 296, 24, cBg_);
    char line[56];
    snprintf(line, sizeof(line), "nonce %08lx  hashes %llu", (unsigned long)snap.nonce,
             (unsigned long long)snap.totalHashes);
    tft_.setTextColor(cMuted_, cBg_);
    tft_.drawString(line, 12, 200, 1);
    tft_.setTextColor(cText_, cBg_);
    tft_.drawString("USB-C only", 220, 200, 1);
    lastNonce_ = snap.nonce;
    lastHashes_ = snap.totalHashes;
  }

  if (forceFull || snap.netTicker != lastTicker_ || animDirty) {
    tft_.fillRect(0, 226, 320, 14, cPanel_);
    tft_.setTextDatum(TL_DATUM);
    tft_.setTextColor(cLime_, cPanel_);
    String tick = snap.netTicker.length() ? snap.netTicker : String("usb: companion linked");
    // Gentle marquee for long tickers.
    if (tick.length() > 38) {
      int off = (frame / 2) % (tick.length() - 37);
      tick = tick.substring(off, off + 38);
    }
    tft_.drawString(tick, 6, 228, 1);
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
  tft_.fillRect(0, 28, 8, 184, cLime_);
  tft_.setTextDatum(MC_DATUM);
  tft_.setTextColor(cLime_, cBg_);
  tft_.drawString("CYD", 160, 70, 4);
  tft_.setTextColor(cText_, cBg_);
  tft_.drawString(title, 160, 120, 2);
  tft_.setTextColor(cMuted_, cBg_);
  tft_.drawString(detail, 160, 150, 2);
}
