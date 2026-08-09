#include "gui.h"

#include <cstdio>
#include <cstring>
#include <math.h>

#include "theme.h"

namespace {
constexpr int SCREEN_W = 320;
constexpr int SCREEN_H = 240;
constexpr int OUTER_PAD = 4;
constexpr int HEADER_Y = 3;
constexpr int HEADER_H = 26;
constexpr int FOOTER_H = 24;
constexpr int FOOTER_Y = SCREEN_H - OUTER_PAD - FOOTER_H;
constexpr int GRID_TOP = HEADER_Y + HEADER_H + 3;
constexpr int GAP = 6;
constexpr int LARGE_W = 172;
constexpr int LARGE_H = 172;
constexpr int SMALL_W = SCREEN_W - OUTER_PAD * 2 - LARGE_W - GAP;  // 134
constexpr int SMALL_H = (LARGE_H - GAP) / 2;                       // 83
constexpr int RADIUS = 12;
constexpr float ARC_START = 225.0f;  // math degrees: 0=east, CCW
constexpr float ARC_SWEEP = 270.0f;
constexpr float SMOOTH = 0.28f;

// TFT_eSPI smooth-arc angles: 0 at 6 o'clock, clockwise.
uint32_t toTftAngle(float mathDeg) {
  float a = 270.0f - mathDeg;
  while (a < 0.0f) a += 360.0f;
  while (a >= 360.0f) a -= 360.0f;
  return static_cast<uint32_t>(a + 0.5f);
}

void polar(int cx, int cy, float deg, float r, float &x, float &y) {
  const float rad = deg * 0.01745329252f;
  x = cx + cosf(rad) * r;
  y = cy - sinf(rad) * r;
}

float approach(float current, float target, float alpha) {
  return current + (target - current) * alpha;
}
}  // namespace

MonitorGui::DialGeom MonitorGui::dialGeom(int index) const {
  const int largeX = OUTER_PAD;
  const int largeY = GRID_TOP;
  const int smallX = OUTER_PAD + LARGE_W + GAP;
  if (index == 0) {
    return DialGeom{largeX, largeY, LARGE_W, LARGE_H, true};
  }
  if (index == 1) {
    return DialGeom{smallX, largeY, SMALL_W, SMALL_H, false};
  }
  return DialGeom{smallX, largeY + SMALL_H + GAP, SMALL_W, SMALL_H, false};
}

void MonitorGui::begin(TFT_eSPI &tft, uint8_t rotation) {
  tft.setRotation(rotation == 3 ? 3 : 1);
  tft.fillScreen(Theme::bg);
  tft.setTextDatum(TL_DATUM);
  if (dialSpr_ == nullptr) {
    dialSpr_ = new TFT_eSprite(&tft);
  }
  dialSpr_->setColorDepth(16);
  spriteReady_ = dialSpr_->createSprite(LARGE_W, LARGE_H);
  chromeDrawn_ = false;
}

void MonitorGui::invalidate() {
  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastStatus_[0] = '\0';
  lastPps_ = 0xFFFF;
  lastDisk_ = -1.0f;
  lastVram_ = -1.0f;
  for (auto &d : dials_) {
    d.lastDrawn = -999.0f;
  }
}

void MonitorGui::setRotation(TFT_eSPI &tft, uint8_t rotation) {
  tft.setRotation(rotation == 3 ? 3 : 1);
  invalidate();
  tft.fillScreen(Theme::bg);
  drawChrome(tft);
}

void MonitorGui::drawDecor(TFT_eSPI &tft) {
  tft.fillRect(0, 0, SCREEN_W, SCREEN_H / 3, Theme::bgDeep);
  tft.fillRect(0, SCREEN_H / 3, SCREEN_W, SCREEN_H / 3, Theme::bg);
  tft.fillRect(0, (SCREEN_H * 2) / 3, SCREEN_W, SCREEN_H / 3, Theme::bgMid);
  tft.fillCircle(-16, 24, 48, Theme::bgDeep);
  tft.fillCircle(SCREEN_W + 18, 56, 56, Theme::bubbleGlow);
  tft.fillCircle(SCREEN_W - 28, -18, 30, Theme::accentSoft);
  tft.fillCircle(96, SCREEN_H / 2 + 8, 78, Theme::bgDeep);
}

void MonitorGui::drawSoftBubble(TFT_eSPI &tft, int x, int y, int w, int h, uint16_t fill) {
  tft.fillRoundRect(x + 2, y + 3, w, h, RADIUS, Theme::bgDeep);
  tft.drawRoundRect(x - 1, y - 1, w + 2, h + 2, RADIUS + 1, Theme::bubbleGlow);
  tft.fillRoundRect(x, y, w, h, RADIUS, fill);
  const int sheen = h / 3;
  tft.fillRoundRect(x + 3, y + 2, w - 6, sheen, RADIUS - 4, Theme::bgPanelHi);
  tft.fillRoundRect(x + 2, y + sheen - 2, w - 4, h - sheen + 2, RADIUS - 3, fill);
  tft.drawRoundRect(x, y, w, h, RADIUS, Theme::border);
}

void MonitorGui::showBoot(TFT_eSPI &tft, const char *message) {
  tft.fillScreen(Theme::bg);
  drawDecor(tft);
  const int cx = SCREEN_W / 2;
  const int cy = SCREEN_H / 2 - 10;
  tft.fillCircle(cx + 3, cy + 4, 62, Theme::bgDeep);
  tft.fillSmoothCircle(cx, cy, 54, Theme::bubbleGlow, Theme::bg);
  tft.fillSmoothCircle(cx, cy, 46, Theme::bgPanel, Theme::bubbleGlow);
  tft.fillSmoothCircle(cx, cy, 38, Theme::dialFace, Theme::bgPanel);
  tft.drawSmoothArc(cx, cy, 48, 44, 20, 340, Theme::accent, Theme::bgPanel, true);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::accentPale, Theme::dialFace);
  tft.drawString("CYD", cx, cy - 8, 4);
  tft.setTextColor(Theme::textMuted, Theme::dialFace);
  tft.drawString("monitor", cx, cy + 14, 2);
  tft.fillRoundRect(cx - 114, SCREEN_H - 38, 228, 26, 13, Theme::bgPanel);
  tft.drawRoundRect(cx - 114, SCREEN_H - 38, 228, 26, 13, Theme::accent);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(message, cx, SCREEN_H - 25, 2);
  tft.setTextDatum(TL_DATUM);
  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastStatus_[0] = '\0';
  lastPps_ = 0xFFFF;
  lastVram_ = -1.0f;
  for (auto &d : dials_) {
    d = DialState{};
  }
}

void MonitorGui::drawChrome(TFT_eSPI &tft) {
  drawDecor(tft);
  chromeDrawn_ = true;
  lastHost_[0] = '\0';
  lastLinked_ = false;
  lastStatus_[0] = '\0';
  lastPps_ = 0xFFFF;
  lastDisk_ = -1.0f;
  lastVram_ = -1.0f;
  for (auto &d : dials_) {
    d.lastDrawn = -999.0f;
  }
}

void MonitorGui::drawHeader(TFT_eSPI &tft, const char *host, bool linked) {
  const int x = OUTER_PAD;
  const int y = HEADER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  drawSoftBubble(tft, x, y, w, HEADER_H, Theme::bgPanel);
  tft.fillRoundRect(x + 6, y + 5, 42, 16, 8, Theme::accent);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::accent);
  tft.drawString("CYD", x + 27, y + HEADER_H / 2, 2);
  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(host && host[0] ? host : "PC", x + 56, y + HEADER_H / 2, 2);
  const int pillW = 72;
  const int pillX = x + w - pillW - 6;
  const uint16_t pillFill = linked ? Theme::ok : Theme::dialFace;
  tft.fillRoundRect(pillX, y + 5, pillW, 16, 8, pillFill);
  tft.drawRoundRect(pillX, y + 5, pillW, 16, 8, linked ? Theme::accentPale : Theme::border);
  if (linked) {
    tft.fillSmoothCircle(pillX + 10, y + HEADER_H / 2, 3, Theme::accentPale, pillFill);
  }
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, pillFill);
  tft.drawString(linked ? "LIVE" : "WAIT", pillX + pillW / 2 + 4, y + HEADER_H / 2, 2);
  tft.setTextDatum(TL_DATUM);
  strncpy(lastHost_, host ? host : "", sizeof(lastHost_) - 1);
  lastHost_[sizeof(lastHost_) - 1] = '\0';
  lastLinked_ = linked;
}

void MonitorGui::drawSmoothGaugeArc(TFT_eSPI &spr, int cx, int cy, int rOuter, int rInner,
                                    float startMath, float endMath, uint16_t fg, uint16_t bg) {
  if (endMath > startMath) {
    return;
  }
  uint32_t a0 = toTftAngle(startMath);
  uint32_t a1 = toTftAngle(endMath);
  if (a0 == a1) {
    return;
  }
  // Smooth arc draws clockwise from a0 → a1 (our gauge sweeps that way).
  spr.drawSmoothArc(cx, cy, rOuter, rInner, a0, a1, fg, bg, true);
}

void MonitorGui::paintDialSprite(int w, int h, bool large, const char *label, float pct, float tempC,
                                 bool showTemp) {
  TFT_eSprite &spr = *dialSpr_;
  spr.fillSprite(Theme::bg);
  spr.fillSmoothRoundRect(0, 0, w, h, RADIUS, Theme::bgPanel, Theme::bg);
  spr.drawRoundRect(0, 0, w, h, RADIUS, Theme::border);

  const int cx = w / 2;
  const int cy = h / 2 + (large ? 8 : 4);
  const int r = (w < h ? w : h) / 2 - (large ? 10 : 8);

  spr.fillSmoothCircle(cx, cy, r + 5, Theme::dialRingLo, Theme::bgPanel);
  spr.fillSmoothCircle(cx, cy, r + 2, Theme::dialRing, Theme::dialRingLo);
  spr.fillSmoothCircle(cx, cy, r - 1, Theme::dialFace, Theme::dialRing);

  // Soft outer ring (AA).
  spr.drawSmoothArc(cx, cy, r + 1, r - 1, 0, 360, Theme::border, Theme::dialFace, false);

  const int tickStep = large ? 10 : 20;
  for (int i = 0; i <= 100; i += tickStep) {
    const float deg = ARC_START - ARC_SWEEP * (i / 100.0f);
    const bool major = (i % 20 == 0);
    float x0, y0, x1, y1;
    const float inner = r - (major ? (large ? 14.0f : 10.0f) : (large ? 8.0f : 6.0f));
    polar(cx, cy, deg, inner, x0, y0);
    polar(cx, cy, deg, r - 3.0f, x1, y1);
    const uint16_t tickCol = (i >= 90) ? Theme::danger : (i >= 70 ? Theme::warn : Theme::accentPale);
    spr.drawWideLine(x0, y0, x1, y1, major ? 2.2f : 1.2f, tickCol, Theme::dialFace);
  }

  const int trackOuter = r - 4;
  const int trackInner = r - (large ? 12 : 10);
  drawSmoothGaugeArc(spr, cx, cy, trackOuter, trackInner, ARC_START, ARC_START - ARC_SWEEP,
                     Theme::barTrack, Theme::dialFace);

  if (pct > 0.4f) {
    const float endDeg = ARC_START - ARC_SWEEP * (pct / 100.0f);
    drawSmoothGaugeArc(spr, cx, cy, trackOuter, trackInner, ARC_START, endDeg, Theme::barColorFor(pct),
                       Theme::dialFace);
    if (large) {
      drawSmoothGaugeArc(spr, cx, cy, trackInner + 1, trackInner - 2, ARC_START, endDeg, Theme::accentGlow,
                         Theme::dialFace);
    }
  }

  const float needleDeg = ARC_START - ARC_SWEEP * (pct / 100.0f);
  float nx, ny;
  polar(cx, cy, needleDeg, r - (large ? 16.0f : 12.0f), nx, ny);
  spr.drawWideLine(static_cast<float>(cx), static_cast<float>(cy), nx, ny, large ? 3.0f : 2.2f,
                   Theme::textPrimary, Theme::dialFace);
  spr.fillSmoothCircle(cx, cy, large ? 6 : 4, Theme::accentSoft, Theme::dialFace);
  spr.fillSmoothCircle(cx, cy, large ? 2 : 1, Theme::accentPale, Theme::accentSoft);

  spr.setTextDatum(MC_DATUM);
  spr.setTextColor(Theme::accentPale, Theme::bgPanel);
  spr.drawString(label, cx, large ? 12 : 8, 2);

  char value[12];
  snprintf(value, sizeof(value), "%d%%", static_cast<int>(pct + 0.5f));
  spr.setTextColor(Theme::textPrimary, Theme::dialFace);
  spr.drawString(value, cx, cy + (large ? 20 : 14), large ? 4 : 2);

  if (showTemp) {
    char tempBuf[12];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%dC", static_cast<int>(tempC + 0.5f));
    } else {
      snprintf(tempBuf, sizeof(tempBuf), "--");
    }
    const int tw = large ? 40 : 34;
    const int th = large ? 16 : 13;
    const int ty = h - th - 4;
    spr.fillSmoothRoundRect(cx - tw / 2, ty, tw, th, th / 2, Theme::bgPanelHi, Theme::bgPanel);
    spr.setTextColor(Theme::tempColorFor(tempC), Theme::bgPanelHi);
    spr.drawString(tempBuf, cx, ty + th / 2, 2);
  }
}

void MonitorGui::drawDial(TFT_eSPI &tft, int index, const char *label, bool showTemp) {
  DialState &d = dials_[index];
  d.shown = approach(d.shown, d.target, SMOOTH);
  if (fabsf(d.shown - d.lastDrawn) < 0.35f && fabsf(d.temp - d.lastTemp) < 0.5f && d.lastDrawn > -900.0f) {
    return;
  }

  const DialGeom g = dialGeom(index);
  if (spriteReady_ && dialSpr_ != nullptr) {
    paintDialSprite(g.w, g.h, g.large, label, d.shown, d.temp, showTemp);
    dialSpr_->pushSprite(g.x, g.y, 0, 0, g.w, g.h);
  } else {
    drawSoftBubble(tft, g.x, g.y, g.w, g.h, Theme::bgPanel);
  }
  d.lastDrawn = d.shown;
  d.lastTemp = d.temp;
}

void MonitorGui::drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link,
                            const char *statusLine) {
  const char *status = statusLine ? statusLine : "";
  const bool same = chromeDrawn_ && lastStatus_[0] != '\0' && strcmp(lastStatus_, status) == 0 &&
                    lastPps_ == link.pps && lastDisk_ == m.diskUsed && lastVram_ == m.vramUsed;
  if (same) {
    return;
  }

  const int x = OUTER_PAD;
  const int y = FOOTER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  drawSoftBubble(tft, x, y, w, FOOTER_H, Theme::bgPanel);

  char line[80];
  snprintf(line, sizeof(line), "%s  %up/s  VRAM %d%%  Disk %d%%", status, static_cast<unsigned>(link.pps),
           static_cast<int>(m.vramUsed + 0.5f), static_cast<int>(m.diskUsed + 0.5f));
  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(line, x + 6, y + FOOTER_H / 2, 1);

  if (m.netDown > 0.05f) {
    char netBuf[16];
    snprintf(netBuf, sizeof(netBuf), "%.1fMb", static_cast<double>(m.netDown));
    tft.fillRoundRect(x + w - 50, y + 4, 44, 16, 8, Theme::accentSoft);
    tft.setTextDatum(MC_DATUM);
    tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
    tft.drawString(netBuf, x + w - 28, y + FOOTER_H / 2, 1);
  }
  tft.setTextDatum(TL_DATUM);

  strncpy(lastStatus_, status, sizeof(lastStatus_) - 1);
  lastStatus_[sizeof(lastStatus_) - 1] = '\0';
  lastPps_ = link.pps;
  lastDisk_ = m.diskUsed;
  lastVram_ = m.vramUsed;
}

void MonitorGui::render(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link, bool linked,
                        const char *statusLine) {
  if (!chromeDrawn_) {
    drawChrome(tft);
  }
  if (strcmp(lastHost_, m.hostName) != 0 || lastLinked_ != linked || lastHost_[0] == '\0') {
    drawHeader(tft, m.hostName, linked);
  }

  dials_[0].target = m.cpuLoad;
  dials_[0].temp = m.cpuTemp;
  dials_[1].target = m.gpuLoad;
  dials_[1].temp = m.gpuTemp;
  dials_[2].target = m.ramUsed;
  dials_[2].temp = 0;

  drawDial(tft, 0, "CPU", true);
  drawDial(tft, 1, "GPU", true);
  drawDial(tft, 2, "RAM", false);
  drawFooter(tft, m, link, statusLine);
}
