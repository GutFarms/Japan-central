#pragma once

#include <TFT_eSPI.h>
#include "metrics.h"

class MonitorGui {
 public:
  void begin(TFT_eSPI &tft, uint8_t rotation = 1);
  void setRotation(TFT_eSPI &tft, uint8_t rotation);
  void invalidate();
  void drawChrome(TFT_eSPI &tft);
  void render(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link, bool linked,
              const char *statusLine);
  void showBoot(TFT_eSPI &tft, const char *message);

 private:
  struct DialState {
    float target = 0.0f;
    float shown = 0.0f;
    float temp = 0.0f;
    float lastDrawn = -999.0f;
    float lastTemp = -999.0f;
  };

  void drawDecor(TFT_eSPI &tft);
  void drawHeader(TFT_eSPI &tft, const char *host, bool linked);
  void drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link, const char *statusLine);
  void drawDial(TFT_eSPI &tft, int index, const char *label, bool showTemp);
  void paintDialSprite(const char *label, float pct, float tempC, bool showTemp);
  void drawArcSpan(TFT_eSPI &spr, int cx, int cy, int r, float startDeg, float endDeg, uint16_t color,
                   int width);
  void drawSoftBubble(TFT_eSPI &tft, int x, int y, int w, int h, uint16_t fill);

  TFT_eSprite *dialSpr_ = nullptr;
  bool spriteReady_ = false;
  bool chromeDrawn_ = false;
  char lastHost_[24] = "";
  bool lastLinked_ = false;
  char lastStatus_[64] = "";
  uint16_t lastPps_ = 0xFFFF;
  float lastDisk_ = -1.0f;
  DialState dials_[4];
};
