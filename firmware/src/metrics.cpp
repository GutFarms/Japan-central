#include "metrics.h"

#include <cstring>

#include <ArduinoJson.h>

bool parseMetricsJson(const char *json, size_t len, SystemMetrics &out) {
  JsonDocument doc;
  const DeserializationError err = deserializeJson(doc, json, len);
  if (err) {
    return false;
  }

  const int version = doc["v"] | 1;
  if (version < 1) {
    return false;
  }

  out.cpuLoad = doc["cpu"] | out.cpuLoad;
  out.cpuTemp = doc["cpu_temp"] | out.cpuTemp;
  out.ramUsed = doc["ram"] | out.ramUsed;
  out.gpuLoad = doc["gpu"] | out.gpuLoad;
  out.gpuTemp = doc["gpu_temp"] | out.gpuTemp;
  out.vramUsed = doc["vram"] | out.vramUsed;
  out.diskUsed = doc["disk"] | out.diskUsed;
  out.swapUsed = doc["swap"] | out.swapUsed;
  out.netUp = doc["net_up"] | out.netUp;
  out.netDown = doc["net_down"] | out.netDown;
  out.fps = doc["fps"] | out.fps;

  const char *host = doc["host"] | "PC";
  strncpy(out.hostName, host, sizeof(out.hostName) - 1);
  out.hostName[sizeof(out.hostName) - 1] = '\0';

  out.lastUpdateMs = millis();
  out.valid = true;
  return true;
}

bool metricsAreStale(const SystemMetrics &m, uint32_t nowMs, uint32_t staleMs) {
  if (!m.valid) {
    return true;
  }
  return (nowMs - m.lastUpdateMs) > staleMs;
}
