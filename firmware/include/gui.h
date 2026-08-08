#pragma once

#include <TFT_eSPI.h>
#include "metrics.h"

class MonitorGui {
 public:
  void begin(TFT_eSPI &tft);
  void drawChrome(TFT_eSPI &tft);
  void render(TFT_eSPI &tft, const SystemMetrics &m, bool linked, const char *statusLine);
  void showBoot(TFT_eSPI &tft, const char *message);

 private:
  struct DialCache {
    float pct = -1.0f;
    float temp = -1.0f;
    bool drawn = false;
  };

  void drawDecor(TFT_eSPI &tft);
  void drawHeader(TFT_eSPI &tft, const char *host, bool linked);
  void drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const char *statusLine);
  void drawDial(TFT_eSPI &tft, int index, const char *label, float pct, float tempC, bool showTemp, bool force);
  void drawArcSpan(TFT_eSPI &tft, int cx, int cy, int r, float startDeg, float endDeg, uint16_t color, int width);
  void drawSoftBubble(TFT_eSPI &tft, int x, int y, int w, int h, uint16_t fill);

  bool chromeDrawn_ = false;
  char lastHost_[24] = "";
  bool lastLinked_ = false;
  char lastStatus_[48] = "";
  uint16_t lastFps_ = 0xFFFF;
  float lastDisk_ = -1.0f;
  float lastNetDown_ = -1.0f;
  DialCache dials_[4];
};
