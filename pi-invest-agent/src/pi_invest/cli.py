from __future__ import annotations

import time
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from pi_invest.factory import build_agent
from pi_invest.wallet import WalletError

app = typer.Typer(
    name="pi-invest",
    help="Raspberry Pi 5 autonomous income investment agent",
    add_completion=False,
)
wallet_app = typer.Typer(help="Send/receive USD and cryptocurrency")
app.add_typer(wallet_app, name="wallet")
coinbase_app = typer.Typer(help="Coinbase App connection (CDP API keys)")
app.add_typer(coinbase_app, name="coinbase")
console = Console()


def _boot(config: Optional[str] = None, simulator: bool = False):
    return build_agent(config_path=config, force_simulator=simulator)


@app.command()
def once(
    dry_run: bool = typer.Option(False, help="Score and plan without placing orders"),
    simulator: bool = typer.Option(False, help="Force offline simulator quotes"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Run a single research + trade cycle."""
    agent, cfg, _env, _db, _wallet, safety, _journal = _boot(config, simulator)
    st = safety.state()
    halt_note = f"  [red]HALTED[/red] ({st.reason})" if st.halted else ""
    console.print(
        f"[bold]{cfg.agent.name}[/bold] mode={cfg.agent.mode} "
        f"backend={cfg.broker.backend}{halt_note}"
    )
    decision = agent.run_cycle(dry_run=dry_run)
    _print_decision(decision)


@app.command()
def run(
    simulator: bool = typer.Option(False, help="Force offline simulator quotes"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Run continuously on the configured interval."""
    agent, cfg, _env, _db, _wallet, safety, _journal = _boot(config, simulator)
    interval = max(1, cfg.schedule.interval_minutes) * 60
    console.print(
        f"Starting loop every {cfg.schedule.interval_minutes}m "
        f"(mode={cfg.agent.mode}, ctrl+c to stop)"
    )
    if safety.is_halted():
        console.print(
            "[yellow]Agent is HALTED — cycles will plan but not trade/send. "
            "Use `pi-invest resume` to unlock.[/yellow]"
        )
    while True:
        try:
            decision = agent.run_cycle(dry_run=False)
            _print_decision(decision)
        except Exception as exc:  # noqa: BLE001
            console.print(f"[red]cycle error:[/red] {exc}")
        time.sleep(interval)


@app.command()
def status(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show portfolio, wallet, halt state, and recent decisions."""
    agent, cfg, _env, db, wallet, safety, journal = _boot(config, True)
    marks = {}
    for sym in cfg.universe:
        try:
            marks[sym] = agent.market.get_quote(sym).price
        except Exception:  # noqa: BLE001
            pass
    acct = agent.broker.get_account(marks)
    st = safety.state()

    if st.halted:
        console.print(f"[bold red]HALTED[/bold red] — {st.reason or 'kill switch'}")
    else:
        console.print("[green]Running[/green] (orders + sends allowed)")

    console.print(
        f"[bold]Equity[/bold] ${acct.equity:,.2f}  "
        f"[bold]Cash[/bold] ${acct.cash:,.2f}  "
        f"[bold]Day PnL[/bold] ${acct.day_pnl:,.2f} ({acct.day_pnl_pct:.2%})"
    )
    summary = journal.summary()
    if summary.latest_nav is not None:
        ret = (
            f"{summary.total_return_pct:.2%}"
            if summary.total_return_pct is not None
            else "—"
        )
        console.print(
            f"[bold]NAV[/bold] ${summary.latest_nav:,.2f}  "
            f"peak ${summary.peak_nav:,.2f}  "
            f"max DD {summary.max_drawdown_pct:.2%}  "
            f"return {ret}  "
            f"({summary.points} journal pts)"
        )

    table = Table(title="Positions")
    table.add_column("Symbol")
    table.add_column("Qty", justify="right")
    table.add_column("Avg", justify="right")
    table.add_column("Mark", justify="right")
    table.add_column("Value", justify="right")
    table.add_column("uPnL", justify="right")
    for p in acct.positions:
        table.add_row(
            p.symbol,
            f"{p.qty:.4f}",
            f"{p.avg_cost:.2f}",
            f"{p.market_price:.2f}",
            f"{p.market_value:.2f}",
            f"{p.unrealized_pnl:.2f}",
        )
    console.print(table)

    _print_wallet(wallet.snapshot())

    decisions = db.recent_decisions(5)
    console.print(f"\n[bold]Recent decisions[/bold]: {len(decisions)}")
    for d in decisions:
        fills = [o for o in d.get("orders", []) if o.get("ok")]
        console.print(
            f"  {d.get('timestamp')}  scores={len(d.get('scores', []))}  "
            f"fills={len(fills)}  source={d.get('meta', {}).get('data_source')}"
        )


@app.command()
def halt(
    reason: str = typer.Option("manual halt", "--reason", "-r"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Freeze brokerage orders and outbound wallet sends."""
    _agent, _cfg, _env, _db, _wallet, safety, _journal = _boot(config, True)
    st = safety.halt(reason)
    console.print(f"[red]HALTED[/red] — {st.reason}")
    console.print("Inbound receives still work. Use `pi-invest resume` to unlock.")


@app.command()
def resume(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Clear the kill switch so trading and sends can resume."""
    _agent, _cfg, _env, _db, _wallet, safety, _journal = _boot(config, True)
    safety.resume()
    console.print("[green]Resumed[/green] — orders and sends allowed again.")


@app.command()
def journal(
    limit: int = typer.Option(20, help="Rows to show"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show performance journal (NAV, peak, drawdown)."""
    _agent, _cfg, _env, _db, _wallet, _safety, journal = _boot(config, True)
    summary = journal.summary()
    if summary.latest_nav is None:
        console.print("No journal points yet — run `pi-invest once` first.")
        return
    ret = (
        f"{summary.total_return_pct:.2%}"
        if summary.total_return_pct is not None
        else "—"
    )
    console.print(
        f"NAV ${summary.latest_nav:,.2f} · start ${summary.start_nav:,.2f} · "
        f"peak ${summary.peak_nav:,.2f} · max DD {summary.max_drawdown_pct:.2%} · "
        f"return {ret}"
    )
    table = Table(title="Equity journal")
    table.add_column("When")
    table.add_column("NAV", justify="right")
    table.add_column("Equity", justify="right")
    table.add_column("Wallet", justify="right")
    table.add_column("DD", justify="right")
    table.add_column("Halt")
    for row in journal.history(limit=limit):
        table.add_row(
            row.timestamp.isoformat(),
            f"{row.total_nav:,.2f}",
            f"{row.equity:,.2f}",
            f"{row.wallet_usd:,.2f}",
            f"{row.drawdown_pct:.2%}",
            "yes" if row.halted else "",
        )
    console.print(table)


@app.command("export-journal")
def export_journal(
    path: str = typer.Option("data/journal.csv", "--path", "-p"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Export the performance journal to CSV."""
    _agent, _cfg, _env, _db, _wallet, _safety, journal = _boot(config, True)
    out = journal.export_csv(path)
    console.print(f"Wrote {out}")


@app.command("reset-paper")
def reset_paper(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
    yes: bool = typer.Option(False, "--yes", help="Skip confirmation"),
) -> None:
    """Wipe the local paper brokerage ledger back to starting cash."""
    if not yes and not typer.confirm("Reset paper brokerage account?"):
        raise typer.Abort()
    agent, cfg, _env, _db, _wallet, _safety, _journal = _boot(config, True)
    agent.broker.reset()
    console.print(f"Paper account reset to ${cfg.broker.starting_cash:,.2f}")


@app.command()
def dashboard(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
    allow_open: bool = typer.Option(
        False,
        "--allow-open",
        help="Allow unauthenticated dashboard (not recommended on LAN)",
    ),
) -> None:
    """Start the local status dashboard (HTTP basic auth by default)."""
    import uvicorn

    from pi_invest.web.app import create_app

    agent, cfg, env, db, wallet, safety, journal = _boot(config)
    if cfg.dashboard.require_auth and not env.dashboard_password and not allow_open:
        console.print(
            "[red]Dashboard auth required.[/red] Set DASHBOARD_PASSWORD in .env "
            "or pass --allow-open for local testing only."
        )
        raise typer.Exit(1)

    api = create_app(agent, cfg, db, wallet, env=env, safety=safety, journal=journal)
    auth_note = (
        "auth=off"
        if allow_open or not env.dashboard_password
        else f"auth=user:{env.dashboard_username}"
    )
    console.print(
        f"Dashboard on http://{cfg.dashboard.host}:{cfg.dashboard.port} ({auth_note})"
    )
    uvicorn.run(api, host=cfg.dashboard.host, port=cfg.dashboard.port, log_level="info")


@wallet_app.command("balances")
def wallet_balances(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show USD + crypto wallet balances and receive addresses."""
    _a, _c, _e, _d, wallet, _s, _j = _boot(config, True)
    _print_wallet(wallet.snapshot())


@wallet_app.command("receive")
def wallet_receive_address(
    asset: str = typer.Argument(..., help="Asset symbol, e.g. USD BTC ETH USDC"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show the address/account id others can send to."""
    _a, _c, _e, _d, wallet, _s, _j = _boot(config, True)
    try:
        info = wallet.receive_info(asset)
    except WalletError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(f"[bold]{info.asset}[/bold] on {info.network}")
    console.print(f"Receive: [green]{info.address}[/green]")
    if info.memo_tag:
        console.print(f"Memo/tag: {info.memo_tag}")
    console.print(
        "[dim]Paper addresses are local simulation tags, not on-chain destinations.[/dim]"
    )


@wallet_app.command("send")
def wallet_send(
    asset: str = typer.Argument(..., help="Asset to send"),
    amount: float = typer.Option(..., "--amount", "-a", help="Amount to send"),
    to: str = typer.Option(..., "--to", "-t", help="Destination address or USD account id"),
    memo: str = typer.Option("", "--memo", "-m", help="Optional memo"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Send USD or cryptocurrency from the wallet."""
    _a, _c, _e, _d, wallet, _s, _j = _boot(config, True)
    try:
        record = wallet.send(asset, amount, to, memo=memo)
    except WalletError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(
        f"[green]sent[/green] {record.amount} {record.asset} → {record.counterparty} "
        f"(fee {record.fee}) ref={record.tx_ref}"
    )


@wallet_app.command("credit")
def wallet_credit(
    asset: str = typer.Argument(..., help="Asset received"),
    amount: float = typer.Option(..., "--amount", "-a", help="Amount received"),
    frm: str = typer.Option("external", "--from", help="Sender label/address"),
    memo: str = typer.Option("", "--memo", "-m", help="Optional memo"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Credit an inbound payment (paper receive / webhook stand-in)."""
    _a, _c, _e, _d, wallet, _s, _j = _boot(config, True)
    try:
        record = wallet.receive(asset, amount, from_address=frm, memo=memo)
    except WalletError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(
        f"[green]received[/green] {record.amount} {record.asset} from {record.counterparty} "
        f"ref={record.tx_ref}"
    )


@wallet_app.command("history")
def wallet_history(
    limit: int = typer.Option(15, help="Rows to show"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show recent wallet transfers."""
    _a, _c, _e, _d, wallet, _s, _j = _boot(config, True)
    rows = wallet.history(limit=limit)
    table = Table(title="Transfers")
    table.add_column("When")
    table.add_column("Dir")
    table.add_column("Asset")
    table.add_column("Amount", justify="right")
    table.add_column("Counterparty")
    table.add_column("Ref")
    for r in rows:
        table.add_row(
            r.timestamp.isoformat(),
            r.direction.value,
            r.asset,
            f"{r.amount}",
            r.counterparty,
            r.tx_ref,
        )
    console.print(table)


@wallet_app.command("bridge-to-broker")
def wallet_bridge_to_broker(
    amount: float = typer.Option(..., "--amount", "-a", help="USD amount"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Move USD from wallet treasury into paper brokerage cash."""
    _a, cfg, _e, db, wallet, _s, _j = _boot(config, True)
    from pi_invest.broker import PaperBroker

    if cfg.broker.backend == "paper":
        PaperBroker(db, cfg.broker.starting_cash)
    try:
        record = wallet.bridge_to_broker(amount)
    except WalletError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(f"[green]bridged[/green] ${record.amount:.2f} wallet → brokerage")


@wallet_app.command("bridge-from-broker")
def wallet_bridge_from_broker(
    amount: float = typer.Option(..., "--amount", "-a", help="USD amount"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Move USD from paper brokerage cash into wallet treasury."""
    _a, cfg, _e, db, wallet, _s, _j = _boot(config, True)
    from pi_invest.broker import PaperBroker

    if cfg.broker.backend == "paper":
        PaperBroker(db, cfg.broker.starting_cash)
    try:
        record = wallet.bridge_from_broker(amount)
    except WalletError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(f"[green]bridged[/green] ${record.amount:.2f} brokerage → wallet")


@coinbase_app.command("status")
def coinbase_status(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Test Coinbase CDP credentials and show live balances."""
    from pi_invest.wallet.coinbase_client import CoinbaseAPIError, CoinbaseClient

    _a, cfg, env, _d, wallet, _s, _j = _boot(config, True)
    if not env.coinbase_api_key or not env.coinbase_api_secret:
        console.print(
            "[red]Missing credentials.[/red] Set COINBASE_API_KEY and "
            "COINBASE_API_SECRET in .env (CDP key name + ECDSA PEM)."
        )
        raise typer.Exit(1)
    try:
        client = CoinbaseClient(env)
        ping = client.ping()
        accounts = client.list_app_accounts()
    except CoinbaseAPIError as exc:
        console.print(f"[red]Coinbase connection failed:[/red] {exc}")
        raise typer.Exit(1) from exc

    console.print(
        f"[green]Connected[/green] to Coinbase "
        f"(advanced_trade_sample_accounts={ping.get('advanced_trade_accounts')})"
    )
    console.print(
        f"wallet.backend={cfg.wallet.backend}  "
        f"ALLOW_LIVE_TRANSFERS={env.allow_live_transfers}"
    )
    table = Table(title="Coinbase App accounts")
    table.add_column("Currency")
    table.add_column("Name")
    table.add_column("Balance", justify="right")
    table.add_column("Account id")
    for a in accounts:
        if a.balance == 0 and a.currency not in {x.upper() for x in cfg.wallet.assets}:
            continue
        table.add_row(a.currency, a.name, f"{a.balance}", a.id[:8] + "…")
    console.print(table)
    if cfg.wallet.backend != "coinbase":
        console.print(
            "[dim]Tip: set wallet.backend: coinbase in config.yaml to use these "
            "balances in the agent wallet.[/dim]"
        )
    elif isinstance(wallet.backend, object):
        from pi_invest.wallet import CoinbaseWallet

        if isinstance(wallet.backend, CoinbaseWallet):
            console.print("[dim]Agent wallet is live-backed by Coinbase.[/dim]")


@coinbase_app.command("address")
def coinbase_address(
    asset: str = typer.Argument(..., help="Asset, e.g. BTC ETH USDC"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Fetch or create a Coinbase receive address for an asset."""
    from pi_invest.wallet.coinbase_client import CoinbaseAPIError, CoinbaseClient

    _a, _c, env, _d, _w, _s, _j = _boot(config, True)
    try:
        client = CoinbaseClient(env)
        addr = client.get_or_create_receive_address(asset.upper())
    except CoinbaseAPIError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(f"[bold]{asset.upper()}[/bold] network={addr.network}")
    console.print(f"Receive: [green]{addr.address}[/green]")


def _print_wallet(snap) -> None:
    console.print(
        f"\n[bold]Wallet[/bold] backend={snap.backend}  "
        f"est. ${snap.total_usd_estimate:,.2f}"
    )
    table = Table(title="Balances")
    table.add_column("Asset")
    table.add_column("Amount", justify="right")
    table.add_column("~USD", justify="right")
    table.add_column("Receive address")
    for b in snap.balances:
        addr = snap.addresses.get(b.asset)
        table.add_row(
            b.asset,
            f"{b.amount:.8f}".rstrip("0").rstrip(".")
            if b.asset != "USD"
            else f"{b.amount:.2f}",
            f"${b.usd_value:,.2f}",
            addr.address if addr else "—",
        )
    console.print(table)


def _print_decision(decision) -> None:
    console.print(f"\n[bold]Cycle[/bold] {decision.cycle_id[:8]}…")
    halted = decision.meta.get("halted")
    console.print(
        f"market_open={decision.market_open}  "
        f"data={decision.meta.get('data_source')}  "
        f"mode={decision.meta.get('mode')}  "
        f"halted={halted}"
    )
    score_table = Table(title="Income scores")
    score_table.add_column("Symbol")
    score_table.add_column("Income", justify="right")
    score_table.add_column("Composite", justify="right")
    score_table.add_column("Notes")
    for s in decision.scores[:8]:
        score_table.add_row(
            s.symbol,
            f"{s.expected_income_proxy:.2f}",
            f"{s.composite:.2f}",
            ", ".join(s.notes) or "—",
        )
    console.print(score_table)

    if decision.orders:
        for o in decision.orders:
            color = "green" if o.ok else "yellow"
            console.print(
                f"[{color}]order[/{color}] {o.side.value} {o.symbol} "
                f"qty={o.qty:.4f} @ {o.fill_price:.2f} — {o.message}"
            )
    for reason in decision.skipped_reasons:
        console.print(f"[dim]skip:[/dim] {reason}")

    if decision.account_after:
        a = decision.account_after
        console.print(
            f"[bold]Account[/bold] equity=${a.equity:,.2f} cash=${a.cash:,.2f} "
            f"positions={len(a.positions)}"
        )


if __name__ == "__main__":
    app()
