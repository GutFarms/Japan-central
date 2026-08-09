#include "protocol.hpp"
#include <windows.h>
#include <cctype>
#include <cstdlib>

std::string urlEncode(const std::string& s) {
  static const char* hex = "0123456789ABCDEF";
  std::string out;
  out.reserve(s.size() * 3);
  for (unsigned char c : s) {
    if (std::isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~') {
      out.push_back((char)c);
    } else if (c == ' ') {
      out.push_back('+');
    } else {
      out.push_back('%');
      out.push_back(hex[c >> 4]);
      out.push_back(hex[c & 0xf]);
    }
  }
  return out;
}

static void drain(SerialPort& port, std::string& rx) {
  for (;;) {
    std::string chunk = port.readAvailable();
    if (chunk.empty()) break;
    rx += chunk;
  }
  if (rx.size() > 16384) rx.erase(0, rx.size() - 8192);
}

static std::string findCmpLine(std::string& rx, const char* prefix) {
  size_t searchFrom = 0;
  while (true) {
    size_t nl = rx.find('\n', searchFrom);
    if (nl == std::string::npos) {
      // Drop stale complete-ish junk older than last incomplete line.
      if (searchFrom > 0) rx.erase(0, searchFrom);
      return {};
    }
    std::string line = rx.substr(searchFrom, nl - searchFrom);
    if (!line.empty() && line.back() == '\r') line.pop_back();
    while (!line.empty() && (line.front() == ' ' || line.front() == '\t')) line.erase(line.begin());

    std::string lower = line;
    for (char& c : lower) c = (char)std::tolower((unsigned char)c);
    std::string p = prefix;
    for (char& c : p) c = (char)std::tolower((unsigned char)c);
    size_t idx = lower.find(p);
    if (idx != std::string::npos) {
      rx.erase(0, nl + 1);
      return line.substr(idx);
    }
    searchFrom = nl + 1;
  }
}

static std::optional<std::string> usbCmdOnce(SerialPort& port, std::string& rx, const std::string& cmd,
                                             int timeoutMs) {
  if (!port.isOpen()) return std::nullopt;
  drain(port, rx);
  // Drop leftover CMP lines from previous commands so we don't mis-match.
  while (true) {
    auto junk = findCmpLine(rx, "CMP");
    if (junk.empty()) break;
  }

  std::string wire = "\r\n" + cmd + "\r\n";
  if (!port.writeAll(wire)) return std::nullopt;

  enum class Kind { Ping, Status, Config, Write };
  Kind kind = Kind::Write;
  if (cmd.find(" ping") != std::string::npos || cmd == "cmp ping" || cmd.rfind("cmp ping", 0) == 0)
    kind = Kind::Ping;
  else if (cmd.find("status") != std::string::npos)
    kind = Kind::Status;
  else if (cmd.find("config") != std::string::npos)
    kind = Kind::Config;

  DWORD start = GetTickCount();
  while ((int)(GetTickCount() - start) < timeoutMs) {
    drain(port, rx);
    if (kind == Kind::Ping) {
      auto line = findCmpLine(rx, "CMP ok");
      if (!line.empty()) return line;
    } else if (kind == Kind::Status) {
      auto line = findCmpLine(rx, "CMPSTATUS");
      if (!line.empty()) return line;
    } else if (kind == Kind::Config) {
      auto line = findCmpLine(rx, "CMPCONFIG");
      if (!line.empty()) return line;
    } else {
      auto ack = findCmpLine(rx, "CMPACK");
      if (!ack.empty()) return ack;
      auto err = findCmpLine(rx, "CMPERR");
      if (!err.empty()) return std::nullopt;
    }
    Sleep(25);
  }
  return std::nullopt;
}

std::optional<std::string> usbCmd(SerialPort& port, std::string& rx, const std::string& cmd,
                                  int timeoutMs, int retries) {
  for (int i = 0; i <= retries; i++) {
    auto r = usbCmdOnce(port, rx, cmd, timeoutMs);
    if (r) return r;
    Sleep(80);
  }
  return std::nullopt;
}

std::string jsonStringField(const std::string& json, const char* key) {
  std::string needle = std::string("\"") + key + "\"";
  size_t p = json.find(needle);
  if (p == std::string::npos) return {};
  p = json.find(':', p + needle.size());
  if (p == std::string::npos) return {};
  p++;
  while (p < json.size() && (json[p] == ' ' || json[p] == '\t')) p++;
  if (p >= json.size() || json[p] != '"') return {};
  p++;
  std::string out;
  while (p < json.size() && json[p] != '"') {
    if (json[p] == '\\' && p + 1 < json.size()) {
      out.push_back(json[p + 1]);
      p += 2;
      continue;
    }
    out.push_back(json[p++]);
  }
  return out;
}

double jsonNumberField(const std::string& json, const char* key, double def) {
  std::string needle = std::string("\"") + key + "\"";
  size_t p = json.find(needle);
  if (p == std::string::npos) return def;
  p = json.find(':', p + needle.size());
  if (p == std::string::npos) return def;
  p++;
  while (p < json.size() && (json[p] == ' ' || json[p] == '\t')) p++;
  try {
    size_t end = p;
    while (end < json.size() &&
           (std::isdigit((unsigned char)json[end]) || json[end] == '.' || json[end] == '-'))
      end++;
    return std::stod(json.substr(p, end - p));
  } catch (...) {
    return def;
  }
}

bool jsonBoolField(const std::string& json, const char* key, bool def) {
  std::string needle = std::string("\"") + key + "\"";
  size_t p = json.find(needle);
  if (p == std::string::npos) return def;
  p = json.find(':', p + needle.size());
  if (p == std::string::npos) return def;
  p++;
  while (p < json.size() && (json[p] == ' ' || json[p] == '\t')) p++;
  if (json.compare(p, 4, "true") == 0) return true;
  if (json.compare(p, 5, "false") == 0) return false;
  return def;
}

bool parseCmpStatus(const std::string& line, BoardStatus& out) {
  size_t sp = line.find(' ');
  if (sp == std::string::npos) return false;
  std::string json = line.substr(sp + 1);
  out.hashrateHs = jsonNumberField(json, "hashrate_hs");
  out.shares = (uint64_t)jsonNumberField(json, "shares");
  out.accepted = (uint32_t)jsonNumberField(json, "accepted");
  out.rejected = (uint32_t)jsonNumberField(json, "rejected");
  out.dropped = (uint32_t)jsonNumberField(json, "dropped");
  out.pool = jsonStringField(json, "pool");
  out.connected = jsonBoolField(json, "connected");
  out.wifi = jsonStringField(json, "wifi");
  out.ip = jsonStringField(json, "ip");
  out.address = jsonStringField(json, "address");
  out.stratum = jsonStringField(json, "stratum");
  out.difficulty = (uint32_t)jsonNumberField(json, "difficulty");
  out.cpuMhz = (uint32_t)jsonNumberField(json, "cpu_mhz", 240);
  out.hashFocus = jsonBoolField(json, "hash_focus", true);
  out.nonce = jsonStringField(json, "nonce");
  return true;
}

bool parseCmpConfig(const std::string& line, BoardConfig& out) {
  size_t sp = line.find(' ');
  if (sp == std::string::npos) return false;
  std::string json = line.substr(sp + 1);
  out.worker = jsonStringField(json, "worker");
  out.stratum = jsonStringField(json, "stratum");
  out.wifiSsid = jsonStringField(json, "wifi_ssid");
  out.wifiPasswordMasked = jsonStringField(json, "wifi_password");
  out.cpuMhz = (uint32_t)jsonNumberField(json, "cpu_mhz", 240);
  out.hashFocus = jsonBoolField(json, "hash_focus", true);
  out.fw = jsonStringField(json, "fw");
  out.configured = jsonBoolField(json, "configured");
  return true;
}
