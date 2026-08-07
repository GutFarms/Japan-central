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
console = Console()


@app.command()
def once(
    dry_run: bool = typer.Option(False, help="Score and plan without placing orders"),
    simulator: bool = typer.Option(False, help="Force offline simulator quotes"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Run a single research + trade cycle."""
    agent, cfg, _env, _db, _wallet = build_agent(
        config_path=config, force_simulator=simulator
    )
    console.print(
        f"[bold]{cfg.agent.name}[/bold] mode={cfg.agent.mode} backend={cfg.broker.backend}"
    )
    decision = agent.run_cycle(dry_run=dry_run)
    _print_decision(decision)


@app.command()
def run(
    simulator: bool = typer.Option(False, help="Force offline simulator quotes"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Run continuously on the configured interval."""
    agent, cfg, _env, _db, _wallet = build_agent(
        config_path=config, force_simulator=simulator
    )
    interval = max(1, cfg.schedule.interval_minutes) * 60
    console.print(
        f"Starting loop every {cfg.schedule.interval_minutes}m "
        f"(mode={cfg.agent.mode}, ctrl+c to stop)"
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
    """Show portfolio, wallet, and recent decisions."""
    agent, cfg, _env, db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
    marks = {}
    for sym in cfg.universe:
        try:
            marks[sym] = agent.market.get_quote(sym).price
        except Exception:  # noqa: BLE001
            pass
    acct = agent.broker.get_account(marks)

    console.print(
        f"[bold]Equity[/bold] ${acct.equity:,.2f}  "
        f"[bold]Cash[/bold] ${acct.cash:,.2f}  "
        f"[bold]Day PnL[/bold] ${acct.day_pnl:,.2f} ({acct.day_pnl_pct:.2%})"
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


@app.command("reset-paper")
def reset_paper(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
    yes: bool = typer.Option(False, "--yes", help="Skip confirmation"),
) -> None:
    """Wipe the local paper brokerage ledger back to starting cash."""
    if not yes and not typer.confirm("Reset paper brokerage account?"):
        raise typer.Abort()
    agent, cfg, _env, _db, _wallet = build_agent(
        config_path=config, force_simulator=True
    )
    agent.broker.reset()
    console.print(f"Paper account reset to ${cfg.broker.starting_cash:,.2f}")


@app.command()
def dashboard(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Start the local status dashboard."""
    import uvicorn

    from pi_invest.web.app import create_app

    agent, cfg, _env, db, wallet = build_agent(config_path=config)
    api = create_app(agent, cfg, db, wallet)
    console.print(f"Dashboard on http://{cfg.dashboard.host}:{cfg.dashboard.port}")
    uvicorn.run(api, host=cfg.dashboard.host, port=cfg.dashboard.port, log_level="info")


@wallet_app.command("balances")
def wallet_balances(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show USD + crypto wallet balances and receive addresses."""
    _agent, _cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
    _print_wallet(wallet.snapshot())


@wallet_app.command("receive")
def wallet_receive_address(
    asset: str = typer.Argument(..., help="Asset symbol, e.g. USD BTC ETH USDC"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show the address/account id others can send to."""
    _agent, _cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
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
    _agent, _cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
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
    _agent, _cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
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
    _agent, _cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
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
    _agent, cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
    # Ensure brokerage account exists
    from pi_invest.broker import PaperBroker

    if cfg.broker.backend == "paper":
        PaperBroker(_db, cfg.broker.starting_cash)
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
    _agent, cfg, _env, _db, wallet = build_agent(
        config_path=config, force_simulator=True
    )
    from pi_invest.broker import PaperBroker

    if cfg.broker.backend == "paper":
        PaperBroker(_db, cfg.broker.starting_cash)
    try:
        record = wallet.bridge_from_broker(amount)
    except WalletError as exc:
        console.print(f"[red]{exc}[/red]")
        raise typer.Exit(1) from exc
    console.print(f"[green]bridged[/green] ${record.amount:.2f} brokerage → wallet")


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
            f"{b.amount:.8f}".rstrip("0").rstrip(".") if b.asset != "USD" else f"{b.amount:.2f}",
            f"${b.usd_value:,.2f}",
            addr.address if addr else "—",
        )
    console.print(table)


def _print_decision(decision) -> None:
    console.print(f"\n[bold]Cycle[/bold] {decision.cycle_id[:8]}…")
    console.print(
        f"market_open={decision.market_open}  "
        f"data={decision.meta.get('data_source')}  "
        f"mode={decision.meta.get('mode')}"
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
