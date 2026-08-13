import AsyncStorage from "@react-native-async-storage/async-storage";
import { LinearGradient } from "expo-linear-gradient";
import { StatusBar } from "expo-status-bar";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ActivityIndicator,
  Pressable,
  SafeAreaView,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";

const MONITOR_PORT = 19285;
const HOST_KEY = "cyd_monitor_host";

type Board = {
  endpoint: string;
  mac: string;
  fw: string;
  hashrate_hs: number;
  hashes: number;
  mining: boolean;
};

type Snapshot = {
  version: string;
  product: string;
  mining: boolean;
  usb_open: boolean;
  pool_phase: string;
  pool_authorized: boolean;
  hashrate_hs: number;
  hashes: number;
  accepted: number;
  rejected: number;
  boards: Board[];
  host: string;
  updated_ms: number;
};

function fmtRate(hs: number): [string, string] {
  const v = Number(hs) || 0;
  if (v < 1000) return [v.toFixed(0), "H/s"];
  if (v < 1_000_000) {
    const k = v / 1000;
    return [k >= 100 ? k.toFixed(0) : k >= 10 ? k.toFixed(1) : k.toFixed(2), "kH/s"];
  }
  return [(v / 1_000_000).toFixed(2), "MH/s"];
}

function normalizeHost(raw: string): string {
  return raw
    .trim()
    .replace(/^https?:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/:\d+$/, "");
}

export default function App() {
  const [host, setHost] = useState("");
  const [draft, setDraft] = useState("");
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    AsyncStorage.getItem(HOST_KEY).then((v) => {
      if (v) {
        setHost(v);
        setDraft(v);
      }
    });
  }, []);

  const poll = useCallback(async () => {
    const h = normalizeHost(host);
    if (!h) {
      setError("Enter your Companion PC LAN IP (Settings → Phone monitor).");
      return;
    }
    setLoading(true);
    try {
      const ctrl = new AbortController();
      const t = setTimeout(() => ctrl.abort(), 4000);
      const res = await fetch(`http://${h}:${MONITOR_PORT}/api/status`, {
        signal: ctrl.signal,
        headers: { Accept: "application/json" },
      });
      clearTimeout(t);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as Snapshot;
      setSnap(json);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error
          ? `Cannot reach Companion at ${h}:${MONITOR_PORT} — ${e.message}`
          : "Cannot reach Companion",
      );
    } finally {
      setLoading(false);
    }
  }, [host]);

  useEffect(() => {
    if (!host) return;
    poll();
    const id = setInterval(poll, 2000);
    return () => clearInterval(id);
  }, [host, poll]);

  const saveHost = async () => {
    const h = normalizeHost(draft);
    setDraft(h);
    setHost(h);
    await AsyncStorage.setItem(HOST_KEY, h);
  };

  const [rateNum, rateUnit] = useMemo(
    () => fmtRate(snap?.hashrate_hs ?? 0),
    [snap?.hashrate_hs],
  );

  const poolLabel = snap?.pool_authorized
    ? "AUTHORIZED"
    : (snap?.pool_phase || "IDLE").toUpperCase();

  return (
    <LinearGradient colors={["#031428", "#020a16", "#01060e"]} style={styles.root}>
      <StatusBar style="light" />
      <SafeAreaView style={styles.safe}>
        <ScrollView contentContainerStyle={styles.scroll} keyboardShouldPersistTaps="handled">
          <Text style={styles.brand}>Njörðr Seas'</Text>
          <Text style={styles.tag}>CYD miner · phone monitor</Text>

          <View style={styles.hero}>
            <Text style={styles.rate}>{host ? rateNum : "—"}</Text>
            <Text style={styles.unit}>
              {rateUnit}
              {snap
                ? ` · ${snap.mining ? "mining" : snap.usb_open ? "linked" : "idle"}`
                : " · waiting"}
            </Text>

            <View style={styles.row}>
              <Chip
                label="Pool"
                value={poolLabel}
                tone={snap?.pool_authorized ? "ok" : "warn"}
              />
              <Chip label="Accept" value={String(snap?.accepted ?? 0)} />
              <Chip label="Reject" value={String(snap?.rejected ?? 0)} />
            </View>

            <View style={styles.boards}>
              {(snap?.boards?.length ?? 0) === 0 ? (
                <Text style={styles.board}>No boards linked</Text>
              ) : (
                snap!.boards.map((b) => {
                  const [n, u] = fmtRate(b.hashrate_hs);
                  return (
                    <View key={b.endpoint} style={styles.boardCard}>
                      <Text style={styles.boardTitle}>{b.mac || b.endpoint}</Text>
                      <Text style={styles.boardMeta}>
                        {b.endpoint} · {n} {u}
                        {b.mining ? " · hashing" : ""}
                      </Text>
                    </View>
                  );
                })
              )}
            </View>
          </View>

          <Text style={styles.fieldLabel}>Companion LAN IP</Text>
          <TextInput
            style={styles.input}
            value={draft}
            onChangeText={setDraft}
            placeholder="192.168.1.20"
            placeholderTextColor="#466c8a"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="numbers-and-punctuation"
          />
          <Pressable style={styles.button} onPress={saveHost}>
            <Text style={styles.buttonText}>Save & refresh</Text>
          </Pressable>

          <View style={styles.metaRow}>
            {loading ? <ActivityIndicator color="#7edcff" /> : null}
            <Text style={styles.meta}>
              {error
                ? error
                : snap
                  ? `${snap.product} ${snap.version} · :${MONITOR_PORT}`
                  : `Polls http://IP:${MONITOR_PORT}/api/status`}
            </Text>
          </View>
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}

function Chip({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "ok" | "warn";
}) {
  return (
    <View style={styles.chip}>
      <Text style={styles.chipLabel}>{label}</Text>
      <Text
        style={[
          styles.chipValue,
          tone === "ok" && styles.ok,
          tone === "warn" && styles.warn,
        ]}
      >
        {value}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1 },
  safe: { flex: 1 },
  scroll: { paddingHorizontal: 18, paddingTop: 18, paddingBottom: 40 },
  brand: {
    fontSize: 30,
    fontWeight: "700",
    color: "#7edcff",
    letterSpacing: 0.3,
  },
  tag: { marginTop: 6, marginBottom: 20, color: "#789eba", fontSize: 14 },
  hero: {
    borderRadius: 22,
    borderWidth: 1,
    borderColor: "rgba(126,220,255,0.28)",
    backgroundColor: "rgba(6,20,38,0.88)",
    paddingHorizontal: 16,
    paddingVertical: 20,
  },
  rate: {
    fontSize: 56,
    fontWeight: "700",
    color: "#e6f2fc",
    letterSpacing: -1,
  },
  unit: { marginTop: 8, color: "#7edcff", fontSize: 16 },
  row: { flexDirection: "row", gap: 10, marginTop: 18, flexWrap: "wrap" },
  chip: {
    flexGrow: 1,
    flexBasis: 100,
    borderRadius: 14,
    paddingVertical: 12,
    paddingHorizontal: 12,
    backgroundColor: "rgba(8,28,48,0.95)",
    borderWidth: 1,
    borderColor: "rgba(36,78,118,0.7)",
  },
  chipLabel: {
    color: "#466c8a",
    fontSize: 11,
    letterSpacing: 1,
    textTransform: "uppercase",
    fontWeight: "600",
  },
  chipValue: { marginTop: 6, color: "#e6f2fc", fontSize: 17, fontWeight: "600" },
  ok: { color: "#7edcff" },
  warn: { color: "#ffc45b" },
  boards: { marginTop: 16 },
  board: { color: "#789eba", fontSize: 13 },
  boardCard: {
    marginTop: 8,
    padding: 12,
    borderRadius: 14,
    backgroundColor: "rgba(5,18,34,0.9)",
    borderWidth: 1,
    borderColor: "rgba(36,78,118,0.55)",
  },
  boardTitle: { color: "#e6f2fc", fontWeight: "600", fontSize: 14 },
  boardMeta: { marginTop: 4, color: "#789eba", fontSize: 12 },
  fieldLabel: { marginTop: 22, color: "#466c8a", fontSize: 12 },
  input: {
    marginTop: 8,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#244e76",
    backgroundColor: "#061426",
    color: "#e6f2fc",
    paddingHorizontal: 14,
    paddingVertical: 12,
    fontSize: 16,
  },
  button: {
    marginTop: 10,
    borderRadius: 12,
    backgroundColor: "#7edcff",
    paddingVertical: 13,
    alignItems: "center",
  },
  buttonText: { color: "#031018", fontWeight: "700", fontSize: 15 },
  metaRow: { marginTop: 16, flexDirection: "row", gap: 10, alignItems: "center" },
  meta: { flex: 1, color: "#466c8a", fontSize: 12, lineHeight: 18 },
});
