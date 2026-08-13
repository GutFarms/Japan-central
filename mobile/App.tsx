import AsyncStorage from "@react-native-async-storage/async-storage";
import { CameraView, useCameraPermissions } from "expo-camera";
import * as Linking from "expo-linking";
import { LinearGradient } from "expo-linear-gradient";
import { StatusBar } from "expo-status-bar";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ActivityIndicator,
  Modal,
  Pressable,
  SafeAreaView,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";

const DEFAULT_PORT = 19285;
const HOST_KEY = "cyd_monitor_host";
const TOKEN_KEY = "cyd_monitor_token";
const ID_KEY = "cyd_monitor_id";
const PORT_KEY = "cyd_monitor_port";

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
  pair_id?: string;
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

type Pairing = {
  host: string;
  port: number;
  id: string;
  token: string;
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

/** Parse Companion personal QR / deep link / web pair URL. */
export function parsePairPayload(raw: string): Pairing | null {
  const text = raw.trim();
  if (!text) return null;

  try {
    if (text.startsWith("njordrseas://") || text.startsWith("http://") || text.startsWith("https://")) {
      const url = Linking.parse(text);
      const q = (url.queryParams || {}) as Record<string, string | string[] | undefined>;
      const one = (k: string) => {
        const v = q[k];
        return Array.isArray(v) ? v[0] || "" : v || "";
      };
      let host = one("host");
      const token = one("token") || one("t");
      const id = one("id");
      const portRaw = one("port");
      const port = portRaw ? Number(portRaw) || DEFAULT_PORT : DEFAULT_PORT;

      if (!host && url.hostname) {
        host = url.hostname;
      }
      host = normalizeHost(host);
      if (host && token) {
        return { host, port, id, token };
      }
    }
  } catch {
    // fall through
  }

  // Bare "host|token|id" paste fallback
  const parts = text.split("|").map((s) => s.trim());
  if (parts.length >= 2 && parts[0] && parts[1]) {
    return {
      host: normalizeHost(parts[0]),
      port: DEFAULT_PORT,
      token: parts[1],
      id: parts[2] || "",
    };
  }
  return null;
}

export default function App() {
  const [host, setHost] = useState("");
  const [token, setToken] = useState("");
  const [pairId, setPairId] = useState("");
  const [port, setPort] = useState(DEFAULT_PORT);
  const [draftHost, setDraftHost] = useState("");
  const [draftToken, setDraftToken] = useState("");
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanLocked, setScanLocked] = useState(false);
  const [permission, requestPermission] = useCameraPermissions();

  const applyPairing = useCallback(async (p: Pairing) => {
    setHost(p.host);
    setDraftHost(p.host);
    setToken(p.token);
    setDraftToken(p.token);
    setPairId(p.id);
    setPort(p.port || DEFAULT_PORT);
    await AsyncStorage.multiSet([
      [HOST_KEY, p.host],
      [TOKEN_KEY, p.token],
      [ID_KEY, p.id],
      [PORT_KEY, String(p.port || DEFAULT_PORT)],
    ]);
    setScanning(false);
    setScanLocked(false);
    setError(null);
  }, []);

  useEffect(() => {
    (async () => {
      const [h, t, id, p] = await Promise.all([
        AsyncStorage.getItem(HOST_KEY),
        AsyncStorage.getItem(TOKEN_KEY),
        AsyncStorage.getItem(ID_KEY),
        AsyncStorage.getItem(PORT_KEY),
      ]);
      if (h) {
        setHost(h);
        setDraftHost(h);
      }
      if (t) {
        setToken(t);
        setDraftToken(t);
      }
      if (id) setPairId(id);
      if (p) setPort(Number(p) || DEFAULT_PORT);
    })();
  }, []);

  // Cold-start / OS deep link into the app
  useEffect(() => {
    const applyUrl = async (url: string | null) => {
      if (!url) return;
      const parsed = parsePairPayload(url);
      if (parsed) await applyPairing(parsed);
    };
    Linking.getInitialURL().then(applyUrl);
    const sub = Linking.addEventListener("url", ({ url }) => {
      void applyUrl(url);
    });
    return () => sub.remove();
  }, [applyPairing]);

  const poll = useCallback(async () => {
    const h = normalizeHost(host);
    if (!h) {
      setError("Scan your personal Companion QR (Settings → Phone monitor).");
      return;
    }
    if (!token) {
      setError("Missing personal token — scan the QR from your PC Companion.");
      return;
    }
    setLoading(true);
    try {
      const ctrl = new AbortController();
      const t = setTimeout(() => ctrl.abort(), 5000);
      const res = await fetch(
        `http://${h}:${port}/api/status?token=${encodeURIComponent(token)}`,
        {
          signal: ctrl.signal,
          headers: {
            Accept: "application/json",
            Authorization: `Bearer ${token}`,
            "X-Cyd-Token": token,
          },
        },
      );
      clearTimeout(t);
      if (res.status === 401) {
        throw new Error("Unauthorized — this QR is not for that Companion");
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const json = (await res.json()) as Snapshot;
      if (pairId && json.pair_id && json.pair_id !== pairId) {
        throw new Error("Pair id mismatch — rescan the QR from your PC");
      }
      setSnap(json);
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error
          ? `Cannot reach your Companion at ${h}:${port} — ${e.message}`
          : "Cannot reach Companion",
      );
    } finally {
      setLoading(false);
    }
  }, [host, token, pairId, port]);

  useEffect(() => {
    if (!host || !token) return;
    poll();
    const id = setInterval(poll, 2000);
    return () => clearInterval(id);
  }, [host, token, poll]);

  const saveManual = async () => {
    const h = normalizeHost(draftHost);
    const t = draftToken.trim();
    setDraftHost(h);
    setHost(h);
    setToken(t);
    setDraftToken(t);
    await AsyncStorage.multiSet([
      [HOST_KEY, h],
      [TOKEN_KEY, t],
    ]);
  };

  const clearPairing = async () => {
    setHost("");
    setDraftHost("");
    setToken("");
    setDraftToken("");
    setPairId("");
    setSnap(null);
    await AsyncStorage.multiRemove([HOST_KEY, TOKEN_KEY, ID_KEY, PORT_KEY]);
  };

  const openScanner = async () => {
    if (!permission?.granted) {
      const res = await requestPermission();
      if (!res.granted) {
        setError("Camera permission is required to scan your personal QR.");
        return;
      }
    }
    setScanLocked(false);
    setScanning(true);
  };

  const onBarcode = async ({ data }: { data: string }) => {
    if (scanLocked) return;
    const parsed = parsePairPayload(data);
    if (parsed) {
      setScanLocked(true);
      await applyPairing(parsed);
    }
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
          <Text style={styles.tag}>CYD miner · personal phone monitor</Text>

          <View style={styles.hero}>
            <Text style={styles.rate}>{host && token ? rateNum : "—"}</Text>
            <Text style={styles.unit}>
              {rateUnit}
              {snap
                ? ` · ${snap.mining ? "mining" : snap.usb_open ? "linked" : "idle"}`
                : " · waiting for your QR"}
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

          <Pressable style={styles.button} onPress={openScanner}>
            <Text style={styles.buttonText}>Scan personal QR</Text>
          </Pressable>

          <Text style={styles.fieldLabel}>Companion host (LAN / DDNS / Tailscale)</Text>
          <TextInput
            style={styles.input}
            value={draftHost}
            onChangeText={setDraftHost}
            placeholder="192.168.1.20"
            placeholderTextColor="#466c8a"
            autoCapitalize="none"
            autoCorrect={false}
            keyboardType="numbers-and-punctuation"
          />
          <Text style={styles.fieldLabel}>Personal token</Text>
          <TextInput
            style={styles.input}
            value={draftToken}
            onChangeText={setDraftToken}
            placeholder="from your Companion QR"
            placeholderTextColor="#466c8a"
            autoCapitalize="none"
            autoCorrect={false}
            secureTextEntry
          />
          <Pressable style={styles.buttonSecondary} onPress={saveManual}>
            <Text style={styles.buttonSecondaryText}>Save & refresh</Text>
          </Pressable>
          {(host || token) && (
            <Pressable style={styles.linkBtn} onPress={clearPairing}>
              <Text style={styles.linkText}>Forget this Companion</Text>
            </Pressable>
          )}

          <View style={styles.metaRow}>
            {loading ? <ActivityIndicator color="#7edcff" /> : null}
            <Text style={styles.meta}>
              {error
                ? error
                : snap
                  ? `${snap.product} ${snap.version} · pair ${snap.pair_id || pairId || "?"}`
                  : "Scan the QR shown in Companion → Settings → Phone monitor"}
            </Text>
          </View>
        </ScrollView>
      </SafeAreaView>

      <Modal visible={scanning} animationType="slide">
        <View style={styles.scanRoot}>
          <CameraView
            style={StyleSheet.absoluteFill}
            facing="back"
            barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
            onBarcodeScanned={onBarcode}
          />
          <SafeAreaView style={styles.scanOverlay}>
            <Text style={styles.scanTitle}>Scan your Companion QR</Text>
            <Text style={styles.scanHint}>
              Only this personal code unlocks your PC miner — not anyone else’s.
            </Text>
            <Pressable style={styles.button} onPress={() => setScanning(false)}>
              <Text style={styles.buttonText}>Cancel</Text>
            </Pressable>
          </SafeAreaView>
        </View>
      </Modal>
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
  fieldLabel: { marginTop: 18, color: "#466c8a", fontSize: 12 },
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
    marginTop: 16,
    borderRadius: 12,
    backgroundColor: "#7edcff",
    paddingVertical: 13,
    alignItems: "center",
  },
  buttonText: { color: "#031018", fontWeight: "700", fontSize: 15 },
  buttonSecondary: {
    marginTop: 10,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "#7edcff",
    paddingVertical: 12,
    alignItems: "center",
  },
  buttonSecondaryText: { color: "#7edcff", fontWeight: "700", fontSize: 15 },
  linkBtn: { marginTop: 14, alignItems: "center" },
  linkText: { color: "#789eba", fontSize: 13 },
  metaRow: { marginTop: 16, flexDirection: "row", gap: 10, alignItems: "center" },
  meta: { flex: 1, color: "#466c8a", fontSize: 12, lineHeight: 18 },
  scanRoot: { flex: 1, backgroundColor: "#01060e" },
  scanOverlay: {
    flex: 1,
    justifyContent: "flex-end",
    padding: 20,
    backgroundColor: "transparent",
  },
  scanTitle: {
    color: "#e6f2fc",
    fontSize: 22,
    fontWeight: "700",
    marginBottom: 8,
  },
  scanHint: { color: "#789eba", fontSize: 14, marginBottom: 12, lineHeight: 20 },
});
