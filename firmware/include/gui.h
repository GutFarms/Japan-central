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

  struct DialGeom {
    int x;
    int y;
    int w;
    int h;
    bool large;
  };

  void drawDecor(TFT_eSPI &tft);
  void drawHeader(TFT_eSPI &tft, const char *host, bool linked);
  void drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link, const char *statusLine);
  void drawDial(TFT_eSPI &tft, int index, const char *label, bool showTemp);
  void paintDialSprite(int w, int h, bool large, const char *label, float pct, float tempC, bool showTemp);
  void drawSmoothGaugeArc(TFT_eSPI &spr, int cx, int cy, int rOuter, int rInner, float startMath,
                          float endMath, uint16_t fg, uint16_t bg);
  void drawChip(TFT_eSPI &tft, int x, int y, int w, int h, const char *text, uint16_t fill,
                uint16_t fg);
  DialGeom dialGeom(int index) const;

  TFT_eSprite *dialSpr_ = nullptr;
  bool spriteReady_ = false;
  bool chromeDrawn_ = false;
  char lastHost_[24] = "";
  bool lastLinked_ = false;
  char lastStatus_[40] = "";
  uint16_t lastPps_ = 0xFFFF;
  float lastDisk_ = -1.0f;
  float lastVram_ = -1.0f;
  float lastNet_ = -1.0f;
  DialState dials_[3];  // 0=CPU large, 1=GPU, 2=RAM
};
