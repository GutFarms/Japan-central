#!/usr/bin/env python3
"""Apply lexicon accuracy corrections from the 2026 scholarly audit."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VOCAB = ROOT / "corpus" / "vocabulary.json"

# Colonial-sentence anchors and widely accepted Spanish loans from Taíno stay attested.
# Demotions below are for forms that circulate in revival lists / Caribbean Spanish
# without clear Classic Taíno colonial lexical attestation.

COMMUNITY_REVIVAL = {
    "ni": {
        "source": "community_revival",
        "confidence": "medium",
        "accuracy_note": (
            "Common in Neo-Taíno / community primers for 'water'. Not among the six "
            "best-attested colonial spoken sentences; treat as revival lexicon, not classical certainty."
        ),
        "definition_en": (
            "Water (Neo-Taíno / community form). Used widely in modern Boriken learning materials; "
            "not a high-certainty Classic Taíno colonial citation in this toolkit."
        ),
        "definition_es": (
            "Agua (forma neo-taína / comunitaria). Muy usada en materiales modernos de Boriken; "
            "no es una cita colonial clásica de alta certeza en este kit."
        ),
        "fun_fact": "Learners still practice ma-ni ('without water')—just remember the label is reconstructed.",
    },
    "bara": {
        "source": "community_revival",
        "confidence": "medium",
        "accuracy_note": (
            "Sea/ocean gloss circulates in community dictionaries; not anchored in the six colonial "
            "spoken sentences. Keep for learning; label as revival."
        ),
        "definition_en": "Sea or ocean in community Neo-Taíno usage—medium confidence, not classical proof.",
        "definition_es": "Mar u océano en uso neo-taíno comunitario: confianza media, no prueba clásica.",
        "fun_fact": "Pair with kanowa (canoe)—a firmly attested loan—when talking about the sea.",
    },
    "turey": {
        "source": "community_revival",
        "confidence": "medium",
        "accuracy_note": "Sky/heaven form common in revival materials; not a colonial-sentence anchor here.",
        "definition_en": "Sky or heaven in community Neo-Taíno materials (medium confidence).",
        "definition_es": "Cielo en materiales neo-taínos comunitarios (confianza media).",
        "fun_fact": "Use turey in poems and prayers—and keep the revival label visible.",
    },
    "guey": {
        "source": "community_revival",
        "confidence": "medium",
        "accuracy_note": (
            "wey/guey 'sun' is frequent in Neo-Taíno greetings (e.g. taí wey) but is not itself "
            "one of the six colonial spoken sentence anchors."
        ),
        "definition_en": "Sun in community Neo-Taíno (wey/guey). Medium confidence revival form.",
        "definition_es": "Sol en neo-taíno comunitario (wey/guey). Forma de revival de confianza media.",
        "fun_fact": "Taí wey ('good day') is a modern greeting built on this community form.",
    },
    "yukubia": {
        "source": "community_revival",
        "confidence": "medium",
        "accuracy_note": "Rain gloss appears in community lists; treat as revival, not colonial certainty.",
        "definition_en": "Rain in community Neo-Taíno usage (medium confidence).",
        "definition_es": "Lluvia en uso neo-taíno comunitario (confianza media).",
        "fun_fact": "Great weather vocabulary for learners—label it reconstructed.",
    },
}

CARIBBEAN_UNCERTAIN = {
    "tiburon": {
        "source": "caribbean_spanish_uncertain",
        "confidence": "low",
        "accuracy_note": (
            "Spanish tiburón 'shark'. Indigenous Antillean origin is sometimes claimed but not securely "
            "demonstrated as a Classic Taíno lexeme in this corpus."
        ),
        "definition_en": "Shark (Caribbean Spanish tiburón; Taíno origin uncertain—low confidence).",
        "definition_es": "Tiburón (español caribeño; origen taíno incierto—baja confianza).",
        "fun_fact": "Useful island vocabulary—don't present it as a proven colonial Taíno citation.",
        "english": "shark (Caribbean Spanish; Taíno origin uncertain)",
        "spanish": "tiburón (origen taíno incierto)",
    },
    "loro": {
        "source": "caribbean_spanish_uncertain",
        "confidence": "low",
        "accuracy_note": "Spanish loro 'parrot' (ultimately Latin/Romance). Not a Taíno lexeme.",
        "definition_en": "Parrot (Spanish loro—not a Taíno word; kept only as regional vocabulary caution).",
        "definition_es": "Loro (español—no es palabra taína; se conserva solo como aviso de vocabulario regional).",
        "fun_fact": "If you need a bird word for drills, prefer clearly labeled reconstructed forms.",
        "english": "parrot (Spanish; not Taíno)",
        "spanish": "loro (español; no taíno)",
        "tags_add": ["not_taino_lexeme", "accuracy_flag"],
    },
    "cimarron": {
        "source": "etymology_disputed",
        "confidence": "low",
        "accuracy_note": (
            "Spanish cimarrón. Sometimes linked to a Taíno form; etymology is disputed. Do not present "
            "simarón as a secure Classic Taíno citation."
        ),
        "definition_en": "Wild / runaway (Spanish cimarrón; possible Taíno link disputed—low confidence).",
        "definition_es": "Cimarrón / salvaje (vínculo taíno disputado—baja confianza).",
        "fun_fact": "Historically powerful word in the Caribbean—etymology remains debated.",
        "english": "wild; runaway (etymology disputed)",
    },
    "mangles": {
        "source": "caribbean_spanish_uncertain",
        "confidence": "low",
        "accuracy_note": "Mangrove (mangle/manglar): Caribbean Spanish; Indigenous etymology uncertain.",
        "definition_en": "Mangrove (Caribbean Spanish manglar; Indigenous origin uncertain).",
        "definition_es": "Manglar (español caribeño; origen indígena incierto).",
        "fun_fact": "Coastal ecology word—keep the uncertain label on.",
        "english": "mangrove (Caribbean Spanish; origin uncertain)",
    },
    "quenepa": {
        "source": "caribbean_spanish_uncertain",
        "confidence": "low",
        "accuracy_note": "Quenepa/kenepa is Caribbean Spanish for Spanish lime; Taíno attribution uncertain.",
        "definition_en": "Spanish lime / genip (Caribbean Spanish quenepa; Taíno link uncertain).",
        "definition_es": "Quenepa (español caribeño; vínculo taíno incierto).",
        "fun_fact": "Delicious island fruit—etymology still soft.",
        "english": "Spanish lime (Caribbean Spanish; Taíno link uncertain)",
    },
    "coqui": {
        "source": "caribbean_spanish_uncertain",
        "confidence": "low",
        "accuracy_note": (
            "Coquí is iconic Puerto Rican Spanish for the frog; often assumed Indigenous but not securely "
            "attested as Classic Taíno in colonial lexical lists used here."
        ),
        "definition_en": "Coquí frog (Puerto Rican Spanish; Indigenous etymology uncertain in this corpus).",
        "definition_es": "Rana coquí (español puertorriqueño; etimología indígena incierta en este corpus).",
        "fun_fact": "Island mascot energy—still label etymology as uncertain here.",
        "english": "coquí frog (Puerto Rican Spanish; etymology uncertain)",
    },
    "pitirre": {
        "source": "caribbean_spanish_uncertain",
        "confidence": "low",
        "accuracy_note": "Pitirre (gray kingbird) is Puerto Rican Spanish; Taíno origin not demonstrated here.",
        "definition_en": "Gray kingbird (Puerto Rican Spanish pitirre; Taíno origin not shown here).",
        "definition_es": "Pitirre (español puertorriqueño; origen taíno no demostrado aquí).",
        "fun_fact": "Fierce little bird—soft etymology label.",
        "english": "gray kingbird (Puerto Rican Spanish; origin uncertain)",
    },
}

ENRIQUILLO = {
    "boriken": "Guarocuya",
    "english": "Guarocuya (cacique; Spanish baptismal name Enriquillo)",
    "spanish": "Guarocuya (cacique; nombre de bautismo español Enriquillo)",
    "pos": "proper_noun",
    "attested": False,
    "source": "ethnohistoric_spanish_name",
    "confidence": "medium",
    "accuracy_note": (
        "Enriquillo / enrikíyo is a Spanish baptismal name, not a Taíno lexeme. Later tradition often "
        "gives the Indigenous name Guarocuya. Do not teach 'enrikíyo' as a Boriken vocabulary word."
    ),
    "tags": ["identity", "historical_note", "not_taino_lexeme", "accuracy_flag"],
    "etymology": "Spanish Enrique + diminutive; Indigenous name traditionally Guarocuya",
    "definition_en": (
        "Historical cacique of Hispaniola known in Spanish sources as Enriquillo (a baptismal name). "
        "His Indigenous name is often given as Guarocuya. This is ethnohistory, not a Taíno dictionary word."
    ),
    "definition_es": (
        "Cacique histórico de La Española conocido en fuentes españolas como Enriquillo (nombre de bautismo). "
        "Su nombre indígena suele darse como Guarocuya. Es etnohistoria, no un lexema taíno de diccionario."
    ),
    "fun_fact": "Teaching tip: say Guarocuya for the person; never list Enriquillo as a Taíno stem.",
    "example": "Guarocuya (Enriquillo)",
}


def apply() -> dict:
    data = json.loads(VOCAB.read_text(encoding="utf-8"))
    changed: list[str] = []

    data["meta"].update(
        {
            "version": "0.3.1",
            "accuracy_audit": "2026-08",
            "notes": (
                "attested=true means documented in colonial Spanish records as Taíno/Antillean, "
                "or a widely accepted Taíno→Spanish loan (hamaca, canoa, huracán, etc.). "
                "attested=false covers Neo-Taíno/community revival forms, comparative reconstructions, "
                "disputed Caribbean Spanish etymologies, and ethnohistoric names that are not Taíno lexemes. "
                "Classic Taíno is extinct (ISO tnq); revival mixes ages and invents forms—see ACCURACY.md. "
                "Learner SVO is pedagogical Neo-Taíno, not proven classical word order."
            ),
            "disclaimer": (
                "This toolkit prefers colonial anchors and labels uncertainty. It is not a claim that "
                "reconstructed forms are Classical Taíno."
            ),
        }
    )

    for entry in data["entries"]:
        eid = entry["id"]

        if eid == "enriquillo":
            entry.update(ENRIQUILLO)
            changed.append(eid)
            continue

        if eid in COMMUNITY_REVIVAL:
            patch = COMMUNITY_REVIVAL[eid]
            entry["attested"] = False
            entry["source"] = patch["source"]
            entry["confidence"] = patch["confidence"]
            entry["accuracy_note"] = patch["accuracy_note"]
            entry["definition_en"] = patch["definition_en"]
            entry["definition_es"] = patch["definition_es"]
            entry["fun_fact"] = patch["fun_fact"]
            tags = list(entry.get("tags") or [])
            if "accuracy_flag" not in tags:
                tags.append("accuracy_flag")
            if "community_revival" not in tags:
                tags.append("community_revival")
            entry["tags"] = tags
            changed.append(eid)
            continue

        if eid in CARIBBEAN_UNCERTAIN:
            patch = CARIBBEAN_UNCERTAIN[eid]
            entry["attested"] = False
            entry["source"] = patch["source"]
            entry["confidence"] = patch["confidence"]
            entry["accuracy_note"] = patch["accuracy_note"]
            entry["definition_en"] = patch["definition_en"]
            entry["definition_es"] = patch["definition_es"]
            entry["fun_fact"] = patch["fun_fact"]
            if "english" in patch:
                entry["english"] = patch["english"]
            if "spanish" in patch:
                entry["spanish"] = patch["spanish"]
            tags = list(entry.get("tags") or [])
            for t in ["accuracy_flag", "etymology_uncertain", *(patch.get("tags_add") or [])]:
                if t not in tags:
                    tags.append(t)
            entry["tags"] = tags
            changed.append(eid)

        # Soft notes on solid loans / ethnohistoric religion terms
        if eid in {"atabey", "yocahu", "zemi", "areito", "batey", "bohio", "behique", "caney"}:
            entry.setdefault(
                "accuracy_note",
                "Ethnohistoric / colonial lexical item widely cited for Taíno culture; orthography regularized.",
            )
            entry.setdefault("confidence", "high")

        if eid in {"isubara", "isibuse"}:
            entry["accuracy_note"] = (
                "Attested colonial-era adaptation of a Spanish loan into Taíno phonology "
                "(espada→isúbara, espejo→isíbuse)."
            )
            entry["confidence"] = "high"

        if eid == "island_blood":
            entry["accuracy_note"] = (
                "Phrase reported in Spanish chronicles for Hispaniola naming traditions; "
                "gloss 'island of blood' follows common modern rendering and may be interpretive."
            )
            entry["confidence"] = "medium"

        if eid == "toa":
            entry["accuracy_note"] = (
                "Mother root often cited from personal names / comparative notes; "
                "treat as medium unless citing a specific colonial passage."
            )
            entry["confidence"] = "medium"
            entry["attested"] = False
            entry["source"] = entry.get("source") or "name_element_uncertain"
            if eid not in changed:
                changed.append(eid)

    VOCAB.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    attested = sum(1 for e in data["entries"] if e.get("attested"))
    return {
        "changed": changed,
        "total": len(data["entries"]),
        "attested": attested,
        "reconstructed": len(data["entries"]) - attested,
    }


if __name__ == "__main__":
    summary = apply()
    print(json.dumps(summary, indent=2))
