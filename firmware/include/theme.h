#pragma once

#include <TFT_eSPI.h>

// Dark + light blue visual language for the CYD dial HUD.
namespace Theme {
  // Background depth
  constexpr uint16_t bg          = 0x00C8;  // #001844 deep navy
  constexpr uint16_t bgDeep      = 0x0064;  // #000C32 darker well
  constexpr uint16_t bgMid       = 0x11B0;  // #113888 mid night blue
  constexpr uint16_t bgPanel     = 0x1A74;  // #1A3A9A raised panel
  constexpr uint16_t bgPanelHi   = 0x2B5A;  // #2A5ACD light panel sheen
  constexpr uint16_t bubbleGlow  = 0x1A9A;  // soft blue halo

  // Light blues / accents
  constexpr uint16_t border      = 0x7D5C;  // pale steel blue rim
  constexpr uint16_t accent      = 0x5E7F;  // bright sky blue
  constexpr uint16_t accentSoft  = 0x3C9F;  // medium azure
  constexpr uint16_t accentPale  = 0xA6FF;  // icy light blue
  constexpr uint16_t accentGlow  = 0x867F;  // soft highlight

  constexpr uint16_t textPrimary = 0xEF7F;  // cool white-blue
  constexpr uint16_t textMuted   = 0x8D3A;  // muted light blue

  constexpr uint16_t ok          = 0x3E1A;  // blue-green live
  constexpr uint16_t warn        = 0xFE4A;  // amber (sparing)
  constexpr uint16_t danger      = 0xFACC;  // soft red (sparing)

  constexpr uint16_t dialFace    = 0x016B;  // dark dial face
  constexpr uint16_t dialRing    = 0x4BDF;  // light blue ring
  constexpr uint16_t dialRingLo  = 0x2256;  // darker ring underlay
  constexpr uint16_t barTrack    = 0x114A;  // inset track
  constexpr uint16_t barFillLo   = 0x3C3F;  // light soft blue
  constexpr uint16_t barFillMid  = 0x4D7F;  // brighter mid blue
  constexpr uint16_t barFillHi   = 0x65BF;  // vivid light blue

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
