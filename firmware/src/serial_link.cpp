#include "serial_link.h"

#include <ArduinoJson.h>
#include <cstring>

void SerialLink::begin(uint32_t baud) {
  Serial.begin(baud);
  lineLen_ = 0;
  lastLen_ = 0;
  delay(50);
}

void SerialLink::sendRaw(const char *line) {
  Serial.println(line);
}

void SerialLink::sendHelloAck(const char *cfgJson) {
  if (cfgJson && cfgJson[0]) {
    // cfgJson already includes ok/cfg wrapper from DeviceSettings::toJson
    Serial.println(cfgJson);
    return;
  }
  Serial.println(F("{\"ok\":1,\"fw\":\"cyd-monitor\",\"proto\":1}"));
}

HostMessageKind SerialLink::poll(SystemMetrics &metrics) {
  HostMessageKind kind = HostMessageKind::None;

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
      lastLen_ = lineLen_;

      JsonDocument probe;
      if (!deserializeJson(probe, lineBuf_, lineLen_)) {
        if (probe["hello"] | 0) {
          kind = HostMessageKind::Hello;
          lineLen_ = 0;
          return kind;
        }
        const char *cmd = probe["cmd"] | "";
        if (cmd[0] != '\0') {
          kind = HostMessageKind::Command;
          lineLen_ = 0;
          return kind;
        }
      }

      if (parseMetricsJson(lineBuf_, lineLen_, metrics)) {
        Serial.printf("{\"ok\":1,\"seq\":%lu}\n", static_cast<unsigned long>(metrics.seq));
        kind = HostMessageKind::Metrics;
      }
      lineLen_ = 0;
      if (kind != HostMessageKind::None) {
        return kind;
      }
      continue;
    }

    if (lineLen_ + 1 >= sizeof(lineBuf_)) {
      lineLen_ = 0;
      continue;
    }

    lineBuf_[lineLen_++] = c;
  }

  return kind;
}
