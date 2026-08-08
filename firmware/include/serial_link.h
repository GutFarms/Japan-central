#pragma once

#include <Arduino.h>
#include "metrics.h"

// USB CDC / UART link: one UTF-8 JSON object per line (NDJSON), 115200 baud.
class SerialLink {
 public:
  void begin(uint32_t baud = 115200);
  // Read available bytes; returns true when a full line was parsed into metrics.
  bool poll(SystemMetrics &metrics);

 private:
  char lineBuf_[512];
  size_t lineLen_ = 0;
};
