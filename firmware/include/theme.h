#pragma once

#include <TFT_eSPI.h>

// Darker navy / deep-blue dial HUD.
namespace Theme {
  constexpr uint16_t bg          = 0x0126;  // near-black navy
  constexpr uint16_t bgDeep      = 0x0082;  // deepest blue
  constexpr uint16_t bgMid       = 0x0149;  // mid navy band
  constexpr uint16_t bgPanel     = 0x0A2E;  // panel navy
  constexpr uint16_t bgPanelHi   = 0x124F;  // soft sheen
  constexpr uint16_t bubbleGlow  = 0x0A4F;  // muted rim glow
  constexpr uint16_t border      = 0x3B38;  // cool steel-blue edge
  constexpr uint16_t accent      = 0x2B5E;  // deep accent blue
  constexpr uint16_t accentSoft  = 0x1A9B;  // soft fill
  constexpr uint16_t accentPale  = 0x7D9C;  // pale label blue
  constexpr uint16_t accentGlow  = 0x54BE;  // arc highlight
  constexpr uint16_t textPrimary = 0xDEFB;  // cool near-white
  constexpr uint16_t textMuted   = 0x63B4;  // muted blue-gray
  constexpr uint16_t ok          = 0x1C0E;  // dark teal live pill
  constexpr uint16_t warn        = 0xFD68;  // amber
  constexpr uint16_t danger      = 0xF28A;  // soft red
  constexpr uint16_t dialFace    = 0x00C5;  // dark dial face
  constexpr uint16_t dialRing    = 0x2B5E;  // deep ring
  constexpr uint16_t dialRingLo  = 0x1129;  // outer ring shade
  constexpr uint16_t barTrack    = 0x08C7;  // track under arc
  constexpr uint16_t barFillLo   = 0x1A9B;
  constexpr uint16_t barFillMid  = 0x2B7E;
  constexpr uint16_t barFillHi   = 0x43DF;

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
