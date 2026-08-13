use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Lexeme {
    pub id: String,
    pub boriken: String,
    pub english: String,
    pub spanish: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub attested: bool,
    #[serde(default)]
    pub definition_en: String,
    #[serde(default)]
    pub definition_es: String,
    #[serde(default)]
    pub fun_fact: String,
    #[serde(default)]
    pub example: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VocabFile {
    entries: Vec<Lexeme>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HistoryQuiz {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub xp: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryEra {
    pub id: String,
    #[serde(default)]
    pub chapter: i32,
    #[serde(default)]
    pub era_label: String,
    #[serde(default)]
    pub when: String,
    #[serde(default)]
    pub title_en: String,
    #[serde(default)]
    pub story_en: String,
    #[serde(default)]
    pub fun_hook: String,
    #[serde(default)]
    pub honesty: String,
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub quiz: HistoryQuiz,
}

#[derive(Debug, Deserialize)]
struct HistoryFile {
    #[serde(default)]
    eras: Vec<HistoryEra>,
}

fn read_candidates(rel: &str) -> Option<String> {
    let candidates = [
        PathBuf::from(rel),
        PathBuf::from("..").join(rel),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rel),
    ];
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Some(text);
        }
    }
    None
}

pub fn load_lexicon() -> Vec<Lexeme> {
    if let Some(text) = read_candidates("corpus/vocabulary.json") {
        if let Ok(v) = serde_json::from_str::<VocabFile>(&text) {
            return v.entries;
        }
    }
    let embedded = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../corpus/vocabulary.json"
    ));
    serde_json::from_str::<VocabFile>(embedded)
        .map(|v| v.entries)
        .unwrap_or_default()
}

pub fn load_history() -> Vec<HistoryEra> {
    if let Some(text) = read_candidates("corpus/history.json") {
        if let Ok(v) = serde_json::from_str::<HistoryFile>(&text) {
            return v.eras;
        }
    }
    let embedded = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../corpus/history.json"
    ));
    serde_json::from_str::<HistoryFile>(embedded)
        .map(|v| v.eras)
        .unwrap_or_default()
}

impl Lexeme {
    pub fn gloss_en(&self) -> &str {
        self.english.split(';').next().unwrap_or(&self.english).trim()
    }
}
