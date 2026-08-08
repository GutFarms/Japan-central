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
  void drawHeader(TFT_eSPI &tft, const char *host, bool linked);
  void drawMetricRow(TFT_eSPI &tft, int y, const char *label, float pct, float tempC, bool hasTemp);
  void drawBar(TFT_eSPI &tft, int x, int y, int w, int h, float pct);
  void drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const char *statusLine);

  bool chromeDrawn_ = false;
  char lastHost_[24] = "";
  bool lastLinked_ = false;
};
