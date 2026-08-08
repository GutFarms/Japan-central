#pragma once

#include <Arduino.h>

// Shared wire format with host agent / desktop app — keep fields in sync.
// Compact JSON example:
// {
//   "v": 1,
//   "cpu": 42.5, "cpu_temp": 61.0,
//   "ram": 58.2,
//   "gpu": 71.0, "gpu_temp": 68.0,
//   "vram": 44.0,
//   "disk": 62.0, "swap": 10.0,
//   "net_up": 1.2, "net_down": 5.4,
//   "fps": 0,
//   "host": "DESKTOP"
// }

struct SystemMetrics {
  float cpuLoad = 0.0f;
  float cpuTemp = 0.0f;
  float ramUsed = 0.0f;
  float gpuLoad = 0.0f;
  float gpuTemp = 0.0f;
  float vramUsed = 0.0f;
  float diskUsed = 0.0f;
  float swapUsed = 0.0f;
  float netUp = 0.0f;    // Mbps
  float netDown = 0.0f;  // Mbps
  uint16_t fps = 0;
  char hostName[24] = "PC";
  uint32_t lastUpdateMs = 0;
  bool valid = false;
};

bool parseMetricsJson(const char *json, size_t len, SystemMetrics &out);
bool metricsAreStale(const SystemMetrics &m, uint32_t nowMs, uint32_t staleMs);
