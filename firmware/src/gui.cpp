#include "gui.h"

#include <cstdio>
#include <cstring>

#include "theme.h"

namespace {
  constexpr int SCREEN_W = 320;
  constexpr int SCREEN_H = 240;
  constexpr int PAD = 8;
  constexpr int HEADER_H = 36;
  constexpr int FOOTER_H = 28;
  constexpr int ROW_H = 42;
  constexpr int BAR_H = 14;
}

void MonitorGui::begin(TFT_eSPI &tft) {
  tft.setRotation(1);  // landscape 320x240
  tft.fillScreen(Theme::bg);
  tft.setTextDatum(TL_DATUM);
  chromeDrawn_ = false;
}

void MonitorGui::showBoot(TFT_eSPI &tft, const char *message) {
  tft.fillScreen(Theme::bg);
  tft.setTextColor(Theme::accent, Theme::bg);
  tft.setTextDatum(MC_DATUM);
  tft.drawString("CYD MONITOR", SCREEN_W / 2, SCREEN_H / 2 - 18, 4);
  tft.setTextColor(Theme::textMuted, Theme::bg);
  tft.drawString(message, SCREEN_W / 2, SCREEN_H / 2 + 14, 2);
  tft.setTextDatum(TL_DATUM);
  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastLinked_ = false;
}

void MonitorGui::drawChrome(TFT_eSPI &tft) {
  tft.fillScreen(Theme::bg);

  // Header band
  tft.fillRect(0, 0, SCREEN_W, HEADER_H, Theme::bgPanel);
  tft.drawFastHLine(0, HEADER_H, SCREEN_W, Theme::border);

  // Footer band
  tft.fillRect(0, SCREEN_H - FOOTER_H, SCREEN_W, FOOTER_H, Theme::bgPanel);
  tft.drawFastHLine(0, SCREEN_H - FOOTER_H, SCREEN_W, Theme::border);

  chromeDrawn_ = true;
}

void MonitorGui::drawHeader(TFT_eSPI &tft, const char *host, bool linked) {
  tft.fillRect(0, 0, SCREEN_W, HEADER_H, Theme::bgPanel);
  tft.drawFastHLine(0, HEADER_H, SCREEN_W, Theme::border);

  tft.setTextColor(Theme::accent, Theme::bgPanel);
  tft.drawString("CYD", PAD, 10, 2);

  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  char title[40];
  snprintf(title, sizeof(title), "  %s", host && host[0] ? host : "PC");
  tft.drawString(title, PAD + 36, 10, 2);

  // Link pill
  const int pillW = 72;
  const int pillX = SCREEN_W - PAD - pillW;
  const uint16_t pillColor = linked ? Theme::ok : Theme::textMuted;
  tft.fillRoundRect(pillX, 8, pillW, 20, 4, Theme::bg);
  tft.drawRoundRect(pillX, 8, pillW, 20, 4, pillColor);
  tft.setTextColor(pillColor, Theme::bg);
  tft.setTextDatum(MC_DATUM);
  tft.drawString(linked ? "LINKED" : "WAIT", pillX + pillW / 2, 18, 2);
  tft.setTextDatum(TL_DATUM);

  strncpy(lastHost_, host ? host : "", sizeof(lastHost_) - 1);
  lastHost_[sizeof(lastHost_) - 1] = '\0';
  lastLinked_ = linked;
}

void MonitorGui::drawBar(TFT_eSPI &tft, int x, int y, int w, int h, float pct) {
  if (pct < 0.0f) pct = 0.0f;
  if (pct > 100.0f) pct = 100.0f;

  tft.fillRoundRect(x, y, w, h, 3, Theme::barTrack);
  const int fillW = static_cast<int>((w * pct) / 100.0f);
  if (fillW > 0) {
    tft.fillRoundRect(x, y, fillW, h, 3, Theme::barColorFor(pct));
  }
  tft.drawRoundRect(x, y, w, h, 3, Theme::border);
}

void MonitorGui::drawMetricRow(TFT_eSPI &tft, int y, const char *label, float pct,
                               float tempC, bool hasTemp) {
  const int contentTop = HEADER_H + 4;
  const int rowY = contentTop + y;

  // Clear row area
  tft.fillRect(PAD, rowY, SCREEN_W - PAD * 2, ROW_H - 2, Theme::bg);

  tft.setTextColor(Theme::textMuted, Theme::bg);
  tft.drawString(label, PAD, rowY + 2, 2);

  char value[24];
  snprintf(value, sizeof(value), "%4.0f%%", pct);
  tft.setTextColor(Theme::textPrimary, Theme::bg);
  tft.setTextDatum(TR_DATUM);
  tft.drawString(value, SCREEN_W - PAD - (hasTemp ? 56 : 0), rowY + 2, 2);

  if (hasTemp) {
    char tempBuf[16];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%3.0fC", tempC);
    } else {
      snprintf(tempBuf, sizeof(tempBuf), " -- ");
    }
    tft.setTextColor(Theme::tempColorFor(tempC), Theme::bg);
    tft.drawString(tempBuf, SCREEN_W - PAD, rowY + 2, 2);
  }
  tft.setTextDatum(TL_DATUM);

  drawBar(tft, PAD, rowY + 20, SCREEN_W - PAD * 2, BAR_H, pct);
}

void MonitorGui::drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const char *statusLine) {
  const int y = SCREEN_H - FOOTER_H;
  tft.fillRect(0, y, SCREEN_W, FOOTER_H, Theme::bgPanel);
  tft.drawFastHLine(0, y, SCREEN_W, Theme::border);

  tft.setTextColor(Theme::textMuted, Theme::bgPanel);
  tft.drawString(statusLine ? statusLine : "", PAD, y + 8, 2);

  if (m.fps > 0) {
    char fpsBuf[16];
    snprintf(fpsBuf, sizeof(fpsBuf), "%u FPS", static_cast<unsigned>(m.fps));
    tft.setTextColor(Theme::accentSoft, Theme::bgPanel);
    tft.setTextDatum(TR_DATUM);
    tft.drawString(fpsBuf, SCREEN_W - PAD, y + 8, 2);
    tft.setTextDatum(TL_DATUM);
  }
}

void MonitorGui::render(TFT_eSPI &tft, const SystemMetrics &m, bool linked,
                        const char *statusLine) {
  if (!chromeDrawn_) {
    drawChrome(tft);
  }

  if (strcmp(lastHost_, m.hostName) != 0 || lastLinked_ != linked) {
    drawHeader(tft, m.hostName, linked);
  }

  drawMetricRow(tft, 0, "CPU", m.cpuLoad, m.cpuTemp, true);
  drawMetricRow(tft, ROW_H, "GPU", m.gpuLoad, m.gpuTemp, true);
  drawMetricRow(tft, ROW_H * 2, "RAM", m.ramUsed, 0.0f, false);
  drawMetricRow(tft, ROW_H * 3, "VRAM", m.vramUsed, 0.0f, false);
  drawFooter(tft, m, statusLine);
}
