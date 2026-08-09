#pragma once
#include <cstdint>
#include <string>
#include <optional>
#include "serial_win.hpp"

struct BoardStatus {
  double hashrateHs = 0;
  uint64_t shares = 0;
  uint32_t accepted = 0;
  uint32_t rejected = 0;
  uint32_t dropped = 0;
  std::string pool;
  bool connected = false;
  std::string wifi;
  std::string ip;
  std::string address;
  std::string stratum;
  uint32_t difficulty = 0;
  uint32_t cpuMhz = 240;
  bool hashFocus = true;
  std::string nonce;
};

struct BoardConfig {
  std::string worker;
  std::string stratum;
  std::string wifiSsid;
  std::string wifiPasswordMasked;
  uint32_t cpuMhz = 240;
  bool hashFocus = true;
  std::string fw;
  bool configured = false;
};

std::string urlEncode(const std::string& s);
std::optional<std::string> usbCmd(SerialPort& port, std::string& rx, const std::string& cmd,
                                  int timeoutMs = 4000);
bool parseCmpStatus(const std::string& line, BoardStatus& out);
bool parseCmpConfig(const std::string& line, BoardConfig& out);
std::string jsonStringField(const std::string& json, const char* key);
double jsonNumberField(const std::string& json, const char* key, double def = 0);
bool jsonBoolField(const std::string& json, const char* key, bool def = false);
