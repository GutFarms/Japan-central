#include "serial_link.h"

#include <ArduinoJson.h>
#include <cstring>

void SerialLink::begin(uint32_t baud) {
  Serial.begin(baud);
  lineLen_ = 0;
  delay(50);
}

void SerialLink::sendHelloAck() {
  Serial.println(F("{\"ok\":1,\"fw\":\"cyd-monitor\",\"proto\":1}"));
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

      JsonDocument probe;
      if (!deserializeJson(probe, lineBuf_, lineLen_)) {
        if (probe["hello"] | 0) {
          sendHelloAck();
          lineLen_ = 0;
          continue;
        }
      }

      if (parseMetricsJson(lineBuf_, lineLen_, metrics)) {
        updated = true;
        Serial.printf("{\"ok\":1,\"seq\":%lu}\n", static_cast<unsigned long>(metrics.seq));
      }
      lineLen_ = 0;
      continue;
    }

    if (lineLen_ + 1 >= sizeof(lineBuf_)) {
      lineLen_ = 0;
      continue;
    }

    lineBuf_[lineLen_++] = c;
  }

  return updated;
}
