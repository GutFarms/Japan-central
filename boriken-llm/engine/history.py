"""Fun Borikén history: Island Time Machine."""

from __future__ import annotations

import json
import random
from pathlib import Path
from typing import Any

from .corpus import Corpus, CORPUS, load_corpus

HISTORY_PATH = CORPUS / "history.json"


class HistoryEngine:
    """Playful timeline + quizzes grounded in ethnohistory labels."""

    def __init__(self, corpus: Corpus | None = None) -> None:
        self.corpus = corpus or load_corpus()
        raw = json.loads(HISTORY_PATH.read_text(encoding="utf-8"))
        self.meta = raw.get("meta", {})
        self.eras: list[dict[str, Any]] = list(raw.get("eras", []))

    def _resolve_words(self, ids: list[str]) -> list[dict[str, Any]]:
        out: list[dict[str, Any]] = []
        for wid in ids:
            hits = [e for e in self.corpus.entries if e.id == wid]
            if not hits:
                hits = self.corpus.lookup(wid)
            if hits:
                out.append(hits[0].to_dict())
        return out

    def _enrich_era(self, era: dict[str, Any]) -> dict[str, Any]:
        item = dict(era)
        item["lexicon"] = self._resolve_words(list(era.get("words") or []))
        item["xp"] = int((era.get("quiz") or {}).get("xp") or 10)
        return item

    def overview(self) -> dict[str, Any]:
        chapters = [
            {
                "id": e["id"],
                "chapter": e.get("chapter"),
                "era_label": e.get("era_label"),
                "when": e.get("when"),
                "title_en": e.get("title_en"),
                "title_es": e.get("title_es"),
                "fun_hook": e.get("fun_hook"),
                "xp": int((e.get("quiz") or {}).get("xp") or 10),
            }
            for e in self.eras
        ]
        return {
            "mode": "history_timeline",
            "title": self.meta.get("title", "Island Time Machine"),
            "subtitle": self.meta.get("subtitle", ""),
            "disclaimer_en": self.meta.get("disclaimer_en", ""),
            "disclaimer_es": self.meta.get("disclaimer_es", ""),
            "accuracy_note": self.meta.get("accuracy_note", ""),
            "version": self.meta.get("version", "0.3.2"),
            "chapters": chapters,
            "total_chapters": len(chapters),
            "total_xp_possible": sum(c["xp"] for c in chapters),
            "badge": {
                "id": "island_chronist",
                "title": "Island Chronist",
                "rule": "Finish the Island Time Machine",
                "xp": 80,
            },
            "cheer": "Waibá through time—eight tiny chapters, one louder Borikén.",
        }

    def era(self, era_id: str | None = None, chapter: int | None = None) -> dict[str, Any]:
        chosen: dict[str, Any] | None = None
        if era_id:
            chosen = next((e for e in self.eras if e["id"] == era_id), None)
        elif chapter is not None:
            chosen = next((e for e in self.eras if e.get("chapter") == chapter), None)
        if chosen is None:
            chosen = self.eras[0] if self.eras else {}
        enriched = self._enrich_era(chosen)
        idx = next((i for i, e in enumerate(self.eras) if e["id"] == enriched.get("id")), 0)
        return {
            "mode": "history_era",
            "index": idx,
            "total": len(self.eras),
            "era": enriched,
            "next_id": self.eras[idx + 1]["id"] if idx + 1 < len(self.eras) else None,
            "prev_id": self.eras[idx - 1]["id"] if idx > 0 else None,
            "cheer": enriched.get("fun_hook", "Keep walking the areyto of ages."),
        }

    def walk(self, index: int = 0) -> dict[str, Any]:
        if not self.eras:
            return {"mode": "history_walk", "done": True, "message": "No chapters yet."}
        if index < 0:
            index = 0
        if index >= len(self.eras):
            return {
                "mode": "history_walk",
                "done": True,
                "title": "Timeline complete",
                "message": "You walked Borikén's areyto of ages. Badge: Island Chronist.",
                "xp_bonus": 20,
                "badge": "island_chronist",
                "cheer": "Taí! History XP banked—now teach one friend waibá.",
            }
        payload = self.era(era_id=self.eras[index]["id"])
        payload["mode"] = "history_walk"
        payload["done"] = False
        payload["walk_index"] = index
        return payload

    def quiz(self, era_id: str | None = None) -> dict[str, Any]:
        pool = self.eras
        if era_id:
            pool = [e for e in self.eras if e["id"] == era_id] or self.eras
        era = random.choice(pool)
        q = dict(era.get("quiz") or {})
        choices = list(q.get("choices") or [])
        random.shuffle(choices)
        return {
            "mode": "history_quiz",
            "title": "History Pop Quiz",
            "era_id": era["id"],
            "era_label": era.get("era_label"),
            "question": q.get("question"),
            "choices": choices,
            "answer": q.get("answer"),
            "xp": int(q.get("xp") or 10),
            "fun_hook": era.get("fun_hook"),
            "honesty": era.get("honesty"),
        }

    def quest(self) -> dict[str, Any]:
        """Full time-machine run: narrative + quiz per chapter."""
        beats = []
        for era in self.eras:
            q = era.get("quiz") or {}
            choices = list(q.get("choices") or [])
            answer = q.get("answer")
            choice_objs = []
            for c in choices:
                choice_objs.append(
                    {
                        "text": c,
                        "correct": c == answer,
                        "xp": int(q.get("xp") or 10) if c == answer else 0,
                    }
                )
            beats.append(
                {
                    "scene": era.get("era_label"),
                    "when": era.get("when"),
                    "title": era.get("title_en"),
                    "narrative": era.get("story_en"),
                    "narrative_es": era.get("story_es"),
                    "fun_hook": era.get("fun_hook"),
                    "mission": era.get("mission"),
                    "honesty": era.get("honesty"),
                    "lexicon": self._resolve_words(list(era.get("words") or [])),
                    "choice_prompt": q.get("question"),
                    "choices": choice_objs,
                }
            )
        return {
            "mode": "history_quest",
            "title": "Island Time Machine",
            "subtitle": self.meta.get("subtitle", ""),
            "disclaimer_en": self.meta.get("disclaimer_en", ""),
            "instructions": "Read each chapter, pick the true beat, bank history XP.",
            "beats": beats,
            "completion_badge": "island_chronist",
            "total_xp_possible": sum(
                max((c["xp"] for c in b["choices"]), default=0) for b in beats
            )
            + 20,
            "cheer": "Eight chapters. One island. Let's go—waibá!",
        }
