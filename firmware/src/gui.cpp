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
constexpr int HEADER_H = 28;
constexpr int FOOTER_H = 24;
constexpr int FOOTER_Y = SCREEN_H - OUTER_PAD - FOOTER_H;
constexpr int GRID_TOP = HEADER_Y + HEADER_H + 4;
constexpr int GRID_GAP = 4;
constexpr int DIAL_W = (SCREEN_W - OUTER_PAD * 2 - GRID_GAP) / 2;
constexpr int DIAL_H = (FOOTER_Y - GRID_TOP - GRID_GAP) / 2;
constexpr int RADIUS = 12;

// Speedometer sweep in standard math degrees (0=east, CCW): 225 -> -45
constexpr float ARC_START = 225.0f;
constexpr float ARC_SWEEP = 270.0f;

void dialRect(int index, int &x, int &y) {
  const int col = index % 2;
  const int row = index / 2;
  x = OUTER_PAD + col * (DIAL_W + GRID_GAP);
  y = GRID_TOP + row * (DIAL_H + GRID_GAP);
}

void polar(int cx, int cy, float deg, int r, int &x, int &y) {
  const float rad = deg * 0.01745329252f;
  x = cx + static_cast<int>(cosf(rad) * r);
  y = cy - static_cast<int>(sinf(rad) * r);
}
}  // namespace

void MonitorGui::begin(TFT_eSPI &tft) {
  tft.setRotation(1);
  tft.fillScreen(Theme::bg);
  tft.setTextDatum(TL_DATUM);
  chromeDrawn_ = false;
}

void MonitorGui::drawDecor(TFT_eSPI &tft) {
  tft.fillCircle(-6, 36, 28, Theme::bgDeep);
  tft.fillCircle(SCREEN_W + 8, 100, 34, Theme::bgDeep);
  tft.fillCircle(SCREEN_W - 20, -4, 18, Theme::bubbleGlow);
}

void MonitorGui::drawSoftBubble(TFT_eSPI &tft, int x, int y, int w, int h, uint16_t fill) {
  tft.fillRoundRect(x + 1, y + 2, w, h, RADIUS, Theme::bgDeep);
  tft.fillRoundRect(x, y, w, h, RADIUS, fill);
  tft.drawRoundRect(x, y, w, h, RADIUS, Theme::border);
}

void MonitorGui::showBoot(TFT_eSPI &tft, const char *message) {
  tft.fillScreen(Theme::bg);
  drawDecor(tft);
  const int cx = SCREEN_W / 2;
  const int cy = SCREEN_H / 2 - 8;
  tft.fillCircle(cx + 2, cy + 3, 54, Theme::bgDeep);
  tft.fillCircle(cx, cy, 52, Theme::bubbleGlow);
  tft.fillCircle(cx, cy, 44, Theme::bgPanel);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::accent, Theme::bgPanel);
  tft.drawString("CYD", cx, cy - 6, 4);
  tft.setTextColor(Theme::textMuted, Theme::bgPanel);
  tft.drawString("dials online", cx, cy + 14, 2);
  tft.fillRoundRect(cx - 110, SCREEN_H - 36, 220, 24, 12, Theme::bgPanel);
  tft.drawRoundRect(cx - 110, SCREEN_H - 36, 220, 24, 12, Theme::border);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(message, cx, SCREEN_H - 24, 2);
  tft.setTextDatum(TL_DATUM);

  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastLinked_ = false;
  lastStatus_[0] = '\0';
  lastFps_ = 0xFFFF;
  lastDisk_ = -1.0f;
  lastNetDown_ = -1.0f;
  for (auto &d : dials_) {
    d = DialCache{};
  }
}

void MonitorGui::drawChrome(TFT_eSPI &tft) {
  tft.fillScreen(Theme::bg);
  drawDecor(tft);
  chromeDrawn_ = true;
  lastHost_[0] = '\0';
  lastLinked_ = false;
  lastStatus_[0] = '\0';
  lastFps_ = 0xFFFF;
  lastDisk_ = -1.0f;
  lastNetDown_ = -1.0f;
  for (auto &d : dials_) {
    d.drawn = false;
    d.pct = -1.0f;
  }
}

void MonitorGui::drawHeader(TFT_eSPI &tft, const char *host, bool linked) {
  const int x = OUTER_PAD;
  const int y = HEADER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  drawSoftBubble(tft, x, y, w, HEADER_H, Theme::bgPanel);

  tft.fillRoundRect(x + 6, y + 5, 40, 18, 9, Theme::accentSoft);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
  tft.drawString("CYD", x + 26, y + HEADER_H / 2, 2);

  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(host && host[0] ? host : "PC", x + 52, y + HEADER_H / 2, 2);

  const int pillW = 70;
  const int pillX = x + w - pillW - 6;
  const uint16_t pillFill = linked ? Theme::ok : Theme::barTrack;
  tft.fillRoundRect(pillX, y + 5, pillW, 18, 9, pillFill);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, pillFill);
  tft.drawString(linked ? "LIVE" : "WAIT", pillX + pillW / 2, y + HEADER_H / 2, 2);
  tft.setTextDatum(TL_DATUM);

  strncpy(lastHost_, host ? host : "", sizeof(lastHost_) - 1);
  lastHost_[sizeof(lastHost_) - 1] = '\0';
  lastLinked_ = linked;
}

void MonitorGui::drawArcSpan(TFT_eSPI &tft, int cx, int cy, int r, float startDeg, float endDeg,
                             uint16_t color, int width) {
  // Sample the arc; degrees decrease for clockwise visual sweep on y-up math.
  const float step = 3.0f;
  float deg = startDeg;
  int px = 0;
  int py = 0;
  polar(cx, cy, deg, r, px, py);
  while (deg > endDeg) {
    deg -= step;
    if (deg < endDeg) {
      deg = endDeg;
    }
    int x = 0;
    int y = 0;
    polar(cx, cy, deg, r, x, y);
    tft.drawLine(px, py, x, y, color);
    if (width > 1) {
      tft.drawLine(px, py + 1, x, y + 1, color);
    }
    px = x;
    py = y;
  }
}

void MonitorGui::drawDial(TFT_eSPI &tft, int index, const char *label, float pct, float tempC,
                          bool showTemp, bool force) {
  DialCache &cache = dials_[index];
  const float pctQ = static_cast<float>(static_cast<int>(pct + 0.5f));
  const float tempQ = static_cast<float>(static_cast<int>(tempC + 0.5f));
  if (!force && cache.drawn && cache.pct == pctQ && cache.temp == tempQ) {
    return;
  }

  int x = 0;
  int y = 0;
  dialRect(index, x, y);
  drawSoftBubble(tft, x, y, DIAL_W, DIAL_H, Theme::bgPanel);

  const int cx = x + DIAL_W / 2;
  const int cy = y + DIAL_H / 2 + 4;
  const int r = (DIAL_W < DIAL_H ? DIAL_W : DIAL_H) / 2 - 10;

  tft.drawCircle(cx, cy, r + 2, Theme::border);
  tft.drawCircle(cx, cy, r, Theme::accentSoft);

  // Track arc
  drawArcSpan(tft, cx, cy, r - 4, ARC_START, ARC_START - ARC_SWEEP, Theme::barTrack, 2);

  // Value arc
  if (pctQ > 0.0f) {
    const float endDeg = ARC_START - ARC_SWEEP * (pctQ / 100.0f);
    drawArcSpan(tft, cx, cy, r - 4, ARC_START, endDeg, Theme::barColorFor(pctQ), 3);
  }

  // Needle
  const float needleDeg = ARC_START - ARC_SWEEP * (pctQ / 100.0f);
  int nx = 0;
  int ny = 0;
  polar(cx, cy, needleDeg, r - 10, nx, ny);
  tft.drawLine(cx, cy, nx, ny, Theme::textPrimary);
  tft.fillCircle(cx, cy, 3, Theme::accent);

  // Labels
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textMuted, Theme::bgPanel);
  tft.drawString(label, cx, y + 10, 2);

  char value[12];
  snprintf(value, sizeof(value), "%d%%", static_cast<int>(pctQ));
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(value, cx, cy + 14, 2);

  if (showTemp) {
    char tempBuf[12];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%dC", static_cast<int>(tempQ));
    } else {
      snprintf(tempBuf, sizeof(tempBuf), "--");
    }
    tft.setTextColor(Theme::tempColorFor(tempC), Theme::bgPanel);
    tft.drawString(tempBuf, cx, y + DIAL_H - 10, 2);
  }

  tft.setTextDatum(TL_DATUM);
  cache.pct = pctQ;
  cache.temp = tempQ;
  cache.drawn = true;
}

void MonitorGui::drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const char *statusLine) {
  const char *status = statusLine ? statusLine : "";
  const bool same = chromeDrawn_ && lastStatus_[0] != '\0' && strcmp(lastStatus_, status) == 0 &&
                    lastFps_ == m.fps && lastDisk_ == m.diskUsed && lastNetDown_ == m.netDown;
  if (same) {
    return;
  }

  const int x = OUTER_PAD;
  const int y = FOOTER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  drawSoftBubble(tft, x, y, w, FOOTER_H, Theme::bgPanel);

  char line[64];
  snprintf(line, sizeof(line), "Disk %d%%  Net %.1fMb  %s", static_cast<int>(m.diskUsed + 0.5f),
           static_cast<double>(m.netDown), status);
  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textMuted, Theme::bgPanel);
  tft.drawString(line, x + 8, y + FOOTER_H / 2, 2);

  if (m.fps > 0) {
    char fpsBuf[12];
    snprintf(fpsBuf, sizeof(fpsBuf), "%u", static_cast<unsigned>(m.fps));
    tft.setTextDatum(MR_DATUM);
    tft.setTextColor(Theme::accentSoft, Theme::bgPanel);
    tft.drawString(fpsBuf, x + w - 8, y + FOOTER_H / 2, 2);
  }
  tft.setTextDatum(TL_DATUM);

  strncpy(lastStatus_, status, sizeof(lastStatus_) - 1);
  lastStatus_[sizeof(lastStatus_) - 1] = '\0';
  lastFps_ = m.fps;
  lastDisk_ = m.diskUsed;
  lastNetDown_ = m.netDown;
}

void MonitorGui::render(TFT_eSPI &tft, const SystemMetrics &m, bool linked, const char *statusLine) {
  if (!chromeDrawn_) {
    drawChrome(tft);
  }
  if (strcmp(lastHost_, m.hostName) != 0 || lastLinked_ != linked || lastHost_[0] == '\0') {
    drawHeader(tft, m.hostName, linked);
  }
  drawDial(tft, 0, "CPU", m.cpuLoad, m.cpuTemp, true, false);
  drawDial(tft, 1, "GPU", m.gpuLoad, m.gpuTemp, true, false);
  drawDial(tft, 2, "RAM", m.ramUsed, 0.0f, false, false);
  drawDial(tft, 3, "VRAM", m.vramUsed, 0.0f, false, false);
  drawFooter(tft, m, statusLine);
}
