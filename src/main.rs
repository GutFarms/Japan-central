use chrono::{DateTime, FixedOffset, Utc};
use eframe::{self, egui, App};
use egui::{
    Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use std::f32::consts::{PI, TAU};

/// Contemplative spin: ease one bead-step (~1s), then rest a beat.
/// Each thought advances by a third-turn so the hold pose walks forward
/// (a full 360° return-to-home reads as a snap-back).
const SPIN_DURATION: f32 = 1.0;
const REST_DURATION: f32 = 0.9;
const SPIN_STEP: f32 = TAU / 3.0;

struct ClockApp {
    japan_time: String,
    central_time: String,
    /// Absolute seconds when the current phase began (`ctx.input.time`).
    phase_t0: f64,
    phase: SpinPhase,
    /// Completed full turns (radians).
    turns: f32,
    boot_time: Option<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpinPhase {
    Thinking,
    Resting,
}

impl Default for ClockApp {
    fn default() -> Self {
        Self {
            japan_time: get_japan_time(),
            central_time: get_central_time(),
            phase_t0: 0.0,
            phase: SpinPhase::Thinking,
            turns: 0.0,
            boot_time: None,
        }
    }
}

impl ClockApp {
    fn spin_angle(&mut self, now: f64) -> f32 {
        if self.boot_time.is_none() {
            self.boot_time = Some(now);
            self.phase_t0 = now;
        }
        let elapsed = (now - self.phase_t0) as f32;
        match self.phase {
            SpinPhase::Thinking => {
                let t = (elapsed / SPIN_DURATION).clamp(0.0, 1.0);
                let eased = ease_in_out_cubic(t);
                if t >= 1.0 {
                    self.turns = (self.turns + SPIN_STEP) % TAU;
                    self.phase = SpinPhase::Resting;
                    self.phase_t0 = now;
                    return self.turns;
                }
                self.turns + eased * SPIN_STEP
            }
            SpinPhase::Resting => {
                if elapsed >= REST_DURATION {
                    self.phase = SpinPhase::Thinking;
                    self.phase_t0 = now;
                }
                self.turns
            }
        }
    }
}

impl App for ClockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.japan_time = get_japan_time();
        self.central_time = get_central_time();

        let now = ctx.input(|i| i.time);
        let angle = self.spin_angle(now);
        let boot = self.boot_time.unwrap_or(now);
        let breath = (((now - boot) as f32) * 0.7).sin() * 0.035;

        // Deep indigo → warm dawn (avoid purple-on-white / cream / terracotta clichés).
        paint_vertical_gradient(
            ctx,
            Color32::from_rgb(10, 24, 42),
            Color32::from_rgb(42, 26, 18),
        );

        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(28.0, 22.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("JC TIME")
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(214, 186, 140))
                            .extra_letter_spacing(4.0),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Japan · Central")
                            .font(FontId::proportional(28.0))
                            .color(Color32::from_rgb(248, 241, 230))
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("One emblem. Two dawns. A second to think.")
                            .font(FontId::proportional(14.0))
                            .color(Color32::from_rgb(168, 176, 188)),
                    );

                    ui.add_space(16.0);

                    let emblem_side = ui.available_width().min(300.0).max(200.0);
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::splat(emblem_side), Sense::hover());
                    paint_thought_emblem(ui, rect, angle, breath, self.phase, now);

                    ui.add_space(20.0);

                    // Single composition — both clocks together, never tabs.
                    let row_w = ui.available_width().min(520.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(row_w, 100.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let half = (ui.available_width() - 16.0) * 0.5;
                            time_column(
                                ui,
                                half,
                                "JAPAN",
                                "JST · UTC+9",
                                &self.japan_time,
                                Color32::from_rgb(232, 92, 72),
                            );
                            ui.add_space(16.0);
                            time_column(
                                ui,
                                half,
                                "CENTRAL",
                                "CT · UTC−6",
                                &self.central_time,
                                Color32::from_rgb(72, 168, 196),
                            );
                        },
                    );

                    ui.add_space(16.0);
                    let (status, accent) = match self.phase {
                        SpinPhase::Thinking => (
                            "thought spin",
                            Color32::from_rgb(232, 150, 96),
                        ),
                        SpinPhase::Resting => (
                            "held a second",
                            Color32::from_rgb(120, 140, 160),
                        ),
                    };
                    ui.label(
                        RichText::new(status)
                            .font(FontId::proportional(13.0))
                            .color(accent)
                            .italics(),
                    );
                });
            });

        // Keep painting every frame so the spin is fluid on VNC/X11.
        ctx.request_repaint();
    }
}

fn time_column(
    ui: &mut egui::Ui,
    width: f32,
    title: &str,
    zone: &str,
    value: &str,
    accent: Color32,
) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 100.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.label(
                RichText::new(title)
                    .font(FontId::proportional(12.0))
                    .color(accent)
                    .extra_letter_spacing(2.5),
            );
            ui.add_space(4.0);
            let (date, clock) = split_datetime(value);
            ui.label(
                RichText::new(clock)
                    .font(FontId::monospace(26.0))
                    .color(Color32::from_rgb(250, 246, 238)),
            );
            ui.label(
                RichText::new(date)
                    .font(FontId::proportional(13.0))
                    .color(Color32::from_rgb(160, 168, 178)),
            );
            ui.label(
                RichText::new(zone)
                    .font(FontId::proportional(11.0))
                    .color(Color32::from_rgb(110, 122, 138)),
            );
        },
    );
}

fn split_datetime(value: &str) -> (&str, &str) {
    value.split_once(' ').unwrap_or(("", value))
}

fn paint_vertical_gradient(ctx: &egui::Context, top: Color32, bottom: Color32) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    const STEPS: i32 = 56;
    for i in 0..STEPS {
        let t0 = i as f32 / STEPS as f32;
        let t1 = (i + 1) as f32 / STEPS as f32;
        let y0 = egui::lerp(screen.top()..=screen.bottom(), t0);
        let y1 = egui::lerp(screen.top()..=screen.bottom(), t1);
        let c = lerp_color(top, bottom, (t0 + t1) * 0.5);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(screen.left(), y0),
                Pos2::new(screen.right(), y1),
            ),
            0.0,
            c,
        );
    }
}

fn paint_thought_emblem(
    ui: &egui::Ui,
    rect: Rect,
    angle: f32,
    breath: f32,
    phase: SpinPhase,
    now: f64,
) {
    let painter = ui.painter();
    let center = rect.center();
    let r = rect.width().min(rect.height()) * 0.5 * (0.90 + breath);

    // Soft halo
    painter.circle_filled(
        center,
        r * 1.05,
        Color32::from_rgba_unmultiplied(48, 64, 80, 70),
    );

    // Outer tick ring — rotates with the thought so the spin is obvious.
    painter.circle_stroke(center, r * 0.98, Stroke::new(2.4, Color32::from_rgb(214, 186, 140)));
    let marks = 24;
    for i in 0..marks {
        let a = angle + (i as f32) * (TAU / marks as f32);
        let long = i % 6 == 0;
        let inner = if long { r * 0.86 } else { r * 0.91 };
        let outer = r * 0.98;
        painter.line_segment(
            [polar(center, inner, a), polar(center, outer, a)],
            Stroke::new(
                if long { 2.2 } else { 1.4 },
                if long {
                    Color32::from_rgb(232, 210, 160)
                } else {
                    Color32::from_rgb(170, 140, 100)
                },
            ),
        );
    }

    let thinking = matches!(phase, SpinPhase::Thinking);
    let arc_a = if thinking { 255u8 } else { 150u8 };

    // Bold sweeping thought wedge — the readable “spin” signal.
    let wedge_span = if thinking { PI * 0.55 } else { PI * 0.28 };
    draw_arc(
        painter,
        center,
        r * 0.74,
        angle - wedge_span,
        angle,
        Stroke::new(5.5, Color32::from_rgba_unmultiplied(232, 110, 70, arc_a)),
        40,
    );
    draw_arc(
        painter,
        center,
        r * 0.62,
        angle + PI * 0.15,
        angle + PI * 0.15 + wedge_span * 0.85,
        Stroke::new(3.5, Color32::from_rgba_unmultiplied(72, 168, 196, arc_a)),
        32,
    );

    // Comet trail behind the lead bead
    let lead = angle;
    for k in 1..8 {
        let fade = 1.0 - (k as f32) / 8.0;
        let a = lead - (k as f32) * 0.12;
        let p = polar(center, r * 0.74, a);
        let alpha = (fade * if thinking { 180.0 } else { 70.0 }) as u8;
        painter.circle_filled(
            p,
            3.0 + fade * 3.0,
            Color32::from_rgba_unmultiplied(255, 170, 120, alpha),
        );
    }

    // Rising-sun core
    let sun_r = r * 0.36;
    painter.circle_filled(center, sun_r, Color32::from_rgb(224, 78, 64));
    painter.circle_stroke(center, sun_r, Stroke::new(2.0, Color32::from_rgb(255, 190, 160)));
    painter.circle_filled(center, sun_r * 0.62, Color32::from_rgb(14, 24, 36));
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        "JC",
        FontId::proportional(sun_r * 0.9),
        Color32::from_rgb(248, 241, 230),
    );

    // Three orbiting thought beads — large enough to read as motion.
    let beads = [
        (0.0, Color32::from_rgb(232, 92, 72), r * 0.74),
        (TAU / 3.0, Color32::from_rgb(214, 186, 140), r * 0.74),
        (2.0 * TAU / 3.0, Color32::from_rgb(72, 168, 196), r * 0.74),
    ];
    for (offset, col, rad) in beads {
        let a = angle + offset;
        let p = polar(center, rad, a);
        painter.circle_filled(p, 8.0, col);
        painter.circle_stroke(p, 8.0, Stroke::new(1.6, Color32::from_rgb(250, 246, 238)));
    }

    // Idle shimmer pulse on the lead bead while resting — still “alive”.
    if matches!(phase, SpinPhase::Resting) {
        let pulse = (((now * 3.2).sin() as f32) * 0.5 + 0.5) * 5.0;
        let p = polar(center, r * 0.74, angle);
        painter.circle_stroke(
            p,
            8.0 + pulse,
            Stroke::new(1.2, Color32::from_rgba_unmultiplied(255, 200, 150, 120)),
        );
    }
}

fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start: f32,
    end: f32,
    stroke: Stroke,
    segments: usize,
) {
    if segments < 2 {
        return;
    }
    let mut prev = polar(center, radius, start);
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let a = start + (end - start) * t;
        let next = polar(center, radius, a);
        painter.line_segment([prev, next], stroke);
        prev = next;
    }
}

fn polar(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    Pos2::new(
        center.x + radius * angle.cos(),
        center.y + radius * angle.sin(),
    )
}

fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        egui::lerp(a.r() as f32..=b.r() as f32, t) as u8,
        egui::lerp(a.g() as f32..=b.g() as f32, t) as u8,
        egui::lerp(a.b() as f32..=b.b() as f32, t) as u8,
        egui::lerp(a.a() as f32..=b.a() as f32, t) as u8,
    )
}

fn get_japan_time() -> String {
    let jst_offset = FixedOffset::east_opt(9 * 3600).expect("Invalid offset for JST");
    let jst_time: DateTime<FixedOffset> = Utc::now().with_timezone(&jst_offset);
    jst_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn get_central_time() -> String {
    let ct_offset = FixedOffset::west_opt(6 * 3600).expect("Invalid offset for CT");
    let ct_time: DateTime<FixedOffset> = Utc::now().with_timezone(&ct_offset);
    ct_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 660.0])
            .with_min_inner_size([380.0, 540.0])
            .with_title("JC Time — Japan & Central"),
        ..Default::default()
    };
    eframe::run_native(
        "JC Time — Japan & Central",
        native_options,
        Box::new(|_| Ok(Box::new(ClockApp::default()))),
    )
}
