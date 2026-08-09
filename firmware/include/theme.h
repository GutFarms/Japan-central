#pragma once

#include <TFT_eSPI.h>

// Deep navy HUD — high contrast dials, muted chrome.
namespace Theme {
  constexpr uint16_t bg          = 0x00A3;  // #001418-ish navy-black
  constexpr uint16_t bgDeep      = 0x0041;  // deepest stage
  constexpr uint16_t bgMid       = 0x00E4;  // lower band
  constexpr uint16_t bgPanel     = 0x0929;  // panel
  constexpr uint16_t bgPanelHi   = 0x114B;  // panel highlight
  constexpr uint16_t bubbleGlow  = 0x08AA;  // soft glow
  constexpr uint16_t border      = 0x3AB6;  // edge
  constexpr uint16_t accent      = 0x2B1D;  // primary action blue
  constexpr uint16_t accentSoft  = 0x19F8;  // soft blue fill
  constexpr uint16_t accentPale  = 0x8DF8;  // labels
  constexpr uint16_t accentGlow  = 0x64DF;  // arc inner glow
  constexpr uint16_t textPrimary = 0xEF7D;  // near-white
  constexpr uint16_t textMuted   = 0x63B3;  // secondary
  constexpr uint16_t ok          = 0x1429;  // live pill
  constexpr uint16_t warn        = 0xFD86;  // amber
  constexpr uint16_t danger      = 0xE9E8;  // alert red
  constexpr uint16_t dialFace    = 0x00A3;  // face matches stage
  constexpr uint16_t dialRing    = 0x22F6;  // ring
  constexpr uint16_t dialRingLo  = 0x10A5;  // outer shade
  constexpr uint16_t barTrack    = 0x0865;  // empty arc
  constexpr uint16_t barFillLo   = 0x1A18;
  constexpr uint16_t barFillMid  = 0x2B5D;
  constexpr uint16_t barFillHi   = 0x4C1F;
  constexpr uint16_t chipIdle    = 0x10A5;
  constexpr uint16_t chipHot     = 0x48C4;

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

  inline uint16_t chipColorFor(float pct) {
    if (pct >= 90.0f) return danger;
    if (pct >= 75.0f) return warn;
    if (pct >= 50.0f) return chipHot;
    return chipIdle;
  }
}
