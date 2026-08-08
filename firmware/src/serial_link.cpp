#include "serial_link.h"

#include <cstring>

void SerialLink::begin(uint32_t baud) {
  Serial.begin(baud);
  lineLen_ = 0;
  delay(50);
}

bool SerialLink::poll(SystemMetrics &metrics) {
  bool updated = false;

  while (Serial.available() > 0) {
    const int raw = Serial.read();
    if (raw < 0) {
      break;
    }

    const char c = static_cast<char>(raw);
    if (c == '\r') {
      continue;
    }

    if (c == '\n') {
      if (lineLen_ == 0) {
        continue;
      }
      lineBuf_[lineLen_] = '\0';
      if (parseMetricsJson(lineBuf_, lineLen_, metrics)) {
        updated = true;
        // Lightweight ACK so the host can confirm the link.
        Serial.println(F("{\"ok\":1}"));
      }
      lineLen_ = 0;
      continue;
    }

    if (lineLen_ + 1 >= sizeof(lineBuf_)) {
      // Overflow — drop the line and resync on the next newline.
      lineLen_ = 0;
      continue;
    }

    lineBuf_[lineLen_++] = c;
  }

  return updated;
}
