#pragma once

#include <TFT_eSPI.h>

// Soft blue "bubbly" visual language for the CYD performance HUD.
namespace Theme {
  constexpr uint16_t bg          = 0x0929;  // deep midnight blue
  constexpr uint16_t bgDeep      = 0x0108;  // near-black navy (shadows)
  constexpr uint16_t bgPanel     = 0x1A55;  // raised bubble fill
  constexpr uint16_t bgPanelHi   = 0x22B8;  // lighter bubble top
  constexpr uint16_t bubbleGlow  = 0x11B4;  // soft outer glow ring
  constexpr uint16_t border      = 0x54BA;  // light steel rim
  constexpr uint16_t accent      = 0x4DFF;  // bright azure
  constexpr uint16_t accentSoft  = 0x3D9F;  // mid azure
  constexpr uint16_t accentPale  = 0x8E7F;  // pale sky
  constexpr uint16_t textPrimary = 0xEF7D;  // cool white
  constexpr uint16_t textMuted   = 0x84D8;  // soft slate
  constexpr uint16_t ok          = 0x2E9A;  // minty blue-green
  constexpr uint16_t warn        = 0xFD60;  // soft amber
  constexpr uint16_t danger      = 0xF98C;  // soft coral-red
  constexpr uint16_t barTrack    = 0x114A;  // inset track
  constexpr uint16_t barFillLo   = 0x34BF;  // soft blue
  constexpr uint16_t barFillMid  = 0x4DDF;  // brighter blue
  constexpr uint16_t barFillHi   = 0x061F;  // electric pop

  inline uint16_t barColorFor(float pct) {
    if (pct >= 90.0f) return danger;
    if (pct >= 75.0f) return warn;
    if (pct >= 60.0f) return barFillHi;
    if (pct >= 45.0f) return barFillMid;
    return barFillLo;
  }

  inline uint16_t tempColorFor(float celsius) {
    if (celsius <= 0.0f) return textMuted;
    if (celsius >= 90.0f) return danger;
    if (celsius >= 75.0f) return warn;
    return accentPale;
  }
}
