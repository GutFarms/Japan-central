"""Fun learning modes: games, XP, streaks, challenges, word-of-the-day."""

from __future__ import annotations

import hashlib
import random
from datetime import date
from typing import Any

from .corpus import Corpus, Lexeme, load_corpus, normalize
from .history import HistoryEngine
from .tutor import TutorEngine
from .sentence import SentenceStructureEngine

BADGES = [
    {"id": "first_spark", "title": "First Spark", "rule": "Earn 10 XP", "xp": 10},
    {"id": "batey_rookie", "title": "Batey Rookie", "rule": "Win 1 match game", "xp": 25},
    {"id": "kasabi_cook", "title": "Kasabi Cook", "rule": "Learn 5 food words", "xp": 40},
    {"id": "areyto_singer", "title": "Areyto Singer", "rule": "Complete a story quest", "xp": 50},
    {"id": "island_chronist", "title": "Island Chronist", "rule": "Finish Island Time Machine", "xp": 80},
    {"id": "hurakan_chaser", "title": "Hurakán Chaser", "rule": "3-day streak", "xp": 60},
    {"id": "cacique_mind", "title": "Kasike Mind", "rule": "Reach 200 XP", "xp": 200},
]

CHEERS = [
    "Waibá! Momentum looks good.",
    "Taí! That one sparkles.",
    "Kasike energy unlocked.",
    "The batey cheers for you.",
    "Coquí chorus approves.",
    "Borikén remembers that word with you.",
]

GENTLE = [
    "Close! Peek the definition and try once more.",
    "Almost—morphology tip: watch those prefixes.",
    "Nice try. Review the fun fact, then rematch.",
    "No rush. Languages grow like konuko gardens.",
]


class FunLearnEngine:
    """Playful practice layer on top of the lexicon + tutor."""

    def __init__(self, corpus: Corpus | None = None) -> None:
        self.corpus = corpus or load_corpus()
        self.tutor = TutorEngine(self.corpus)
        self.sentences = SentenceStructureEngine(self.corpus)
        self.history = HistoryEngine(self.corpus)

    def word_of_the_day(self, day: str | None = None) -> dict[str, Any]:
        day = day or date.today().isoformat()
        pool = [e for e in self.corpus.entries if e.pos in {"noun", "verb", "phrase", "adjective"}]
        seed = int(hashlib.sha256(day.encode()).hexdigest()[:8], 16)
        rng = random.Random(seed)
        word = rng.choice(pool)
        return {
            "date": day,
            "mode": "word_of_the_day",
            "title": "Island Word of the Day",
            "cheer": "Catch today's word like a flying fotuto note.",
            "word": word.to_dict(),
            "challenge": {
                "prompt": f"Use '{word.boriken}' in a greeting or sentence today.",
                "xp": 15,
            },
        }

    def define(self, query: str) -> dict[str, Any]:
        hits = self.corpus.lookup(query)
        if not hits:
            return {
                "query": query,
                "found": False,
                "message": "No match yet—try a related gloss or ask /v1/reconstruct.",
                "items": [],
            }
        return {
            "query": query,
            "found": True,
            "items": [h.to_dict() for h in hits[:8]],
            "spotlight": hits[0].to_dict(),
            "cheer": random.choice(CHEERS),
        }

    def flashcards(self, n: int = 8, tag: str | None = None) -> dict[str, Any]:
        pool = self.corpus.by_tag(tag) if tag else list(self.corpus.entries)
        pool = [e for e in pool if e.pos != "unknown"]
        sample = random.sample(pool, k=min(n, len(pool)))
        cards = []
        for e in sample:
            cards.append(
                {
                    "id": e.id,
                    "front": e.english.split(";")[0].strip(),
                    "front_es": e.spanish.split(";")[0].strip(),
                    "back": e.boriken,
                    "definition_en": e.definition_en,
                    "definition_es": e.definition_es,
                    "fun_fact": e.fun_fact,
                    "example": e.example,
                    "attested": e.attested,
                    "xp": 5 if e.attested else 4,
                }
            )
        return {
            "mode": "flashcards",
            "title": "Memory Bohío",
            "instructions": "Say the Boriken form out loud before flipping. Bonus whisper: the fun fact.",
            "cards": cards,
            "total_xp": sum(c["xp"] for c in cards),
        }

    def match_game(self, n: int = 4, tag: str | None = None) -> dict[str, Any]:
        pool = self.corpus.by_tag(tag) if tag else [e for e in self.corpus.entries if e.pos in {"noun", "verb"}]
        sample = random.sample(pool, k=min(n, len(pool)))
        pairs = [{"id": e.id, "boriken": e.boriken, "gloss": e.english.split(";")[0].strip()} for e in sample]
        glosses = [p["gloss"] for p in pairs]
        random.shuffle(glosses)
        return {
            "mode": "match",
            "title": "Batey Match",
            "instructions": "Match each Boriken word to its meaning. Speed is fun; accuracy earns XP.",
            "left": [{"id": p["id"], "text": p["boriken"]} for p in pairs],
            "right": glosses,
            "answer_key": {p["boriken"]: p["gloss"] for p in pairs},
            "xp_reward": 10 + 2 * len(pairs),
            "fun_tip": random.choice([e.fun_fact for e in sample]),
        }

    def fill_blank(self, n: int = 5) -> dict[str, Any]:
        pool = [e for e in self.corpus.entries if e.example and e.boriken in e.example]
        if len(pool) < n:
            pool = [e for e in self.corpus.entries if e.pos in {"noun", "phrase"}]
        sample = random.sample(pool, k=min(n, len(pool)))
        items = []
        for e in sample:
            sentence = e.example or e.boriken
            blanked = sentence.replace(e.boriken, "____", 1)
            items.append(
                {
                    "prompt": blanked,
                    "hint": e.definition_en.split(".")[0],
                    "answer": e.boriken,
                    "gloss": e.english,
                    "fun_fact": e.fun_fact,
                    "xp": 8,
                }
            )
        return {
            "mode": "fill_blank",
            "title": "Konuko Fill-In",
            "instructions": "Grow the sentence—plant the missing Boriken word.",
            "items": items,
        }

    def story_quest(self) -> dict[str, Any]:
        beats = [
            {
                "scene": "Dawn over Borikén",
                "narrative": "The wey climbs. A coquí calls from the seyba.",
                "teach": self.corpus.lookup("sun")[0].to_dict() if self.corpus.lookup("sun") else None,
                "choice_prompt": "Greet the morning:",
                "choices": [
                    {"text": "Taí wey", "correct": True, "xp": 10},
                    {"text": "Makabuka", "correct": False, "xp": 0},
                ],
            },
            {
                "scene": "At the konuko",
                "narrative": "You help grate yuka for kasabi. The scent is ancient and warm.",
                "teach": self.corpus.lookup("cassava bread")[0].to_dict()
                if self.corpus.lookup("cassava bread")
                else None,
                "choice_prompt": "Name the bread:",
                "choices": [
                    {"text": "kasabi", "correct": True, "xp": 10},
                    {"text": "hamaka", "correct": False, "xp": 0},
                ],
            },
            {
                "scene": "Into the batey",
                "narrative": "Drums for an areyto. The kasike nods—your turn to speak.",
                "teach": self.corpus.lookup("I am")[0].to_dict() if self.corpus.lookup("I am") else None,
                "choice_prompt": "Introduce yourself as good/Taíno:",
                "choices": [
                    {"text": "Taíno daka", "correct": True, "xp": 12},
                    {"text": "Waibá ni", "correct": False, "xp": 0},
                ],
            },
            {
                "scene": "Farewell path",
                "narrative": "Friends call from the kanowa. Adventure continues.",
                "teach": self.corpus.lookup("let's go")[0].to_dict() if self.corpus.lookup("let's go") else None,
                "choice_prompt": "Rally the crew:",
                "choices": [
                    {"text": "Waibá", "correct": True, "xp": 10},
                    {"text": "Ua", "correct": False, "xp": 0},
                ],
            },
        ]
        return {
            "mode": "story_quest",
            "title": "Areyto Quest: Morning in Borikén",
            "instructions": "Play the scenes, pick the living word, bank XP.",
            "beats": beats,
            "completion_badge": "areyto_singer",
            "total_xp_possible": sum(
                max((c["xp"] for c in b["choices"]), default=0) for b in beats
            ),
        }

    def daily_challenge(self) -> dict[str, Any]:
        wotd = self.word_of_the_day()
        match = self.match_game(n=4)
        lesson = self.tutor.lesson()
        return {
            "mode": "daily_challenge",
            "title": "Daily Island Run",
            "steps": [
                {"step": 1, "task": "Learn Word of the Day", "payload": wotd, "xp": 15},
                {"step": 2, "task": "Win Batey Match", "payload": match, "xp": match["xp_reward"]},
                {"step": 3, "task": "Finish mini-lesson", "payload": lesson, "xp": 12},
                {
                    "step": 4,
                    "task": "One Island Time Machine quiz",
                    "payload": self.history.quiz(),
                    "xp": 10,
                },
            ],
            "bonus_badge": "hurakan_chaser",
            "cheer": "Four tiny quests. One louder Borikén.",
        }

    def grade(self, mode: str, answer: str, expected: str) -> dict[str, Any]:
        ok = normalize(answer) == normalize(expected)
        return {
            "correct": ok,
            "expected": expected,
            "answer": answer,
            "xp_earned": 8 if ok else 1,
            "message": random.choice(CHEERS) if ok else random.choice(GENTLE),
            "mode": mode,
        }

    def progress_preview(self, xp: int = 0, streak: int = 0) -> dict[str, Any]:
        earned = []
        for badge in BADGES:
            unlocked = xp >= badge["xp"] or (badge["id"] == "hurakan_chaser" and streak >= 3)
            if badge["id"] == "first_spark":
                unlocked = xp >= 10
            earned.append({**badge, "unlocked": unlocked})
        level = 1 + xp // 50
        titles = {
            1: "Konuko Seedling",
            2: "Bohío Builder",
            3: "Batey Player",
            4: "Areyto Voice",
            5: "Kasike Apprentice",
        }
        title = titles.get(min(level, 5), "Island Elder")
        return {
            "xp": xp,
            "streak": streak,
            "level": level,
            "title": title,
            "badges": earned,
            "next_level_xp": level * 50,
            "tip": "Open Word of the Day every morning—streaks love sunrise routines.",
        }

    def play_menu(self) -> dict[str, Any]:
        return {
            "title": "Play Borikén",
            "subtitle": "Learn like an areyto—move, match, remember, smile.",
            "modes": [
                {"id": "word_of_the_day", "name": "Word of the Day", "blurb": "One island word. Fifteen XP.", "route": "/v1/fun/word-of-the-day"},
                {"id": "flashcards", "name": "Memory Bohío", "blurb": "Flip cards with definitions + fun facts.", "route": "/v1/fun/flashcards"},
                {"id": "match", "name": "Batey Match", "blurb": "Pair Boriken ↔ meaning against the clock.", "route": "/v1/fun/match"},
                {"id": "fill_blank", "name": "Konuko Fill-In", "blurb": "Plant the missing word in a sentence.", "route": "/v1/fun/fill-blank"},
                {"id": "story_quest", "name": "Areyto Quest", "blurb": "A tiny adventure that teaches as you choose.", "route": "/v1/fun/story"},
                {"id": "history", "name": "Island Time Machine", "blurb": "Fun history of Borikén in eight playable chapters.", "route": "/v1/fun/history"},
                {"id": "sentence_builder", "name": "Sentence Builder", "blurb": "Assemble SVO / identity / possession patterns.", "route": "/v1/sentences/lesson"},
                {"id": "sentence_scramble", "name": "Sentence Scramble", "blurb": "Restore Boriken word order for XP.", "route": "/v1/fun/sentence-scramble"},
                {"id": "daily_challenge", "name": "Daily Island Run", "blurb": "Three quests. One streak-friendly combo.", "route": "/v1/fun/daily"},
            ],
            "badges": BADGES,
        }

    def sentence_scramble(self, n: int = 3) -> dict[str, Any]:
        return self.sentences.scramble_game(n=n)

    def history_timeline(self) -> dict[str, Any]:
        return self.history.overview()

    def history_quest(self) -> dict[str, Any]:
        return self.history.quest()

    def history_era(self, era_id: str | None = None, chapter: int | None = None) -> dict[str, Any]:
        return self.history.era(era_id=era_id, chapter=chapter)

    def history_quiz(self, era_id: str | None = None) -> dict[str, Any]:
        return self.history.quiz(era_id=era_id)

    def history_walk(self, index: int = 0) -> dict[str, Any]:
        return self.history.walk(index=index)
