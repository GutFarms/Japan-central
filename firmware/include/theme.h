#pragma once

#include <TFT_eSPI.h>

// Blue-hue visual language for the CYD performance HUD.
namespace Theme {
  // Deep navy canvas → cool blue accents
  constexpr uint16_t bg          = 0x0A2F;  // ~#0A1930
  constexpr uint16_t bgPanel     = 0x11B4;  // ~#122A69 slightly lifted
  constexpr uint16_t border      = 0x3B9A;  // soft steel blue
  constexpr uint16_t accent      = 0x3D9F;  // vivid azure
  constexpr uint16_t accentSoft  = 0x22D6;  // muted cyan-blue
  constexpr uint16_t textPrimary = 0xE73C;  // near-white cool
  constexpr uint16_t textMuted   = 0x7C17;  // slate blue
  constexpr uint16_t ok          = 0x2E7A;  // calm blue-green
  constexpr uint16_t warn        = 0xFD20;  // amber (sparingly)
  constexpr uint16_t danger      = 0xF800;  // red (sparingly)
  constexpr uint16_t barTrack    = 0x1169;  // dark track
  constexpr uint16_t barFillLo   = 0x24BF;  // cool blue
  constexpr uint16_t barFillMid  = 0x3DDF;  // brighter blue
  constexpr uint16_t barFillHi   = 0x051F;  // electric blue

  inline uint16_t barColorFor(float pct) {
    if (pct >= 90.0f) return danger;
    if (pct >= 75.0f) return warn;
    if (pct >= 45.0f) return barFillMid;
    return barFillLo;
  }

  inline uint16_t tempColorFor(float celsius) {
    if (celsius <= 0.0f) return textMuted;
    if (celsius >= 90.0f) return danger;
    if (celsius >= 75.0f) return warn;
    return accentSoft;
  }
}
