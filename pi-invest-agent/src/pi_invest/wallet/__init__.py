from __future__ import annotations

import hashlib
import hmac
import secrets
import uuid
from abc import ABC, abstractmethod

from pi_invest.config import AppConfig, EnvSettings, WalletConfig
from pi_invest.models import (
    AssetBalance,
    ReceiveAddress,
    TransferDirection,
    TransferRecord,
    TransferStatus,
    WalletSnapshot,
)
from pi_invest.storage.db import Database


# Approximate USD marks for paper display
DEFAULT_MARKS_USD: dict[str, float] = {
    "USD": 1.0,
    "USDC": 1.0,
    "USDT": 1.0,
    "BTC": 95_000.0,
    "ETH": 3_500.0,
    "SOL": 180.0,
}


class WalletError(Exception):
    pass


class WalletBackend(ABC):
    @abstractmethod
    def snapshot(self) -> WalletSnapshot: ...

    @abstractmethod
    def receive_address(self, asset: str, network: str | None = None) -> ReceiveAddress: ...

    @abstractmethod
    def send(
        self,
        asset: str,
        amount: float,
        to_address: str,
        memo: str = "",
        network: str | None = None,
    ) -> TransferRecord: ...

    @abstractmethod
    def credit_inbound(
        self,
        asset: str,
        amount: float,
        from_address: str = "external",
        memo: str = "",
    ) -> TransferRecord: ...

    @abstractmethod
    def history(self, limit: int = 25) -> list[TransferRecord]: ...


def _device_suffix(db_path: str) -> str:
    return hashlib.sha256(db_path.encode()).hexdigest()[:10]


def _paper_address(asset: str, suffix: str, network: str) -> str:
    asset = asset.upper()
    if asset == "USD":
        return f"usd:piinvest:{suffix}"
    if asset in {"USDC", "USDT"}:
        return f"0xpaper{suffix}{asset.lower()}"
    if asset == "BTC":
        return f"paper-btc-{suffix}"
    if asset == "ETH":
        return f"0xpaper{suffix}"
    if asset == "SOL":
        return f"paper-sol-{suffix}"
    return f"paper-{asset.lower()}-{suffix}"


class PaperWallet(WalletBackend):
    """Local fiat + crypto treasury with simulated send/receive."""

    def __init__(self, db: Database, cfg: WalletConfig) -> None:
        self.db = db
        self.cfg = cfg
        self.suffix = _device_suffix(str(db.path))
        self.db.ensure_wallet(
            starting=cfg.starting_balances,
            assets=cfg.assets,
            address_fn=lambda asset: _paper_address(
                asset,
                self.suffix,
                cfg.networks.get(asset.upper(), "paper"),
            ),
            networks=cfg.networks,
        )

    def snapshot(self) -> WalletSnapshot:
        balances_raw = self.db.wallet_balances()
        addresses = {a.asset: a for a in self.db.wallet_addresses()}
        balances = [
            AssetBalance(
                asset=asset,
                amount=amount,
                available=amount,
                usd_mark=DEFAULT_MARKS_USD.get(asset, 0.0),
            )
            for asset, amount in balances_raw.items()
        ]
        total = sum(b.amount * (b.usd_mark or 0.0) for b in balances)
        return WalletSnapshot(
            backend="paper",
            balances=balances,
            addresses=addresses,
            total_usd_estimate=round(total, 2),
        )

    def receive_address(self, asset: str, network: str | None = None) -> ReceiveAddress:
        asset = asset.upper()
        if asset not in {a.upper() for a in self.cfg.assets}:
            raise WalletError(f"{asset} is not an enabled wallet asset")
        return self.db.get_or_create_address(
            asset,
            _paper_address(
                asset,
                self.suffix,
                network or self.cfg.networks.get(asset, "paper"),
            ),
            network or self.cfg.networks.get(asset, "paper"),
        )

    def send(
        self,
        asset: str,
        amount: float,
        to_address: str,
        memo: str = "",
        network: str | None = None,
    ) -> TransferRecord:
        asset = asset.upper()
        if amount <= 0:
            raise WalletError("amount must be positive")
        if not to_address.strip():
            raise WalletError("destination required")
        if amount < self.cfg.min_send.get(asset, 0.0):
            raise WalletError(
                f"amount below minimum send for {asset} "
                f"({self.cfg.min_send.get(asset, 0.0)})"
            )

        bal = self.db.wallet_balances().get(asset, 0.0)
        fee = self.cfg.send_fee.get(asset, 0.0)
        total = amount + fee
        if total > bal + 1e-12:
            raise WalletError(
                f"insufficient {asset}: have {bal}, need {total} (incl. fee {fee})"
            )

        self.db.adjust_wallet_balance(asset, -total)
        record = TransferRecord(
            transfer_id=str(uuid.uuid4()),
            direction=TransferDirection.SEND,
            asset=asset,
            amount=amount,
            fee=fee,
            counterparty=to_address.strip(),
            network=network or self.cfg.networks.get(asset, "paper"),
            status=TransferStatus.COMPLETED,
            memo=memo,
            paper=True,
            tx_ref=f"paper-{secrets.token_hex(8)}",
        )
        self.db.save_transfer(record)
        return record

    def credit_inbound(
        self,
        asset: str,
        amount: float,
        from_address: str = "external",
        memo: str = "",
    ) -> TransferRecord:
        asset = asset.upper()
        if amount <= 0:
            raise WalletError("amount must be positive")
        if asset not in {a.upper() for a in self.cfg.assets}:
            raise WalletError(f"{asset} is not an enabled wallet asset")
        self.db.adjust_wallet_balance(asset, amount)
        record = TransferRecord(
            transfer_id=str(uuid.uuid4()),
            direction=TransferDirection.RECEIVE,
            asset=asset,
            amount=amount,
            fee=0.0,
            counterparty=from_address,
            network=self.cfg.networks.get(asset, "paper"),
            status=TransferStatus.COMPLETED,
            memo=memo or "inbound credit",
            paper=True,
            tx_ref=f"paper-in-{secrets.token_hex(8)}",
        )
        self.db.save_transfer(record)
        return record

    def history(self, limit: int = 25) -> list[TransferRecord]:
        return self.db.list_transfers(limit=limit)


class CoinbaseWallet(WalletBackend):
    """
    Optional live-capable wallet shell.

    Requires COINBASE_API_KEY / COINBASE_API_SECRET and ALLOW_LIVE_TRANSFERS=true.
    Automatic live withdrawals are disabled by design — extend send() for your
    exchange withdrawal API. Balances/history use the local mirror so the Pi
    stays usable.
    """

    def __init__(self, env: EnvSettings, cfg: WalletConfig, db: Database) -> None:
        if not env.allow_live_transfers:
            raise WalletError(
                "Live wallet backend requires ALLOW_LIVE_TRANSFERS=true"
            )
        if not env.coinbase_api_key or not env.coinbase_api_secret:
            raise WalletError("Coinbase API key/secret required for live wallet")
        self.env = env
        self.cfg = cfg
        self.db = db
        self._paper = PaperWallet(db, cfg)

    def _sign(self, timestamp: str, method: str, path: str, body: str = "") -> str:
        message = f"{timestamp}{method}{path}{body}".encode()
        try:
            import base64

            key = base64.b64decode(self.env.coinbase_api_secret)
        except Exception:  # noqa: BLE001
            key = self.env.coinbase_api_secret.encode()
        return hmac.new(key, message, hashlib.sha256).hexdigest()

    def snapshot(self) -> WalletSnapshot:
        snap = self._paper.snapshot()
        snap.backend = "coinbase"
        snap.meta = {
            "note": "live keys present; balances mirrored locally until withdrawal API wired"
        }
        return snap

    def receive_address(self, asset: str, network: str | None = None) -> ReceiveAddress:
        addr = self._paper.receive_address(asset, network)
        addr.network = f"coinbase:{addr.network}"
        return addr

    def send(
        self,
        asset: str,
        amount: float,
        to_address: str,
        memo: str = "",
        network: str | None = None,
    ) -> TransferRecord:
        raise WalletError(
            "Live Coinbase withdrawals are intentionally not auto-fired from this "
            "agent. Use backend=paper for simulated sends, or extend "
            "CoinbaseWallet.send with your account's withdrawal API."
        )

    def credit_inbound(
        self,
        asset: str,
        amount: float,
        from_address: str = "external",
        memo: str = "",
    ) -> TransferRecord:
        return self._paper.credit_inbound(asset, amount, from_address, memo)

    def history(self, limit: int = 25) -> list[TransferRecord]:
        return self._paper.history(limit=limit)


class WalletService:
    """High-level wallet ops including USD bridge to paper brokerage cash."""

    def __init__(
        self,
        backend: WalletBackend,
        db: Database,
        cfg: WalletConfig,
        env: EnvSettings,
        safety: "SafetyGate | None" = None,
    ) -> None:
        self.backend = backend
        self.db = db
        self.cfg = cfg
        self.env = env
        self.safety = safety

    def snapshot(self) -> WalletSnapshot:
        return self.backend.snapshot()

    def receive_info(self, asset: str) -> ReceiveAddress:
        return self.backend.receive_address(asset)

    def _usd_value(self, asset: str, amount: float) -> float:
        mark = DEFAULT_MARKS_USD.get(asset.upper(), 0.0)
        return amount * mark

    def _assert_can_send(self, asset: str, amount: float) -> None:
        if self.safety is not None:
            try:
                self.safety.assert_send_allowed()
            except Exception as exc:  # HaltedError
                raise WalletError(str(exc)) from exc
        usd = self._usd_value(asset, amount)
        spent = self.db.outbound_send_usd_today(DEFAULT_MARKS_USD)
        cap = self.cfg.max_daily_send_usd
        if spent + usd > cap + 1e-9:
            raise WalletError(
                f"daily send limit exceeded: ${spent:.2f} + ${usd:.2f} > ${cap:.2f}"
            )

    def send(
        self,
        asset: str,
        amount: float,
        to_address: str,
        memo: str = "",
        network: str | None = None,
    ) -> TransferRecord:
        self._assert_can_send(asset, amount)
        return self.backend.send(asset, amount, to_address, memo=memo, network=network)

    def receive(
        self,
        asset: str,
        amount: float,
        from_address: str = "external",
        memo: str = "",
    ) -> TransferRecord:
        # Inbound receives remain allowed during halt
        return self.backend.credit_inbound(asset, amount, from_address, memo)

    def history(self, limit: int = 25) -> list[TransferRecord]:
        return self.backend.history(limit=limit)

    def bridge_to_broker(self, amount_usd: float) -> TransferRecord:
        if amount_usd <= 0:
            raise WalletError("amount must be positive")
        self._assert_can_send("USD", amount_usd)
        bal = self.db.wallet_balances().get("USD", 0.0)
        if amount_usd > bal + 1e-9:
            raise WalletError(f"insufficient wallet USD ({bal})")
        self.db.ensure_paper_account(0.0)
        cash, _, _ = self.db.load_paper_state()
        self.db.adjust_wallet_balance("USD", -amount_usd)
        self.db.set_paper_cash(cash + amount_usd)
        record = TransferRecord(
            transfer_id=str(uuid.uuid4()),
            direction=TransferDirection.SEND,
            asset="USD",
            amount=amount_usd,
            counterparty="broker:paper",
            network="internal",
            status=TransferStatus.COMPLETED,
            memo="bridge wallet → brokerage cash",
            paper=True,
            tx_ref=f"bridge-out-{secrets.token_hex(6)}",
        )
        self.db.save_transfer(record)
        return record

    def bridge_from_broker(self, amount_usd: float) -> TransferRecord:
        if amount_usd <= 0:
            raise WalletError("amount must be positive")
        if self.safety is not None:
            try:
                self.safety.assert_send_allowed()
            except Exception as exc:
                raise WalletError(str(exc)) from exc
        self.db.ensure_paper_account(0.0)
        cash, _, _ = self.db.load_paper_state()
        if amount_usd > cash + 1e-9:
            raise WalletError(f"insufficient brokerage cash ({cash})")
        self.db.set_paper_cash(cash - amount_usd)
        self.db.adjust_wallet_balance("USD", amount_usd)
        record = TransferRecord(
            transfer_id=str(uuid.uuid4()),
            direction=TransferDirection.RECEIVE,
            asset="USD",
            amount=amount_usd,
            counterparty="broker:paper",
            network="internal",
            status=TransferStatus.COMPLETED,
            memo="bridge brokerage cash → wallet",
            paper=True,
            tx_ref=f"bridge-in-{secrets.token_hex(6)}",
        )
        self.db.save_transfer(record)
        return record


def build_wallet(
    cfg: AppConfig,
    env: EnvSettings,
    db: Database,
    safety: "SafetyGate | None" = None,
) -> WalletService:
    from pi_invest.safety import SafetyGate

    wcfg = cfg.wallet
    gate = safety or SafetyGate(db)
    if wcfg.backend == "coinbase":
        backend: WalletBackend = CoinbaseWallet(env, wcfg, db)
    else:
        backend = PaperWallet(db, wcfg)
    return WalletService(backend, db, wcfg, env, safety=gate)

