#pragma once

#include <TFT_eSPI.h>

// Dark + light blue dial HUD.
namespace Theme {
  constexpr uint16_t bg          = 0x00C8;
  constexpr uint16_t bgDeep      = 0x0064;
  constexpr uint16_t bgMid       = 0x11B0;
  constexpr uint16_t bgPanel     = 0x1A74;
  constexpr uint16_t bgPanelHi   = 0x2B5A;
  constexpr uint16_t bubbleGlow  = 0x1A9A;
  constexpr uint16_t border      = 0x7D5C;
  constexpr uint16_t accent      = 0x5E7F;
  constexpr uint16_t accentSoft  = 0x3C9F;
  constexpr uint16_t accentPale  = 0xA6FF;
  constexpr uint16_t accentGlow  = 0x867F;
  constexpr uint16_t textPrimary = 0xEF7F;
  constexpr uint16_t textMuted   = 0x8D3A;
  constexpr uint16_t ok          = 0x3E1A;
  constexpr uint16_t warn        = 0xFE4A;
  constexpr uint16_t danger      = 0xFACC;
  constexpr uint16_t dialFace    = 0x016B;
  constexpr uint16_t dialRing    = 0x4BDF;
  constexpr uint16_t dialRingLo  = 0x2256;
  constexpr uint16_t barTrack    = 0x114A;
  constexpr uint16_t barFillLo   = 0x3C3F;
  constexpr uint16_t barFillMid  = 0x4D7F;
  constexpr uint16_t barFillHi   = 0x65BF;

  inline uint16_t barColorFor(float pct) {
    if (pct >= 90.0f) return danger;
    if (pct >= 75.0f) return warn;
    if (pct >= 55.0f) return barFillHi;
    if (pct >= 30.0f) return barFillMid;
    return barFillLo;
  }

  inline uint16_t tempColorFor(float celsius) {
    if (celsius <= 0.0f) return textMuted;
    if (celsius >= 90.0f) return danger;
    if (celsius >= 75.0f) return warn;
    return accentPale;
  }
}
