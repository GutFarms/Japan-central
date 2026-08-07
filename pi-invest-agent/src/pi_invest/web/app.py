from __future__ import annotations

from fastapi import FastAPI, HTTPException
from fastapi.responses import HTMLResponse
from pydantic import BaseModel, Field

from pi_invest.agent import InvestAgent
from pi_invest.config import AppConfig
from pi_invest.storage.db import Database
from pi_invest.wallet import WalletError, WalletService

DASHBOARD_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Pi Invest Agent</title>
  <style>
    :root {
      --bg: #0f1410;
      --panel: #182018;
      --ink: #e7efe6;
      --muted: #8fa08c;
      --accent: #7cb87c;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      background:
        radial-gradient(1200px 600px at 10% -10%, #1d3320 0%, transparent 55%),
        radial-gradient(900px 500px at 100% 0%, #243028 0%, transparent 50%),
        var(--bg);
      color: var(--ink);
      min-height: 100vh;
    }
    main { max-width: 960px; margin: 0 auto; padding: 2.5rem 1.25rem 4rem; }
    h1 {
      font-family: "IBM Plex Serif", Georgia, serif;
      font-weight: 600;
      font-size: clamp(1.8rem, 4vw, 2.6rem);
      margin: 0 0 0.35rem;
      letter-spacing: -0.02em;
    }
    .sub { color: var(--muted); margin-bottom: 2rem; }
    .grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); }
    .stat {
      padding: 1rem 1.1rem;
      border: 1px solid #2a3a2c;
      background: color-mix(in srgb, var(--panel) 90%, transparent);
    }
    .stat .label { color: var(--muted); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.06em; }
    .stat .value { font-size: 1.45rem; margin-top: 0.35rem; font-variant-numeric: tabular-nums; }
    h2 { margin: 2rem 0 0.75rem; font-size: 1.05rem; color: var(--accent); font-weight: 600; }
    table { width: 100%; border-collapse: collapse; font-size: 0.95rem; }
    th, td { text-align: left; padding: 0.55rem 0.4rem; border-bottom: 1px solid #243028; }
    th { color: var(--muted); font-weight: 500; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .muted { color: var(--muted); }
    .pill {
      display: inline-block;
      border: 1px solid #3a523c;
      padding: 0.15rem 0.55rem;
      font-size: 0.75rem;
      color: var(--accent);
    }
    .forms { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
    .form {
      border: 1px solid #2a3a2c;
      padding: 1rem;
      background: color-mix(in srgb, var(--panel) 88%, transparent);
    }
    .form h3 { margin: 0 0 0.75rem; font-size: 0.95rem; }
    label { display: block; font-size: 0.78rem; color: var(--muted); margin: 0.45rem 0 0.2rem; }
    input, select {
      width: 100%;
      background: #101610;
      border: 1px solid #2f4032;
      color: var(--ink);
      padding: 0.45rem 0.55rem;
    }
    button, .btn {
      margin-top: 0.85rem;
      background: var(--accent);
      color: #0c120e;
      border: 0;
      padding: 0.65rem 1rem;
      font-weight: 600;
      cursor: pointer;
    }
    button.secondary { background: transparent; color: var(--accent); border: 1px solid #3a523c; }
    button:hover { filter: brightness(1.08); }
    #err { color: #e08080; margin-top: 1rem; white-space: pre-wrap; }
    code { font-size: 0.82rem; word-break: break-all; }
  </style>
</head>
<body>
  <main>
    <h1>Pi Invest</h1>
    <p class="sub">Income agent + USD/crypto wallet · <span id="mode" class="pill">…</span></p>
    <div class="grid">
      <div class="stat"><div class="label">Equity</div><div class="value" id="equity">—</div></div>
      <div class="stat"><div class="label">Broker cash</div><div class="value" id="cash">—</div></div>
      <div class="stat"><div class="label">Wallet ~USD</div><div class="value" id="walletusd">—</div></div>
      <div class="stat"><div class="label">Day PnL</div><div class="value" id="pnl">—</div></div>
    </div>

    <h2>Wallet balances</h2>
    <table>
      <thead><tr><th>Asset</th><th>Amount</th><th>~USD</th><th>Receive</th></tr></thead>
      <tbody id="wallet"></tbody>
    </table>

    <h2>Send / receive</h2>
    <div class="forms">
      <div class="form">
        <h3>Send</h3>
        <label>Asset</label>
        <select id="send-asset"></select>
        <label>Amount</label>
        <input id="send-amount" type="number" step="any" />
        <label>To address / USD account</label>
        <input id="send-to" placeholder="paper-btc-… or usd:…" />
        <label>Memo</label>
        <input id="send-memo" />
        <button id="send-btn">Send</button>
      </div>
      <div class="form">
        <h3>Receive (credit inbound)</h3>
        <label>Asset</label>
        <select id="recv-asset"></select>
        <label>Amount</label>
        <input id="recv-amount" type="number" step="any" />
        <label>From</label>
        <input id="recv-from" value="external" />
        <button id="recv-btn">Credit receive</button>
        <p class="muted" style="margin-top:0.75rem;font-size:0.85rem">
          Paper mode: use this to simulate an inbound payment to your receive address.
        </p>
      </div>
    </div>

    <h2>Holdings</h2>
    <table>
      <thead><tr><th>Symbol</th><th>Qty</th><th>Value</th><th>uPnL</th></tr></thead>
      <tbody id="positions"></tbody>
    </table>
    <h2>Recent wallet transfers</h2>
    <table>
      <thead><tr><th>When</th><th>Dir</th><th>Asset</th><th>Amount</th><th>Counterparty</th></tr></thead>
      <tbody id="transfers"></tbody>
    </table>
    <h2>Latest income scores</h2>
    <table>
      <thead><tr><th>Symbol</th><th>Income</th><th>Composite</th><th>Notes</th></tr></thead>
      <tbody id="scores"></tbody>
    </table>
    <button id="cycle" class="secondary">Run one invest cycle</button>
    <div id="err"></div>
  </main>
  <script>
    const fmt = (n) => Number(n).toLocaleString(undefined, {style:'currency', currency:'USD'});
    const amt = (n, asset) => asset === 'USD' || asset === 'USDC'
      ? Number(n).toFixed(2)
      : Number(n).toPrecision(6);
    function fillAssets(assets) {
      for (const id of ['send-asset', 'recv-asset']) {
        const el = document.getElementById(id);
        const cur = el.value;
        el.innerHTML = assets.map(a => `<option value="${a}">${a}</option>`).join('');
        if (assets.includes(cur)) el.value = cur;
      }
    }
    async function refresh() {
      const r = await fetch('/api/status');
      const d = await r.json();
      document.getElementById('mode').textContent = `${d.mode} / ${d.backend} · wallet ${d.wallet.backend}`;
      document.getElementById('equity').textContent = fmt(d.account.equity);
      document.getElementById('cash').textContent = fmt(d.account.cash);
      document.getElementById('walletusd').textContent = fmt(d.wallet.total_usd_estimate);
      document.getElementById('pnl').textContent = `${fmt(d.account.day_pnl)} (${(d.account.day_pnl_pct*100).toFixed(2)}%)`;
      const assets = d.wallet.balances.map(b => b.asset);
      fillAssets(assets);
      document.getElementById('wallet').innerHTML = d.wallet.balances.map(b => {
        const addr = (d.wallet.addresses[b.asset] || {}).address || '—';
        return `<tr><td>${b.asset}</td><td>${amt(b.amount, b.asset)}</td><td>${fmt(b.amount * (b.usd_mark||0))}</td><td><code>${addr}</code></td></tr>`;
      }).join('') || '<tr><td colspan=4 class="muted">Empty wallet</td></tr>';
      document.getElementById('positions').innerHTML = d.account.positions.map(p =>
        `<tr><td>${p.symbol}</td><td>${p.qty.toFixed(4)}</td><td>${fmt(p.market_value)}</td><td>${fmt(p.unrealized_pnl)}</td></tr>`
      ).join('') || '<tr><td colspan=4 class="muted">No positions</td></tr>';
      document.getElementById('transfers').innerHTML = (d.transfers||[]).map(t =>
        `<tr><td class="muted">${t.timestamp}</td><td>${t.direction}</td><td>${t.asset}</td><td>${amt(t.amount, t.asset)}</td><td>${t.counterparty}</td></tr>`
      ).join('') || '<tr><td colspan=5 class="muted">No transfers yet</td></tr>';
      document.getElementById('scores').innerHTML = (d.latest_scores||[]).map(s =>
        `<tr><td>${s.symbol}</td><td>${s.expected_income_proxy.toFixed(2)}</td><td>${s.composite.toFixed(2)}</td><td class="muted">${(s.notes||[]).join(', ')}</td></tr>`
      ).join('') || '<tr><td colspan=4 class="muted">No cycle yet</td></tr>';
    }
    async function post(url, body) {
      document.getElementById('err').textContent = '';
      const r = await fetch(url, {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(body),
      });
      if (!r.ok) {
        const t = await r.text();
        throw new Error(t);
      }
      return r.json();
    }
    document.getElementById('send-btn').onclick = async () => {
      try {
        await post('/api/wallet/send', {
          asset: document.getElementById('send-asset').value,
          amount: Number(document.getElementById('send-amount').value),
          to_address: document.getElementById('send-to').value,
          memo: document.getElementById('send-memo').value,
        });
        await refresh();
      } catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('recv-btn').onclick = async () => {
      try {
        await post('/api/wallet/receive', {
          asset: document.getElementById('recv-asset').value,
          amount: Number(document.getElementById('recv-amount').value),
          from_address: document.getElementById('recv-from').value,
        });
        await refresh();
      } catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('cycle').onclick = async () => {
      try {
        const r = await fetch('/api/cycle', {method:'POST'});
        if (!r.ok) throw new Error(await r.text());
        await refresh();
      } catch (e) { document.getElementById('err').textContent = String(e); }
    };
    refresh();
    setInterval(refresh, 15000);
  </script>
</body>
</html>
"""


class SendBody(BaseModel):
    asset: str
    amount: float = Field(gt=0)
    to_address: str
    memo: str = ""


class ReceiveBody(BaseModel):
    asset: str
    amount: float = Field(gt=0)
    from_address: str = "external"
    memo: str = ""


class BridgeBody(BaseModel):
    amount: float = Field(gt=0)


def create_app(
    agent: InvestAgent,
    cfg: AppConfig,
    db: Database,
    wallet: WalletService,
) -> FastAPI:
    app = FastAPI(title="Pi Invest Agent", version="0.1.0")

    @app.get("/", response_class=HTMLResponse)
    def home() -> str:
        return DASHBOARD_HTML

    @app.get("/api/status")
    def status() -> dict:
        marks = {}
        for sym in cfg.universe:
            try:
                marks[sym] = agent.market.get_quote(sym).price
            except Exception:  # noqa: BLE001
                pass
        account = agent.broker.get_account(marks)
        decisions = db.recent_decisions(1)
        latest_scores = decisions[0].get("scores", []) if decisions else []
        snap = wallet.snapshot()
        return {
            "mode": cfg.agent.mode,
            "backend": cfg.broker.backend,
            "agent": cfg.agent.name,
            "account": account.model_dump(),
            "wallet": snap.model_dump(mode="json"),
            "transfers": [t.model_dump(mode="json") for t in wallet.history(15)],
            "latest_scores": latest_scores,
            "orders": db.recent_orders(15),
        }

    @app.post("/api/cycle")
    def cycle() -> dict:
        decision = agent.run_cycle(dry_run=False)
        return decision.model_dump(mode="json")

    @app.get("/api/wallet")
    def wallet_status() -> dict:
        return wallet.snapshot().model_dump(mode="json")

    @app.get("/api/wallet/receive/{asset}")
    def wallet_receive_address(asset: str) -> dict:
        try:
            return wallet.receive_info(asset).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/wallet/send")
    def wallet_send(body: SendBody) -> dict:
        try:
            return wallet.send(
                body.asset, body.amount, body.to_address, memo=body.memo
            ).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/wallet/receive")
    def wallet_receive(body: ReceiveBody) -> dict:
        try:
            return wallet.receive(
                body.asset,
                body.amount,
                from_address=body.from_address,
                memo=body.memo,
            ).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/wallet/bridge/to-broker")
    def bridge_to_broker(body: BridgeBody) -> dict:
        try:
            return wallet.bridge_to_broker(body.amount).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/wallet/bridge/from-broker")
    def bridge_from_broker(body: BridgeBody) -> dict:
        try:
            return wallet.bridge_from_broker(body.amount).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.get("/api/health")
    def health() -> dict:
        return {"ok": True, "agent": cfg.agent.name}

    return app
