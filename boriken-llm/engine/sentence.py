"""Sentence-structure teaching and builder for Boriken."""

from __future__ import annotations

import random
from typing import Any

from .corpus import Corpus, load_corpus, normalize
from .reconstruct import ReconstructionEngine


class SentenceStructureEngine:
    def __init__(self, corpus: Corpus | None = None) -> None:
        self.corpus = corpus or load_corpus()
        self.recon = ReconstructionEngine(self.corpus)
        self.grammar = self.corpus.grammar
        self.patterns = list(self.grammar.get("sentence_patterns", []))
        self.structure = self.grammar.get("sentence_structure", {})

    def overview(self) -> dict[str, Any]:
        return {
            "word_order": self.grammar.get("meta", {}).get("word_order"),
            "overview": self.structure.get("overview", {}),
            "slot_guide": self.structure.get("slot_guide", []),
            "build_order": self.structure.get("build_order", []),
            "prefixes": {
                "possessive": self.grammar.get("possessive_prefixes"),
                "verbal": self.grammar.get("verbal_prefixes"),
                "derivational": self.grammar.get("derivational_affixes"),
            },
            "pattern_count": len(self.patterns),
        }

    def list_patterns(self, level: int | None = None) -> list[dict[str, Any]]:
        rows = self.patterns
        if level is not None:
            rows = [p for p in rows if int(p.get("level", 1)) <= level]
        return rows

    def pattern(self, pattern_id: str) -> dict[str, Any] | None:
        for p in self.patterns:
            if p.get("id") == pattern_id:
                return p
        return None

    def examples_for(self, pattern_id: str) -> list[dict[str, Any]]:
        out = []
        for bucket in ("sentences", "learner_sentences"):
            for s in self.corpus.sentences.get(bucket, []):
                if s.get("pattern_id") == pattern_id:
                    out.append(s)
        return out

    def lesson(self, pattern_id: str | None = None) -> dict[str, Any]:
        if pattern_id:
            pat = self.pattern(pattern_id)
        else:
            pat = random.choice(self.patterns)
        if not pat:
            return {"error": "unknown pattern"}
        examples = self.examples_for(pat["id"])[:4]
        if not examples:
            examples = [
                {
                    "boriken": pat.get("example"),
                    "english": pat.get("gloss"),
                    "spanish": pat.get("spanish"),
                    "structure": pat.get("structure"),
                    "attested": False,
                }
            ]
        practice = examples[0]
        return {
            "mode": "sentence_structure_lesson",
            "title": f"Sentence structure: {pat.get('name', pat['id'])}",
            "pattern": pat,
            "overview": self.structure.get("overview"),
            "slot_guide": [
                s
                for s in self.structure.get("slot_guide", [])
                if s.get("slot") in set(pat.get("slots", []))
                or s.get("slot") in str(pat.get("structure", ""))
            ]
            or self.structure.get("slot_guide", [])[:3],
            "examples": examples,
            "practice": {
                "prompt": f"Build this meaning: {practice.get('english')}",
                "answer": practice.get("boriken"),
                "structure": practice.get("structure") or pat.get("structure"),
                "xp": 12,
            },
            "cheer": "Slots first, then music — sentences are areytos too.",
        }

    def build(
        self,
        pattern_id: str,
        subject: str | None = None,
        verb: str | None = None,
        obj: str | None = None,
        noun: str | None = None,
        quality: str | None = None,
        place: str | None = None,
        recipient: str | None = None,
        predicate: str | None = None,
        vocative: str | None = None,
    ) -> dict[str, Any]:
        pat = self.pattern(pattern_id)
        if not pat:
            return {"ok": False, "error": f"Unknown pattern '{pattern_id}'"}

        def stem(word: str | None, fallback: str = "") -> str:
            if not word:
                return fallback
            hits = self.corpus.lookup(word)
            if hits:
                form = hits[0].boriken
                for p in ("da-", "wa-", "li-", "to-", "tu-", "ma-", "ka-"):
                    if form.startswith(p):
                        return form[len(p) :]
                return form
            return self.recon.adapt_loanword(word)

        subj_map = {
            "i": "da-",
            "yo": "da-",
            "we": "wa-",
            "nosotros": "wa-",
            "he": "li-",
            "el": "li-",
            "él": "li-",
            "she": "to-",
            "ella": "to-",
            "my": "da-",
            "mi": "da-",
            "our": "wa-",
            "nuestro": "wa-",
            "nuestra": "wa-",
            "his": "li-",
            "her": "to-",
            "su": "li-",
        }
        prefix = subj_map.get(normalize(subject or ""), "da-")

        pieces: list[str] = []
        morphology: list[str] = []
        pid = pat["id"]

        if pid == "identity":
            pred = stem(predicate) or stem(noun) or "taíno"
            pieces = [pred, "daka"]
            morphology = ["PREDICATE", "daka (I am)"]
        elif pid in {"svo_present", "possessed_object_svo"}:
            v = stem(verb, "sá")
            pieces = [f"{prefix}{v}"]
            morphology = [f"SUBJECT {prefix}", f"VERB {v}"]
            if pid == "possessed_object_svo":
                poss = subj_map.get(normalize(subject or "his"), "li-")
                # allow explicit object possessor via subject field reuse: prefer 'his/her'
                if normalize(obj or "").startswith(("his ", "her ", "su ")):
                    pass
                o_prefix = "li-" if "his" in normalize(obj or subject or "his") or normalize(subject or "") in {
                    "his",
                    "he",
                } else "to-" if "her" in normalize(obj or "") else "li-"
                # simpler: object noun with optional possessor word in obj like "his canoe"
                raw_obj = obj or "canoe"
                o_pref = "li-"
                o_word = raw_obj
                for key, pref in (("his ", "li-"), ("her ", "to-"), ("my ", "da-"), ("our ", "wa-"), ("su ", "li-")):
                    if normalize(raw_obj).startswith(normalize(key).strip()):
                        o_pref = pref
                        o_word = raw_obj.split(" ", 1)[-1]
                        break
                pieces.append(f"{o_pref}{stem(o_word, 'kanowa')}")
                morphology += [f"OBJECT {o_pref}…"]
            elif obj:
                pieces.append(stem(obj))
                morphology.append("OBJECT")
        elif pid == "svo_no_object":
            v = stem(verb, "ahiya")
            pieces = [f"{prefix}{v}"]
            morphology = [f"SUBJECT {prefix}", f"VERB {v}"]
        elif pid == "possessed_noun":
            n = stem(noun or obj, "bohío")
            pieces = [f"{prefix}{n}"]
            morphology = [f"POSSESSOR {prefix}", f"NOUN {n}"]
        elif pid == "negative_noun":
            n = stem(noun or obj, "ni")
            pieces = [f"ma-{n}"]
            morphology = ["ma- NEGATION", f"STEM {n}"]
        elif pid == "attributive":
            n = stem(noun or obj, "kawóna")
            pieces = [f"ka-{n}"]
            morphology = ["ka- ATTRIBUTE", f"STEM {n}"]
            if noun and obj and noun != obj:
                pieces.append(stem(obj))
        elif pid == "imperative":
            v = stem(verb, "kãma")
            pieces = [v]
            morphology = ["VERB (imperative)"]
            if vocative or noun:
                pieces.append(stem(vocative or noun))
                morphology.append("VOCATIVE")
        elif pid == "hortative":
            v = stem(verb, "iba")
            # waibá is the famous fused form for go
            if normalize(verb or "go") in {"go", "ir", "iba"}:
                pieces = ["waibá"]
            else:
                pieces = [f"wa-{v}"]
            morphology = ["wa- hortative", f"VERB {v}"]
            if place:
                pieces.append(stem(place))
                morphology.append("PLACE")
        elif pid == "descriptor":
            n = stem(noun, "isa")
            # keep possession if subject implies it
            if subject and normalize(subject) in subj_map:
                n = f"{subj_map[normalize(subject)]}{n}"
            q = stem(quality or verb, "chali")
            pieces = [n, q]
            morphology = ["NOUN", "QUALITY"]
        elif pid == "location_motion":
            pieces = ["waibá", stem(place or noun or obj, "batey")]
            morphology = ["MOTION waibá", "PLACE"]
        elif pid == "give_dative":
            pieces = ["busika", stem(recipient or noun or "us", "wakía")]
            morphology = ["GIVE busika", "RECIPIENT"]
        elif pid == "negated_clause":
            pieces = ["mayani", stem(obj or noun, "makana")]
            morphology = ["NEGATION mayani", "STEM"]
        else:
            # fallback to free translate
            text = " ".join(x for x in [subject, verb, obj, noun, place] if x)
            result = self.recon.translate(text)
            return {
                "ok": True,
                "pattern_id": pattern_id,
                "boriken": result.boriken,
                "structure": pat.get("structure"),
                "morphology": result.morphology,
                "attestation": result.attestation,
                "confidence": result.confidence,
                "notes": "Fell back to free composition.",
            }

        boriken = " ".join(pieces)
        return {
            "ok": True,
            "pattern_id": pattern_id,
            "pattern_name": pat.get("name"),
            "boriken": boriken,
            "structure": pat.get("structure"),
            "pattern": pat.get("pattern"),
            "morphology": morphology,
            "slots_used": pat.get("slots", []),
            "attestation": "composition",
            "confidence": "medium",
            "english": pat.get("gloss"),
            "spanish": pat.get("spanish"),
            "xp": 10,
            "notes": pat.get("notes") or "Built from Neo-Taíno learner sentence structure.",
        }

    def scramble_game(self, n: int = 1) -> dict[str, Any]:
        pool = [
            s
            for s in self.corpus.sentences.get("learner_sentences", [])
            if s.get("boriken") and len(s["boriken"].replace("-", " ").split()) >= 2
        ]
        sample = random.sample(pool, k=min(n, len(pool)))
        cards = []
        for s in sample:
            tokens = s["boriken"].replace(",", " ").split()
            shuffled = tokens[:]
            random.shuffle(shuffled)
            # avoid identical shuffle
            if shuffled == tokens and len(tokens) > 1:
                shuffled = list(reversed(tokens))
            cards.append(
                {
                    "english": s["english"],
                    "spanish": s.get("spanish"),
                    "structure": s.get("structure"),
                    "pattern_id": s.get("pattern_id"),
                    "tokens": shuffled,
                    "answer": s["boriken"],
                    "xp": 10,
                }
            )
        return {
            "mode": "sentence_scramble",
            "title": "Sentence Batey — restore the order",
            "instructions": "Drag the slots back into Boriken order. Watch prefixes stick to stems.",
            "cards": cards,
        }

    def quiz(self, n: int = 5) -> list[dict[str, Any]]:
        pats = self.patterns[:]
        random.shuffle(pats)
        cards = []
        for p in pats[:n]:
            cards.append(
                {
                    "prompt": f"Which pattern builds: {p.get('gloss')}?",
                    "choices": random.sample(
                        [x.get("structure") for x in pats[:6]],
                        k=min(4, len(pats)),
                    )
                    if len(pats) >= 2
                    else [p.get("structure")],
                    "answer": p.get("structure"),
                    "example": p.get("example"),
                    "pattern_id": p.get("id"),
                    "xp": 8,
                }
            )
            # ensure correct answer present
            if cards[-1]["answer"] not in cards[-1]["choices"]:
                cards[-1]["choices"][0] = cards[-1]["answer"]
                random.shuffle(cards[-1]["choices"])
        return cards
