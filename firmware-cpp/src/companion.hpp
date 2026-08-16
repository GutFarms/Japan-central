#pragma once
#include "config.hpp"
#include <Arduino.h>
#include <functional>

struct MinerSnapshot {
  float hashrateHs = 0;
  uint64_t shares = 0;
  uint64_t totalHashes = 0;
  uint32_t accepted = 0;
  uint32_t rejected = 0;
  String pool = "usb";
  bool connected = false;
  bool mining = false;
  uint32_t difficulty = 0;
  uint32_t nonce = 0;
  uint8_t cpuMhz = 240;
  bool hashFocus = true;
  String netTicker;
  String jobId;
  String shaMode;
  float benchHs = 0;
  bool fullV = false;
  String mac;
  String wifiMode;   // off / ap / sta / apsta
  String wifiAp;     // SoftAP SSID
  String wifiIp;     // best client IP for TCP
  bool meshRoot = false;
  bool meshBridging = false;
  uint8_t meshPeers = 0;
  /// Board mines to pool itself (Companion monitors H/s only).
  bool mineIndep = false;
  String poolEndpoint;
  String poolPhase;
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

// Line companion protocol — USB Serial and/or Wi‑Fi TCP (same verbs).
class CompanionLink {
 public:
  using ApplyFn = std::function<bool(AppConfig& updated, bool& reboot)>;
  using JobFn = std::function<void(const UsbJob& job)>;
  using StopFn = std::function<void()>;
  using StatsFn = std::function<void(uint32_t accepted, uint32_t rejected)>;
  /// Persist Wi‑Fi credentials to NVS (return false on failure).
  using WifiPersistFn = std::function<bool()>;
  /// Apply SoftAP/STA after credentials are ACK'd on the wire.
  using WifiApplyFn = std::function<void()>;
  /// Hold/release hashing for USB/Wi‑Fi OTA (indep pool must not resume mid-transfer).
  using MiningHoldFn = std::function<void(bool hold)>;

  void begin(uint32_t baud = 460800);
  void setShareMirror(Print* mirror) { shareMirror_ = mirror; }
  /// Second sink (mesh leaf ESP-NOW) — used with TCP mirror so SoftAP clients
  /// cannot steal shares from the USB root path.
  void setShareMirror2(Print* mirror) { shareMirror2_ = mirror; }
  void setWifiPersist(WifiPersistFn fn) { onWifiPersist_ = std::move(fn); }
  void setWifiApply(WifiApplyFn fn) { onWifiApply_ = std::move(fn); }
  void setMiningHold(MiningHoldFn fn) { onMiningHold_ = std::move(fn); }
  /// True while `cmp ota` binary receive is active — USB/TCP must not be starved.
  bool otaBusy() const { return otaActive_; }

  bool poll(AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply, NetFeed* net, JobFn onJob,
            StopFn onStop, StatsFn onStats);
  bool pollStream(Stream& in, Print& out, AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply,
                  NetFeed* net, JobFn onJob, StopFn onStop, StatsFn onStats);
  bool pollTcp(Stream& in, Print& out, AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply,
               NetFeed* net, JobFn onJob, StopFn onStop, StatsFn onStats);

  void emitShare(const PendingShare& share);

 private:
  static constexpr size_t kLineCap = 768;
  char lineBuf_[kLineCap]{};
  size_t lineLen_ = 0;
  char tcpLineBuf_[kLineCap]{};
  size_t tcpLineLen_ = 0;

  uint8_t stagedHeader_[80]{};
  uint8_t stagedTarget_[32]{};
  bool haveHeader_ = false;
  bool haveTarget_ = false;

  /// After `cmp ota size=N` — next bytes are raw app image for Update.write.
  size_t otaRemain_ = 0;
  bool otaActive_ = false;
  uint32_t otaLastRxMs_ = 0;

  Print* out_ = &Serial;
  Print* shareMirror_ = nullptr;
  Print* shareMirror2_ = nullptr;
  WifiPersistFn onWifiPersist_;
  WifiApplyFn onWifiApply_;
  MiningHoldFn onMiningHold_;

  char* activeLineBuf_ = lineBuf_;
  size_t* activeLineLen_ = &lineLen_;

  bool pollOtaBinary(Stream& in, Print& out);
  void handleLine(const String& line, AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply,
                  NetFeed* net, JobFn onJob, StopFn onStop, StatsFn onStats);
  void replyStatus(const AppConfig& cfg, const MinerSnapshot& snap);
  void replyConfig(const AppConfig& cfg, const MinerSnapshot& snap);
  static String urlDecode(const String& in);
  static void parseBody(const String& body, AppConfig& cfg, bool& reboot);
  static void parseWifiBody(const String& body, AppConfig& cfg);
  static void parseNetData(const String& body, NetFeed& net);
  static bool parseJob(const String& body, UsbJob& job);
  static bool parseJobMeta(const String& body, UsbJob& job);
  static bool hexDecodeFixed(const String& hex, uint8_t* out, size_t n);
};
