#pragma once
#include "config.hpp"
#include <Arduino.h>
#include <functional>

struct MinerSnapshot {
  float hashrateHs = 0;
  uint64_t shares = 0;
  uint32_t accepted = 0;
  uint32_t rejected = 0;
  uint32_t dropped = 0;
  String pool = "off";
  bool connected = false;
  String wifi = "off";
  String ip = "---";
  uint32_t difficulty = 0;
  uint32_t nonce = 0;
  uint8_t cpuMhz = 240;
  bool hashFocus = true;
  String netTicker;  // USB-pushed network/market line from companion
};

// Network / market feed pushed from the PC over USB (`cmp netdata`).
struct NetFeed {
  String ticker;
  String source;
  uint32_t updatedMs = 0;
  bool fresh = false;
};

// UART0 companion protocol only — never prompts for field text.
class CompanionLink {
 public:
  using ApplyFn = std::function<bool(AppConfig& updated, bool& reboot, bool reconnect)>;

  void begin(uint32_t baud = 115200);
  // Call often from loop(). Returns true if config was applied.
  bool poll(AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply, NetFeed* net = nullptr);

 private:
  String line_;
  void handleLine(const String& line, AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply,
                  NetFeed* net);
  void replyStatus(const AppConfig& cfg, const MinerSnapshot& snap);
  void replyConfig(const AppConfig& cfg);
  static String urlDecode(const String& in);
  static void parseBody(const String& body, AppConfig& cfg, bool& reboot, bool& reconnect,
                        String& auth);
  static void parseNetData(const String& body, NetFeed& net);
};
