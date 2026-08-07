from __future__ import annotations

import uuid
from datetime import datetime
from zoneinfo import ZoneInfo

from pi_invest.agent.llm import LlmAdvisor
from pi_invest.agent.risk import RiskGate
from pi_invest.agent.scoring import heuristic_intents, score_symbol
from pi_invest.broker import Broker
from pi_invest.config import AppConfig, EnvSettings
from pi_invest.data import ResilientMarketData
from pi_invest.models import Decision, Side, TradeIntent, utcnow
from pi_invest.storage.db import Database


def _market_open(tz_name: str) -> bool:
    try:
        now = datetime.now(ZoneInfo(tz_name))
    except Exception:  # noqa: BLE001
        now = utcnow()
    if now.weekday() >= 5:
        return False
    minutes = now.hour * 60 + now.minute
    return 9 * 60 + 30 <= minutes <= 16 * 60


class InvestAgent:
    def __init__(
        self,
        cfg: AppConfig,
        env: EnvSettings,
        market: ResilientMarketData,
        broker: Broker,
        db: Database,
    ) -> None:
        self.cfg = cfg
        self.env = env
        self.market = market
        self.broker = broker
        self.db = db
        self.risk = RiskGate(cfg.risk, cfg.universe)
        self.llm = LlmAdvisor(cfg.llm, env)

    def run_cycle(self, dry_run: bool = False) -> Decision:
        cycle_id = str(uuid.uuid4())
        skipped: list[str] = []
        market_open = _market_open(self.cfg.schedule.market_tz)

        quotes: dict[str, float] = {}
        scores = []
        for symbol in self.cfg.universe:
            try:
                hist = self.market.get_history(
                    symbol, lookback_days=self.cfg.market.lookback_days
                )
                quote = self.market.get_quote(symbol)
                quotes[symbol.upper()] = quote.price
                scores.append(score_symbol(quote, hist))
            except Exception as exc:  # noqa: BLE001
                skipped.append(f"{symbol}: data error ({exc})")

        marks = dict(quotes)
        account = self.broker.get_account(marks)
        try:
            self.db.rollover_day_start_if_needed(account.equity)
            account = self.broker.get_account(marks)
        except Exception:  # noqa: BLE001
            pass

        intents: list[TradeIntent] = self.risk.trim_overweight(account)

        llm_raw = None
        if self.llm.enabled() and scores:
            llm_intents, llm_raw = self.llm.advise(scores, account, self.cfg.universe)
            if llm_intents:
                intents.extend(llm_intents)
            elif llm_raw and str(llm_raw).startswith("llm error"):
                skipped.append(str(llm_raw))
                intents.extend(heuristic_intents(scores))
            else:
                intents.extend(heuristic_intents(scores))
        else:
            intents.extend(heuristic_intents(scores))

        held = {p.symbol for p in account.positions}
        filtered_intents: list[TradeIntent] = []
        for intent in intents:
            if intent.side == Side.SELL and intent.symbol not in held:
                continue
            filtered_intents.append(intent)

        if (
            self.cfg.schedule.prefer_market_hours
            and not market_open
            and self.cfg.agent.mode == "live"
        ):
            decision = Decision(
                cycle_id=cycle_id,
                market_open=market_open,
                scores=scores,
                intents=filtered_intents,
                skipped_reasons=skipped + ["live trading blocked outside hours"],
                llm_raw=llm_raw,
                account_after=account,
                meta={"data_source": self.market.last_source},
            )
            self.db.save_decision(decision)
            return decision

        if self.cfg.schedule.prefer_market_hours and not market_open:
            skipped.append("outside preferred US market hours (paper still executes)")

        safe_intents, risk_notes = self.risk.filter_intents(filtered_intents, account)
        skipped.extend(risk_notes)

        orders = []
        if not dry_run:
            for intent in safe_intents:
                account = self.broker.get_account(marks)
                px = marks.get(intent.symbol)
                if px is None:
                    skipped.append(f"{intent.symbol}: missing mark")
                    continue
                order_req, reason = self.risk.to_order(intent, account, px)
                if order_req is None:
                    skipped.append(f"{intent.symbol}: {reason}")
                    continue
                result = self.broker.place_order(order_req, px)
                orders.append(result)
        else:
            skipped.append("dry-run: orders not sent")

        account_after = self.broker.get_account(marks)
        decision = Decision(
            cycle_id=cycle_id,
            market_open=market_open,
            scores=sorted(scores, key=lambda s: s.expected_income_proxy, reverse=True),
            intents=safe_intents,
            orders=orders,
            skipped_reasons=skipped,
            llm_raw=llm_raw,
            account_after=account_after,
            meta={
                "data_source": self.market.last_source,
                "mode": self.cfg.agent.mode,
                "backend": self.cfg.broker.backend,
                "agent": self.cfg.agent.name,
            },
        )
        self.db.save_decision(decision)
        return decision
