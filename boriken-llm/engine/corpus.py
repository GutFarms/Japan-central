"""Boriken language corpus loader."""

from __future__ import annotations

import json
import re
import unicodedata
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "corpus"


def normalize(text: str) -> str:
    text = unicodedata.normalize("NFC", text or "").strip().lower()
    replacements = {
        "é": "e",
        "í": "i",
        "ó": "o",
        "ú": "u",
        "á": "a",
        "ñ": "n",
        "ẽ": "e",
        "õ": "o",
        "ã": "a",
        "â": "a",
        "ê": "e",
        "ü": "u",
    }
    for src, dst in replacements.items():
        text = text.replace(src, dst)
    return re.sub(r"[^\w\s\-]", "", text)


@dataclass(frozen=True)
class Lexeme:
    id: str
    boriken: str
    english: str
    spanish: str
    pos: str
    attested: bool
    tags: tuple[str, ...]
    etymology: str | None = None
    source: str | None = None

    @property
    def confidence(self) -> str:
        if self.attested:
            return "high"
        if self.source in {"neo_from_warike", "neo_from_waiba", "grammar_demo", "composition"}:
            return "medium"
        if self.source:
            return "medium"
        return "low"

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "boriken": self.boriken,
            "english": self.english,
            "spanish": self.spanish,
            "pos": self.pos,
            "attested": self.attested,
            "confidence": self.confidence,
            "tags": list(self.tags),
            "etymology": self.etymology,
            "source": self.source,
        }


class Corpus:
    def __init__(self, root: Path | None = None) -> None:
        self.root = root or CORPUS
        raw = json.loads((self.root / "vocabulary.json").read_text(encoding="utf-8"))
        self.meta = raw.get("meta", {})
        self.entries: list[Lexeme] = []
        for item in raw["entries"]:
            self.entries.append(
                Lexeme(
                    id=item["id"],
                    boriken=item["boriken"],
                    english=item["english"],
                    spanish=item["spanish"],
                    pos=item.get("pos", "unknown"),
                    attested=bool(item.get("attested", False)),
                    tags=tuple(item.get("tags", [])),
                    etymology=item.get("etymology"),
                    source=item.get("source"),
                )
            )
        self.grammar = json.loads((self.root / "grammar.json").read_text(encoding="utf-8"))
        self.sentences = json.loads((self.root / "sentences.json").read_text(encoding="utf-8"))
        self._by_norm: dict[str, list[Lexeme]] = {}
        for lex in self.entries:
            for field in (lex.boriken, lex.english, lex.spanish, lex.id):
                key = normalize(field)
                self._by_norm.setdefault(key, [])
                if lex not in self._by_norm[key]:
                    self._by_norm[key].append(lex)
            # Also index first english sense before semicolon
            for piece in re.split(r"[;/]|,", lex.english):
                key = normalize(piece)
                if key:
                    self._by_norm.setdefault(key, [])
                    if lex not in self._by_norm[key]:
                        self._by_norm[key].append(lex)
            for piece in re.split(r"[;/]|,", lex.spanish):
                key = normalize(piece)
                if key:
                    self._by_norm.setdefault(key, [])
                    if lex not in self._by_norm[key]:
                        self._by_norm[key].append(lex)

    def lookup(self, query: str) -> list[Lexeme]:
        key = normalize(query)
        exact = list(self._by_norm.get(key, []))
        if exact:
            return exact
        # Fuzzy contains
        hits: list[Lexeme] = []
        for lex in self.entries:
            blob = normalize(f"{lex.boriken} {lex.english} {lex.spanish} {' '.join(lex.tags)}")
            if key and key in blob:
                hits.append(lex)
        return hits[:20]

    def by_tag(self, tag: str) -> list[Lexeme]:
        tag_n = normalize(tag)
        return [lex for lex in self.entries if tag_n in {normalize(t) for t in lex.tags}]

    def attested(self) -> list[Lexeme]:
        return [lex for lex in self.entries if lex.attested]

    def all_parallel_lines(self) -> list[str]:
        lines: list[str] = []
        for lex in self.entries:
            lines.append(f"{lex.english} = {lex.boriken}")
            lines.append(f"{lex.spanish} = {lex.boriken}")
            lines.append(f"{lex.boriken} :: {lex.english} / {lex.spanish}")
        for s in self.sentences.get("sentences", []):
            lines.append(f"{s['english']} => {s['boriken']}")
            lines.append(f"{s['spanish']} => {s['boriken']}")
            lines.append(s["boriken"])
        for s in self.sentences.get("learner_sentences", []):
            lines.append(f"{s['english']} => {s['boriken']}")
            lines.append(f"{s['spanish']} => {s['boriken']}")
            lines.append(s["boriken"])
        return lines


@lru_cache(maxsize=1)
def load_corpus() -> Corpus:
    return Corpus()


def iter_training_pairs(corpus: Corpus | None = None) -> Iterable[dict[str, str]]:
    c = corpus or load_corpus()
    for lex in c.entries:
        yield {
            "instruction": f"Translate to Boriken: {lex.english}",
            "output": lex.boriken,
            "meta": "translate_en",
        }
        yield {
            "instruction": f"Traduce al boriken: {lex.spanish}",
            "output": lex.boriken,
            "meta": "translate_es",
        }
        yield {
            "instruction": f"What does '{lex.boriken}' mean?",
            "output": f"{lex.english} / {lex.spanish} [{('attested' if lex.attested else 'reconstructed')}]",
            "meta": "define",
        }
    for s in c.sentences.get("sentences", []) + c.sentences.get("learner_sentences", []):
        yield {
            "instruction": f"Translate to Boriken: {s['english']}",
            "output": s["boriken"],
            "meta": "sentence_en",
        }
        yield {
            "instruction": f"Traduce al boriken: {s['spanish']}",
            "output": s["boriken"],
            "meta": "sentence_es",
        }
