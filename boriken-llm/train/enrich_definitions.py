#!/usr/bin/env python3
"""Enrich vocabulary.json with learner-friendly definitions + fun facts."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VOCAB = ROOT / "corpus" / "vocabulary.json"

# Hand-crafted definitions / fun facts for key cultural terms.
SPECIAL: dict[str, dict[str, str]] = {
    "boriken": {
        "definition_en": "The Taíno name for Puerto Rico—literally 'native land' (bori + -ken).",
        "definition_es": "El nombre taíno de Puerto Rico: literalmente 'tierra nativa' (bori + -ken).",
        "fun_fact": "Saying Borikén is a tiny act of reclaiming the island's first name.",
    },
    "taino": {
        "definition_en": "Originally meant 'good' or 'noble'; later used as the ethnonym for the people.",
        "definition_es": "Originalmente significaba 'bueno' o 'noble'; luego se usó como nombre del pueblo.",
        "fun_fact": "Columbus heard 'Taíno, Taíno' and thought it was their tribal name—it was a greeting of goodness.",
    },
    "huracan": {
        "definition_en": "A powerful storm; also linked to a fierce wind/storm spirit in Taíno cosmology.",
        "definition_es": "Una tormenta poderosa; también ligada a un espíritu del viento en la cosmología taína.",
        "fun_fact": "English 'hurricane' and Spanish 'huracán' both come from this word.",
    },
    "hamaca": {
        "definition_en": "A hanging bed woven from plant fiber—perfect for island naps and trade voyages.",
        "definition_es": "Una cama colgante tejida de fibra vegetal—ideal para siestas y viajes.",
        "fun_fact": "Europeans had never seen hammocks before meeting Taíno communities.",
    },
    "canoa": {
        "definition_en": "A dugout boat carved from a single tree trunk, used for fishing and island travel.",
        "definition_es": "Una embarcación tallada de un solo tronco, usada para pescar y viajar entre islas.",
        "fun_fact": "'Canoe' entered European languages from Taíno kanowa.",
    },
    "barbacoa": {
        "definition_en": "A raised wooden grill or frame for slow-cooking food over fire.",
        "definition_es": "Una parrilla elevada de madera para cocinar lento sobre el fuego.",
        "fun_fact": "Yes—barbecue is a Caribbean gift to the world.",
    },
    "casabe": {
        "definition_en": "Flat bread made from grated and pressed cassava (yuka), a staple food of Borikén.",
        "definition_es": "Pan plano de yuca rallada y prensada; alimento básico de Borikén.",
        "fun_fact": "Kasabi travels well—voyagers packed it for long canoe journeys.",
    },
    "yuca": {
        "definition_en": "Cassava root; the foundation of kasabi bread and a sacred crop linked to Yokahú.",
        "definition_es": "Raíz de yuca/casava; base del casabe y cultivo sagrado ligado a Yokahú.",
        "fun_fact": "Yuka wasn't just food—it was spiritual technology for survival.",
    },
    "zemi": {
        "definition_en": "A sacred figure or spirit presence—carved, kept, and honored in the home and ceremony.",
        "definition_es": "Figura o presencia sagrada—tallada, guardada y honrada en el hogar y la ceremonia.",
        "fun_fact": "Every family could keep semí that protected health, harvest, and journeys.",
    },
    "atabey": {
        "definition_en": "Mother spirit of fresh water, fertility, and the moon—source of life on the islands.",
        "definition_es": "Espíritu madre del agua dulce, la fertilidad y la luna—fuente de vida en las islas.",
        "fun_fact": "She is often honored as the feminine counterpart to Yokahú.",
    },
    "yocahu": {
        "definition_en": "Spirit associated with cassava and the sky—provider of staple food and life.",
        "definition_es": "Espíritu asociado a la yuca y el cielo—proveedor de alimento y vida.",
        "fun_fact": "When you eat kasabi, you're tasting a story older than the colony.",
    },
    "batey": {
        "definition_en": "The village plaza and ceremonial ball court—where community, sport, and ritual met.",
        "definition_es": "La plaza y cancha ceremonial del pueblo—donde se unían comunidad, deporte y rito.",
        "fun_fact": "Think town square + sacred arena in one word.",
    },
    "areito": {
        "definition_en": "A communal ceremony of song, dance, and storytelling that preserved history.",
        "definition_es": "Ceremonia comunal de canto, baile y relato que preservaba la historia.",
        "fun_fact": "Areytos were living libraries—history you could dance.",
    },
    "cacique": {
        "definition_en": "A community leader or chief responsible for guidance, diplomacy, and ceremony.",
        "definition_es": "Líder o jefe comunitario responsable de guía, diplomacia y ceremonia.",
        "fun_fact": "Spanish kept the word 'cacique' and still uses it today.",
    },
    "bohio": {
        "definition_en": "A round thatched house—the classic Taíno family dwelling.",
        "definition_es": "Casa redonda de techo de paja—la vivienda familiar taína clásica.",
        "fun_fact": "Walk into a bohío and you're inside island architecture 101.",
    },
    "coqui": {
        "definition_en": "The tiny frog whose night song is the soundtrack of Borikén.",
        "definition_es": "La ranita cuyo canto nocturno es la banda sonora de Borikén.",
        "fun_fact": "If you can hear 'ko-KEE', you're home.",
    },
    "ceiba": {
        "definition_en": "The great silk-cotton tree, often treated as a sacred axis connecting earth and sky.",
        "definition_es": "El gran árbol de ceiba, a menudo visto como eje sagrado entre tierra y cielo.",
        "fun_fact": "Many Caribbean communities still gather under the ceiba.",
    },
    "daka": {
        "definition_en": "First-person identity verb: 'I am'—the keystone of introducing yourself.",
        "definition_es": "Verbo de identidad en primera persona: 'yo soy'—clave para presentarte.",
        "fun_fact": "Start with 'Taíno daka' and you've already begun reclaiming voice.",
    },
    "waiba": {
        "definition_en": "Hortative/motion form meaning 'we go' or 'let's go'.",
        "definition_es": "Forma de movimiento/exhortación: 'vamos' o 'vámonos'.",
        "fun_fact": "The perfect quest-start button in Boriken.",
    },
    "anichi": {
        "definition_en": "To love; also associated with the heart in Neo-Taíno learning use.",
        "definition_es": "Amar; también asociado al corazón en el aprendizaje neo-taíno.",
        "fun_fact": "Try 'Anichi borikén'—love the native land.",
    },
}


def make_definition(entry: dict) -> tuple[str, str, str]:
    eid = entry["id"]
    if eid in SPECIAL:
        s = SPECIAL[eid]
        return s["definition_en"], s["definition_es"], s["fun_fact"]

    gloss_en = entry["english"].split(";")[0].strip()
    gloss_es = entry["spanish"].split(";")[0].strip()
    pos = entry.get("pos", "word")
    attested = entry.get("attested", False)
    tags = entry.get("tags", [])
    etym = entry.get("etymology")
    source = entry.get("source")

    pos_label = {
        "noun": "A noun meaning",
        "verb": "A verb meaning",
        "adjective": "An adjective meaning",
        "adverb": "An adverb meaning",
        "phrase": "A set phrase meaning",
        "particle": "A particle meaning",
        "pronoun": "A pronoun meaning",
        "numeral": "A number word for",
        "proper_noun": "A proper name for",
    }.get(pos, "A word meaning")

    pos_label_es = {
        "noun": "Sustantivo que significa",
        "verb": "Verbo que significa",
        "adjective": "Adjetivo que significa",
        "adverb": "Adverbio que significa",
        "phrase": "Frase hecha que significa",
        "particle": "Partícula que significa",
        "pronoun": "Pronombre que significa",
        "numeral": "Numeral para",
        "proper_noun": "Nombre propio para",
    }.get(pos, "Palabra que significa")

    status = "attested in colonial records" if attested else "used in Neo-Taíno / reconstruction teaching"
    status_es = "atestiguada en crónicas coloniales" if attested else "usada en enseñanza neo-taína / reconstrucción"

    extra = ""
    if etym:
        extra = f" Etymology: {etym}."
    elif "loanword_to_spanish" in tags:
        extra = " It entered Spanish (and often English) from Taíno."
    elif "spirit" in tags:
        extra = " It belongs to the spiritual and ceremonial vocabulary of the islands."
    elif "grammar" in tags:
        extra = " Learn the morphology—prefixes like da-/wa-/ma-/ka- unlock dozens of phrases."
    elif source:
        extra = f" Source label: {source}."

    definition_en = f"{pos_label} '{gloss_en}'. This form is {status}.{extra}".strip()
    definition_es = f"{pos_label_es} '{gloss_es}'. Forma {status_es}.{extra}".strip()

    fun_bits = []
    if "loanword_to_spanish" in tags:
        fun_bits.append(f"You already know a cousin of this word in Spanish: {gloss_es}.")
    if "food" in tags:
        fun_bits.append("Flavor unlock: say it before your next bite.")
    if "animal" in tags:
        fun_bits.append("Nature bingo: spot it, say it.")
    if "greeting" in tags or eid in {"taiguey", "tainoti"}:
        fun_bits.append("Use it on a friend today—greetings stick fastest.")
    if "grammar" in tags:
        fun_bits.append("Grammar combo move: swap the prefix and invent a new phrase.")
    if not fun_bits:
        if attested:
            fun_bits.append("High-confidence classic—flashcard gold.")
        else:
            fun_bits.append("Reconstruction practice—label it, learn it, keep it humble.")
    fun_fact = fun_bits[0]
    return definition_en, definition_es, fun_fact


def main() -> None:
    data = json.loads(VOCAB.read_text(encoding="utf-8"))
    for entry in data["entries"]:
        den, des, fun = make_definition(entry)
        entry["definition_en"] = den
        entry["definition_es"] = des
        entry["fun_fact"] = fun
        # short learner gloss kept as english/spanish; add example if missing
        if "example" not in entry:
            b = entry["boriken"]
            if entry.get("pos") == "phrase":
                entry["example"] = b
            elif entry["id"] in {"daka", "taino"}:
                entry["example"] = "Taíno daka"
            elif entry.get("pos") == "noun":
                entry["example"] = f"da-{b}" if not b.startswith(("da-", "wa-", "li-", "to-")) else b
            else:
                entry["example"] = b
    data["meta"]["version"] = "0.2.0"
    data["meta"]["has_definitions"] = True
    VOCAB.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Enriched {len(data['entries'])} entries with definitions + fun facts")


if __name__ == "__main__":
    main()
