#!/usr/bin/env python3
"""Build a self-contained offline Boriken learner (HTML+JS) for download."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VOCAB = ROOT / "corpus" / "vocabulary.json"
OUT_DIR = ROOT / "webapp"
OUT_HTML = OUT_DIR / "index.html"


def main() -> None:
    data = json.loads(VOCAB.read_text(encoding="utf-8"))
    grammar = json.loads((ROOT / "corpus" / "grammar.json").read_text(encoding="utf-8"))
    sentences = json.loads((ROOT / "corpus" / "sentences.json").read_text(encoding="utf-8"))
    entries = []
    for e in data["entries"]:
        if e.get("pos") in {"unknown"}:
            continue
        entries.append(
            {
                "id": e["id"],
                "boriken": e["boriken"],
                "english": e["english"],
                "spanish": e["spanish"],
                "pos": e.get("pos", ""),
                "attested": bool(e.get("attested")),
                "confidence": e.get("confidence")
                or ("high" if e.get("attested") else "medium"),
                "source": e.get("source"),
                "accuracy_note": e.get("accuracy_note"),
                "definition_en": e.get("definition_en", ""),
                "definition_es": e.get("definition_es", ""),
                "fun_fact": e.get("fun_fact", ""),
                "example": e.get("example", e["boriken"]),
                "tags": e.get("tags", []),
            }
        )

    patterns = grammar.get("sentence_patterns", [])
    learner = sentences.get("learner_sentences", [])
    history = json.loads((ROOT / "corpus" / "history.json").read_text(encoding="utf-8"))
    payload = json.dumps(entries, ensure_ascii=False)
    patterns_json = json.dumps(patterns, ensure_ascii=False)
    learner_json = json.dumps(learner, ensure_ascii=False)
    overview = json.dumps(grammar.get("sentence_structure", {}).get("overview", {}), ensure_ascii=False)
    history_json = json.dumps(history, ensure_ascii=False)
    accuracy_banner = json.dumps(
        {
            "version": data.get("meta", {}).get("version"),
            "disclaimer": data.get("meta", {}).get("disclaimer")
            or "Prefer colonial anchors; Neo-Taíno is labeled.",
        },
        ensure_ascii=False,
    )
    html = (
        HTML_TEMPLATE.replace("__LEXICON_JSON__", payload)
        .replace("__PATTERNS_JSON__", patterns_json)
        .replace("__LEARNER_JSON__", learner_json)
        .replace("__OVERVIEW_JSON__", overview)
        .replace("__ACCURACY_JSON__", accuracy_banner)
        .replace("__HISTORY_JSON__", history_json)
    )
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    OUT_HTML.write_text(html, encoding="utf-8")
    (OUT_DIR / "README.txt").write_text(
        "Boriken Offline Learner\n"
        "======================\n\n"
        "Open index.html in any browser (iPhone Safari works).\n"
        "On iOS: Share → Add to Home Screen for an app-like icon.\n"
        "Includes Island Time Machine (fun history), sentence structure, definitions, and games.\n"
        "No server required — lexicon and games are embedded.\n",
        encoding="utf-8",
    )
    print(f"Wrote {OUT_HTML} with {len(entries)} words and {len(patterns)} sentence patterns")


HTML_TEMPLATE = r"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
<meta name="apple-mobile-web-app-capable" content="yes"/>
<meta name="apple-mobile-web-app-title" content="Borikén"/>
<meta name="theme-color" content="#0b4a3f"/>
<title>Borikén — Learn Offline</title>
<style>
  :root {
    --bg1: #072820;
    --bg2: #0f6a58;
    --gold: #f2c75a;
    --ink: #f7fff8;
    --muted: rgba(247,255,248,.82);
    --panel: rgba(0,0,0,.28);
    --line: rgba(242,199,90,.35);
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    min-height: 100vh;
    font-family: Georgia, "Times New Roman", serif;
    color: var(--ink);
    background:
      radial-gradient(1200px 600px at 10% -10%, rgba(242,199,90,.25), transparent 60%),
      linear-gradient(145deg, var(--bg1), var(--bg2) 55%, #d9a83a);
  }
  .wrap { max-width: 720px; margin: 0 auto; padding: 22px 18px 48px; }
  h1 {
    font-size: clamp(2.2rem, 8vw, 3.4rem);
    letter-spacing: .18em;
    margin: 0 0 .2rem;
  }
  .sub { color: var(--muted); margin-bottom: 1rem; }
  .hud {
    display: flex; gap: 10px; flex-wrap: wrap; margin-bottom: 14px;
  }
  .chip {
    background: var(--panel); border: 1px solid var(--line);
    padding: .45rem .7rem; font-size: .9rem;
  }
  .panel {
    background: var(--panel); border: 1px solid var(--line);
    padding: 14px; margin: 12px 0;
  }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  button, .btn {
    appearance: none; border: 0; cursor: pointer;
    background: rgba(255,255,255,.14); color: var(--ink);
    padding: .85rem .9rem; font: inherit; width: 100%;
    border: 1px solid rgba(255,255,255,.18);
  }
  button.primary { background: var(--gold); color: #152015; font-weight: 700; }
  button.choice { text-align: left; margin-top: 8px; }
  button:active { transform: scale(.98); }
  input {
    width: 100%; padding: .8rem; border: 0; font: inherit;
    margin-bottom: 10px;
  }
  .boriken { font-size: 1.8rem; font-weight: 700; margin: .2rem 0; }
  .gold { color: var(--gold); }
  .muted { color: var(--muted); font-size: .95rem; }
  .hidden { display: none; }
  .ok { color: #b7f7c8; }
  .bad { color: #ffd0c8; }
  footer { margin-top: 18px; color: var(--muted); font-size: .85rem; }
</style>
</head>
<body>
  <div class="wrap">
    <h1>BORIKÉN</h1>
    <p class="sub">Offline learner — rebuild the language of the native land.</p>
    <p class="muted" id="accuracyBanner"></p>
    <div class="hud">
      <div class="chip">XP <strong id="xp">0</strong></div>
      <div class="chip">Streak <strong id="streak">1</strong></div>
      <div class="chip" id="level">Konuko Seedling</div>
    </div>

    <div class="grid" id="menu">
      <button class="primary" onclick="wordOfDay()">Word of Day</button>
      <button onclick="startMatch()">Batey Match</button>
      <button onclick="flash()">Memory Flip</button>
      <button onclick="fillBlank()">Konuko Fill</button>
      <button onclick="showSearch()">Define</button>
      <button onclick="story()">Areyto Quest</button>
      <button onclick="historyStart()">Time Machine</button>
      <button onclick="showStructure()">Sentences</button>
      <button onclick="scramble()">Scramble</button>
    </div>

    <div class="panel" id="stage">
      <p class="muted">Taí wey — pick a game. Everything works offline.</p>
    </div>

    <div class="panel hidden" id="searchPanel">
      <input id="q" placeholder="Search English / Spanish / Boriken" oninput="search()"/>
      <div id="searchOut"></div>
    </div>

    <footer>
      Attested colonial forms preferred. Revival / uncertain etymologies stay labeled. See ACCURACY.md.
    </footer>
  </div>

<script>
const LEXICON = __LEXICON_JSON__;
const PATTERNS = __PATTERNS_JSON__;
const LEARNER = __LEARNER_JSON__;
const OVERVIEW = __OVERVIEW_JSON__;
const ACCURACY = __ACCURACY_JSON__;
const HISTORY = __HISTORY_JSON__;
const state = {
  xp: Number(localStorage.getItem("boriken_xp") || 0),
  streak: Number(localStorage.getItem("boriken_streak") || 1),
  match: null,
  flashIdx: 0,
  flashCards: [],
  storyIdx: 0,
  scramble: null,
  historyIdx: 0,
  histQ: null,
};

function save() {
  localStorage.setItem("boriken_xp", state.xp);
  localStorage.setItem("boriken_streak", state.streak);
  document.getElementById("xp").textContent = state.xp;
  document.getElementById("streak").textContent = state.streak;
  const level = 1 + Math.floor(state.xp / 50);
  const titles = {1:"Konuko Seedling",2:"Bohío Builder",3:"Batey Player",4:"Areyto Voice",5:"Kasike Apprentice"};
  document.getElementById("level").textContent = titles[Math.min(level,5)] || "Island Elder";
}
save();
const banner = document.getElementById("accuracyBanner");
if (banner && ACCURACY) {
  banner.textContent = `Accuracy ${ACCURACY.version || ""} — ${ACCURACY.disclaimer || ""}`;
}

function stage(html) {
  document.getElementById("searchPanel").classList.add("hidden");
  document.getElementById("stage").innerHTML = html;
}

function addXp(n) { state.xp += n; save(); }

function pick(n=1, filter=null) {
  let pool = LEXICON.slice();
  if (filter) pool = pool.filter(filter);
  for (let i = pool.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [pool[i], pool[j]] = [pool[j], pool[i]];
  }
  return pool.slice(0, n);
}

function wordCard(w) {
  const label = w.attested
    ? "· attested"
    : `· reconstructed${w.confidence ? " · " + w.confidence : ""}`;
  const note = w.accuracy_note
    ? `<p class="muted">Accuracy: ${w.accuracy_note}</p>`
    : "";
  return `<div class="boriken">${w.boriken}</div>
    <div>${w.english} · <span class="muted">${w.spanish}</span></div>
    <p>${w.definition_en || ""}</p>
    <p class="gold">✦ ${w.fun_fact || ""}</p>
    <p class="muted">Example: ${w.example || w.boriken} ${label}</p>
    ${note}`;
}

function wordOfDay() {
  const day = new Date().toISOString().slice(0,10);
  let h = 0; for (const c of day) h = (h * 33 + c.charCodeAt(0)) >>> 0;
  const w = LEXICON[h % LEXICON.length];
  addXp(15);
  stage(`<h3>Island Word of the Day</h3>${wordCard(w)}
    <p class="ok">+15 XP — use <strong>${w.boriken}</strong> once today.</p>`);
}

function startMatch() {
  const words = pick(4, w => w.pos === "noun" || w.pos === "verb");
  const answer = words[0];
  const choices = words.map(w => w.english.split(";")[0].trim());
  for (let i = choices.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [choices[i], choices[j]] = [choices[j], choices[i]];
  }
  state.match = { answer: answer.english.split(";")[0].trim(), word: answer.boriken };
  stage(`<h3>Batey Match</h3>
    <div class="boriken">${answer.boriken}</div>
    <p class="muted">What does it mean?</p>
    ${choices.map(c => `<button class="choice" onclick="gradeMatch('${c.replace(/'/g, "\\'")}')">${c}</button>`).join("")}
    <p class="gold">✦ ${answer.fun_fact || ""}</p>`);
}

function gradeMatch(choice) {
  const ok = choice === state.match.answer;
  addXp(ok ? 8 : 1);
  stage(`<h3>${ok ? "The batey cheers for you." : "Almost — try again soon."}</h3>
    <p class="${ok ? "ok" : "bad"}">${state.match.word} = ${state.match.answer}</p>
    <button class="primary" onclick="startMatch()">Next match</button>`);
}

function flash() {
  state.flashCards = pick(8);
  state.flashIdx = 0;
  showFlash(false);
}

function showFlash(revealed) {
  const w = state.flashCards[state.flashIdx];
  if (!w) { stage("<p>Deck complete. Waibá!</p>"); return; }
  stage(`<h3>Memory Bohío (${state.flashIdx+1}/${state.flashCards.length})</h3>
    <div class="boriken">${revealed ? w.boriken : w.english.split(";")[0]}</div>
    ${revealed ? wordCard(w) : '<p class="muted">Say the Boriken form, then flip.</p>'}
    <div class="grid" style="margin-top:10px">
      <button onclick="showFlash(true)">Flip</button>
      <button class="primary" onclick="(addXp(5), state.flashIdx++, showFlash(false))">Got it</button>
    </div>`);
}

function fillBlank() {
  const w = pick(1, x => x.example && x.example.includes(x.boriken))[0] || pick(1)[0];
  const prompt = (w.example || w.boriken).replace(w.boriken, "____");
  stage(`<h3>Konuko Fill-In</h3>
    <p class="boriken">${prompt}</p>
    <p class="muted">${w.definition_en.split(".")[0]}.</p>
    <input id="fill" placeholder="type Boriken word"/>
    <button class="primary" onclick="checkFill('${w.boriken.replace(/'/g, "\\'")}')">Plant it</button>`);
}

function checkFill(answer) {
  const val = (document.getElementById("fill").value || "").trim().toLowerCase();
  const ok = val === answer.toLowerCase();
  addXp(ok ? 8 : 1);
  stage(`<p class="${ok ? "ok" : "bad"}">${ok ? "Root takes hold." : "Answer: " + answer}</p>
    <button class="primary" onclick="fillBlank()">Another</button>`);
}

function showSearch() {
  document.getElementById("searchPanel").classList.remove("hidden");
  stage('<p class="muted">Search the lexicon below.</p>');
  search();
}

function search() {
  const q = (document.getElementById("q").value || "").toLowerCase().trim();
  const hits = !q ? pick(5) : LEXICON.filter(w =>
    (w.boriken + w.english + w.spanish + w.definition_en).toLowerCase().includes(q)
  ).slice(0, 8);
  document.getElementById("searchOut").innerHTML = hits.map(w =>
    `<div class="panel" style="margin:8px 0">${wordCard(w)}</div>`
  ).join("") || "<p>No matches.</p>";
}

const STORY = [
  { scene: "Dawn", text: "The wey climbs. A coquí calls.", prompt: "Greet the morning", answer: "Taí wey", choices: ["Taí wey", "Makabuka"] },
  { scene: "Konuko", text: "You grate yuka for bread.", prompt: "Name the bread", answer: "kasabi", choices: ["kasabi", "hamaka"] },
  { scene: "Batey", text: "Drums for an areyto. Introduce yourself.", prompt: "I am Taíno", answer: "Taíno daka", choices: ["Taíno daka", "Waibá ni"] },
  { scene: "Path", text: "Friends wait by the kanowa.", prompt: "Rally the crew", answer: "Waibá", choices: ["Waibá", "Ua"] },
];

function story() {
  state.storyIdx = 0;
  showStory();
}

function showStory() {
  const b = STORY[state.storyIdx];
  if (!b) {
    addXp(20);
    stage("<h3>Areyto complete</h3><p class='ok'>+20 XP — you walked a morning in Borikén.</p>");
    return;
  }
  stage(`<h3>Areyto Quest · ${b.scene}</h3>
    <p>${b.text}</p>
    <p class="muted">${b.prompt}</p>
    ${b.choices.map((c, i) => `<button class="choice" onclick="storyPickIdx(${i})">${escapeHtml(c)}</button>`).join("")}`);
}

function storyPickIdx(i) {
  const b = STORY[state.storyIdx];
  if (!b) return;
  storyPick(b.choices[i], b.answer);
}

function storyPick(choice, answer) {
  const ok = choice === answer;
  addXp(ok ? 10 : 2);
  state.storyIdx += 1;
  stage(`<p class="${ok ? "ok" : "bad"}">${ok ? "Kasike energy unlocked." : "Remember: " + escapeHtml(answer)}</p>
    <button class="primary" onclick="showStory()">Continue</button>`);
}

function showStructure() {
  const ov = OVERVIEW.english || "Neo-Taíno Boriken usually follows Subject–Verb–Object (SVO).";
  const rows = PATTERNS.map(p =>
    `<div class="panel" style="margin:8px 0">
      <strong class="gold">${p.name || p.id}</strong>
      <div class="boriken" style="font-size:1.3rem">${p.example}</div>
      <div>${p.gloss || ""} · <span class="muted">${p.structure || ""}</span></div>
      <div class="muted">${p.pattern || ""}</div>
    </div>`
  ).join("");
  stage(`<h3>Sentence structure</h3>
    <p>${ov}</p>
    <p class="gold">da- I/my · wa- we/our · li- he/his · to- she/her · ma- without · ka- having</p>
    ${rows}
    <button class="primary" onclick="scramble()">Practice scramble</button>`);
}

function scramble() {
  const pool = LEARNER.filter(s => (s.boriken || "").replace(/-/g," ").trim().split(/\s+/).length >= 2);
  const s = pool[Math.floor(Math.random() * pool.length)];
  let tokens = s.boriken.replace(/,/g," ").split(/\s+/).filter(Boolean);
  let shuffled = tokens.slice().sort(() => Math.random() - 0.5);
  if (shuffled.join(" ") === tokens.join(" ")) shuffled = tokens.slice().reverse();
  state.scramble = { answer: s.boriken, picked: [] };
  stage(`<h3>Sentence Scramble</h3>
    <p class="muted">${s.english} · ${s.structure || ""}</p>
    <p id="built" class="boriken">____</p>
    <div id="toks">${shuffled.map(t =>
      `<button class="choice" onclick="pickTok('${t.replace(/'/g, "\\'")}')">${t}</button>`
    ).join("")}</div>
    <button onclick="scramble()">Skip</button>
    <button class="primary" onclick="checkScramble()">Check</button>`);
}

function pickTok(t) {
  if (!state.scramble) return;
  state.scramble.picked.push(t);
  document.getElementById("built").textContent = state.scramble.picked.join(" ");
}

function checkScramble() {
  if (!state.scramble) return;
  const got = state.scramble.picked.join(" ");
  const ok = got === state.scramble.answer;
  addXp(ok ? 10 : 1);
  stage(`<p class="${ok ? "ok" : "bad"}">${ok ? "Word order restored." : "Answer: " + state.scramble.answer}</p>
    <button class="primary" onclick="scramble()">Another</button>
    <button onclick="showStructure()">See structures</button>`);
}

function historyStart() {
  state.historyIdx = 0;
  renderHistory();
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function renderHistory() {
  const eras = HISTORY.eras || [];
  if (state.historyIdx >= eras.length) {
    addXp(20);
    stage(`<h3>Island Time Machine · Complete</h3>
      <p class="ok">+20 XP — Island Chronist energy. You walked Borikén's areyto of ages.</p>
      <p class="muted">${(HISTORY.meta && HISTORY.meta.disclaimer_en) || ""}</p>
      <button class="primary" onclick="historyStart()">Replay</button>`);
    return;
  }
  const e = eras[state.historyIdx];
  const words = (e.words || []).map(id => {
    const w = LEXICON.find(x => x.id === id);
    return w ? `<span class="chip">${escapeHtml(w.boriken)}</span>` : "";
  }).join(" ");
  const q = e.quiz || {};
  const choices = (q.choices || []).slice().sort(() => Math.random() - 0.5);
  state.histQ = { choices, answer: q.answer, xp: q.xp || 10 };
  stage(`<h3>Time Machine · Ch. ${e.chapter}</h3>
    <p class="gold">${escapeHtml(e.era_label)} · ${escapeHtml(e.when)}</p>
    <div class="boriken">${escapeHtml(e.title_en)}</div>
    <p>${escapeHtml(e.story_en)}</p>
    <p class="gold">✦ ${escapeHtml(e.fun_hook || "")}</p>
    <p>${words}</p>
    <p class="muted">${escapeHtml(e.honesty || "")}</p>
    <p><strong>${escapeHtml(q.question || "")}</strong></p>
    ${choices.map((c, i) => `<button class="choice" onclick="historyPickIdx(${i})">${escapeHtml(c)}</button>`).join("")}`);
}

function historyPickIdx(i) {
  const q = state.histQ || { choices: [], answer: "", xp: 10 };
  historyPick(q.choices[i], q.answer, q.xp);
}

function historyPick(choice, answer, xp) {
  const ok = choice === answer;
  addXp(ok ? xp : 1);
  state.historyIdx += 1;
  stage(`<p class="${ok ? "ok" : "bad"}">${ok ? "Timeline unlocked." : "True beat: " + escapeHtml(answer)}</p>
    <button class="primary" onclick="renderHistory()">Next chapter</button>`);
}
</script>
</body>
</html>
"""


if __name__ == "__main__":
    main()
