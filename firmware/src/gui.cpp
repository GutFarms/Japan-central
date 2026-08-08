#include "gui.h"

#include <cstdio>
#include <cstring>

#include "theme.h"

namespace {
  constexpr int SCREEN_W = 320;
  constexpr int SCREEN_H = 240;

  constexpr int OUTER_PAD = 6;
  constexpr int HEADER_Y = 6;
  constexpr int HEADER_H = 34;
  constexpr int FOOTER_H = 26;
  constexpr int FOOTER_Y = SCREEN_H - OUTER_PAD - FOOTER_H;

  constexpr int GRID_TOP = HEADER_Y + HEADER_H + 6;
  constexpr int GRID_GAP = 6;
  constexpr int BUBBLE_W = (SCREEN_W - OUTER_PAD * 2 - GRID_GAP) / 2;  // 151
  constexpr int BUBBLE_H = (FOOTER_Y - GRID_TOP - GRID_GAP) / 2;       // ~81

  constexpr int BAR_H = 12;
  constexpr int RADIUS = 16;

  void bubbleRect(int index, int &x, int &y) {
    const int col = index % 2;
    const int row = index / 2;
    x = OUTER_PAD + col * (BUBBLE_W + GRID_GAP);
    y = GRID_TOP + row * (BUBBLE_H + GRID_GAP);
  }
}

void MonitorGui::begin(TFT_eSPI &tft) {
  tft.setRotation(1);  // landscape 320x240
  tft.fillScreen(Theme::bg);
  tft.setTextDatum(TL_DATUM);
  chromeDrawn_ = false;
}

void MonitorGui::fillSoftCircle(TFT_eSPI &tft, int cx, int cy, int r, uint16_t color) {
  tft.fillCircle(cx, cy, r, color);
}

void MonitorGui::drawDecor(TFT_eSPI &tft) {
  // Soft ambient bubbles behind the UI (static, drawn once with chrome).
  fillSoftCircle(tft, -8, 40, 36, Theme::bgDeep);
  fillSoftCircle(tft, SCREEN_W + 10, 90, 44, Theme::bgDeep);
  fillSoftCircle(tft, 40, SCREEN_H + 6, 28, Theme::bubbleGlow);
  fillSoftCircle(tft, SCREEN_W - 30, -6, 22, Theme::bubbleGlow);
  fillSoftCircle(tft, SCREEN_W / 2, SCREEN_H / 2 + 10, 50, Theme::bgDeep);
}

void MonitorGui::drawSoftBubble(TFT_eSPI &tft, int x, int y, int w, int h, uint16_t fill) {
  // Drop shadow
  tft.fillRoundRect(x + 2, y + 3, w, h, RADIUS, Theme::bgDeep);
  // Outer glow rim
  tft.fillRoundRect(x - 1, y - 1, w + 2, h + 2, RADIUS + 1, Theme::bubbleGlow);
  // Body
  tft.fillRoundRect(x, y, w, h, RADIUS, fill);
  // Soft highlight band along the top (bubbly sheen)
  const int sheenH = h / 3;
  tft.fillRoundRect(x + 3, y + 2, w - 6, sheenH, RADIUS - 4, Theme::bgPanelHi);
  // Re-cover lower body so only the top stays lighter
  tft.fillRoundRect(x + 2, y + sheenH - 2, w - 4, h - sheenH, RADIUS - 2, fill);
  // Crisp rim
  tft.drawRoundRect(x, y, w, h, RADIUS, Theme::border);
}

void MonitorGui::showBoot(TFT_eSPI &tft, const char *message) {
  tft.fillScreen(Theme::bg);
  drawDecor(tft);

  // Big center bubble
  const int cx = SCREEN_W / 2;
  const int cy = SCREEN_H / 2 - 6;
  tft.fillCircle(cx + 2, cy + 3, 62, Theme::bgDeep);
  tft.fillCircle(cx, cy, 60, Theme::bubbleGlow);
  tft.fillCircle(cx, cy, 52, Theme::bgPanel);
  tft.fillCircle(cx - 14, cy - 16, 16, Theme::bgPanelHi);  // shine

  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::accent, Theme::bgPanel);
  tft.drawString("CYD", cx, cy - 8, 4);
  tft.setTextColor(Theme::textMuted, Theme::bgPanel);
  tft.drawString("monitor", cx, cy + 16, 2);

  // Message pill
  tft.fillRoundRect(cx - 110, SCREEN_H - 40, 220, 26, 13, Theme::bgPanel);
  tft.drawRoundRect(cx - 110, SCREEN_H - 40, 220, 26, 13, Theme::border);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(message, cx, SCREEN_H - 27, 2);
  tft.setTextDatum(TL_DATUM);

  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastLinked_ = false;
  lastStatus_[0] = '\0';
  lastFps_ = 0xFFFF;
  for (auto &b : bubbles_) {
    b = BubbleCache{};
  }
}

void MonitorGui::drawChrome(TFT_eSPI &tft) {
  tft.fillScreen(Theme::bg);
  drawDecor(tft);
  chromeDrawn_ = true;
  // Force full redraw of overlays on next render.
  lastHost_[0] = '\0';
  lastLinked_ = false;
  lastStatus_[0] = '\0';
  lastFps_ = 0xFFFF;
  for (auto &b : bubbles_) {
    b.drawn = false;
    b.pct = -1.0f;
  }
}

void MonitorGui::drawHeader(TFT_eSPI &tft, const char *host, bool linked) {
  const int x = OUTER_PAD;
  const int y = HEADER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;

  drawSoftBubble(tft, x, y, w, HEADER_H, Theme::bgPanel);

  // Brand blob
  tft.fillRoundRect(x + 8, y + 7, 44, 20, 10, Theme::accentSoft);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
  tft.drawString("CYD", x + 30, y + HEADER_H / 2, 2);

  // Host name
  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(host && host[0] ? host : "PC", x + 60, y + HEADER_H / 2, 2);

  // Status blob (right)
  const int pillW = 78;
  const int pillX = x + w - pillW - 8;
  const uint16_t pillFill = linked ? Theme::ok : Theme::barTrack;
  const uint16_t pillText = linked ? Theme::textPrimary : Theme::textMuted;
  tft.fillRoundRect(pillX, y + 7, pillW, 20, 10, pillFill);
  if (linked) {
    tft.fillCircle(pillX + 12, y + HEADER_H / 2, 3, Theme::textPrimary);
  } else {
    tft.drawCircle(pillX + 12, y + HEADER_H / 2, 3, Theme::textMuted);
  }
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(pillText, pillFill);
  tft.drawString(linked ? "LIVE" : "WAIT", pillX + pillW / 2 + 6, y + HEADER_H / 2, 2);
  tft.setTextDatum(TL_DATUM);

  strncpy(lastHost_, host ? host : "", sizeof(lastHost_) - 1);
  lastHost_[sizeof(lastHost_) - 1] = '\0';
  lastLinked_ = linked;
}

void MonitorGui::drawCapsuleBar(TFT_eSPI &tft, int x, int y, int w, int h, float pct) {
  if (pct < 0.0f) pct = 0.0f;
  if (pct > 100.0f) pct = 100.0f;

  const int r = h / 2;
  tft.fillRoundRect(x, y, w, h, r, Theme::barTrack);

  int fillW = static_cast<int>((w * pct) / 100.0f);
  if (fillW > 0) {
    if (fillW < h) fillW = h;  // keep capsule round at low %
    if (fillW > w) fillW = w;
    const uint16_t fill = Theme::barColorFor(pct);
    tft.fillRoundRect(x, y, fillW, h, r, fill);
    // Tiny highlight bubble on the leading end
    const int hx = x + fillW - r;
    const int hy = y + h / 2;
    if (hx > x + 4) {
      tft.fillCircle(hx, hy - 1, 2, Theme::accentPale);
    }
  }
}

void MonitorGui::drawMetricBubble(TFT_eSPI &tft, int index, const char *label, float pct,
                                  float tempC, bool hasTemp, bool force) {
  BubbleCache &cache = bubbles_[index];
  const float pctQ = static_cast<float>(static_cast<int>(pct + 0.5f));
  const float tempQ = static_cast<float>(static_cast<int>(tempC + 0.5f));

  if (!force && cache.drawn && cache.pct == pctQ && cache.temp == tempQ &&
      cache.hasTemp == hasTemp) {
    return;
  }

  int x = 0;
  int y = 0;
  bubbleRect(index, x, y);

  drawSoftBubble(tft, x, y, BUBBLE_W, BUBBLE_H, Theme::bgPanel);

  // Label chip
  tft.fillRoundRect(x + 10, y + 8, 46, 16, 8, Theme::accentSoft);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
  tft.drawString(label, x + 33, y + 16, 2);

  // Temp chip (optional)
  if (hasTemp) {
    char tempBuf[12];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%dC", static_cast<int>(tempQ));
    } else {
      snprintf(tempBuf, sizeof(tempBuf), "--");
    }
    const uint16_t tc = Theme::tempColorFor(tempC);
    tft.fillRoundRect(x + BUBBLE_W - 48, y + 8, 38, 16, 8, Theme::barTrack);
    tft.setTextColor(tc, Theme::barTrack);
    tft.drawString(tempBuf, x + BUBBLE_W - 29, y + 16, 2);
  }

  // Big percentage
  char value[12];
  snprintf(value, sizeof(value), "%d%%", static_cast<int>(pctQ));
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(value, x + BUBBLE_W / 2, y + BUBBLE_H / 2 + 2, 4);

  // Capsule bar
  drawCapsuleBar(tft, x + 12, y + BUBBLE_H - 20, BUBBLE_W - 24, BAR_H, pct);

  tft.setTextDatum(TL_DATUM);
  cache.pct = pctQ;
  cache.temp = tempQ;
  cache.hasTemp = hasTemp;
  cache.drawn = true;
}

void MonitorGui::drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const char *statusLine) {
  const char *status = statusLine ? statusLine : "";
  if (strcmp(lastStatus_, status) == 0 && lastFps_ == m.fps && chromeDrawn_) {
    // Still need to paint once after chrome — handled by empty lastStatus_
  }

  const bool same =
      chromeDrawn_ && strcmp(lastStatus_, status) == 0 && lastFps_ == m.fps && lastStatus_[0] != '\0';
  if (same) {
    return;
  }

  const int x = OUTER_PAD;
  const int y = FOOTER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;

  drawSoftBubble(tft, x, y, w, FOOTER_H, Theme::bgPanel);

  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textMuted, Theme::bgPanel);
  tft.drawString(status, x + 12, y + FOOTER_H / 2, 2);

  if (m.fps > 0) {
    char fpsBuf[16];
    snprintf(fpsBuf, sizeof(fpsBuf), "%u FPS", static_cast<unsigned>(m.fps));
    tft.fillRoundRect(x + w - 70, y + 5, 58, 16, 8, Theme::accentSoft);
    tft.setTextDatum(MC_DATUM);
    tft.setTextColor(Theme::textPrimary, Theme::accentSoft);
    tft.drawString(fpsBuf, x + w - 41, y + FOOTER_H / 2, 2);
  }

  tft.setTextDatum(TL_DATUM);
  strncpy(lastStatus_, status, sizeof(lastStatus_) - 1);
  lastStatus_[sizeof(lastStatus_) - 1] = '\0';
  lastFps_ = m.fps;
}

void MonitorGui::render(TFT_eSPI &tft, const SystemMetrics &m, bool linked,
                        const char *statusLine) {
  if (!chromeDrawn_) {
    drawChrome(tft);
  }

  if (strcmp(lastHost_, m.hostName) != 0 || lastLinked_ != linked || lastHost_[0] == '\0') {
    drawHeader(tft, m.hostName, linked);
  }

  drawMetricBubble(tft, 0, "CPU", m.cpuLoad, m.cpuTemp, true, false);
  drawMetricBubble(tft, 1, "GPU", m.gpuLoad, m.gpuTemp, true, false);
  drawMetricBubble(tft, 2, "RAM", m.ramUsed, 0.0f, false, false);
  drawMetricBubble(tft, 3, "VRAM", m.vramUsed, 0.0f, false, false);
  drawFooter(tft, m, statusLine);
}
