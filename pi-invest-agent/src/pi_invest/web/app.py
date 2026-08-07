from __future__ import annotations

import secrets
from typing import Literal

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
from pi_invest.wallet.confirm import bridge_phrase, confirmation_phrase

security = HTTPBasic(auto_error=False)

DASHBOARD_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Pi Invest</title>
  <script src="https://cdn.jsdelivr.net/npm/qrcode-generator@1.4.4/qrcode.min.js"></script>
  <style>
    :root {
      --bg: #0f1410; --panel: #182018; --ink: #e7efe6; --muted: #8fa08c;
      --accent: #7cb87c; --danger: #c45c5c; --line: #2a3a2c;
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
    main { max-width: 980px; margin: 0 auto; padding: 2rem 1.1rem 4rem; }
    h1 {
      font-family: "IBM Plex Serif", Georgia, serif; font-weight: 600;
      font-size: clamp(1.8rem, 4vw, 2.5rem); margin: 0 0 0.3rem; letter-spacing: -0.02em;
    }
    .sub { color: var(--muted); margin-bottom: 1.2rem; }
    .grid { display: grid; gap: 0.85rem; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); }
    .stat {
      padding: 0.9rem 1rem; border: 1px solid var(--line);
      background: color-mix(in srgb, var(--panel) 90%, transparent);
    }
    .stat .label { color: var(--muted); font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.06em; }
    .stat .value { font-size: 1.25rem; margin-top: 0.3rem; font-variant-numeric: tabular-nums; }
    h2 { margin: 1.75rem 0 0.7rem; font-size: 1.02rem; color: var(--accent); font-weight: 600; }
    table { width: 100%; border-collapse: collapse; font-size: 0.92rem; }
    th, td { text-align: left; padding: 0.5rem 0.35rem; border-bottom: 1px solid #243028; }
    th { color: var(--muted); font-weight: 500; font-size: 0.74rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .muted { color: var(--muted); }
    .pill {
      display: inline-block; border: 1px solid #3a523c; padding: 0.15rem 0.55rem;
      font-size: 0.75rem; color: var(--accent);
    }
    .pill.danger { border-color: #6a3030; color: #e08080; }
    .forms { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); }
    .form, .panel {
      border: 1px solid var(--line); padding: 1rem;
      background: color-mix(in srgb, var(--panel) 88%, transparent);
    }
    .form h3, .panel h3 { margin: 0 0 0.7rem; font-size: 0.95rem; }
    label { display: block; font-size: 0.76rem; color: var(--muted); margin: 0.45rem 0 0.2rem; }
    input, select {
      width: 100%; background: #101610; border: 1px solid #2f4032;
      color: var(--ink); padding: 0.55rem 0.6rem; font-size: 1rem;
    }
    button {
      margin-top: 0.75rem; background: var(--accent); color: #0c120e; border: 0;
      padding: 0.75rem 1rem; font-weight: 600; cursor: pointer; margin-right: 0.45rem;
      min-height: 44px;
    }
    button.danger { background: var(--danger); color: #fff; }
    button.secondary { background: transparent; color: var(--accent); border: 1px solid #3a523c; }
    #err { color: #e08080; margin-top: 1rem; white-space: pre-wrap; }
    #ok { color: var(--accent); margin-top: 0.75rem; }
    code { font-size: 0.8rem; word-break: break-all; }
    .chart-wrap { border: 1px solid var(--line); padding: 0.75rem; background: #121812; }
    canvas { width: 100%; height: 180px; display: block; }
    .qr-grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); }
    .qr-card { text-align: center; border: 1px solid var(--line); padding: 0.75rem; background: #101610; }
    .qr-card img, .qr-card canvas { margin: 0.4rem auto; background: #fff; padding: 0.4rem; }
    .hint { font-size: 0.82rem; color: var(--muted); margin-top: 0.4rem; }
    .row-actions button { margin-top: 0.25rem; padding: 0.4rem 0.65rem; min-height: 36px; font-size: 0.85rem; }
  </style>
</head>
<body>
  <main>
    <h1>Pi Invest</h1>
    <p class="sub">Income agent + secured wallet · <span id="mode" class="pill">…</span> <span id="haltpill" class="pill">…</span> <span id="rolepill" class="pill">…</span></p>

    <div class="grid">
      <div class="stat"><div class="label">Equity</div><div class="value" id="equity">—</div></div>
      <div class="stat"><div class="label">Total NAV</div><div class="value" id="nav">—</div></div>
      <div class="stat"><div class="label">Max drawdown</div><div class="value" id="dd">—</div></div>
      <div class="stat"><div class="label">Wallet ~USD</div><div class="value" id="walletusd">—</div></div>
    </div>

    <h2>NAV chart</h2>
    <div class="chart-wrap"><canvas id="nav-chart" width="900" height="180"></canvas></div>

    <div id="admin-controls">
    <h2>Safety</h2>
    <button id="halt-btn" class="danger">Halt trading + sends</button>
    <button id="resume-btn" class="secondary">Resume</button>
    <button id="cycle" class="secondary">Run invest cycle</button>
    </div>
    <p class="muted" id="halt-reason"></p>
    <p class="muted" id="viewer-note" style="display:none">Read-only session — halt, send, and allowlist edits require the admin user.</p>

    <h2>Receive (QR)</h2>
    <div class="qr-grid" id="qr-grid"></div>

    <div id="admin-send">
    <h2>Send wizard</h2>
    <div class="forms">
      <div class="form">
        <h3>1. Destination must be allowlisted</h3>
        <label>Asset</label><select id="send-asset"></select>
        <label>Amount</label><input id="send-amount" type="number" step="any" />
        <label>To (allowlisted only)</label>
        <select id="send-to"></select>
        <label>Memo</label><input id="send-memo" />
        <p class="hint" id="confirm-hint">Confirmation phrase will appear here</p>
        <label>Type confirmation exactly</label>
        <input id="send-confirm" placeholder="SEND 25.00 USD" autocomplete="off" />
        <button id="send-btn">Confirm &amp; send</button>
      </div>
      <div class="form">
        <h3>Allowlist</h3>
        <label>New destination</label>
        <input id="allow-dest" placeholder="email, BTC address, usd:…" />
        <label>Label</label>
        <input id="allow-label" placeholder="optional" />
        <button id="allow-add">Add to allowlist</button>
        <table style="margin-top:0.75rem">
          <thead><tr><th>Destination</th><th></th></tr></thead>
          <tbody id="allow-rows"></tbody>
        </table>
      </div>
    </div>
    </div>

    <div id="viewer-allowlist" style="display:none">
    <h2>Allowlist (read-only)</h2>
    <table>
      <thead><tr><th>Destination</th><th>Label</th></tr></thead>
      <tbody id="allow-rows-ro"></tbody>
    </table>
    </div>

    <h2>Wallet balances</h2>
    <table>
      <thead><tr><th>Asset</th><th>Amount</th><th>~USD</th><th>Receive</th></tr></thead>
      <tbody id="wallet"></tbody>
    </table>

    <h2>Holdings</h2>
    <table>
      <thead><tr><th>Symbol</th><th>Qty</th><th>Value</th><th>uPnL</th></tr></thead>
      <tbody id="positions"></tbody>
    </table>

    <h2>Audit</h2>
    <table>
      <thead><tr><th>When</th><th>Kind</th><th>Detail</th></tr></thead>
      <tbody id="audit"></tbody>
    </table>

    <div id="ok"></div>
    <div id="err"></div>
  </main>
  <script>
    const fmt = (n) => Number(n).toLocaleString(undefined, {style:'currency', currency:'USD'});
    const amt = (n, asset) => (asset === 'USD' || asset === 'USDC')
      ? Number(n).toFixed(2) : Number(n).toPrecision(6);

    function phrase(asset, amount) {
      let a;
      if (asset === 'USD' || asset === 'USDC' || asset === 'USDT') a = Number(amount).toFixed(2);
      else a = Number(amount).toFixed(8).replace(/\\.?0+$/, '');
      return `SEND ${a} ${asset}`;
    }

    function drawNav(rows) {
      const canvas = document.getElementById('nav-chart');
      const ctx = canvas.getContext('2d');
      const w = canvas.width, h = canvas.height;
      ctx.clearRect(0, 0, w, h);
      const pts = (rows || []).slice().reverse();
      if (pts.length < 2) {
        ctx.fillStyle = '#8fa08c';
        ctx.fillText('Run a few cycles to build the NAV curve', 16, h/2);
        return;
      }
      const vals = pts.map(p => p.total_nav);
      const min = Math.min(...vals), max = Math.max(...vals);
      const span = Math.max(max - min, 1);
      ctx.strokeStyle = '#2a3a2c';
      ctx.beginPath(); ctx.moveTo(0, h-0.5); ctx.lineTo(w, h-0.5); ctx.stroke();
      ctx.strokeStyle = '#7cb87c';
      ctx.lineWidth = 2;
      ctx.beginPath();
      pts.forEach((p, i) => {
        const x = (i / (pts.length - 1)) * (w - 8) + 4;
        const y = h - 8 - ((p.total_nav - min) / span) * (h - 20);
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      });
      ctx.stroke();
      // drawdown fill hint using peak line
      ctx.fillStyle = '#7cb87c';
      ctx.font = '12px sans-serif';
      ctx.fillText(`NAV ${fmt(vals[vals.length-1])}`, 10, 16);
    }

    function qrDataUrl(text) {
      if (!window.qrcode) return '';
      const qr = qrcode(0, 'M');
      qr.addData(text);
      qr.make();
      return qr.createDataURL(6, 2);
    }

    function fillAssets(assets) {
      const el = document.getElementById('send-asset');
      const cur = el.value;
      el.innerHTML = assets.map(a => `<option value="${a}">${a}</option>`).join('');
      if (assets.includes(cur)) el.value = cur;
    }

    function fillDestinations(list) {
      const el = document.getElementById('send-to');
      const cur = el.value;
      el.innerHTML = (list || []).map(r =>
        `<option value="${r.destination}">${r.label ? r.label + ' — ' : ''}${r.destination}</option>`
      ).join('') || '<option value="">(add allowlist entries first)</option>';
      if ([...el.options].some(o => o.value === cur)) el.value = cur;
    }

    function updateConfirmHint() {
      const asset = document.getElementById('send-asset').value;
      const amount = document.getElementById('send-amount').value;
      const hint = document.getElementById('confirm-hint');
      if (!asset || !amount) { hint.textContent = 'Enter asset + amount to see the required phrase'; return; }
      hint.textContent = `Type exactly: ${phrase(asset, amount)}`;
    }

    async function refresh() {
      const r = await fetch('/api/status');
      const d = await r.json();
      const role = d.role || 'admin';
      const isAdmin = role === 'admin';
      document.getElementById('admin-controls').style.display = isAdmin ? '' : 'none';
      document.getElementById('admin-send').style.display = isAdmin ? '' : 'none';
      document.getElementById('viewer-note').style.display = isAdmin ? 'none' : '';
      document.getElementById('viewer-allowlist').style.display = isAdmin ? 'none' : '';
      document.getElementById('rolepill').textContent = role.toUpperCase();
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
      if (isAdmin) {
        fillAssets(d.wallet.balances.map(b => b.asset));
        fillDestinations(d.allowlist || []);
      }
      drawNav(d.journal_rows || []);

      document.getElementById('wallet').innerHTML = d.wallet.balances.map(b => {
        const addr = (d.wallet.addresses[b.asset] || {}).address || '—';
        return `<tr><td>${b.asset}</td><td>${amt(b.amount, b.asset)}</td>
          <td>${fmt(b.amount * (b.usd_mark || 0))}</td><td><code>${addr}</code></td></tr>`;
      }).join('') || '<tr><td colspan=4 class="muted">Empty</td></tr>';

      document.getElementById('positions').innerHTML = d.account.positions.map(p =>
        `<tr><td>${p.symbol}</td><td>${p.qty.toFixed(4)}</td>
         <td>${fmt(p.market_value)}</td><td>${fmt(p.unrealized_pnl)}</td></tr>`
      ).join('') || '<tr><td colspan=4 class="muted">No positions</td></tr>';

      if (isAdmin) {
        document.getElementById('allow-rows').innerHTML = (d.allowlist || []).map(r =>
          `<tr><td><code>${r.destination}</code>${r.label ? ' · ' + r.label : ''}</td>
           <td class="row-actions"><button data-rm="${r.destination}" class="secondary allow-rm">Remove</button></td></tr>`
        ).join('') || '<tr><td colspan=2 class="muted">Empty — sends blocked until you add destinations</td></tr>';
        document.querySelectorAll('.allow-rm').forEach(btn => {
          btn.onclick = async () => {
            await post('/api/wallet/allowlist/remove', {destination: btn.dataset.rm});
            await refresh();
          };
        });
      } else {
        document.getElementById('allow-rows-ro').innerHTML = (d.allowlist || []).map(r =>
          `<tr><td><code>${r.destination}</code></td><td>${r.label || '—'}</td></tr>`
        ).join('') || '<tr><td colspan=2 class="muted">Empty</td></tr>';
      }

      // QR cards for crypto receive addresses
      const qg = document.getElementById('qr-grid');
      const addrs = d.wallet.addresses || {};
      const cards = Object.keys(addrs).filter(a => a !== 'USD' && addrs[a].address);
      qg.innerHTML = cards.map(asset => {
        const address = addrs[asset].address;
        const url = qrDataUrl(address);
        return `<div class="qr-card"><strong>${asset}</strong><div>
          ${url ? `<img alt="QR ${asset}" src="${url}" width="132" height="132" />` : ''}
          </div><code>${address}</code></div>`;
      }).join('') || '<p class="muted">No crypto receive addresses yet — open receive via CLI or Coinbase.</p>';

      document.getElementById('audit').innerHTML = (d.audit || []).map(a =>
        `<tr><td class="muted">${a.created_at}</td><td>${a.kind}</td><td>${a.detail}</td></tr>`
      ).join('') || '<tr><td colspan=3 class="muted">No audit events</td></tr>';

      if (isAdmin) updateConfirmHint();
    }

    async function post(url, body) {
      document.getElementById('err').textContent = '';
      document.getElementById('ok').textContent = '';
      const r = await fetch(url, {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: body ? JSON.stringify(body) : '{}'
      });
      if (!r.ok) throw new Error(await r.text());
      return r.json();
    }

    document.getElementById('send-asset').onchange = updateConfirmHint;
    document.getElementById('send-amount').oninput = updateConfirmHint;

    document.getElementById('halt-btn').onclick = async () => {
      try { await post('/api/halt', {reason: 'dashboard halt'}); await refresh(); }
      catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('resume-btn').onclick = async () => {
      try { await post('/api/resume'); await refresh(); }
      catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('cycle').onclick = async () => {
      try { await post('/api/cycle'); document.getElementById('ok').textContent = 'Cycle complete'; await refresh(); }
      catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('send-btn').onclick = async () => {
      try {
        const asset = document.getElementById('send-asset').value;
        const amount = Number(document.getElementById('send-amount').value);
        await post('/api/wallet/send', {
          asset, amount,
          to_address: document.getElementById('send-to').value,
          memo: document.getElementById('send-memo').value,
          confirm: document.getElementById('send-confirm').value
        });
        document.getElementById('ok').textContent = 'Send submitted';
        document.getElementById('send-confirm').value = '';
        await refresh();
      } catch (e) { document.getElementById('err').textContent = String(e); }
    };
    document.getElementById('allow-add').onclick = async () => {
      try {
        await post('/api/wallet/allowlist/add', {
          destination: document.getElementById('allow-dest').value,
          label: document.getElementById('allow-label').value
        });
        document.getElementById('allow-dest').value = '';
        document.getElementById('allow-label').value = '';
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
    confirm: str = ""


class ReceiveBody(BaseModel):
    asset: str
    amount: float = Field(gt=0)
    from_address: str = "external"
    memo: str = ""


class HaltBody(BaseModel):
    reason: str = "dashboard halt"


class AllowAddBody(BaseModel):
    destination: str
    label: str = ""


class AllowRemoveBody(BaseModel):
    destination: str


class DashboardUser(BaseModel):
    username: str
    role: Literal["admin", "viewer"] = "admin"


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

    def _match(username: str, password: str, expect_user: str, expect_pass: str) -> bool:
        if not expect_pass:
            return False
        user_ok = secrets.compare_digest(
            username.encode(), expect_user.encode()
        )
        pass_ok = secrets.compare_digest(
            password.encode(), expect_pass.encode()
        )
        return user_ok and pass_ok

    def require_user(
        credentials: HTTPBasicCredentials | None = Depends(security),
    ) -> DashboardUser:
        """Return admin or viewer identity."""
        if not cfg.dashboard.require_auth or not env.dashboard_password:
            return DashboardUser(
                username=env.dashboard_username or "anonymous",
                role="admin",
            )
        if credentials is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="Authentication required",
                headers={"WWW-Authenticate": "Basic"},
            )
        if _match(
            credentials.username,
            credentials.password,
            env.dashboard_username,
            env.dashboard_password,
        ):
            return DashboardUser(username=credentials.username, role="admin")
        if _match(
            credentials.username,
            credentials.password,
            env.dashboard_readonly_username,
            env.dashboard_readonly_password,
        ):
            return DashboardUser(username=credentials.username, role="viewer")
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid credentials",
            headers={"WWW-Authenticate": "Basic"},
        )

    def require_admin(
        user: DashboardUser = Depends(require_user),
    ) -> DashboardUser:
        if user.role != "admin":
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="Admin credentials required for this action",
            )
        return user

    @app.get("/", response_class=HTMLResponse)
    def home(_user: DashboardUser = Depends(require_user)) -> str:
        return DASHBOARD_HTML

    @app.get("/api/status")
    def status_api(user: DashboardUser = Depends(require_user)) -> dict:
        marks = {}
        for sym in cfg.universe:
            try:
                marks[sym] = agent.market.get_quote(sym).price
            except Exception:  # noqa: BLE001
                pass
        account = agent.broker.get_account(marks)
        snap = wallet.snapshot()
        # Ensure receive addresses present for QR (paper or coinbase)
        addresses = dict(snap.addresses)
        for asset in cfg.wallet.assets:
            if asset.upper() == "USD":
                continue
            if asset.upper() in addresses and addresses[asset.upper()].address:
                continue
            try:
                addresses[asset.upper()] = wallet.receive_info(asset)
            except Exception:  # noqa: BLE001
                pass
        snap.addresses = addresses
        summary = journal.summary()
        halt = safety.state()
        return {
            "mode": cfg.agent.mode,
            "backend": cfg.broker.backend,
            "agent": cfg.agent.name,
            "role": user.role,
            "username": user.username,
            "halted": halt.halted,
            "halt_reason": halt.reason,
            "account": account.model_dump(),
            "wallet": snap.model_dump(mode="json"),
            "journal": summary.model_dump(mode="json"),
            "journal_rows": [r.model_dump(mode="json") for r in journal.history(60)],
            "allowlist": wallet.allowlist(),
            "audit": db.recent_audit(20),
            "confirm_example": confirmation_phrase("USD", 25.0),
            "require_send_confirmation": cfg.wallet.require_send_confirmation,
            "allowlist_required": cfg.wallet.allowlist_required,
            "transfers": [t.model_dump(mode="json") for t in wallet.history(15)],
            "orders": db.recent_orders(15),
        }

    @app.post("/api/cycle")
    def cycle(_user: DashboardUser = Depends(require_admin)) -> dict:
        decision = agent.run_cycle(dry_run=False)
        db.audit("invest.cycle", decision.cycle_id)
        return decision.model_dump(mode="json")

    @app.post("/api/halt")
    def halt_api(
        body: HaltBody, _user: DashboardUser = Depends(require_admin)
    ) -> dict:
        st = safety.halt(body.reason)
        db.audit("safety.halt", body.reason)
        return st.model_dump(mode="json")

    @app.post("/api/resume")
    def resume_api(_user: DashboardUser = Depends(require_admin)) -> dict:
        st = safety.resume()
        db.audit("safety.resume", "ok")
        return st.model_dump(mode="json")

    @app.get("/api/journal")
    def journal_api(_user: DashboardUser = Depends(require_user)) -> dict:
        return {
            "summary": journal.summary().model_dump(mode="json"),
            "rows": [r.model_dump(mode="json") for r in journal.history(100)],
        }

    @app.post("/api/wallet/send")
    def wallet_send(
        body: SendBody, _user: DashboardUser = Depends(require_admin)
    ) -> dict:
        try:
            return wallet.send(
                body.asset,
                body.amount,
                body.to_address,
                memo=body.memo,
                confirm=body.confirm,
            ).model_dump(mode="json")
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

    @app.post("/api/wallet/receive")
    def wallet_receive(
        body: ReceiveBody, _user: DashboardUser = Depends(require_admin)
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

    @app.get("/api/wallet/confirm-phrase")
    def confirm_phrase_api(
        asset: str,
        amount: float,
        _user: DashboardUser = Depends(require_user),
    ) -> dict:
        return {
            "phrase": confirmation_phrase(asset, amount),
            "bridge_phrase": bridge_phrase(amount) if asset.upper() == "USD" else None,
        }

    @app.post("/api/wallet/allowlist/add")
    def allow_add(
        body: AllowAddBody, _user: DashboardUser = Depends(require_admin)
    ) -> dict:
        try:
            wallet.allowlist_add(body.destination, label=body.label)
        except Exception as exc:  # noqa: BLE001
            raise HTTPException(status_code=400, detail=str(exc)) from exc
        return {"ok": True, "allowlist": wallet.allowlist()}

    @app.post("/api/wallet/allowlist/remove")
    def allow_remove(
        body: AllowRemoveBody, _user: DashboardUser = Depends(require_admin)
    ) -> dict:
        try:
            wallet.allowlist_remove(body.destination)
        except WalletError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc
        return {"ok": True, "allowlist": wallet.allowlist()}

    @app.get("/api/health")
    def health() -> dict:
        return {"ok": True, "agent": cfg.agent.name, "halted": safety.is_halted()}

    return app
