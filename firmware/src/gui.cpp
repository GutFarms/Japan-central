#include "gui.h"

#include <cstdio>
#include <cstring>
#include <math.h>

#include "theme.h"

namespace {
constexpr int SCREEN_W = 320;
constexpr int SCREEN_H = 240;
constexpr int OUTER_PAD = 3;
constexpr int HEADER_Y = 2;
constexpr int HEADER_H = 28;
constexpr int FOOTER_H = 28;
constexpr int FOOTER_Y = SCREEN_H - OUTER_PAD - FOOTER_H;
constexpr int GRID_TOP = HEADER_Y + HEADER_H + 2;
constexpr int GAP = 5;
// Large CPU gauge dominates mid/top-left; two compact circular gauges on the right.
constexpr int LARGE_W = 178;
constexpr int LARGE_H = FOOTER_Y - GRID_TOP;          // fills content band
constexpr int SMALL_W = SCREEN_W - OUTER_PAD * 2 - LARGE_W - GAP;
constexpr int SMALL_H = (LARGE_H - GAP) / 2;
constexpr int RADIUS = 14;
constexpr float ARC_START = 225.0f;
constexpr float ARC_SWEEP = 270.0f;
constexpr float SMOOTH = 0.30f;

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
  lastNet_ = -1.0f;
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
  // Quiet stage: deep top wash, subtle glow — no busy bubbles.
  tft.fillScreen(Theme::bg);
  tft.fillRect(0, 0, SCREEN_W, 56, Theme::bgDeep);
  tft.fillCircle(64, 120, 90, Theme::bgDeep);
  tft.fillCircle(280, 40, 50, Theme::bubbleGlow);
}

void MonitorGui::drawChip(TFT_eSPI &tft, int x, int y, int w, int h, const char *text, uint16_t fill,
                          uint16_t fg) {
  tft.fillSmoothRoundRect(x, y, w, h, h / 2, fill, Theme::bgPanel);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(fg, fill);
  tft.drawString(text, x + w / 2, y + h / 2, 1);
}

void MonitorGui::showBoot(TFT_eSPI &tft, const char *message) {
  tft.fillScreen(Theme::bg);
  drawDecor(tft);
  const int cx = SCREEN_W / 2;
  const int cy = SCREEN_H / 2 - 8;
  tft.fillSmoothCircle(cx, cy, 58, Theme::dialRingLo, Theme::bg);
  tft.fillSmoothCircle(cx, cy, 50, Theme::bgPanel, Theme::dialRingLo);
  tft.fillSmoothCircle(cx, cy, 42, Theme::dialFace, Theme::bgPanel);
  tft.drawSmoothArc(cx, cy, 56, 50, 30, 330, Theme::accent, Theme::bgPanel, true);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::dialFace);
  tft.drawString("CYD", cx, cy - 6, 4);
  tft.setTextColor(Theme::accentPale, Theme::dialFace);
  tft.drawString("PC MONITOR", cx, cy + 16, 1);
  tft.fillSmoothRoundRect(cx - 118, SCREEN_H - 40, 236, 28, 14, Theme::bgPanel, Theme::bg);
  tft.setTextColor(Theme::accentPale, Theme::bgPanel);
  tft.drawString(message, cx, SCREEN_H - 26, 2);
  tft.setTextDatum(TL_DATUM);
  chromeDrawn_ = false;
  lastHost_[0] = '\0';
  lastStatus_[0] = '\0';
  lastPps_ = 0xFFFF;
  lastVram_ = -1.0f;
  lastNet_ = -1.0f;
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
  lastNet_ = -1.0f;
  for (auto &d : dials_) {
    d.lastDrawn = -999.0f;
  }
}

void MonitorGui::drawHeader(TFT_eSPI &tft, const char *host, bool linked) {
  const int x = OUTER_PAD;
  const int y = HEADER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  tft.fillSmoothRoundRect(x, y, w, HEADER_H, 10, Theme::bgPanel, Theme::bg);
  tft.fillSmoothRoundRect(x + 6, y + 5, 46, 18, 9, Theme::accent, Theme::bgPanel);
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::accent);
  tft.drawString("CYD", x + 29, y + HEADER_H / 2, 2);

  tft.setTextDatum(ML_DATUM);
  tft.setTextColor(Theme::textPrimary, Theme::bgPanel);
  tft.drawString(host && host[0] ? host : "PC", x + 60, y + HEADER_H / 2, 2);

  const int pillW = 78;
  const int pillX = x + w - pillW - 6;
  const uint16_t pillFill = linked ? Theme::ok : Theme::chipIdle;
  tft.fillSmoothRoundRect(pillX, y + 5, pillW, 18, 9, pillFill, Theme::bgPanel);
  if (linked) {
    tft.fillSmoothCircle(pillX + 12, y + HEADER_H / 2, 3, Theme::accentPale, pillFill);
  }
  tft.setTextDatum(MC_DATUM);
  tft.setTextColor(Theme::textPrimary, pillFill);
  tft.drawString(linked ? "LIVE" : "WAIT", pillX + pillW / 2 + 6, y + HEADER_H / 2, 2);
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
  const uint32_t a0 = toTftAngle(startMath);
  const uint32_t a1 = toTftAngle(endMath);
  if (a0 == a1) {
    return;
  }
  spr.drawSmoothArc(cx, cy, rOuter, rInner, a0, a1, fg, bg, true);
}

void MonitorGui::paintDialSprite(int w, int h, bool large, const char *label, float pct, float tempC,
                                 bool showTemp) {
  TFT_eSprite &spr = *dialSpr_;
  spr.fillSprite(Theme::bg);
  spr.fillSmoothRoundRect(0, 0, w, h, RADIUS, Theme::bgPanel, Theme::bg);

  // Circle sits slightly high so value/temp read cleanly under the hub.
  const int cx = w / 2;
  const int cy = large ? (h / 2 + 2) : (h / 2 + 3);
  const int r = (w < h ? w : h) / 2 - (large ? 8 : 7);

  spr.fillSmoothCircle(cx, cy, r + 4, Theme::dialRingLo, Theme::bgPanel);
  spr.fillSmoothCircle(cx, cy, r + 1, Theme::dialRing, Theme::dialRingLo);
  spr.fillSmoothCircle(cx, cy, r - 2, Theme::dialFace, Theme::dialRing);

  const int tickStep = large ? 10 : 20;
  for (int i = 0; i <= 100; i += tickStep) {
    const float deg = ARC_START - ARC_SWEEP * (i / 100.0f);
    const bool major = (i % 20 == 0) || i == 0 || i == 100;
    float x0, y0, x1, y1;
    const float inner = r - (major ? (large ? 15.0f : 11.0f) : (large ? 9.0f : 7.0f));
    polar(cx, cy, deg, inner, x0, y0);
    polar(cx, cy, deg, r - 3.0f, x1, y1);
    const uint16_t tickCol = (i >= 90) ? Theme::danger : (i >= 70 ? Theme::warn : Theme::accentPale);
    spr.drawWideLine(x0, y0, x1, y1, major ? 2.4f : 1.3f, tickCol, Theme::dialFace);
  }

  // Readable scale on the hero dial only.
  if (large) {
    spr.setTextDatum(MC_DATUM);
    spr.setTextColor(Theme::textMuted, Theme::dialFace);
    float lx, ly;
    polar(cx, cy, ARC_START, r - 28.0f, lx, ly);
    spr.drawString("0", static_cast<int>(lx), static_cast<int>(ly), 1);
    polar(cx, cy, ARC_START - ARC_SWEEP * 0.5f, r - 26.0f, lx, ly);
    spr.drawString("50", static_cast<int>(lx), static_cast<int>(ly), 1);
    polar(cx, cy, ARC_START - ARC_SWEEP, r - 28.0f, lx, ly);
    spr.drawString("100", static_cast<int>(lx), static_cast<int>(ly), 1);
  }

  const int trackOuter = r - 3;
  const int trackInner = r - (large ? 13 : 11);
  drawSmoothGaugeArc(spr, cx, cy, trackOuter, trackInner, ARC_START, ARC_START - ARC_SWEEP,
                     Theme::barTrack, Theme::dialFace);

  if (pct > 0.35f) {
    const float endDeg = ARC_START - ARC_SWEEP * (pct / 100.0f);
    drawSmoothGaugeArc(spr, cx, cy, trackOuter, trackInner, ARC_START, endDeg, Theme::barColorFor(pct),
                       Theme::dialFace);
    if (large) {
      drawSmoothGaugeArc(spr, cx, cy, trackInner, trackInner - 3, ARC_START, endDeg, Theme::accentGlow,
                         Theme::dialFace);
    }
  }

  // Tapered needle — thick at hub, fine at tip.
  const float needleDeg = ARC_START - ARC_SWEEP * (pct / 100.0f);
  float nx, ny;
  polar(cx, cy, needleDeg, r - (large ? 18.0f : 14.0f), nx, ny);
  const float hubW = large ? 4.2f : 3.0f;
  const float tipW = large ? 1.0f : 0.8f;
  spr.drawWedgeLine(static_cast<float>(cx), static_cast<float>(cy), nx, ny, hubW, tipW,
                    Theme::textPrimary, Theme::dialFace);
  spr.fillSmoothCircle(cx, cy, large ? 7 : 5, Theme::accentSoft, Theme::dialFace);
  spr.fillSmoothCircle(cx, cy, large ? 3 : 2, Theme::accentPale, Theme::accentSoft);

  // Title sits in the panel above the arc opening.
  spr.setTextDatum(MC_DATUM);
  spr.setTextColor(Theme::accentPale, Theme::bgPanel);
  spr.drawString(label, cx, large ? 11 : 8, 2);

  char value[12];
  snprintf(value, sizeof(value), "%d%%", static_cast<int>(pct + 0.5f));
  spr.setTextColor(Theme::textPrimary, Theme::dialFace);
  spr.drawString(value, cx, cy + (large ? 22 : 15), large ? 4 : 2);

  if (showTemp) {
    char tempBuf[12];
    if (tempC > 0.0f) {
      snprintf(tempBuf, sizeof(tempBuf), "%dC", static_cast<int>(tempC + 0.5f));
    } else {
      snprintf(tempBuf, sizeof(tempBuf), "--C");
    }
    const int tw = large ? 48 : 40;
    const int th = large ? 16 : 13;
    const int ty = h - th - 5;
    const uint16_t tfill = Theme::bgPanelHi;
    spr.fillSmoothRoundRect(cx - tw / 2, ty, tw, th, th / 2, tfill, Theme::bgPanel);
    spr.setTextColor(Theme::tempColorFor(tempC), tfill);
    spr.drawString(tempBuf, cx, ty + th / 2, 1);
  } else if (!large) {
    // RAM: secondary cue under value so the dial still feels complete.
    spr.setTextColor(Theme::textMuted, Theme::dialFace);
    spr.drawString("mem", cx, cy + 28, 1);
  }
}

void MonitorGui::drawDial(TFT_eSPI &tft, int index, const char *label, bool showTemp) {
  DialState &d = dials_[index];
  d.shown = approach(d.shown, d.target, SMOOTH);
  if (fabsf(d.shown - d.lastDrawn) < 0.30f && fabsf(d.temp - d.lastTemp) < 0.4f && d.lastDrawn > -900.0f) {
    return;
  }

  const DialGeom g = dialGeom(index);
  if (spriteReady_ && dialSpr_ != nullptr) {
    paintDialSprite(g.w, g.h, g.large, label, d.shown, d.temp, showTemp);
    dialSpr_->pushSprite(g.x, g.y, 0, 0, g.w, g.h);
  } else {
    tft.fillSmoothRoundRect(g.x, g.y, g.w, g.h, RADIUS, Theme::bgPanel, Theme::bg);
  }
  d.lastDrawn = d.shown;
  d.lastTemp = d.temp;
}

void MonitorGui::drawFooter(TFT_eSPI &tft, const SystemMetrics &m, const LinkStats &link,
                            const char *statusLine) {
  const char *status = statusLine ? statusLine : "";
  const bool same = chromeDrawn_ && lastStatus_[0] != '\0' && strcmp(lastStatus_, status) == 0 &&
                    lastPps_ == link.pps && lastDisk_ == m.diskUsed && lastVram_ == m.vramUsed &&
                    fabsf(lastNet_ - m.netDown) < 0.05f;
  if (same) {
    return;
  }

  const int x = OUTER_PAD;
  const int y = FOOTER_Y;
  const int w = SCREEN_W - OUTER_PAD * 2;
  tft.fillSmoothRoundRect(x, y, w, FOOTER_H, 10, Theme::bgPanel, Theme::bg);

  // Intuitive chips: link · VRAM · Disk · Net (glanceable, not a dense sentence).
  char linkBuf[20];
  if (status[0] != '\0') {
    snprintf(linkBuf, sizeof(linkBuf), "%s", status);
  } else {
    snprintf(linkBuf, sizeof(linkBuf), "—");
  }
  // Truncate long IP for chip width.
  if (strlen(linkBuf) > 11) {
    linkBuf[11] = '\0';
  }

  char vramBuf[16];
  snprintf(vramBuf, sizeof(vramBuf), "VRAM %d%%", static_cast<int>(m.vramUsed + 0.5f));
  char diskBuf[16];
  snprintf(diskBuf, sizeof(diskBuf), "DISK %d%%", static_cast<int>(m.diskUsed + 0.5f));
  char netBuf[16];
  if (m.netDown > 0.05f) {
    snprintf(netBuf, sizeof(netBuf), "%.1f Mb", static_cast<double>(m.netDown));
  } else {
    snprintf(netBuf, sizeof(netBuf), "NET —");
  }

  const int chipY = y + 5;
  const int chipH = 18;
  const int gap = 4;
  int cx = x + 5;
  const int linkW = 72;
  const int vramW = 70;
  const int diskW = 66;
  const int netW = w - (5 + linkW + gap + vramW + gap + diskW + gap + 5);

  drawChip(tft, cx, chipY, linkW, chipH, linkBuf, Theme::accentSoft, Theme::textPrimary);
  cx += linkW + gap;
  drawChip(tft, cx, chipY, vramW, chipH, vramBuf, Theme::chipColorFor(m.vramUsed), Theme::textPrimary);
  cx += vramW + gap;
  drawChip(tft, cx, chipY, diskW, chipH, diskBuf, Theme::chipColorFor(m.diskUsed), Theme::textPrimary);
  cx += diskW + gap;
  drawChip(tft, cx, chipY, netW, chipH, netBuf, Theme::chipIdle, Theme::accentPale);

  tft.setTextDatum(TL_DATUM);
  strncpy(lastStatus_, status, sizeof(lastStatus_) - 1);
  lastStatus_[sizeof(lastStatus_) - 1] = '\0';
  lastPps_ = link.pps;
  lastDisk_ = m.diskUsed;
  lastVram_ = m.vramUsed;
  lastNet_ = m.netDown;
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
