#pragma once

#include <Arduino.h>
#include "metrics.h"

enum class HostMessageKind : uint8_t {
  None = 0,
  Hello,
  Metrics,
  Command,
};

// USB CDC / UART link: one UTF-8 JSON object per line (NDJSON), 115200 baud.
class SerialLink {
 public:
  void begin(uint32_t baud = 115200);
  // Poll bytes; kind indicates what the last completed line was.
  // Metrics lines update `metrics`. Command/hello lines leave raw JSON in lastLine().
  HostMessageKind poll(SystemMetrics &metrics);
  void sendHelloAck(const char *cfgJson = nullptr);
  void sendRaw(const char *line);
  const char *lastLine() const { return lineBuf_; }
  size_t lastLineLen() const { return lastLen_; }

 private:
  char lineBuf_[512];
  size_t lineLen_ = 0;
  size_t lastLen_ = 0;
};
