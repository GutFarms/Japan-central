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
                "definition_en": e.get("definition_en", ""),
                "definition_es": e.get("definition_es", ""),
                "fun_fact": e.get("fun_fact", ""),
                "example": e.get("example", e["boriken"]),
                "tags": e.get("tags", []),
            }
        )

    payload = json.dumps(entries, ensure_ascii=False)
    html = HTML_TEMPLATE.replace("__LEXICON_JSON__", payload)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    OUT_HTML.write_text(html, encoding="utf-8")
    (OUT_DIR / "README.txt").write_text(
        "Boriken Offline Learner\n"
        "======================\n\n"
        "Open index.html in any browser (iPhone Safari works).\n"
        "On iOS: Share → Add to Home Screen for an app-like icon.\n"
        "No server required — lexicon and games are embedded.\n",
        encoding="utf-8",
    )
    print(f"Wrote {OUT_HTML} with {len(entries)} words")


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
    </div>

    <div class="panel" id="stage">
      <p class="muted">Taí wey — pick a game. Everything works offline.</p>
    </div>

    <div class="panel hidden" id="searchPanel">
      <input id="q" placeholder="Search English / Spanish / Boriken" oninput="search()"/>
      <div id="searchOut"></div>
    </div>

    <footer>
      Attested forms preferred. Reconstructions are labeled. Made for iOS &amp; desktop download.
    </footer>
  </div>

<script>
const LEXICON = __LEXICON_JSON__;
const state = {
  xp: Number(localStorage.getItem("boriken_xp") || 0),
  streak: Number(localStorage.getItem("boriken_streak") || 1),
  match: null,
  flashIdx: 0,
  flashCards: [],
  storyIdx: 0,
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
  return `<div class="boriken">${w.boriken}</div>
    <div>${w.english} · <span class="muted">${w.spanish}</span></div>
    <p>${w.definition_en || ""}</p>
    <p class="gold">✦ ${w.fun_fact || ""}</p>
    <p class="muted">Example: ${w.example || w.boriken} ${w.attested ? "· attested" : "· reconstructed"}</p>`;
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
    ${b.choices.map(c => `<button class="choice" onclick="storyPick('${c.replace(/'/g, "\\'")}', '${b.answer.replace(/'/g, "\\'")}')">${c}</button>`).join("")}`);
}

function storyPick(choice, answer) {
  const ok = choice === answer;
  addXp(ok ? 10 : 2);
  state.storyIdx += 1;
  stage(`<p class="${ok ? "ok" : "bad"}">${ok ? "Kasike energy unlocked." : "Remember: " + answer}</p>
    <button class="primary" onclick="showStory()">Continue</button>`);
}
</script>
</body>
</html>
"""


if __name__ == "__main__":
    main()
