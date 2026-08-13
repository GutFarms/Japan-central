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

pub fn load_lexicon() -> Vec<Lexeme> {
    // Prefer nearby corpus (packaged / run from desktop/), else embedded.
    let candidates = [
        PathBuf::from("corpus/vocabulary.json"),
        PathBuf::from("../corpus/vocabulary.json"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus/vocabulary.json"),
    ];
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<VocabFile>(&text) {
                return v.entries;
            }
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

impl Lexeme {
    pub fn gloss_en(&self) -> &str {
        self.english.split(';').next().unwrap_or(&self.english).trim()
    }
}
