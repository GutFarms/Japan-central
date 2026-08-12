#pragma once

#include <Arduino.h>

// Shared wire format with host agent / desktop app — keep fields in sync.
// {
//   "v":1,"seq":12,
//   "cpu":42.5,"cpu_temp":61,"ram":58,"gpu":71,"gpu_temp":68,"vram":44,
//   "disk":62,"swap":10,"net_up":1.2,"net_down":5.4,"fps":0,"host":"DESKTOP"
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
  float netUp = 0.0f;
  float netDown = 0.0f;
  uint16_t fps = 0;
  uint32_t seq = 0;
  char hostName[24] = "PC";
  uint32_t lastUpdateMs = 0;
  bool valid = false;
};

struct LinkStats {
  uint32_t rxCount = 0;
  uint32_t lastSeq = 0;
  uint16_t pps = 0;  // packets per second (approx)
  uint32_t windowStartMs = 0;
  uint16_t windowCount = 0;
};

bool parseMetricsJson(const char *json, size_t len, SystemMetrics &out);
bool metricsAreStale(const SystemMetrics &m, uint32_t nowMs, uint32_t staleMs);
void notePacketReceived(LinkStats &stats, uint32_t seq, uint32_t nowMs);
