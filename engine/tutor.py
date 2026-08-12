"""Conversational tutor helpers for Boriken learning."""

from __future__ import annotations

import random
from typing import Any

from .corpus import Corpus, load_corpus
from .reconstruct import ReconstructionEngine


LESSON_SKILLS = [
    "greetings",
    "identity",
    "home",
    "nature",
    "food",
    "kinship",
    "grammar_possessive",
    "grammar_negation",
]


class TutorEngine:
    def __init__(self, corpus: Corpus | None = None) -> None:
        self.corpus = corpus or load_corpus()
        self.recon = ReconstructionEngine(self.corpus)

    def lesson(self, skill: str | None = None) -> dict[str, Any]:
        skill = skill or random.choice(LESSON_SKILLS)
        if skill == "greetings":
            examples = [
                ("Taí wey", "Good day", "Buenos días"),
                ("Taíno tí", "Good spirit be with you", "Que el buen espíritu te acompañe"),
                ("Waibá", "Let's go", "Vámonos"),
            ]
            prompt = "Say 'good day' in Boriken."
            answer = "Taí wey"
        elif skill == "identity":
            examples = [
                ("Taíno daka", "I am Taíno / I am good", "Soy taíno"),
                ("Wa-borikén", "Our native land", "Nuestra tierra nativa"),
                ("Borikén", "Puerto Rico / native land", "Puerto Rico"),
            ]
            prompt = "How do you say 'I am Taíno'?"
            answer = "Taíno daka"
        elif skill == "grammar_possessive":
            examples = [
                ("da-bohío", "my house", "mi casa"),
                ("wa-borikén", "our Borikén", "nuestro Borikén"),
                ("li-kanowa", "his canoe", "su canoa"),
                ("to-isa", "her child", "su hijo/a"),
            ]
            prompt = "Build 'my house' using da-."
            answer = "da-bohío"
        elif skill == "grammar_negation":
            examples = [
                ("ma-ni", "without water", "sin agua"),
                ("makabuka", "it is not important", "no importa"),
                ("mayani", "do not kill", "no mates"),
            ]
            prompt = "Make 'without gold' using ma- + kawóna."
            answer = "ma-kawóna"
        elif skill == "food":
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("food")[:4]]
            prompt = "What is cassava bread in Boriken?"
            answer = "kasabi"
        elif skill == "nature":
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("nature")[:4]]
            prompt = "Translate 'sun' (attested form)."
            answer = "wey"
        elif skill == "kinship":
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("kinship")[:4]]
            prompt = "How do you say 'our father'?"
            answer = "wakía baba"
        else:  # home
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("home")[:4]]
            prompt = "Translate 'house'."
            answer = "bohío"

        return {
            "skill": skill,
            "title": f"Play Borikén: {skill.replace('_', ' ')}",
            "explanation": self._explain_skill(skill),
            "vibe": "areyto",
            "examples": [
                {"boriken": b, "english": e, "spanish": s} for b, e, s in examples if b
            ],
            "practice": {"prompt": prompt, "answer": answer, "xp": 12},
            "cheer": "Small reps, big reclaiming. Say it out loud once.",
            "fun_hook": self._fun_hook(skill),
        }

    def _fun_hook(self, skill: str) -> str:
        return {
            "greetings": "Mission: greet one person (or your mirror) with Taí wey.",
            "identity": "Mission: whisper Taíno daka like a power-up.",
            "home": "Mission: point at a room and name bohío.",
            "nature": "Mission: look outside and name wey, yukubiya, or bara.",
            "food": "Mission: before your next snack, say the Boriken food word.",
            "kinship": "Mission: text a family member using baba or nana in a joke caption.",
            "grammar_possessive": "Mission: invent da- + any noun you know.",
            "grammar_negation": "Mission: make a silly ma- phrase (ma-hurakán = no storm vibes).",
        }.get(skill, "Mission: use one new word before sunset.")

    def _explain_skill(self, skill: str) -> str:
        return {
            "greetings": "Start with warm everyday phrases used in Neo-Taíno community learning.",
            "identity": "Borikén means native land. Taíno originally meant good/noble.",
            "home": "Bohío is the classic dwelling; kaney is the chiefly rectangular house.",
            "nature": "Many nature terms survive in Caribbean Spanish (huracán, sabana, ceiba).",
            "food": "Cassava (yuka) and kasabi were staple foods of Borikén.",
            "kinship": "Possessive prefixes attach directly to kinship stems.",
            "grammar_possessive": "da- my, wa- our, li- his, to-/tu- her.",
            "grammar_negation": "ma- marks absence/negation; ka- marks having/with.",
        }.get(skill, "Practice core Boriken forms with transparent morphology.")

    def chat(self, message: str) -> dict[str, Any]:
        msg = message.strip()
        lower = msg.lower()

        if any(k in lower for k in ("hello", "hi", "hola", "taiguey", "taí wey", "play", "fun")):
            return {
                "reply_boriken": "Taí wey! Waibá — let's play Borikén.",
                "reply_english": "Good day! Let's make learning feel like an areyto.",
                "reply_spanish": "¡Buenos días! Aprendamos como un areíto.",
                "correction": None,
                "suggestion": self.lesson("greetings"),
                "play_idea": "Try /v1/fun/daily for a three-step island run.",
            }

        if lower.startswith("translate:") or lower.startswith("traduce:"):
            payload = msg.split(":", 1)[1].strip()
            result = self.recon.translate(payload)
            return {
                "reply_boriken": result.boriken,
                "reply_english": result.english or payload,
                "reply_spanish": result.spanish,
                "correction": None,
                "result": result.to_dict(),
                "fun_fact": result.fun_fact,
            }

        if "?" in msg or lower.startswith("what") or lower.startswith("cómo") or lower.startswith("que") or lower.startswith("define"):
            term = msg.replace("?", "")
            for prefix in ("what is", "what's", "define", "qué es", "que es", "cómo se dice"):
                if term.lower().startswith(prefix):
                    term = term[len(prefix) :]
                    break
            explained = self.recon.explain(term.strip())
            if explained["matches"]:
                m = explained["matches"][0]
                return {
                    "reply_boriken": m["boriken"],
                    "reply_english": m.get("definition_en") or m["english"],
                    "reply_spanish": m.get("definition_es") or m["spanish"],
                    "correction": None,
                    "result": explained,
                    "fun_fact": m.get("fun_fact", ""),
                }

        result = self.recon.translate(msg)
        lesson = self.lesson()
        return {
            "reply_boriken": result.boriken or "Ahiya wakía.",
            "reply_english": (
                f"I read that as: {result.english or msg}. "
                f"Quest: {lesson['fun_hook']}"
            ),
            "reply_spanish": result.spanish or "Practiquemos boriken juntos.",
            "correction": None,
            "result": result.to_dict(),
            "suggestion": lesson,
            "fun_fact": result.fun_fact,
        }

    def quiz(self, n: int = 5) -> list[dict[str, Any]]:
        pool = [e for e in self.corpus.entries if e.pos in {"noun", "verb", "phrase", "adjective"}]
        sample = random.sample(pool, k=min(n, len(pool)))
        cards = []
        for e in sample:
            cards.append(
                {
                    "prompt": f"Translate to Boriken: {e.english.split(';')[0].strip()}",
                    "answer": e.boriken,
                    "spanish": e.spanish,
                    "definition_en": e.definition_en,
                    "fun_fact": e.fun_fact,
                    "attested": e.attested,
                    "confidence": e.confidence,
                    "xp": 8,
                }
            )
        return cards
