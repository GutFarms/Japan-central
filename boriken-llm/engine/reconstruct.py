"""Morphology helpers and reconstruction suggestions for Boriken."""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

from .corpus import Corpus, Lexeme, load_corpus, normalize

POSSESSIVES = {
    "my": "da-",
    "our": "wa-",
    "his": "li-",
    "her": "to-",
    "mi": "da-",
    "nuestro": "wa-",
    "nuestra": "wa-",
    "su_el": "li-",
    "su_ella": "to-",
}

SUBJECTS = {
    "i": "da-",
    "we": "wa-",
    "he": "li-",
    "she": "to-",
    "yo": "da-",
    "nosotros": "wa-",
    "nosotras": "wa-",
    "el": "li-",
    "ella": "to-",
}


@dataclass
class ReconstructionResult:
    boriken: str
    english: str
    spanish: str
    confidence: str
    attestation: str
    morphology: list[str]
    notes: str
    matches: list[dict[str, Any]]

    def to_dict(self) -> dict[str, Any]:
        return {
            "boriken": self.boriken,
            "english": self.english,
            "spanish": self.spanish,
            "confidence": self.confidence,
            "attestation": self.attestation,
            "morphology": self.morphology,
            "notes": self.notes,
            "matches": self.matches,
        }


class ReconstructionEngine:
    """Rule-based + lexicon reconstruction for Neo-Taíno / Boriken."""

    def __init__(self, corpus: Corpus | None = None) -> None:
        self.corpus = corpus or load_corpus()

    def translate(self, text: str, source_lang: str = "auto") -> ReconstructionResult:
        text = text.strip()
        if not text:
            return ReconstructionResult(
                boriken="",
                english="",
                spanish="",
                confidence="low",
                attestation="none",
                morphology=[],
                notes="Empty input.",
                matches=[],
            )

        # Direct lexicon hit
        hits = self.corpus.lookup(text)
        if hits:
            best = hits[0]
            if best.attested:
                attestation = "attested"
            elif best.source in {"grammar_demo", "composition", "neo_composition"}:
                attestation = "composition"
            else:
                attestation = "reconstructed"
            return ReconstructionResult(
                boriken=best.boriken,
                english=best.english,
                spanish=best.spanish,
                confidence=best.confidence,
                attestation=attestation,
                morphology=self._morphology_notes(best.boriken) or (
                    ["composition"] if attestation == "composition" else []
                ),
                notes=best.etymology or best.source or "Lexicon match.",
                matches=[h.to_dict() for h in hits[:5]],
            )

        composed = self._try_compose(text)
        if composed:
            return composed

        # Phonotactic adaptation of unknown modern term
        adapted = self.adapt_loanword(text)
        return ReconstructionResult(
            boriken=adapted,
            english=text if source_lang != "es" else "",
            spanish=text if source_lang == "es" else "",
            confidence="low",
            attestation="reconstructed",
            morphology=["loanword_adaptation"],
            notes=(
                "No lexicon match. Proposed phonotactic adaptation for community review. "
                "Compare with Ta-Arawakan cognates before adopting."
            ),
            matches=[],
        )

    def _try_compose(self, text: str) -> ReconstructionResult | None:
        tokens = re.findall(r"[A-Za-zÁÉÍÓÚáéíóúñÑ']+", text)
        if len(tokens) < 2:
            return None

        lower = [t.lower() for t in tokens]
        morphology: list[str] = []
        boriken_parts: list[str] = []
        english_bits: list[str] = []
        spanish_bits: list[str] = []
        confidence = "medium"
        attestation = "composition"

        i = 0
        while i < len(lower):
            tok = lower[i]
            # possessive + noun
            if tok in POSSESSIVES and i + 1 < len(lower):
                prefix = POSSESSIVES[tok]
                noun_hits = self.corpus.lookup(lower[i + 1])
                if noun_hits:
                    stem = self._strip_prefix(noun_hits[0].boriken)
                    boriken_parts.append(f"{prefix}{stem}")
                    english_bits.append(f"{tok} {noun_hits[0].english.split(';')[0].strip()}")
                    spanish_bits.append(noun_hits[0].spanish.split(";")[0].strip())
                    morphology.append(f"{prefix} ({POSSESSIVES[tok] and tok})")
                    morphology.append(f"stem:{stem}")
                    if not noun_hits[0].attested:
                        confidence = "low"
                        attestation = "composition"
                    i += 2
                    continue

            # subject + verb (+ optional object)
            if tok in SUBJECTS and i + 1 < len(lower):
                prefix = SUBJECTS[tok]
                verb_hits = self.corpus.lookup(lower[i + 1])
                if verb_hits and verb_hits[0].pos in {"verb", "phrase"}:
                    stem = self._verb_stem(verb_hits[0].boriken)
                    chunk = f"{prefix}{stem}"
                    obj = ""
                    eng = f"{tok} {verb_hits[0].english.split(';')[0].strip()}"
                    if i + 2 < len(lower):
                        obj_hits = self.corpus.lookup(lower[i + 2])
                        if obj_hits:
                            obj = obj_hits[0].boriken
                            eng += f" {obj_hits[0].english.split(';')[0].strip()}"
                            i += 1
                    boriken_parts.append((chunk + (" " + obj if obj else "")).strip())
                    english_bits.append(eng)
                    spanish_bits.append(verb_hits[0].spanish)
                    morphology.append(f"subject:{prefix}")
                    morphology.append(f"verb:{stem}")
                    i += 2
                    continue

            # negation / attributive
            if tok in {"no", "without", "sin"} and i + 1 < len(lower):
                noun_hits = self.corpus.lookup(lower[i + 1])
                if noun_hits:
                    stem = self._strip_prefix(noun_hits[0].boriken)
                    boriken_parts.append(f"ma-{stem}")
                    english_bits.append(f"without {noun_hits[0].english.split(';')[0].strip()}")
                    spanish_bits.append(f"sin {noun_hits[0].spanish.split(';')[0].strip()}")
                    morphology.append("ma- negative/privative")
                    i += 2
                    continue

            if tok in {"with", "having", "con"} and i + 1 < len(lower):
                noun_hits = self.corpus.lookup(lower[i + 1])
                if noun_hits:
                    stem = self._strip_prefix(noun_hits[0].boriken)
                    boriken_parts.append(f"ka-{stem}")
                    english_bits.append(f"having {noun_hits[0].english.split(';')[0].strip()}")
                    spanish_bits.append(f"con {noun_hits[0].spanish.split(';')[0].strip()}")
                    morphology.append("ka- attributive")
                    i += 2
                    continue

            hit = self.corpus.lookup(tok)
            if hit:
                boriken_parts.append(hit[0].boriken)
                english_bits.append(hit[0].english.split(";")[0].strip())
                spanish_bits.append(hit[0].spanish.split(";")[0].strip())
                if not hit[0].attested:
                    confidence = "low"
            else:
                boriken_parts.append(self.adapt_loanword(tok))
                english_bits.append(tok)
                spanish_bits.append(tok)
                confidence = "low"
                morphology.append(f"adapted:{tok}")
            i += 1

        if not boriken_parts:
            return None

        return ReconstructionResult(
            boriken=" ".join(boriken_parts),
            english=" ".join(english_bits),
            spanish=" ".join(spanish_bits),
            confidence=confidence,
            attestation=attestation,
            morphology=morphology or ["composition"],
            notes="Composed from lexicon stems and attested Arawakan-style affixes.",
            matches=[],
        )

    def explain(self, text: str) -> dict[str, Any]:
        hits = self.corpus.lookup(text)
        morphology = self._morphology_notes(text)
        return {
            "query": text,
            "morphology": morphology,
            "matches": [h.to_dict() for h in hits[:10]],
            "grammar": {
                "possessive_prefixes": self.corpus.grammar.get("possessive_prefixes"),
                "derivational_affixes": self.corpus.grammar.get("derivational_affixes"),
            },
        }

    def reconstruct_concept(self, gloss: str) -> ReconstructionResult:
        """Suggest a Boriken form for a missing modern concept."""
        hits = self.corpus.lookup(gloss)
        if hits:
            return self.translate(gloss)

        # Simple compounding heuristics for modern concepts
        words = normalize(gloss).split()
        parts = []
        notes = []
        for w in words:
            h = self.corpus.lookup(w)
            if h:
                parts.append(self._strip_prefix(h[0].boriken))
                notes.append(f"{w}←{h[0].boriken}")
            else:
                parts.append(self.adapt_loanword(w))
                notes.append(f"{w}←adapted")
        proposal = "-".join(parts) if parts else self.adapt_loanword(gloss)
        return ReconstructionResult(
            boriken=proposal,
            english=gloss,
            spanish=gloss,
            confidence="low",
            attestation="reconstructed",
            morphology=["comparative_compound"],
            notes="Proposed compound/adaptation for review. " + "; ".join(notes),
            matches=[],
        )

    def adapt_loanword(self, word: str) -> str:
        """Adapt a foreign word toward Taíno-like phonotactics."""
        w = normalize(word)
        w = w.replace("ph", "f").replace("th", "t").replace("ch", "sh").replace("qu", "k")
        w = w.replace("c", "k").replace("j", "h").replace("v", "b").replace("z", "s").replace("f", "p")
        w = re.sub(r"([^aeiou])\1+", r"\1", w)
        # Break onset clusters
        out = []
        i = 0
        vowels = set("aeiou")
        while i < len(w):
            ch = w[i]
            out.append(ch)
            if ch not in vowels and i + 1 < len(w) and w[i + 1] not in vowels:
                out.append("a")
            i += 1
        adapted = "".join(out)
        # Prefer vowel ending
        if adapted and adapted[-1] not in vowels and not adapted.endswith("s"):
            adapted += "a"
        # Historical-ish Spanish loan pattern: leading e/i often -> i
        if adapted.startswith("es"):
            adapted = "i" + adapted[1:]
        return adapted or "aka"

    def _strip_prefix(self, form: str) -> str:
        for p in ("da-", "wa-", "li-", "to-", "tu-", "ma-", "ka-"):
            if form.startswith(p):
                return form[len(p) :]
        return form

    def _verb_stem(self, form: str) -> str:
        form = self._strip_prefix(form)
        # waibá -> iba already handled if prefixed; keep known stems short
        if form.endswith("wo"):
            return form[:-2]  # ahiyakawo -> ahiyaka? keep ahiya from lexicon preferably
        return form

    def _morphology_notes(self, form: str) -> list[str]:
        notes: list[str] = []
        f = form.strip()
        for p, meaning in (
            ("da-", "1sg my/I"),
            ("wa-", "1pl our/we"),
            ("li-", "3sg.m his/he"),
            ("to-", "3sg.f her/she"),
            ("tu-", "3sg.f her/she"),
            ("ma-", "negative / without"),
            ("ka-", "attributive / having"),
        ):
            if f.startswith(p) or f.startswith(p.replace("-", "")):
                notes.append(f"{p} = {meaning}")
        if f.endswith("el"):
            notes.append("-el = masculine")
        return notes
