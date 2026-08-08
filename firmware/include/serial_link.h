#pragma once

#include <Arduino.h>
#include "metrics.h"

// USB CDC / UART link: one UTF-8 JSON object per line (NDJSON), 115200 baud.
class SerialLink {
 public:
  void begin(uint32_t baud = 115200);
  // Returns true when a metrics line was parsed. Emits ACK {"ok":1,"seq":N}.
  bool poll(SystemMetrics &metrics);
  void sendHelloAck();

 private:
  char lineBuf_[512];
  size_t lineLen_ = 0;
};
