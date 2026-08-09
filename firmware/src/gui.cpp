#include "gui.h"

#include <cstdio>
#include <cstring>
#include <math.h>

#include "theme.h"

namespace {
constexpr int SCREEN_W = 320;
constexpr int SCREEN_H = 240;
constexpr int OUTER_PAD = 4;
constexpr int HEADER_Y = 4;
constexpr int HEADER_H = 30;
constexpr int FOOTER_H = 26;
constexpr int FOOTER_Y = SCREEN_H - OUTER_PAD - FOOTER_H;
constexpr int GRID_TOP = HEADER_Y + HEADER_H + 4;
constexpr int GRID_GAP = 4;
constexpr int DIAL_W = (SCREEN_W - OUTER_PAD * 2 - GRID_GAP) / 2;
constexpr int DIAL_H = (FOOTER_Y - GRID_TOP - GRID_GAP) / 2;
constexpr int RADIUS = 14;
constexpr float ARC_START = 225.0f;
constexpr float ARC_SWEEP = 270.0f;
constexpr float SMOOTH = 0.28f;

void dialRect(int index, int &x, int &y) {
  x = OUTER_PAD + (index % 2) * (DIAL_W + GRID_GAP);
  y = GRID_TOP + (index / 2) * (DIAL_H + GRID_GAP);
}

void polar(int cx, int cy, float deg, int r, int &x, int &y) {
  const float rad = deg * 0.01745329252f;
  x = cx + static_cast<int>(cosf(rad) * r);
  y = cy - static_cast<int>(sinf(rad) * r);
}

float approach(float current, float target, float alpha) {
  return current + (target - current) * alpha;
}
}  // namespace

void MonitorGui::begin(TFT_eSPI &tft, uint8_t rotation) {
  tft.setRotation(rotation == 3 ? 3 : 1);
  tft.fillScreen(Theme::bg);
  tft.setTextDatum(TL_DATUM);
  if (dialSpr_ == nullptr) {
    dialSpr_ = new TFT_eSprite(&tft);
  }
  dialSpr_->setColorDepth(16);
  spriteReady_ = dialSpr_->createSprite(DIAL_W, DIAL_H);
  chromeDrawn_ = false;
}

void MonitorGui::invalidate() {
  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastStatus_[0] = '\0';
  lastPps_ = 0xFFFF;
  lastDisk_ = -1.0f;
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
  tft.fillCircle(-10, 30, 40, Theme::bgDeep);
  tft.fillCircle(SCREEN_W + 12, 70, 48, Theme::bubbleGlow);
  tft.fillCircle(SCREEN_W - 40, -10, 26, Theme::accentSoft);
  tft.fillCircle(SCREEN_W / 2, SCREEN_H / 2 + 20, 70, Theme::bgDeep);
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
  tft.fillCircle(cx, cy, 58, Theme::bubbleGlow);
  tft.fillCircle(cx, cy, 50, Theme::bgPanel);
  tft.fillCircle(cx, cy, 42, Theme::dialFace);
  tft.fillCircle(cx - 14, cy - 16, 12, Theme::bgPanelHi);
  tft.drawCircle(cx, cy, 50, Theme::dialRing);
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
  for (auto &d : dials_) {
    d.lastDrawn = -999.0f;
  }
}

void MonitorGui::drawHeader(TFT_eSPI &tft, const char *host, bool linked) {
  const int x = OUTER_PAD;
  const int y = HEADER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  drawSoftBubble(tft, x, y, w, HEADER_H, Theme::bgPanel);
  tft.fillRoundRect(x + 7, y + 6, 44, 18, 9, Theme::accent);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::bgDeep, Theme::accent);
  tft.drawString("CYD", x + 29, y + HEADER_H / 2, 2);
  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(host && host[0] ? host : "PC", x + 58, y + HEADER_H / 2, 2);
  const int pillW = 74;
  const int pillX = x + w - pillW - 7;
  const uint16_t pillFill = linked ? Theme::ok : Theme::dialFace;
  tft.fillRoundRect(pillX, y + 6, pillW, 18, 9, pillFill);
  tft.drawRoundRect(pillX, y + 6, pillW, 18, 9, linked ? Theme::accentPale : Theme::border);
  if (linked) {
    tft.fillCircle(pillX + 11, y + HEADER_H / 2, 3, Theme::accentPale);
  }
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, pillFill);
  tft.drawString(linked ? "LIVE" : "WAIT", pillX + pillW / 2 + 5, y + HEADER_H / 2, 2);
  tft.setTextDatum(TL_DATUM);
  strncpy(lastHost_, host ? host : "", sizeof(lastHost_) - 1);
  lastHost_[sizeof(lastHost_) - 1] = '\0';
  lastLinked_ = linked;
}

void MonitorGui::drawArcSpan(TFT_eSPI &spr, int cx, int cy, int r, float startDeg, float endDeg,
                             uint16_t color, int width) {
  const float step = 2.5f;
  float deg = startDeg;
  int px = 0, py = 0;
  polar(cx, cy, deg, r, px, py);
  while (deg > endDeg + 0.01f) {
    deg -= step;
    if (deg < endDeg) deg = endDeg;
    int x = 0, y = 0;
    polar(cx, cy, deg, r, x, y);
    for (int t = 0; t < width; ++t) {
      spr.drawLine(px, py + t, x, y + t, color);
    }
    px = x;
    py = y;
  }
}

void MonitorGui::paintDialSprite(const char *label, float pct, float tempC, bool showTemp) {
  TFT_eSprite &spr = *dialSpr_;
  spr.fillSprite(Theme::bgPanel);
  spr.fillRoundRect(0, 0, DIAL_W, DIAL_H, RADIUS, Theme::bgPanel);
  spr.drawRoundRect(0, 0, DIAL_W, DIAL_H, RADIUS, Theme::border);

  const int cx = DIAL_W / 2;
  const int cy = DIAL_H / 2 + 6;
  const int r = (DIAL_W < DIAL_H ? DIAL_W : DIAL_H) / 2 - 12;

  spr.fillCircle(cx, cy, r + 6, Theme::dialRingLo);
  spr.fillCircle(cx, cy, r + 3, Theme::dialRing);
  spr.fillCircle(cx, cy, r, Theme::dialFace);
  spr.drawCircle(cx, cy, r - 1, Theme::border);

  for (int i = 0; i <= 100; i += 10) {
    const float deg = ARC_START - ARC_SWEEP * (i / 100.0f);
    int x0, y0, x1, y1;
    polar(cx, cy, deg, r - ((i % 20 == 0) ? 11 : 7), x0, y0);
    polar(cx, cy, deg, r - 3, x1, y1);
    const uint16_t tickCol = (i >= 90) ? Theme::danger : (i >= 70 ? Theme::warn : Theme::accentPale);
    spr.drawLine(x0, y0, x1, y1, tickCol);
  }

  drawArcSpan(spr, cx, cy, r - 5, ARC_START, ARC_START - ARC_SWEEP, Theme::barTrack, 3);
  if (pct > 0.5f) {
    const float endDeg = ARC_START - ARC_SWEEP * (pct / 100.0f);
    drawArcSpan(spr, cx, cy, r - 5, ARC_START, endDeg, Theme::barColorFor(pct), 4);
    drawArcSpan(spr, cx, cy, r - 9, ARC_START, endDeg, Theme::accentGlow, 1);
  }

  const float needleDeg = ARC_START - ARC_SWEEP * (pct / 100.0f);
  int nx, ny;
  polar(cx, cy, needleDeg, r - 12, nx, ny);
  spr.drawLine(cx, cy, nx, ny, Theme::textPrimary);
  spr.fillCircle(cx, cy, 5, Theme::accentSoft);
  spr.fillCircle(cx, cy, 2, Theme::accentPale);

  spr.setTextDatum(MC_DATUM);
  spr.setTextColor(Theme::accentPale, Theme::bgPanel);
  spr.drawString(label, cx, 9, 2);
  char value[12];
  snprintf(value, sizeof(value), "%d%%", static_cast<int>(pct + 0.5f));
  spr.setTextColor(Theme::textPrimary, Theme::dialFace);
  spr.drawString(value, cx, cy + 16, 2);
  if (showTemp) {
    char tempBuf[12];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%dC", static_cast<int>(tempC + 0.5f));
    } else {
      snprintf(tempBuf, sizeof(tempBuf), "--");
    }
    spr.fillRoundRect(cx - 18, DIAL_H - 18, 36, 14, 7, Theme::dialFace);
    spr.setTextColor(Theme::tempColorFor(tempC), Theme::dialFace);
    spr.drawString(tempBuf, cx, DIAL_H - 11, 2);
  }
}

void MonitorGui::drawDial(TFT_eSPI &tft, int index, const char *label, bool showTemp) {
  DialState &d = dials_[index];
  d.shown = approach(d.shown, d.target, SMOOTH);
  // Redraw when needle moved enough or first paint.
  if (fabsf(d.shown - d.lastDrawn) < 0.4f && fabsf(d.temp - d.lastTemp) < 0.5f && d.lastDrawn > -900.0f) {
    return;
  }

  int x, y;
  dialRect(index, x, y);
  if (spriteReady_ && dialSpr_ != nullptr) {
    paintDialSprite(label, d.shown, d.temp, showTemp);
    dialSpr_->pushSprite(x, y);
  } else {
    drawSoftBubble(tft, x, y, DIAL_W, DIAL_H, Theme::bgPanel);
  }
  d.lastDrawn = d.shown;
  d.lastTemp = d.temp;
}

void MonitorGui::drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link,
                            const char *statusLine) {
  const char *status = statusLine ? statusLine : "";
  const bool same = chromeDrawn_ && lastStatus_[0] != '\0' && strcmp(lastStatus_, status) == 0 &&
                    lastPps_ == link.pps && lastDisk_ == m.diskUsed;
  if (same) {
    return;
  }

  const int x = OUTER_PAD;
  const int y = FOOTER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  drawSoftBubble(tft, x, y, w, FOOTER_H, Theme::bgPanel);

  char line[72];
  snprintf(line, sizeof(line), "%s  %up/s  Disk %d%%", status, static_cast<unsigned>(link.pps),
           static_cast<int>(m.diskUsed + 0.5f));
  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(line, x + 8, y + FOOTER_H / 2, 2);

  if (m.netDown > 0.05f) {
    char netBuf[16];
    snprintf(netBuf, sizeof(netBuf), "%.1fMb", static_cast<double>(m.netDown));
    tft.fillRoundRect(x + w - 54, y + 5, 46, 16, 8, Theme::accentSoft);
    tft.setTextDatum(MC_DATUM);
    tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
    tft.drawString(netBuf, x + w - 31, y + FOOTER_H / 2, 2);
  }
  tft.setTextDatum(TL_DATUM);

  strncpy(lastStatus_, status, sizeof(lastStatus_) - 1);
  lastStatus_[sizeof(lastStatus_) - 1] = '\0';
  lastPps_ = link.pps;
  lastDisk_ = m.diskUsed;
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
  dials_[3].target = m.vramUsed;
  dials_[3].temp = 0;

  drawDial(tft, 0, "CPU", true);
  drawDial(tft, 1, "GPU", true);
  drawDial(tft, 2, "RAM", false);
  drawDial(tft, 3, "VRAM", false);
  drawFooter(tft, m, link, statusLine);
}
