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
  // Layered dark → light blue atmosphere
  tft.fillRect(0, 0, SCREEN_W, SCREEN_H / 3, Theme::bgDeep);
  tft.fillRect(0, SCREEN_H / 3, SCREEN_W, SCREEN_H / 3, Theme::bg);
  tft.fillRect(0, (SCREEN_H * 2) / 3, SCREEN_W, SCREEN_H / 3, Theme::bgMid);

  tft.fillCircle(-10, 30, 40, Theme::bgDeep);
  tft.fillCircle(SCREEN_W + 12, 70, 48, Theme::bubbleGlow);
  tft.fillCircle(36, SCREEN_H + 8, 34, Theme::bgPanel);
  tft.fillCircle(SCREEN_W - 40, -10, 26, Theme::accentSoft);
  tft.fillCircle(SCREEN_W / 2, SCREEN_H / 2 + 20, 70, Theme::bgDeep);
}

void MonitorGui::drawSoftBubble(TFT_eSPI &tft, int x, int y, int w, int h, uint16_t fill) {
  // Soft drop shadow
  tft.fillRoundRect(x + 2, y + 3, w, h, RADIUS, Theme::bgDeep);
  // Outer light-blue glow
  tft.drawRoundRect(x - 1, y - 1, w + 2, h + 2, RADIUS + 1, Theme::bubbleGlow);
  // Body
  tft.fillRoundRect(x, y, w, h, RADIUS, fill);
  // Top sheen (light blue)
  const int sheen = h / 3;
  tft.fillRoundRect(x + 3, y + 2, w - 6, sheen, RADIUS - 4, Theme::bgPanelHi);
  tft.fillRoundRect(x + 2, y + sheen - 2, w - 4, h - sheen + 2, RADIUS - 3, fill);
  // Rim
  tft.drawRoundRect(x, y, w, h, RADIUS, Theme::border);
  tft.drawRoundRect(x + 1, y + 1, w - 2, h - 2, RADIUS - 1, Theme::dialRingLo);
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
  // Shine
  tft.fillCircle(cx - 14, cy - 16, 12, Theme::bgPanelHi);
  tft.drawCircle(cx, cy, 50, Theme::dialRing);
  tft.drawCircle(cx, cy, 44, Theme::accentPale);

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

void MonitorGui::drawArcSpan(TFT_eSPI &tft, int cx, int cy, int r, float startDeg, float endDeg,
                             uint16_t color, int width) {
  const float step = 2.5f;
  float deg = startDeg;
  int px = 0;
  int py = 0;
  polar(cx, cy, deg, r, px, py);
  while (deg > endDeg + 0.01f) {
    deg -= step;
    if (deg < endDeg) {
      deg = endDeg;
    }
    int x = 0;
    int y = 0;
    polar(cx, cy, deg, r, x, y);
    for (int t = 0; t < width; ++t) {
      tft.drawLine(px, py + t, x, y + t, color);
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
  const int cy = y + DIAL_H / 2 + 6;
  const int r = (DIAL_W < DIAL_H ? DIAL_W : DIAL_H) / 2 - 12;

  // Dial face stack: dark core + light rings
  tft.fillCircle(cx, cy, r + 6, Theme::dialRingLo);
  tft.fillCircle(cx, cy, r + 3, Theme::dialRing);
  tft.fillCircle(cx, cy, r, Theme::dialFace);
  tft.drawCircle(cx, cy, r - 1, Theme::border);
  // Inner light highlight crescent
  tft.drawCircle(cx - 1, cy - 1, r - 6, Theme::dialRingLo);

  // Tick marks
  for (int i = 0; i <= 100; i += 10) {
    const float deg = ARC_START - ARC_SWEEP * (i / 100.0f);
    int x0 = 0, y0 = 0, x1 = 0, y1 = 0;
    const int outer = r - 3;
    const int inner = r - ((i % 20 == 0) ? 11 : 7);
    polar(cx, cy, deg, inner, x0, y0);
    polar(cx, cy, deg, outer, x1, y1);
    const uint16_t tickCol = (i >= 90) ? Theme::danger : (i >= 70 ? Theme::warn : Theme::accentPale);
    tft.drawLine(x0, y0, x1, y1, tickCol);
  }

  // Track + value arcs (dual width for depth)
  drawArcSpan(tft, cx, cy, r - 5, ARC_START, ARC_START - ARC_SWEEP, Theme::barTrack, 3);
  if (pctQ > 0.0f) {
    const float endDeg = ARC_START - ARC_SWEEP * (pctQ / 100.0f);
    const uint16_t fill = Theme::barColorFor(pctQ);
    drawArcSpan(tft, cx, cy, r - 5, ARC_START, endDeg, fill, 4);
    drawArcSpan(tft, cx, cy, r - 9, ARC_START, endDeg, Theme::accentGlow, 1);
  }

  // Needle with light-blue hub
  const float needleDeg = ARC_START - ARC_SWEEP * (pctQ / 100.0f);
  int nx = 0;
  int ny = 0;
  polar(cx, cy, needleDeg, r - 12, nx, ny);
  tft.drawLine(cx, cy, nx, ny, Theme::textPrimary);
  tft.fillCircle(cx, cy, 5, Theme::accentSoft);
  tft.fillCircle(cx, cy, 2, Theme::accentPale);

  // Labels
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(label, cx, y + 9, 2);

  char value[12];
  snprintf(value, sizeof(value), "%d%%", static_cast<int>(pctQ));
  tft.setTextColor(Theme::textPrimary, Theme::dialFace);
  tft.drawString(value, cx, cy + 16, 2);

  if (showTemp) {
    char tempBuf[12];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%dC", static_cast<int>(tempQ));
    } else {
      snprintf(tempBuf, sizeof(tempBuf), "--");
    }
    tft.fillRoundRect(cx - 18, y + DIAL_H - 18, 36, 14, 7, Theme::dialFace);
    tft.setTextColor(Theme::tempColorFor(tempC), Theme::dialFace);
    tft.drawString(tempBuf, cx, y + DIAL_H - 11, 2);
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
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(line, x + 8, y + FOOTER_H / 2, 2);

  if (m.fps > 0) {
    char fpsBuf[12];
    snprintf(fpsBuf, sizeof(fpsBuf), "%u", static_cast<unsigned>(m.fps));
    tft.fillRoundRect(x + w - 44, y + 5, 36, 16, 8, Theme::accentSoft);
    tft.setTextDatum(MC_DATUM);
    tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
    tft.drawString(fpsBuf, x + w - 26, y + FOOTER_H / 2, 2);
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
