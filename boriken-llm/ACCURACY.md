# BorikenLLM Accuracy Audit (2026-08)

This document records what is **reliable**, what is **learner Neo-Taíno**, and what was **corrected** so the iOS/desktop apps do not overclaim Classical Taíno certainty.

## Scope of the language

| Claim | Status in this toolkit |
|---|---|
| Classic Taíno is extinct; ISO code `tnq`; Ta-Arawakan family | **Supported** |
| Sparse colonial documentation (including about six spoken sentence strings commonly cited) | **Supported** |
| Possessive/verbal person prefixes `da-` / `wa-` / `li-` / `to~tu-` | **Supported** (colonial + comparative) |
| Negation `ma-`, attributive `ka-` | **Supported** as productive morphology in reconstructions |
| Default **SVO** word order | **Pedagogical Neo-Taíno only** — classical order is poorly attested (may have been freer / SOV-leaning) |
| Modern revival projects (Borikenaíki, Hiwatahia, community primers) invent and regularize forms | **Acknowledged** — labeled `attested=false` / `community_revival` |

## Colonial sentence anchors (high confidence)

Regularized forms of the six commonly cited colonial spoken strings remain in `corpus/sentences.json` with `attested=true`, including pieces such as:

- `daka` (I am)
- `waibá` (let’s go)
- `warikẽ` (we see)
- `kãma` (hear)
- `ahiyakawo` (speak to us)
- `mayani` / `makabuka` / `wamekina` / `sinato` / `teketa` / `warokoel`

These are the strongest **sentence-level** anchors in the corpus.

## Strong colonial / loan vocabulary (generally kept `attested=true`)

Widely accepted Taíno→European loans and ethnohistoric culture terms, e.g.:

`kanowa`, `hamaka`, `hurakán`, `barabakoa`, `kasabi`, `yuka`, `batata`, `aji`, `bohío`, `batey`, `areíto`, `kasike`, `semí`/`cemí`, `Atabey`, `Yúcahu`, `naboriya`, gold/jewelry terms from the colonial sentences (`kawóna`, `yari`), Spanish→Taíno adaptations `isúbara` / `isíbuse`.

Orthography here is **modern regularized** (Granberry-style), not a claim of exact colonial spelling.

## Corrections applied in v0.3.1

| Item | Problem | Fix |
|---|---|---|
| **Enriquillo / enrikíyo** | Spanish baptismal name of a cacique, not a Taíno lexeme | Relabeled to **Guarocuya** ethnohistoric note; `attested=false`; `not_taino_lexeme` |
| **ni, bara, turey, wey/guey, yukubiya** | Common in community primers; not colonial-sentence anchors | `attested=false`, `source=community_revival`, confidence **medium** |
| **tiburón, loro, manglar, quenepa, coquí, pitirre, cimarrón** | Caribbean Spanish and/or disputed etymology | `attested=false`, uncertain sources, confidence **low** |
| **toa** ‘mother’ | Often from names / comparative notes | Demoted to uncertain / medium |
| **oubao moín** | Chronicle phrase; gloss may be interpretive | Kept with medium confidence note |

## What “attested” means here

- **`attested=true`**: Documented in colonial Spanish records as Taíno/Antillean, **or** a widely accepted Taíno→Spanish/English loan used in this toolkit.
- **`attested=false`**: Neo-Taíno, community revival, comparative guess, disputed Caribbean Spanish etymology, or ethnohistoric name that is **not** a Taíno dictionary stem.

Always show the label in UI. Fun games must not hide attestation.

## Sources consulted (non-exhaustive)

- Wikipedia “Taíno language” overview (colonial sentences, morphology notes, ISO `tnq`)
- Granberry & Vescelius-style regularizations commonly used in revival pedagogy
- Ethnohistoric notes on Borinquen/Borikén (`bori` + land suffix)
- Scholarship cautioning that Caribbean language revival mixes ages and invents forms (e.g. discussions in Rybka/Sabogal; Faria on invention risks)
- Community dictionaries / primers (Borikenaíki, Hiwatahia–Hekexi, Living Dictionaries)—**used as revival evidence, not classical proof**

## API

`GET /v1/accuracy` returns live counts, disclaimer text, and flagged entry ids for clients.

## Honest product rule

**Prefer a smaller high-confidence core over a fluent fake language.** Reconstruct in the open; never upgrade a revival form to “attested” without a colonial citation.
