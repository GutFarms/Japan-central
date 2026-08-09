#include "companion.hpp"
#include <ArduinoJson.h>

void CompanionLink::begin(uint32_t baud) {
  // CYD CH340 is UART0 (Serial). Companion owns this link — no log spam.
  Serial.setRxBufferSize(1024);
  Serial.begin(baud);
  Serial.setTimeout(0);
  line_.reserve(768);
  line_ = "";
}

bool CompanionLink::poll(AppConfig& cfg, const MinerSnapshot& snap, ApplyFn onApply) {
  bool applied = false;
  while (Serial.available() > 0) {
    char c = (char)Serial.read();
    if (c == '\n' || c == '\r') {
      if (line_.length() > 0) {
        String cmd = line_;
        line_ = "";
        cmd.trim();
        if (cmd.length() == 0) continue;
        handleLine(cmd, cfg, snap, [&](AppConfig& u, bool& reboot, bool reconnect) {
          bool ok = onApply(u, reboot, reconnect);
          if (ok) applied = true;
          return ok;
        });
      }
    } else if (c >= 32 && c < 127) {
      if (line_.length() < 760) line_ += c;
    }
  }
  return applied;
}

void CompanionLink::handleLine(const String& line, AppConfig& cfg, const MinerSnapshot& snap,
                               ApplyFn onApply) {
  String t = line;
  int idx = -1;
  {
    String lower = t;
    lower.toLowerCase();
    for (int i = 0; i + 3 <= (int)lower.length(); i++) {
      if (lower[i] == 'c' && lower[i + 1] == 'm' && lower[i + 2] == 'p' &&
          (i + 3 == (int)lower.length() || lower[i + 3] == ' ' || lower[i + 3] == '\t')) {
        idx = i;
        break;
      }
    }
    if (idx < 0) return;
  }
  t = t.substring(idx);
  t.trim();

  String rest = t.substring(3);
  rest.trim();
  if (rest.length() == 0) rest = "ping";

  int sp = rest.indexOf(' ');
  String verb = (sp < 0) ? rest : rest.substring(0, sp);
  String args = (sp < 0) ? "" : rest.substring(sp + 1);
  verb.toLowerCase();
  args.trim();

  if (verb == "ping") {
    Serial.println("CMP ok usb");
    return;
  }
  if (verb == "status") {
    replyStatus(cfg, snap);
    return;
  }
  if (verb == "config") {
    replyConfig(cfg);
    return;
  }
  if (verb == "set" || verb == "clock" || verb == "reboot") {
    AppConfig updated = cfg;
    bool reboot = false;
    bool reconnect = false;
    String authVal;
    parseBody(args, updated, reboot, reconnect, authVal);
    if (verb == "reboot") reboot = true;
    if (verb == "clock" && args.indexOf("cpu_mhz") < 0 && args.indexOf("clock") < 0) {
      Serial.println("CMPERR cpu_mhz required");
      return;
    }
    if (!cfg.authorizeOrSetup(authVal)) {
      Serial.println("CMPERR bad auth");
      return;
    }
    Serial.println("CMPACK queued");
    (void)onApply(updated, reboot, reconnect);
    return;
  }
  Serial.println("CMPERR unknown (ping|status|config|set|clock|reboot)");
}

void CompanionLink::replyStatus(const AppConfig& cfg, const MinerSnapshot& snap) {
  JsonDocument doc;
  doc["hashrate_hs"] = snap.hashrateHs;
  doc["shares"] = snap.shares;
  doc["accepted"] = snap.accepted;
  doc["rejected"] = snap.rejected;
  doc["dropped"] = snap.dropped;
  doc["pool"] = snap.pool;
  doc["connected"] = snap.connected;
  doc["wifi"] = snap.wifi;
  doc["ip"] = snap.ip;
  doc["address"] = cfg.worker;
  doc["stratum"] = cfg.stratum;
  doc["difficulty"] = snap.difficulty;
  doc["uptime_secs"] = (uint32_t)(millis() / 1000);
  doc["cpu_mhz"] = snap.cpuMhz ? snap.cpuMhz : cfg.cpuMhz;
  doc["hash_focus"] = snap.hashFocus;
  char nonceHex[9];
  snprintf(nonceHex, sizeof(nonceHex), "%08x", snap.nonce);
  doc["nonce"] = nonceHex;
  Serial.print("CMPSTATUS ");
  serializeJson(doc, Serial);
  Serial.println();
}

void CompanionLink::replyConfig(const AppConfig& cfg) {
  JsonDocument doc;
  doc["worker"] = cfg.worker;
  doc["stratum"] = cfg.stratum;
  doc["wifi_ssid"] = cfg.wifiSsid;
  doc["wifi_password"] = cfg.wifiPassword.length() ? "********" : "";
  doc["cpu_mhz"] = cfg.cpuMhz;
  doc["hash_focus"] = cfg.hashFocus;
  doc["fw"] = "0.2.0-cpp";
  doc["configured"] = cfg.isComplete();
  Serial.print("CMPCONFIG ");
  serializeJson(doc, Serial);
  Serial.println();
}

String CompanionLink::urlDecode(const String& in) {
  String out;
  out.reserve(in.length());
  for (unsigned i = 0; i < in.length(); i++) {
    char c = in[i];
    if (c == '+') {
      out += ' ';
    } else if (c == '%' && i + 2 < in.length()) {
      char h[3] = {in[i + 1], in[i + 2], 0};
      out += (char)strtol(h, nullptr, 16);
      i += 2;
    } else {
      out += c;
    }
  }
  return out;
}

void CompanionLink::parseBody(const String& body, AppConfig& cfg, bool& reboot, bool& reconnect,
                              String& auth) {
  int start = 0;
  while (start < (int)body.length()) {
    int amp = body.indexOf('&', start);
    String pair = (amp < 0) ? body.substring(start) : body.substring(start, amp);
    int eq = pair.indexOf('=');
    String key = (eq < 0) ? pair : pair.substring(0, eq);
    String val = (eq < 0) ? "" : urlDecode(pair.substring(eq + 1));
    key.toLowerCase();
    if (key == "wifi_ssid" || key == "ssid") cfg.wifiSsid = val;
    else if (key == "wifi_password" || key == "wifi_pass") cfg.wifiPassword = val;
    else if (key == "stratum") cfg.stratum = val;
    else if (key == "worker" || key == "address") cfg.worker = val;
    else if (key == "password" || key == "pool_password") cfg.password = val;
    else if (key == "auth" || key == "password_auth" || key == "current_password") auth = val;
    else if (key == "cpu_mhz" || key == "clock") {
      cfg.cpuMhz = cfg.normalizeCpu((uint8_t)val.toInt());
      reboot = true;
    } else if (key == "hash_focus" || key == "perf") {
      cfg.hashFocus = (val == "1" || val.equalsIgnoreCase("true"));
    } else if (key == "touch_map") {
      cfg.touchMap = (uint8_t)val.toInt();
    } else if (key == "reboot") {
      reboot = (val == "1" || val.equalsIgnoreCase("true"));
    } else if (key == "reconnect") {
      reconnect = (val == "1" || val.equalsIgnoreCase("true"));
    }
    if (amp < 0) break;
    start = amp + 1;
  }
}
