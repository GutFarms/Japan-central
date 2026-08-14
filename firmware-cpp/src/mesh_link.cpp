#include "mesh_link.hpp"

#include <cstring>
#include <esp_now.h>
#include <esp_wifi.h>
#include <WiFi.h>

MeshLink g_mesh;

namespace {
constexpr uint8_t kMagic = 0x4E;  // 'N'
constexpr uint8_t kVer = 1;
constexpr uint8_t kTypeHello = 1;
constexpr uint8_t kTypeLine = 2;

struct __attribute__((packed)) MeshPkt {
  uint8_t magic;
  uint8_t ver;
  uint8_t type;
  uint8_t flags;  // bit0 = root hello
  uint8_t src[6];
  uint8_t dst[6];
  uint8_t seq;
  uint8_t len;
  // payload follows (not included in sizeof for flexible use)
};

uint8_t kBcast[6] = {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF};

bool macEq(const uint8_t a[6], const uint8_t b[6]) { return memcmp(a, b, 6) == 0; }
bool macBcast(const uint8_t m[6]) { return macEq(m, kBcast); }

#if ESP_IDF_VERSION_MAJOR >= 5
void onRecvCb(const esp_now_recv_info_t* info, const uint8_t* data, int len) {
  if (!info || !data || len <= 0) return;
  int8_t rssi = info->rx_ctrl ? info->rx_ctrl->rssi : 0;
  g_mesh.onEspNowRecv(info->src_addr, data, len, rssi);
}
#else
void onRecvCb(const uint8_t* mac, const uint8_t* data, int len) {
  g_mesh.onEspNowRecv(mac, data, len, 0);
}
#endif
}  // namespace

size_t MeshPrint::write(uint8_t c) {
  if (c == '\r') return 1;
  if (c == '\n') {
    flushLine();
    return 1;
  }
  if (len_ + 1 < sizeof(buf_)) {
    buf_[len_++] = (char)c;
  } else {
    // Overflow — drop line.
    len_ = 0;
  }
  return 1;
}

size_t MeshPrint::write(const uint8_t* buffer, size_t size) {
  for (size_t i = 0; i < size; i++) write(buffer[i]);
  return size;
}

void MeshPrint::setTarget(const uint8_t mac[6]) {
  memcpy(target_, mac, 6);
  haveTarget_ = true;
}

void MeshPrint::clearTarget() { haveTarget_ = false; }

void MeshPrint::flushLine() {
  if (!mesh_ || len_ == 0) {
    len_ = 0;
    return;
  }
  buf_[len_] = 0;
  // Untargeted = leaf → root (sendLineTo picks root when dst is broadcast).
  mesh_->sendLineTo(haveTarget_ ? target_ : kBcast, buf_);
  len_ = 0;
}

void MeshLink::pinChannel() {
  lastChannelPinMs_ = millis();
  // When STA is associated, the radio MUST stay on the home AP channel.
  // Forcing ch1 (old SoftAP pin) made esp_now_send fail → CMPERR via send failed.
  if (WiFi.status() == WL_CONNECTED) {
    uint8_t ch = (uint8_t)WiFi.channel();
    if (ch >= 1 && ch <= 14) meshChan_ = ch;
    return;
  }
  // SoftAP-only / APSTA before STA link — keep SoftAP + ESP-NOW on ch1.
  esp_wifi_set_channel(1, WIFI_SECOND_CHAN_NONE);
  meshChan_ = 1;
}

void MeshLink::begin(const uint8_t selfMac[6]) {
  memcpy(selfMac_, selfMac, 6);
  leafOut_.begin(this);
  if (WiFi.getMode() == WIFI_OFF) {
    WiFi.mode(WIFI_AP);
  }
  pinChannel();

  if (esp_now_init() != ESP_OK) {
    ready_ = false;
    return;
  }
#if ESP_IDF_VERSION_MAJOR >= 5
  esp_now_register_recv_cb(onRecvCb);
#else
  esp_now_register_recv_cb(onRecvCb);
#endif

  esp_now_peer_info_t peer{};
  memset(&peer, 0, sizeof(peer));
  memcpy(peer.peer_addr, kBcast, 6);
  // channel 0 = follow current Wi‑Fi channel (STA or SoftAP).
  peer.channel = 0;
  peer.encrypt = false;
  if (!esp_now_is_peer_exist(kBcast)) {
    esp_now_add_peer(&peer);
  }
  ready_ = true;
  lastHelloMs_ = 0;
  lastSendErr_ = ESP_OK;
}

bool MeshLink::popRx(RxItem& out) {
  bool got = false;
  portENTER_CRITICAL(&rxMux_);
  for (size_t i = 0; i < kRxQ; i++) {
    if (rxQ_[i].used) {
      out = rxQ_[i];
      rxQ_[i].used = false;
      got = true;
      break;
    }
  }
  portEXIT_CRITICAL(&rxMux_);
  return got;
}

bool MeshLink::pushRx(const RxItem& item) {
  bool ok = false;
  portENTER_CRITICAL(&rxMux_);
  for (size_t i = 0; i < kRxQ; i++) {
    if (!rxQ_[i].used) {
      rxQ_[i] = item;
      rxQ_[i].used = true;
      ok = true;
      break;
    }
  }
  portEXIT_CRITICAL(&rxMux_);
  return ok;
}

void MeshLink::noteUsbActivity() { lastUsbMs_ = millis(); }

bool MeshLink::isRoot() const {
  return lastUsbMs_ != 0 && (millis() - lastUsbMs_) < MESH_USB_ROOT_MS;
}

bool MeshLink::hasRootPeer() const {
  uint8_t tmp[6];
  return pickRoot(tmp);
}

size_t MeshLink::leafCount() const {
  // Only true leaves (non-root peers). Another USB root nearby must not look
  // like a bridged leaf — that falsely throttles HW hashrate and parks SW assist.
  size_t n = 0;
  for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
    if (peers_[i].used && !peers_[i].root) n++;
  }
  return n;
}

bool MeshLink::isBridging() const { return isRoot() && leafCount() > 0; }

void MeshLink::macToStr(const uint8_t mac[6], char out[18]) const {
  snprintf(out, 18, "%02x:%02x:%02x:%02x:%02x:%02x", mac[0], mac[1], mac[2], mac[3], mac[4],
           mac[5]);
}

bool MeshLink::parseMac(const String& s, uint8_t out[6]) const {
  String hex;
  for (size_t i = 0; i < s.length(); i++) {
    char c = s[i];
    if ((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) hex += c;
  }
  if (hex.length() < 12) return false;
  for (int i = 0; i < 6; i++) {
    out[i] = (uint8_t)strtoul(hex.substring(i * 2, i * 2 + 2).c_str(), nullptr, 16);
  }
  return true;
}

size_t MeshLink::peerCount() const {
  size_t n = 0;
  for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
    if (peers_[i].used) n++;
  }
  return n;
}

MeshPeer* MeshLink::findPeer(const uint8_t mac[6]) {
  for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
    if (peers_[i].used && macEq(peers_[i].mac, mac)) return &peers_[i];
  }
  return nullptr;
}

MeshPeer* MeshLink::upsertPeer(const uint8_t mac[6], bool root, int8_t rssi) {
  if (macEq(mac, selfMac_)) return nullptr;
  MeshPeer* p = findPeer(mac);
  if (!p) {
    for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
      if (!peers_[i].used) {
        p = &peers_[i];
        break;
      }
    }
    if (!p) {
      // Evict oldest.
      uint32_t oldest = UINT32_MAX;
      size_t idx = 0;
      for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
        if (peers_[i].lastMs < oldest) {
          oldest = peers_[i].lastMs;
          idx = i;
        }
      }
      p = &peers_[idx];
    }
    memset(p, 0, sizeof(*p));
    memcpy(p->mac, mac, 6);
    p->used = true;
  }
  p->root = root;
  p->rssi = rssi;
  p->lastMs = millis();
  return p;
}

void MeshLink::prunePeers() {
  uint32_t now = millis();
  for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
    if (peers_[i].used && now - peers_[i].lastMs > MESH_PEER_TTL_MS) {
      peers_[i].used = false;
    }
  }
}

bool MeshLink::pickRoot(uint8_t out[6]) const {
  int bestRssi = -127;
  bool found = false;
  for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
    if (!peers_[i].used || !peers_[i].root) continue;
    if (!found || peers_[i].rssi > bestRssi) {
      bestRssi = peers_[i].rssi;
      memcpy(out, peers_[i].mac, 6);
      found = true;
    }
  }
  return found;
}

void MeshLink::ensurePeer(const uint8_t mac[6]) {
  if (macBcast(mac)) return;
  // channel 0 = use whatever channel the radio is on right now (STA or SoftAP).
  if (esp_now_is_peer_exist(mac)) {
    esp_now_peer_info_t peer{};
    if (esp_now_get_peer(mac, &peer) == ESP_OK && peer.channel != 0) {
      peer.channel = 0;
      peer.encrypt = false;
      esp_now_mod_peer(&peer);
    }
    return;
  }
  esp_now_peer_info_t peer{};
  memset(&peer, 0, sizeof(peer));
  memcpy(peer.peer_addr, mac, 6);
  peer.channel = 0;
  peer.encrypt = false;
  esp_now_add_peer(&peer);
}

bool MeshLink::sendLineTo(const uint8_t dstIn[6], const char* line) {
  if (!ready_ || !line) return false;
  uint8_t dst[6];
  memcpy(dst, dstIn, 6);
  if (macBcast(dst)) {
    // Leaf → root
    if (isRoot()) return false;
    if (!pickRoot(dst)) return false;
  }

  size_t n = strlen(line);
  if (n > MESH_LINE_CAP - 1) n = MESH_LINE_CAP - 1;

  uint8_t pkt[sizeof(MeshPkt) + MESH_LINE_CAP];
  MeshPkt* h = (MeshPkt*)pkt;
  h->magic = kMagic;
  h->ver = kVer;
  h->type = kTypeLine;
  h->flags = isRoot() ? 1 : 0;
  memcpy(h->src, selfMac_, 6);
  memcpy(h->dst, dst, 6);
  h->seq = ++seq_;
  h->len = (uint8_t)n;
  memcpy(pkt + sizeof(MeshPkt), line, n);

  // Retry: first send often fails right after STA channel change / peer add.
  esp_err_t err = ESP_FAIL;
  for (int attempt = 0; attempt < 4; attempt++) {
    if (attempt == 0 || attempt == 2) pinChannel();
    ensurePeer(dst);
    err = esp_now_send(dst, pkt, sizeof(MeshPkt) + n);
    if (err == ESP_OK) {
      lastSendErr_ = ESP_OK;
      return true;
    }
    // Peer table stale after channel hop — drop and re-add.
    if (err == ESP_ERR_ESPNOW_NOT_FOUND || err == ESP_ERR_ESPNOW_ARG) {
      esp_now_del_peer(dst);
      ensurePeer(dst);
    }
    delay(3 + attempt);
    yield();
  }
  lastSendErr_ = err;
  return false;
}

void MeshLink::sendHello() {
  if (!ready_) return;
  pinChannel();
  uint8_t pkt[sizeof(MeshPkt) + 8];
  MeshPkt* h = (MeshPkt*)pkt;
  h->magic = kMagic;
  h->ver = kVer;
  h->type = kTypeHello;
  h->flags = isRoot() ? 1 : 0;
  memcpy(h->src, selfMac_, 6);
  memcpy(h->dst, kBcast, 6);
  h->seq = ++seq_;
  h->len = 0;
  esp_now_send(kBcast, pkt, sizeof(MeshPkt));
  // Leaf: also unicast hello to the known root so discovery survives broadcast loss.
  if (!isRoot()) {
    uint8_t root[6];
    if (pickRoot(root)) {
      memcpy(h->dst, root, 6);
      ensurePeer(root);
      esp_now_send(root, pkt, sizeof(MeshPkt));
    }
  }
}

void MeshLink::onEspNowRecv(const uint8_t* mac, const uint8_t* data, int len, int8_t rssi) {
  if (!mac || !data || len < (int)sizeof(MeshPkt)) return;
  const MeshPkt* h = (const MeshPkt*)data;
  if (h->magic != kMagic || h->ver != kVer) return;
  if (macEq(h->src, selfMac_)) return;

  if (h->type == kTypeHello) {
    upsertPeer(h->src, (h->flags & 1) != 0, rssi);
    return;
  }
  if (h->type != kTypeLine) return;
  if (h->len > MESH_LINE_CAP - 1) return;
  if (len < (int)(sizeof(MeshPkt) + h->len)) return;
  // Accept if addressed to us or broadcast.
  if (!macBcast(h->dst) && !macEq(h->dst, selfMac_)) return;

  upsertPeer(h->src, (h->flags & 1) != 0, rssi);

  portENTER_CRITICAL(&rxMux_);
  size_t slot = kRxQ;
  for (size_t i = 0; i < kRxQ; i++) {
    if (!rxQ_[i].used) {
      slot = i;
      break;
    }
  }
  if (slot == kRxQ) {
    // Prefer keeping shares — overwrite a non-share slot if needed.
    for (size_t i = 0; i < kRxQ; i++) {
      if (strncmp(rxQ_[i].line, "CMPSHARE ", 9) != 0) {
        slot = i;
        break;
      }
    }
    if (slot == kRxQ) slot = 0;
  }
  memcpy(rxQ_[slot].mac, h->src, 6);
  memcpy(rxQ_[slot].line, data + sizeof(MeshPkt), h->len);
  rxQ_[slot].line[h->len] = 0;
  rxQ_[slot].used = true;
  portEXIT_CRITICAL(&rxMux_);
}

void MeshLink::replyMeshList(Print& out) const {
  char buf[360];
  char selfStr[18];
  macToStr(selfMac_, selfStr);
  size_t o = 0;
  // self= is this board — peers= lists OTHER boards only (never self).
  o += snprintf(buf + o, sizeof(buf) - o,
                "CMPMESH self=%s root=%u bridging=%u peers=", selfStr, isRoot() ? 1u : 0u,
                isBridging() ? 1u : 0u);
  bool first = true;
  for (size_t i = 0; i < MESH_MAX_PEERS; i++) {
    if (!peers_[i].used) continue;
    if (macEq(peers_[i].mac, selfMac_)) continue;
    // List every other board the root can hear (including briefly dual-root boards).
    char m[18];
    macToStr(peers_[i].mac, m);
    if (!first) {
      if (o + 2 < sizeof(buf)) buf[o++] = ',';
    }
    first = false;
    int n = snprintf(buf + o, sizeof(buf) - o, "%s", m);
    if (n > 0) o += (size_t)n;
  }
  if (first && o + 1 < sizeof(buf)) {
    buf[o++] = '-';
    buf[o] = 0;
  }
  snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), " count=%u", (unsigned)leafCount());
  out.println(buf);
  out.flush();
}

void MeshLink::handleIncomingLine(const uint8_t* from, const char* line, CompanionLink& cmp,
                                  AppConfig& cfg, const MinerSnapshot& snap,
                                  CompanionLink::ApplyFn onApply, NetFeed* net,
                                  CompanionLink::JobFn onJob, CompanionLink::StopFn onStop,
                                  CompanionLink::StatsFn onStats) {
  if (!line || !line[0]) return;

  if (isRoot()) {
    // Shares and CMP* replies from leaves → USB Companion (pass-through).
    if (strncmp(line, "CMP", 3) == 0) {
      Serial.println(line);
      Serial.flush();
    }
    return;
  }

  // Leaf: feed cmp command into local handler; replies go back via MeshPrint to root.
  leafOut_.setTarget(from);
  // Inject line into a tiny Stream.
  class LineStream : public Stream {
   public:
    const char* p;
    size_t n;
    size_t i = 0;
    int available() override { return (int)(n - i); }
    int read() override { return i < n ? (unsigned char)p[i++] : -1; }
    int peek() override { return i < n ? (unsigned char)p[i] : -1; }
    void flush() override {}
    size_t write(uint8_t) override { return 0; }
  } in;
  // Append newline so pollStream completes the line.
  char tmp[MESH_LINE_CAP + 2];
  size_t ln = strlen(line);
  if (ln > MESH_LINE_CAP) ln = MESH_LINE_CAP;
  memcpy(tmp, line, ln);
  tmp[ln] = '\n';
  tmp[ln + 1] = 0;
  in.p = tmp;
  in.n = ln + 1;
  cmp.pollTcp(in, leafOut_, cfg, snap, onApply, net, onJob, onStop, onStats);
  leafOut_.clearTarget();
}

bool MeshLink::handleVia(const String& macArg, const String& cmdRest) {
  noteUsbActivity();
  uint8_t mac[6];
  if (!parseMac(macArg, mac)) {
    Serial.println("CMPERR via bad mac");
    Serial.flush();
    return true;
  }
  String cmd = cmdRest;
  cmd.trim();
  if (cmd.length() == 0) cmd = "ping";
  // Ensure verb has cmp prefix for leaf handler.
  String wire = cmd;
  String lower = cmd;
  lower.toLowerCase();
  if (!lower.startsWith("cmp")) {
    wire = String("cmp ") + cmd;
  }

  viaPending_ = true;
  memcpy(viaMac_, mac, 6);
  viaStartMs_ = millis();
  pinChannel();
  ensurePeer(mac);

  if (!sendLineTo(mac, wire.c_str())) {
    viaPending_ = false;
    char err[72];
    snprintf(err, sizeof(err), "CMPERR via send failed ch=%u esp=0x%x",
             (unsigned)meshChan_, (unsigned)lastSendErr_);
    Serial.println(err);
    Serial.flush();
    return true;
  }

  // Hold non-target frames aside so we never drop shares / other peer traffic
  // while Companion is blocked waiting on this via.
  RxItem deferred[kRxQ];
  size_t nDef = 0;
  auto restoreDeferred = [&]() {
    for (size_t i = 0; i < nDef; i++) {
      if (!pushRx(deferred[i])) {
        // Queue full — forward shares so Companion still sees them.
        if (strncmp(deferred[i].line, "CMPSHARE ", 9) == 0) {
          Serial.println(deferred[i].line);
          Serial.flush();
        }
      }
    }
    nDef = 0;
  };

  while (millis() - viaStartMs_ < MESH_VIA_TIMEOUT_MS) {
    RxItem item{};
    if (popRx(item)) {
      const bool fromTarget = macEq(item.mac, viaMac_) && item.line[0];
      const bool isCmp = strncmp(item.line, "CMP", 3) == 0;
      const bool isShare = strncmp(item.line, "CMPSHARE ", 9) == 0;
      if (fromTarget && isCmp) {
        Serial.println(item.line);
        Serial.flush();
        if (!isShare) {
          viaPending_ = false;
          restoreDeferred();
          return true;
        }
        // Unsolicited share from the via target — keep waiting for the reply.
      } else if (isShare) {
        // Any leaf share during via: forward immediately (cmp_reply_line ignores them).
        Serial.println(item.line);
        Serial.flush();
      } else if (nDef < kRxQ) {
        deferred[nDef++] = item;
      } else if (isCmp && isRoot()) {
        // Last resort: bridge CMP lines so they are not lost forever.
        Serial.println(item.line);
        Serial.flush();
      }
    }
    delay(2);
    yield();
  }
  viaPending_ = false;
  restoreDeferred();
  Serial.println("CMPERR via timeout");
  Serial.flush();
  return true;
}

void MeshLink::poll(CompanionLink& cmp, AppConfig& cfg, const MinerSnapshot& snap,
                    CompanionLink::ApplyFn onApply, NetFeed* net, CompanionLink::JobFn onJob,
                    CompanionLink::StopFn onStop, CompanionLink::StatsFn onStats) {
  if (!ready_) return;
  prunePeers();

  uint32_t now = millis();
  if (now - lastChannelPinMs_ >= 4000) {
    pinChannel();
  }
  const uint32_t helloPeriod =
      (!isRoot() && !hasRootPeer()) ? MESH_HELLO_SEEK_MS : MESH_HELLO_MS;
  if (now - lastHelloMs_ >= helloPeriod) {
    lastHelloMs_ = now;
    sendHello();
  }

  (void)cmp;
  (void)cfg;
  (void)snap;
  (void)onApply;
  (void)net;
  (void)onJob;
  (void)onStop;
  (void)onStats;

  for (;;) {
    RxItem item{};
    if (!popRx(item)) break;

    if (isRoot() && viaPending_ && macEq(item.mac, viaMac_) && strncmp(item.line, "CMP", 3) == 0 &&
        strncmp(item.line, "CMPSHARE ", 9) != 0) {
      Serial.println(item.line);
      Serial.flush();
      viaPending_ = false;
      continue;
    }
    handleIncomingLine(item.mac, item.line, cmp, cfg, snap, onApply, net, onJob, onStop, onStats);
  }
}
