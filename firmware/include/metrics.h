#pragma once

#include <Arduino.h>

// Shared wire format with host/agent.py — keep fields in sync.
// JSON example:
// {
//   "v": 1,
//   "cpu": 42.5,
//   "cpu_temp": 61.0,
//   "ram": 58.2,
//   "gpu": 71.0,
//   "gpu_temp": 68.0,
//   "vram": 44.0,
//   "fps": 144,
//   "host": "DESKTOP"
// }

struct SystemMetrics {
  float cpuLoad = 0.0f;      // 0-100 %
  float cpuTemp = 0.0f;      // °C (0 if unavailable)
  float ramUsed = 0.0f;      // 0-100 %
  float gpuLoad = 0.0f;      // 0-100 %
  float gpuTemp = 0.0f;      // °C
  float vramUsed = 0.0f;     // 0-100 %
  uint16_t fps = 0;          // optional frame counter from host
  char hostName[24] = "PC";  // short host label
  uint32_t lastUpdateMs = 0;
  bool valid = false;
};

bool parseMetricsJson(const char *json, size_t len, SystemMetrics &out);
bool metricsAreStale(const SystemMetrics &m, uint32_t nowMs, uint32_t staleMs);
