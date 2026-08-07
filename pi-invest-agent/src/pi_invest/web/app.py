from __future__ import annotations

import secrets
from typing import Annotated

from fastapi import Depends, FastAPI, HTTPException, status
from fastapi.responses import HTMLResponse
from fastapi.security import HTTPBasic, HTTPBasicCredentials
from pydantic import BaseModel, Field

from pi_invest.agent import InvestAgent
from pi_invest.config import AppConfig, EnvSettings
from pi_invest.journal import PerformanceJournal
from pi_invest.safety import SafetyGate
from pi_invest.storage.db import Database
from pi_invest.wallet import WalletError, WalletService

security = HTTPBasic(auto_error=False)

DASHBOARD_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Pi Invest Agent</title>
  <style>
    :root {
      --bg: #0f1410; --panel: #182018; --ink: #e7efe6; --muted: #8fa08c;
      --accent: #7cb87c; --danger: #c45c5c;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0; font-family: "IBM Plex Sans", "Segoe UI", sans-serif; color: var(--ink);
      min-height: 100vh;
      background:
        radial-gradient(1200px 600px at 10% -10%, #1d3320 0%, transparent 55%),
        radial-gradient(900px 500px at 100% 0%, #243028 0%, transparent 50%),
        var(--bg);
    }
    main { max-width: 960px; margin: 0 auto; padding: 2.5rem 1.25rem 4rem; }
    h1 {
      font-family: "IBM Plex Serif", Georgia, serif; font-weight: 600;
      font-size: clamp(1.8rem, 4vw, 2.6rem); margin: 0 0 0.35rem; letter-spacing: -0.02em;
    }
    .sub { color: var(--muted); margin-bottom: 1.25rem; }
    .grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); }
    .stat {
      padding: 1rem 1.1rem; border: 1px solid #2a3a2c;
      background: color-mix(in srgb, var(--panel) 90%, transparent);
    }
    .stat .label { color: var(--muted); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.06em; }
    .stat .value { font-size: 1.35rem; margin-top: 0.35rem; font-variant-numeric: tabular-nums; }
    h2 { margin: 2rem 0 0.75rem; font-size: 1.05rem; color: var(--accent); font-weight: 600; }
    table { width: 100%; border-collapse: collapse; font-size: 0.95rem; }
    th, td { text-align: left; padding: 0.55rem 0.4rem; border-bottom: 1px solid #243028; }
    th { color: var(--muted); font-weight: 500; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .muted { color: var(--muted); }
    .pill {
      display: inline-block; border: 1px solid #3a523c; padding: 0.15rem 0.55rem;
      font-size: 0.75rem; color: var(--accent);
    }
    .pill.danger { border-color: #6a3030; color: #e08080; }
    .forms { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
    .form {
      border: 1px solid #2a3a2c; padding: 1rem;
      background: color-mix(in srgb, var(--panel) 88%, transparent);
    }
    .form h3 { margin: 0 0 0.75rem; font-size: 0.95rem; }
    label { display: block; font-size: 0.78rem; color: var(--muted); margin: 0.45rem 0 0.2rem; }
    input, select {
      width: 100%; background: #101610; border: 1px solid #2f4032;
      color: var(--ink); padding: 0.45rem 0.55rem;
    }
    button {
      margin-top: 0.85rem; background: var(--accent); color: #0c120e; border: 0;
      padding: 0.65rem 1rem; font-weight: 600; cursor: pointer; margin-right: 0.5rem;
    }
    button.danger { background: var(--danger); color: #fff; }
    button.secondary { background: transparent; color: var(--accent); border: 1px solid #3a523c; }
    #err { color: #e08080; margin-top: 1rem; white-space: pre-wrap; }
    code { font-size: 0.82rem; word-break: break-all; }
  </style>
</head>
<body>
  <main>
    <h1>Pi Invest</h1>
    <p class="sub">Income agent + wallet · <span id="mode" class="pill">…</span> <span id="haltpill" class="pill">…</span></p>
    <div class="grid">
      <div class="stat"><div class="label">Equity</div><div class="value" id="equity">—</div></div>
      <div class="stat"><div class="label">Total NAV</div><div class="value" id="nav">—</div></div>
      <div class="stat"><div class="label">Max drawdown</div><div class="value" id="dd">—</div></div>
      <div class="stat"><div class="label">Wallet ~USD</div><div class="value" id="walletusd">—</div></div>
    </div>

    <h2>Safety</h2>
    <button id="halt-btn" class="danger">Halt trading + sends</button>
    <button id="resume-btn" class="secondary">Resume</button>
    <p class="muted" id="halt-reason"></p>

    <h2>Performance journal</h2>
    <table>
      <thead><tr><th>When</th><th>NAV</th><th>Equity</th><th>Wallet</th><th>DD</th></tr></thead>
      <tbody id="journal"></tbody>
    </table>

    <h2>Wallet balances</h2>
    <table>
      <thead><tr><th>Asset</th><th>Amount</th><th>~USD</th><th>Receive</th></tr></thead>
      <tbody id="wallet"></tbody>
    </table>

    <h2>Send / receive</h2>
    <div class="forms">
      <div class="form">
        <h3>Send</h3>
        <label>Asset</label><select id="send-asset"></select>
        <label>Amount</label><input id="send-amount" type="number" step="any" />
        <label>To</label><input id="send-to" />
        <label>Memo</label><input id="send-memo" />
        <button id="send-btn">Send</button>
      </div>
      <div class="form">
        <h3>Receive (credit inbound)</h3>
        <label>Asset</label><select id="recv-asset"></select>
        <label>Amount</label><input id="recv-amount" type="number" step="any" />
        <label>From</label><input id="recv-from" value="external" />
        <button id="recv-btn">Credit receive</button>
      </div>
    </div>

    <h2>Holdings</h2>
    <table>
      <thead><tr><th>Symbol</th><th>Qty</th><th>Value</th><th>uPnL</th></tr></thead>
      <tbody id="positions"></tbody>
    </table>
    <button id="cycle" class="secondary">Run one invest cycle</button>
    <div id="err"></div>
  </main>
  <script>
    const fmt = (n) => Number(n).toLocaleString(undefined, {style:'currency', currency:'USD'});
    const amt = (n, asset) => (asset === 'USD' || asset === 'USDC')
      ? Number(n).toFixed(2) : Number(n).toPrecision(6);

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
      document.getElementById('mode').textContent =
        `${d.mode} / ${d.backend} · wallet ${d.wallet.backend}`;
      const hp = document.getElementById('haltpill');
      hp.textContent = d.halted ? 'HALTED' : 'LIVE';
      hp.className = d.halted ? 'pill danger' : 'pill';
      document.getElementById('halt-reason').textContent = d.halt_reason || '';
      document.getElementById('equity').textContent = fmt(d.account.equity);
      const nav = d.journal.latest_nav || (d.account.equity + d.wallet.total_usd_estimate);
      document.getElementById('nav').textContent = fmt(nav);
      document.getElementById('dd').textContent =
        `${((d.journal.max_drawdown_pct || 0) * 100).toFixed(2)}%`;
      document.getElementById('walletusd').textContent = fmt(d.wallet.total_usd_estimate);
      fillAssets(d.wallet.balances.map(b => b.asset));
      document.getElementById('wallet').innerHTML = d.wallet.balances.map(b => {
        const addr = (d.wallet.addresses[b.asset] || {}).address || '—';
        return `<tr><td>${b.asset}</td><td>${amt(b.amount, b.asset)}</td>
          <td>${fmt(b.amount * (b.usd_mark || 0))}</td><td><code>${addr}</code></td></tr>`;
      }).join('') || '<tr><td colspan=4 class="muted">Empty</td></tr>';
      document.getElementById('journal').innerHTML = (d.journal_rows || []).map(j =>
        `<tr><td class="muted">${j.timestamp}</td><td>${fmt(j.total_nav)}</td>
         <td>${fmt(j.equity)}</td><td>${fmt(j.wallet_usd)}</td>
         <td>${(j.drawdown_pct * 100).toFixed(2)}%</td></tr>`
      ).join('') || '<tr><td colspan=5 class="muted">No snapshots yet</td></tr>';
      document.getElementById('positions').innerHTML = d.account.positions.map(p =>
        `<tr><td>${p.symbol}</td><td>${p.qty.toFixed(4)}</td>
         <td>${fmt(p.market_value)}</td><td>${fmt(p.unrealized_pnl)}</td></tr>`
      ).join('') || '<tr><td colspan=4 class="muted">No positions</td></tr>';
    }

    async function post(url, body) {
      document.getElementById('err').textContent = '';
      const r = await fetch(url, {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: body ? JSON.stringify(body) : '{}'
      });
      if (!r.ok) throw new Error(await r.text());
      return r.json();
    }

    document.getElementById('halt-btn').onclick = async () => {
      try { await post('/api/halt', {reason: 'dashboard halt'}); await refresh(); }
      catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('resume-btn').onclick = async () => {
      try { await post('/api/resume'); await refresh(); }
      catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('send-btn').onclick = async () => {
      try {
        await post('/api/wallet/send', {
          asset: document.getElementById('send-asset').value,
          amount: Number(document.getElementById('send-amount').value),
          to_address: document.getElementById('send-to').value,
          memo: document.getElementById('send-memo').value
        });
        await refresh();
      } catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('recv-btn').onclick = async () => {
      try {
        await post('/api/wallet/receive', {
          asset: document.getElementById('recv-asset').value,
          amount: Number(document.getElementById('recv-amount').value),
          from_address: document.getElementById('recv-from').value
        });
        await refresh();
      } catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('cycle').onclick = async () => {
      try { await post('/api/cycle'); await refresh(); }
      catch (e) { document.getElementById('err').textContent = String(e); }
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


class HaltBody(BaseModel):
    reason: str = "dashboard halt"


def create_app(
    agent: InvestAgent,
    cfg: AppConfig,
    db: Database,
    wallet: WalletService,
    env: EnvSettings | None = None,
    safety: SafetyGate | None = None,
    journal: PerformanceJournal | None = None,
) -> FastAPI:
    env = env or EnvSettings()
    safety = safety or SafetyGate(db)
    journal = journal or PerformanceJournal(db, safety)
    app = FastAPI(title="Pi Invest Agent", version="0.1.0")

    def require_user(
        credentials: Annotated[HTTPBasicCredentials | None, Depends(security)],
    ) -> str:
        if not cfg.dashboard.require_auth or not env.dashboard_password:
            return env.dashboard_username or "anonymous"
        if credentials is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Authentication required",
                headers={"WWW-Authenticate": "Basic"},
            )
        user_ok = secrets.compare_digest(
            credentials.username.encode(), env.dashboard_username.encode()
        )
        pass_ok = secrets.compare_digest(
            credentials.password.encode(), env.dashboard_password.encode()
        )
        if not (user_ok and pass_ok):
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Invalid credentials",
                headers={"WWW-Authenticate": "Basic"},
            )
        return credentials.username

    @app.get("/", response_class=HTMLResponse)
    def home(_user: Annotated[str, Depends(require_user)]) -> str:
        return DASHBOARD_HTML

    @app.get("/api/status")
    def status_api(_user: Annotated[str, Depends(require_user)]) -> dict:
        marks = {}
        for sym in cfg.universe:
            try:
                marks[sym] = agent.market.get_quote(sym).price
            except Exception:  # noqa: BLE001
                pass
        account = agent.broker.get_account(marks)
        snap = wallet.snapshot()
        summary = journal.summary()
        halt = safety.state()
        return {
            "mode": cfg.agent.mode,
            "backend": cfg.broker.backend,
            "agent": cfg.agent.name,
            "halted": halt.halted,
            "halt_reason": halt.reason,
            "account": account.model_dump(),
            "wallet": snap.model_dump(mode="json"),
            "journal": summary.model_dump(mode="json"),
            "journal_rows": [r.model_dump(mode="json") for r in journal.history(12)],
            "transfers": [t.model_dump(mode="json") for t in wallet.history(15)],
            "orders": db.recent_orders(15),
        }

    @app.post("/api/cycle")
    def cycle(_user: Annotated[str, Depends(require_user)]) -> dict:
        return agent.run_cycle(dry_run=False).model_dump(mode="json")

    @app.post("/api/halt")
    def halt_api(
        body: HaltBody, _user: Annotated[str, Depends(require_user)]
    ) -> dict:
        return safety.halt(body.reason).model_dump(mode="json")

    @app.post("/api/resume")
    def resume_api(_user: Annotated[str, Depends(require_user)]) -> dict:
        return safety.resume().model_dump(mode="json")

    @app.get("/api/journal")
    def journal_api(_user: Annotated[str, Depends(require_user)]) -> dict:
        return {
            "summary": journal.summary().model_dump(mode="json"),
            "rows": [r.model_dump(mode="json") for r in journal.history(100)],
        }

    @app.get("/api/wallet")
    def wallet_status(_user: Annotated[str, Depends(require_user)]) -> dict:
        return wallet.snapshot().model_dump(mode="json")

    @app.post("/api/wallet/send")
    def wallet_send(
        body: SendBody, _user: Annotated[str, Depends(require_user)]
    ) -> dict:
        try:
            return wallet.send(
                body.asset, body.amount, body.to_address, memo=body.memo
            ).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/wallet/receive")
    def wallet_receive(
        body: ReceiveBody, _user: Annotated[str, Depends(require_user)]
    ) -> dict:
        try:
            return wallet.receive(
                body.asset,
                body.amount,
                from_address=body.from_address,
                memo=body.memo,
            ).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.get("/api/health")
    def health() -> dict:
        return {"ok": True, "agent": cfg.agent.name, "halted": safety.is_halted()}

    return app
