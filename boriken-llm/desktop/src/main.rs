mod corpus;
mod scene;

use corpus::{load_lexicon, Lexeme};
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Key, RichText, Sense};
use rand::seq::SliceRandom;
use rand::Rng;
use scene::{brand_title, glass_panel, Scene};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Home,
    WordOfDay,
    Match,
    Flash,
    Fill,
    Define,
    Story,
}

#[derive(Clone)]
struct MatchState {
    word: Lexeme,
    choices: Vec<String>,
    answer: String,
    feedback: Option<(bool, String)>,
}

struct FlashState {
    cards: Vec<Lexeme>,
    idx: usize,
    revealed: bool,
}

#[derive(Clone)]
struct FillState {
    word: Lexeme,
    prompt: String,
    typed: String,
    feedback: Option<(bool, String)>,
}

struct StoryBeat {
    scene: &'static str,
    text: &'static str,
    prompt: &'static str,
    answer: &'static str,
    choices: [&'static str; 2],
}

struct StoryState {
    idx: usize,
    feedback: Option<(bool, String)>,
}

struct Confetti {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: Color32,
    life: f32,
}

pub struct BorikenApp {
    lexicon: Vec<Lexeme>,
    scene: Scene,
    mode: Mode,
    xp: i32,
    streak: i32,
    query: String,
    define_hits: Vec<Lexeme>,
    spotlight: Option<Lexeme>,
    match_state: Option<MatchState>,
    flash: Option<FlashState>,
    fill: Option<FillState>,
    story: StoryState,
    confetti: Vec<Confetti>,
    last: Instant,
    status: String,
    fonts_ready: bool,
}

impl Default for BorikenApp {
    fn default() -> Self {
        let lexicon = load_lexicon();
        Self {
            lexicon,
            scene: Scene::new(1280.0, 800.0),
            mode: Mode::Home,
            xp: 0,
            streak: 1,
            query: "hurricane".into(),
            define_hits: Vec::new(),
            spotlight: None,
            match_state: None,
            flash: None,
            fill: None,
            story: StoryState {
                idx: 0,
                feedback: None,
            },
            confetti: Vec::new(),
            last: Instant::now(),
            status: "Taí wey — welcome to the high-seas classroom.".into(),
            fonts_ready: false,
        }
    }
}

impl BorikenApp {
    fn setup_fonts(&mut self, ctx: &egui::Context) {
        if self.fonts_ready {
            return;
        }
        let mut fonts = FontDefinitions::default();
        // Prefer expressive serif (Tinos) for brand-first typography
        if let Ok(data) = std::fs::read("/usr/share/fonts/truetype/croscore/Tinos-Regular.ttf") {
            fonts
                .font_data
                .insert("boriken_serif".into(), FontData::from_owned(data));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "boriken_serif".into());
        }
        if let Ok(data) = std::fs::read("/usr/share/fonts/truetype/croscore/Tinos-Bold.ttf") {
            fonts
                .font_data
                .insert("boriken_serif_bold".into(), FontData::from_owned(data));
            fonts
                .families
                .entry(FontFamily::Name("bold".into()))
                .or_default()
                .insert(0, "boriken_serif_bold".into());
        }
        ctx.set_fonts(fonts);
        self.fonts_ready = true;
    }

    fn add_xp(&mut self, n: i32) {
        self.xp += n;
    }

    fn level_title(&self) -> &'static str {
        let level = 1 + self.xp / 50;
        match level {
            1 => "Konuko Seedling",
            2 => "Bohío Builder",
            3 => "Batey Player",
            4 => "Areyto Voice",
            5 => "Kasike Apprentice",
            _ => "Island Elder",
        }
    }

    fn burst(&mut self, origin_x: f32, origin_y: f32) {
        let mut rng = rand::thread_rng();
        for _ in 0..48 {
            let ang = rng.gen_range(0.0..std::f32::consts::TAU);
            let spd = rng.gen_range(40.0..180.0);
            self.confetti.push(Confetti {
                x: origin_x,
                y: origin_y,
                vx: ang.cos() * spd,
                vy: ang.sin() * spd - 40.0,
                color: Color32::from_rgb(
                    rng.gen_range(180..255),
                    rng.gen_range(140..230),
                    rng.gen_range(60..140),
                ),
                life: rng.gen_range(0.6..1.3),
            });
        }
    }

    fn start_match(&mut self) {
        let mut rng = rand::thread_rng();
        let mut pool: Vec<_> = self
            .lexicon
            .iter()
            .filter(|w| w.pos == "noun" || w.pos == "verb")
            .cloned()
            .collect();
        pool.shuffle(&mut rng);
        let word = pool.first().cloned().unwrap_or_else(|| self.lexicon[0].clone());
        let mut choices: Vec<String> = pool.iter().take(4).map(|w| w.gloss_en().to_string()).collect();
        while choices.len() < 4 {
            choices.push("—".into());
        }
        choices.shuffle(&mut rng);
        let answer = word.gloss_en().to_string();
        self.match_state = Some(MatchState {
            word,
            choices,
            answer,
            feedback: None,
        });
        self.mode = Mode::Match;
        self.status = "Batey Match — trust your ear, then your eye.".into();
    }

    fn start_flash(&mut self) {
        let mut rng = rand::thread_rng();
        let mut cards = self.lexicon.clone();
        cards.shuffle(&mut rng);
        cards.truncate(10);
        self.flash = Some(FlashState {
            cards,
            idx: 0,
            revealed: false,
        });
        self.mode = Mode::Flash;
        self.status = "Memory Bohío — speak it before you flip it.".into();
    }

    fn start_fill(&mut self) {
        let mut rng = rand::thread_rng();
        let mut pool: Vec<_> = self
            .lexicon
            .iter()
            .filter(|w| !w.example.is_empty() && w.example.contains(&w.boriken))
            .cloned()
            .collect();
        if pool.is_empty() {
            pool = self.lexicon.clone();
        }
        pool.shuffle(&mut rng);
        let word = pool[0].clone();
        let prompt = word.example.replace(&word.boriken, "____");
        self.fill = Some(FillState {
            word,
            prompt,
            typed: String::new(),
            feedback: None,
        });
        self.mode = Mode::Fill;
        self.status = "Konuko Fill-In — plant the missing word.".into();
    }

    fn word_of_day(&mut self) {
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut h: u32 = 0;
        for b in day.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u32);
        }
        let idx = (h as usize) % self.lexicon.len().max(1);
        self.spotlight = self.lexicon.get(idx).cloned();
        self.add_xp(15);
        self.mode = Mode::WordOfDay;
        self.status = "Island Word of the Day — +15 XP".into();
        self.burst(640.0, 280.0);
    }

    fn run_define(&mut self) {
        let q = self.query.to_lowercase();
        let mut hits: Vec<_> = self
            .lexicon
            .iter()
            .filter(|w| {
                format!(
                    "{} {} {} {}",
                    w.boriken, w.english, w.spanish, w.definition_en
                )
                .to_lowercase()
                .contains(&q)
            })
            .cloned()
            .collect();
        hits.truncate(6);
        self.define_hits = hits;
        self.mode = Mode::Define;
        if !self.define_hits.is_empty() {
            self.add_xp(5);
            self.status = format!("Defined · {} match(es)", self.define_hits.len());
        } else {
            self.status = "No match — try another gloss.".into();
        }
    }

    fn story_beats() -> &'static [StoryBeat] {
        &[
            StoryBeat {
                scene: "Dawn",
                text: "The wey climbs over seyba crowns. A kokí answers the light.",
                prompt: "Greet the morning",
                answer: "Taí wey",
                choices: ["Taí wey", "Makabuka"],
            },
            StoryBeat {
                scene: "Konuko",
                text: "You grate yuka; steam and starch smell like survival stories.",
                prompt: "Name the cassava bread",
                answer: "kasabi",
                choices: ["kasabi", "hamaka"],
            },
            StoryBeat {
                scene: "Batey",
                text: "Drums gather for an areyto. The kasike waits for your voice.",
                prompt: "Introduce yourself",
                answer: "Taíno daka",
                choices: ["Taíno daka", "Waibá ni"],
            },
            StoryBeat {
                scene: "Shore",
                text: "Friends push the kanowa into bara. Adventure has a name.",
                prompt: "Rally the crew",
                answer: "Waibá",
                choices: ["Waibá", "Ua"],
            },
        ]
    }
}

impl eframe::App for BorikenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.setup_fonts(ctx);

        let dt = self.last.elapsed().as_secs_f32().min(0.05);
        self.last = Instant::now();
        self.scene.tick(dt, ctx.screen_rect());

        // Confetti physics
        for c in &mut self.confetti {
            c.vy += 120.0 * dt;
            c.x += c.vx * dt;
            c.y += c.vy * dt;
            c.life -= dt;
        }
        self.confetti.retain(|c| c.life > 0.0);

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                self.scene.paint(ui, rect);
                // confetti overlay
                let painter = ui.painter();
                for c in &self.confetti {
                    painter.circle_filled(
                        egui::pos2(c.x, c.y),
                        3.5,
                        c.color.linear_multiply(c.life.clamp(0.0, 1.0)),
                    );
                }

                ui.allocate_ui_at_rect(rect.shrink(24.0), |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            brand_title(ui, self.scene.pulse);
                            ui.label(
                                RichText::new("High-seas classroom · rebuild the native tongue")
                                    .size(18.0)
                                    .color(Color32::from_rgba_unmultiplied(245, 255, 245, 220)),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            glass_panel(ui, 220.0, |ui| {
                                ui.label(
                                    RichText::new(format!("{} XP", self.xp))
                                        .size(22.0)
                                        .color(Color32::from_rgb(242, 199, 90))
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(self.level_title())
                                        .size(14.0)
                                        .color(Color32::WHITE),
                                );
                                ui.label(
                                    RichText::new(format!("streak {}", self.streak))
                                        .size(13.0)
                                        .color(Color32::from_rgb(200, 230, 210)),
                                );
                            });
                        });
                    });

                    ui.add_space(12.0);
                    ui.horizontal_wrapped(|ui| {
                        let buttons = [
                            ("Word of Day", Mode::WordOfDay),
                            ("Batey Match", Mode::Match),
                            ("Memory Flip", Mode::Flash),
                            ("Konuko Fill", Mode::Fill),
                            ("Areyto Quest", Mode::Story),
                            ("Define", Mode::Define),
                        ];
                        for (label, mode) in buttons {
                            let selected = self.mode == mode
                                || (mode == Mode::WordOfDay && self.mode == Mode::WordOfDay);
                            let btn = egui::Button::new(
                                RichText::new(label)
                                    .size(16.0)
                                    .color(if selected {
                                        Color32::from_rgb(20, 30, 20)
                                    } else {
                                        Color32::WHITE
                                    })
                                    .strong(),
                            )
                            .fill(if selected {
                                Color32::from_rgb(242, 199, 90)
                            } else {
                                Color32::from_rgba_unmultiplied(255, 255, 255, 28)
                            })
                            .min_size(egui::vec2(128.0, 40.0));
                            if ui.add(btn).clicked() {
                                match mode {
                                    Mode::WordOfDay => self.word_of_day(),
                                    Mode::Match => self.start_match(),
                                    Mode::Flash => self.start_flash(),
                                    Mode::Fill => self.start_fill(),
                                    Mode::Story => {
                                        self.story.idx = 0;
                                        self.story.feedback = None;
                                        self.mode = Mode::Story;
                                        self.status = "Areyto Quest begins at dawn.".into();
                                    }
                                    Mode::Define => {
                                        self.mode = Mode::Define;
                                        self.run_define();
                                    }
                                    Mode::Home => self.mode = Mode::Home,
                                }
                            }
                        }
                    });

                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(&self.status)
                            .size(15.0)
                            .color(Color32::from_rgb(242, 199, 90)),
                    );
                    ui.add_space(8.0);

                    match self.mode {
                        Mode::Home => {
                            glass_panel(ui, 900.0, |ui| {
                                ui.label(
                                    RichText::new("Waibá — pick a game and learn like an areyto.")
                                        .size(22.0)
                                        .color(Color32::WHITE),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "{} words loaded with definitions, fun facts, and examples.",
                                        self.lexicon.len()
                                    ))
                                    .size(16.0)
                                    .color(Color32::from_rgb(210, 235, 220)),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(
                                        "Motion canvas · particle weather · ocean parallax · cinematic sun",
                                    )
                                    .size(14.0)
                                    .color(Color32::from_rgb(180, 210, 195)),
                                );
                            });
                        }
                        Mode::WordOfDay => {
                            if let Some(w) = &self.spotlight {
                                let w = w.clone();
                                glass_panel(ui, 900.0, |ui| {
                                    word_card(ui, &w, true);
                                });
                            }
                        }
                        Mode::Match => {
                            if let Some(state) = self.match_state.clone() {
                                glass_panel(ui, 900.0, |ui| {
                                    ui.label(
                                        RichText::new("Batey Match")
                                            .size(20.0)
                                            .color(Color32::from_rgb(242, 199, 90)),
                                    );
                                    ui.label(
                                        RichText::new(&state.word.boriken)
                                            .size(48.0)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(&state.word.fun_fact)
                                            .size(14.0)
                                            .color(Color32::from_rgb(230, 210, 140)),
                                    );
                                    ui.add_space(8.0);
                                    if let Some((ok, msg)) = &state.feedback {
                                        ui.label(
                                            RichText::new(msg)
                                                .size(18.0)
                                                .color(if *ok {
                                                    Color32::from_rgb(160, 240, 180)
                                                } else {
                                                    Color32::from_rgb(255, 190, 170)
                                                }),
                                        );
                                        if ui
                                            .add(
                                                egui::Button::new("Next wave")
                                                    .fill(Color32::from_rgb(242, 199, 90)),
                                            )
                                            .clicked()
                                        {
                                            self.start_match();
                                        }
                                    } else {
                                        for choice in &state.choices {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(choice).size(18.0),
                                                    )
                                                    .min_size(egui::vec2(ui.available_width(), 42.0))
                                                    .fill(Color32::from_rgba_unmultiplied(
                                                        255, 255, 255, 24,
                                                    )),
                                                )
                                                .clicked()
                                            {
                                                let ok = choice == &state.answer;
                                                if ok {
                                                    self.add_xp(8);
                                                    self.burst(
                                                        ui.cursor().center().x,
                                                        ui.cursor().center().y,
                                                    );
                                                } else {
                                                    self.add_xp(1);
                                                }
                                                if let Some(ms) = &mut self.match_state {
                                                    ms.feedback = Some((
                                                        ok,
                                                        if ok {
                                                            "The batey cheers for you.".into()
                                                        } else {
                                                            format!("Answer: {}", state.answer)
                                                        },
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        }
                        Mode::Flash => {
                            if let Some(flash) = &self.flash {
                                if let Some(w) = flash.cards.get(flash.idx).cloned() {
                                    let revealed = flash.revealed;
                                    let idx = flash.idx;
                                    let total = flash.cards.len();
                                    glass_panel(ui, 900.0, |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "Memory Bohío · {}/{}",
                                                idx + 1,
                                                total
                                            ))
                                            .size(18.0)
                                            .color(Color32::from_rgb(242, 199, 90)),
                                        );
                                        if revealed {
                                            word_card(ui, &w, true);
                                        } else {
                                            ui.add_space(20.0);
                                            ui.label(
                                                RichText::new(w.gloss_en())
                                                    .size(40.0)
                                                    .color(Color32::WHITE)
                                                    .strong(),
                                            );
                                            ui.label(
                                                RichText::new("Say the Boriken form, then flip.")
                                                    .size(16.0)
                                                    .color(Color32::from_rgb(200, 230, 215)),
                                            );
                                        }
                                        ui.add_space(10.0);
                                        ui.horizontal(|ui| {
                                            if ui.button("Flip").clicked() {
                                                if let Some(f) = &mut self.flash {
                                                    f.revealed = true;
                                                }
                                            }
                                            if ui
                                                .add(
                                                    egui::Button::new("Got it · +5 XP")
                                                        .fill(Color32::from_rgb(242, 199, 90)),
                                                )
                                                .clicked()
                                            {
                                                self.add_xp(5);
                                                if let Some(f) = &mut self.flash {
                                                    f.idx += 1;
                                                    f.revealed = false;
                                                    if f.idx >= f.cards.len() {
                                                        self.status =
                                                            "Deck complete. Waibá!".into();
                                                        self.burst(640.0, 360.0);
                                                        self.mode = Mode::Home;
                                                    }
                                                }
                                            }
                                        });
                                    });
                                }
                            }
                        }
                        Mode::Fill => {
                            if let Some(fill) = self.fill.clone() {
                                glass_panel(ui, 900.0, |ui| {
                                    ui.label(
                                        RichText::new("Konuko Fill-In")
                                            .size(20.0)
                                            .color(Color32::from_rgb(242, 199, 90)),
                                    );
                                    ui.label(
                                        RichText::new(&fill.prompt)
                                            .size(36.0)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(
                                            fill.word
                                                .definition_en
                                                .split('.')
                                                .next()
                                                .unwrap_or(""),
                                        )
                                        .size(15.0)
                                        .color(Color32::from_rgb(210, 230, 220)),
                                    );
                                    ui.add_space(8.0);
                                    if let Some((ok, msg)) = &fill.feedback {
                                        ui.label(
                                            RichText::new(msg).size(18.0).color(if *ok {
                                                Color32::from_rgb(160, 240, 180)
                                            } else {
                                                Color32::from_rgb(255, 190, 170)
                                            }),
                                        );
                                        if ui.button("Another plot").clicked() {
                                            self.start_fill();
                                        }
                                    } else {
                                        let resp = ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.fill.as_mut().unwrap().typed,
                                            )
                                            .desired_width(ui.available_width())
                                            .hint_text("type Boriken word"),
                                        );
                                        let submit = ui
                                            .add(
                                                egui::Button::new("Plant it")
                                                    .fill(Color32::from_rgb(242, 199, 90)),
                                            )
                                            .clicked()
                                            || (resp.lost_focus()
                                                && ui.input(|i| i.key_pressed(Key::Enter)));
                                        if submit {
                                            let typed = self
                                                .fill
                                                .as_ref()
                                                .map(|f| f.typed.trim().to_lowercase())
                                                .unwrap_or_default();
                                            let answer = fill.word.boriken.to_lowercase();
                                            let ok = typed == answer;
                                            if ok {
                                                self.add_xp(8);
                                                self.burst(640.0, 360.0);
                                            } else {
                                                self.add_xp(1);
                                            }
                                            if let Some(f) = &mut self.fill {
                                                f.feedback = Some((
                                                    ok,
                                                    if ok {
                                                        "Root takes hold.".into()
                                                    } else {
                                                        format!("Answer: {}", fill.word.boriken)
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                });
                            }
                        }
                        Mode::Define => {
                            glass_panel(ui, 900.0, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.query)
                                            .desired_width(ui.available_width() - 120.0)
                                            .hint_text("English / Spanish / Boriken"),
                                    );
                                    if ui
                                        .add(
                                            egui::Button::new("Define")
                                                .fill(Color32::from_rgb(242, 199, 90)),
                                        )
                                        .clicked()
                                    {
                                        self.run_define();
                                    }
                                });
                                ui.add_space(8.0);
                                for w in self.define_hits.clone() {
                                    egui::Frame::none()
                                        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 50))
                                        .inner_margin(10.0)
                                        .show(ui, |ui| word_card(ui, &w, false));
                                    ui.add_space(6.0);
                                }
                            });
                        }
                        Mode::Story => {
                            let beats = Self::story_beats();
                            glass_panel(ui, 900.0, |ui| {
                                if self.story.idx >= beats.len() {
                                    ui.label(
                                        RichText::new("Areyto complete")
                                            .size(28.0)
                                            .color(Color32::from_rgb(242, 199, 90))
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(
                                            "You walked a morning in Borikén. +20 XP voyage bonus.",
                                        )
                                        .size(18.0)
                                        .color(Color32::WHITE),
                                    );
                                    if ui.button("Sail home").clicked() {
                                        self.mode = Mode::Home;
                                    }
                                } else {
                                    let beat = &beats[self.story.idx];
                                    ui.label(
                                        RichText::new(format!("Areyto Quest · {}", beat.scene))
                                            .size(20.0)
                                            .color(Color32::from_rgb(242, 199, 90)),
                                    );
                                    ui.label(
                                        RichText::new(beat.text)
                                            .size(20.0)
                                            .color(Color32::WHITE),
                                    );
                                    ui.label(
                                        RichText::new(beat.prompt)
                                            .size(16.0)
                                            .color(Color32::from_rgb(200, 230, 215)),
                                    );
                                    ui.add_space(8.0);
                                    if let Some((ok, msg)) = &self.story.feedback {
                                        ui.label(
                                            RichText::new(msg).size(18.0).color(if *ok {
                                                Color32::from_rgb(160, 240, 180)
                                            } else {
                                                Color32::from_rgb(255, 190, 170)
                                            }),
                                        );
                                        if ui
                                            .add(
                                                egui::Button::new("Continue")
                                                    .fill(Color32::from_rgb(242, 199, 90)),
                                            )
                                            .clicked()
                                        {
                                            self.story.feedback = None;
                                            self.story.idx += 1;
                                            if self.story.idx >= beats.len() {
                                                self.add_xp(20);
                                                self.burst(640.0, 320.0);
                                            }
                                        }
                                    } else {
                                        for choice in beat.choices {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(choice).size(18.0),
                                                    )
                                                    .min_size(egui::vec2(
                                                        ui.available_width(),
                                                        42.0,
                                                    ))
                                                    .fill(Color32::from_rgba_unmultiplied(
                                                        255, 255, 255, 24,
                                                    )),
                                                )
                                                .clicked()
                                            {
                                                let ok = choice == beat.answer;
                                                if ok {
                                                    self.add_xp(10);
                                                    self.burst(
                                                        ui.max_rect().center().x,
                                                        ui.max_rect().center().y,
                                                    );
                                                } else {
                                                    self.add_xp(2);
                                                }
                                                self.story.feedback = Some((
                                                    ok,
                                                    if ok {
                                                        "Kasike energy unlocked.".into()
                                                    } else {
                                                        format!("Remember: {}", beat.answer)
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
            });

        // Continuous animation
        ctx.request_repaint();
        // Consume click sense so background feels alive
        let _ = ctx.input(|i| i.pointer.any_click());
        let _ = Sense::hover();
    }
}

fn word_card(ui: &mut egui::Ui, w: &Lexeme, big: bool) {
    ui.label(
        RichText::new(&w.boriken)
            .size(if big { 46.0 } else { 28.0 })
            .color(Color32::WHITE)
            .strong(),
    );
    ui.label(
        RichText::new(format!("{} · {}", w.english, w.spanish))
            .size(16.0)
            .color(Color32::from_rgb(210, 235, 220)),
    );
    if !w.definition_en.is_empty() {
        ui.add_space(6.0);
        ui.label(
            RichText::new(&w.definition_en)
                .size(15.0)
                .color(Color32::from_rgb(235, 245, 240)),
        );
    }
    if !w.fun_fact.is_empty() {
        ui.label(
            RichText::new(format!("✦ {}", w.fun_fact))
                .size(14.0)
                .color(Color32::from_rgb(242, 199, 90)),
        );
    }
    ui.label(
        RichText::new(format!(
            "Example: {} · {}",
            if w.example.is_empty() {
                &w.boriken
            } else {
                &w.example
            },
            if w.attested {
                "attested"
            } else {
                "reconstructed"
            }
        ))
        .size(13.0)
        .color(Color32::from_rgb(180, 210, 195)),
    );
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title("BORIKÉN — Desktop Learner"),
        vsync: true,
        // 0 keeps software/Xvfb GL configs working; hardware GPUs still look sharp via custom paint.
        multisampling: 0,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "BORIKÉN",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(BorikenApp::default()))
        }),
    )
}
