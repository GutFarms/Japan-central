#pragma once
#include "companion.hpp"
#include <Arduino.h>

// ESP-NOW connectivity mesh (not a hashrate multiplier).
// USB-linked board = root bridge; other boards relay cmp lines through it.

static constexpr size_t MESH_MAX_PEERS = 8;
static constexpr size_t MESH_LINE_CAP = 220;
static constexpr uint32_t MESH_PEER_TTL_MS = 20000;
static constexpr uint32_t MESH_HELLO_MS = 1500;
// Keep USB-elected root long enough across slow Companion poll / stratum stalls.
static constexpr uint32_t MESH_USB_ROOT_MS = 30000;
static constexpr uint32_t MESH_VIA_TIMEOUT_MS = 2800;

struct MeshPeer {
  uint8_t mac[6]{};
  bool root = false;
  int8_t rssi = 0;
  uint32_t lastMs = 0;
  bool used = false;
};

// Print that batches a line and sends it over ESP-NOW to the current root (leaf)
// or to a targeted peer (root via-forward path uses MeshLink::sendLineTo).
class MeshPrint : public Print {
 public:
  void begin(class MeshLink* mesh) { mesh_ = mesh; }
  size_t write(uint8_t c) override;
  size_t write(const uint8_t* buffer, size_t size) override;
  void setTarget(const uint8_t mac[6]);
  void clearTarget();

 private:
  class MeshLink* mesh_ = nullptr;
  uint8_t target_[6]{};
  bool haveTarget_ = false;
  char buf_[MESH_LINE_CAP]{};
  size_t len_ = 0;
  void flushLine();
};

class MeshLink {
 public:
  void begin(const uint8_t selfMac[6]);
  void noteUsbActivity();
  bool isRoot() const;
  bool hasRootPeer() const;
  /// Root with at least one leaf peer — keep hashing, but yield for bridge traffic.
  bool isBridging() const;
  size_t leafCount() const;
  bool ready() const { return ready_; }

  // Leaf: Print used as CompanionLink out_ / share mirror toward root.
  MeshPrint& leafOut() { return leafOut_; }
  // Drain RX into local CompanionLink (leaf) or Serial bridge (root).
  void poll(CompanionLink& cmp, AppConfig& cfg, const MinerSnapshot& snap,
            CompanionLink::ApplyFn onApply, NetFeed* net, CompanionLink::JobFn onJob,
            CompanionLink::StopFn onStop, CompanionLink::StatsFn onStats);

  // Root: forward a cmp command to a peer and wait for CMP* reply on Serial.
  bool handleVia(const String& macArg, const String& cmdRest);

  // `cmp mesh` → peer list line printed to Serial/out.
  void replyMeshList(Print& out) const;

  size_t peerCount() const;
  bool parseMac(const String& s, uint8_t out[6]) const;
  void macToStr(const uint8_t mac[6], char out[18]) const;

  // Used by MeshPrint / ISR-safe queues.
  bool sendLineTo(const uint8_t dst[6], const char* line);
  void onEspNowRecv(const uint8_t* mac, const uint8_t* data, int len, int8_t rssi);

 private:
  bool ready_ = false;
  uint8_t selfMac_[6]{};
  uint32_t lastUsbMs_ = 0;
  uint32_t lastHelloMs_ = 0;
  uint8_t seq_ = 0;
  MeshPeer peers_[MESH_MAX_PEERS]{};
  MeshPrint leafOut_;

  // RX queue (filled from ESP-NOW cb, drained in poll).
  static constexpr size_t kRxQ = 6;
  struct RxItem {
    uint8_t mac[6];
    char line[MESH_LINE_CAP];
    bool used = false;
  };
  RxItem rxQ_[kRxQ]{};
  portMUX_TYPE rxMux_ = portMUX_INITIALIZER_UNLOCKED;

  // Via wait state (root).
  bool viaPending_ = false;
  uint8_t viaMac_[6]{};
  uint32_t viaStartMs_ = 0;

  void sendHello();
  void prunePeers();
  MeshPeer* findPeer(const uint8_t mac[6]);
  MeshPeer* upsertPeer(const uint8_t mac[6], bool root, int8_t rssi);
  bool pickRoot(uint8_t out[6]) const;
  void ensurePeer(const uint8_t mac[6]);
  void handleIncomingLine(const uint8_t* from, const char* line, CompanionLink& cmp, AppConfig& cfg,
                          const MinerSnapshot& snap, CompanionLink::ApplyFn onApply, NetFeed* net,
                          CompanionLink::JobFn onJob, CompanionLink::StopFn onStop,
                          CompanionLink::StatsFn onStats);
};

extern MeshLink g_mesh;
