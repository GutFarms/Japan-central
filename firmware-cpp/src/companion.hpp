#pragma once
#include "config.hpp"
#include <Arduino.h>
#include <functional>

struct MinerSnapshot {
  float hashrateHs = 0;
  uint64_t shares = 0;
  uint32_t accepted = 0;
  uint32_t rejected = 0;
  String pool = "usb";
  bool connected = false;
  uint32_t difficulty = 0;
  uint32_t nonce = 0;
  uint8_t cpuMhz = 240;
  bool hashFocus = true;
  String netTicker;
  String jobId;
  float benchHs = 0;
  bool fullV = false;
};

struct NetFeed {
  String ticker;
  String source;
  uint32_t updatedMs = 0;
  bool fresh = false;
};

struct UsbJob {
  uint8_t header[80]{};
  uint8_t target[32]{};
  String jobId;
  String extranonce2;
  String ntime;
  uint32_t startNonce = 0;
  bool valid = false;
  bool fresh = false;
};

struct PendingShare {
  uint32_t nonce = 0;
  String jobId;
  String extranonce2;
  String ntime;
  bool pending = false;
};

// UART0 companion protocol — board mines only; PC owns pool + WiFi.
class CompanionLink {
 public:
  using ApplyFn = std::function<bool(AppConfig& updated, bool& reboot)>;
  using JobFn = std::function<void(const UsbJob& job)>;
  using StopFn = std::function<void()>;
  using StatsFn = std::function<void(uint32_t accepted, uint32_t rejected)>;

  void begin(uint32_t baud = 115200);
  bool poll(AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply, NetFeed* net, JobFn onJob,
            StopFn onStop, StatsFn onStats);

  void emitShare(const PendingShare& share);

 private:
  static constexpr size_t kLineCap = 768;
  char lineBuf_[kLineCap]{};
  size_t lineLen_ = 0;

  // Staged multi-part job (short USB lines — avoids one huge `cmp job …` timeout).
  uint8_t stagedHeader_[80]{};
  uint8_t stagedTarget_[32]{};
  bool haveHeader_ = false;
  bool haveTarget_ = false;

  void handleLine(const String& line, AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply,
                  NetFeed* net, JobFn onJob, StopFn onStop, StatsFn onStats);
  void replyStatus(const AppConfig& cfg, const MinerSnapshot& snap);
  void replyConfig(const AppConfig& cfg);
  static String urlDecode(const String& in);
  static void parseBody(const String& body, AppConfig& cfg, bool& reboot);
  static void parseNetData(const String& body, NetFeed& net);
  static bool parseJob(const String& body, UsbJob& job);
  static bool parseJobMeta(const String& body, UsbJob& job);
  static bool hexDecodeFixed(const String& hex, uint8_t* out, size_t n);
};
