use chrono::{DateTime, FixedOffset, Utc};
use eframe::{self, egui, App};
use egui::{
    Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use std::f32::consts::{PI, TAU};
use std::time::{Duration, Instant};

/// Contemplative spin: ease through one turn, then rest for a beat.
const SPIN_DURATION: f32 = 1.0;
const REST_DURATION: f32 = 0.85;

struct ClockApp {
    japan_time: String,
    central_time: String,
    started: Instant,
    phase: SpinPhase,
    phase_t0: Instant,
    /// Accumulated completed turns (radians) so the emblem keeps its place.
    turns: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpinPhase {
    Thinking,
    Resting,
}

impl Default for ClockApp {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            japan_time: get_japan_time(),
            central_time: get_central_time(),
            started: now,
            phase: SpinPhase::Thinking,
            phase_t0: now,
            turns: 0.0,
        }
    }
}

impl ClockApp {
    fn spin_angle(&mut self) -> f32 {
        let elapsed = self.phase_t0.elapsed().as_secs_f32();
        match self.phase {
            SpinPhase::Thinking => {
                let t = (elapsed / SPIN_DURATION).clamp(0.0, 1.0);
                let eased = ease_in_out_cubic(t);
                if t >= 1.0 {
                    self.turns += TAU;
                    self.phase = SpinPhase::Resting;
                    self.phase_t0 = Instant::now();
                    return self.turns;
                }
                self.turns + eased * TAU
            }
            SpinPhase::Resting => {
                if elapsed >= REST_DURATION {
                    self.phase = SpinPhase::Thinking;
                    self.phase_t0 = Instant::now();
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

        let angle = self.spin_angle();
        let breath = ((self.started.elapsed().as_secs_f32() * 0.55).sin() * 0.5 + 0.5) * 0.04;

        // Deep indigo → warm dawn atmosphere (not purple-on-white, not cream).
        let bg_top = Color32::from_rgb(12, 28, 48);
        let bg_bot = Color32::from_rgb(36, 22, 18);
        paint_vertical_gradient(ctx, bg_top, bg_bot);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(28.0, 24.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
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
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("One emblem. Two dawns. A second to think.")
                            .font(FontId::proportional(14.0))
                            .color(Color32::from_rgb(168, 176, 188)),
                    );

                    ui.add_space(18.0);

                    let emblem_side = ui.available_width().min(280.0).max(180.0);
                    let (rect, _resp) = ui.allocate_exact_size(
                        Vec2::splat(emblem_side),
                        Sense::hover(),
                    );
                    paint_thought_emblem(ui, rect, angle, breath, self.phase);

                    ui.add_space(22.0);

                    // Single composition: both clocks side by side, not tabs.
                    let row_w = ui.available_width().min(520.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(row_w, 96.0),
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

                    ui.add_space(18.0);
                    let status = match self.phase {
                        SpinPhase::Thinking => "thought spin",
                        SpinPhase::Resting => "held a second",
                    };
                    ui.label(
                        RichText::new(status)
                            .font(FontId::proportional(12.0))
                            .color(Color32::from_rgb(120, 132, 148))
                            .italics(),
                    );
                });
            });

        // Smooth animation: ~60fps while spinning, slower while resting.
        let after = match self.phase {
            SpinPhase::Thinking => Duration::from_millis(16),
            SpinPhase::Resting => Duration::from_millis(50),
        };
        ctx.request_repaint_after(after);
    }
}

fn time_column(ui: &mut egui::Ui, width: f32, title: &str, zone: &str, value: &str, accent: Color32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 96.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.label(
                RichText::new(title)
                    .font(FontId::proportional(12.0))
                    .color(accent)
                    .extra_letter_spacing(2.5),
            );
            ui.add_space(4.0);
            // Show HH:MM:SS prominently if we can split the string.
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
    if let Some((d, t)) = value.split_once(' ') {
        (d, t)
    } else {
        ("", value)
    }
}

fn paint_vertical_gradient(ctx: &egui::Context, top: Color32, bottom: Color32) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    const STEPS: i32 = 48;
    for i in 0..STEPS {
        let t0 = i as f32 / STEPS as f32;
        let t1 = (i + 1) as f32 / STEPS as f32;
        let y0 = egui::lerp(screen.top()..=screen.bottom(), t0);
        let y1 = egui::lerp(screen.top()..=screen.bottom(), t1);
        let c = lerp_color(top, bottom, (t0 + t1) * 0.5);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(screen.left(), y0), Pos2::new(screen.right(), y1)),
            0.0,
            c,
        );
    }
}

fn paint_thought_emblem(ui: &egui::Ui, rect: Rect, angle: f32, breath: f32, phase: SpinPhase) {
    let painter = ui.painter();
    let center = rect.center();
    let r = rect.width().min(rect.height()) * 0.5 * (0.92 + breath);

    // Soft outer glow disc
    painter.circle_filled(center, r * 1.02, Color32::from_rgba_unmultiplied(40, 56, 72, 90));

    // Outer ring — slow counter-spin of hash marks (thought orbit)
    let ring_stroke = Stroke::new(2.2, Color32::from_rgb(214, 186, 140));
    painter.circle_stroke(center, r * 0.96, ring_stroke);

    let marks = 12;
    for i in 0..marks {
        let a = angle * 0.35 + (i as f32) * (TAU / marks as f32);
        let inner = r * 0.88;
        let outer = r * 0.96;
        let p0 = polar(center, inner, a);
        let p1 = polar(center, outer, a);
        painter.line_segment([p0, p1], Stroke::new(1.6, Color32::from_rgb(190, 160, 110)));
    }

    // Mid thought arc — incomplete circle that leads the spin
    let arc_alpha = match phase {
        SpinPhase::Thinking => 220u8,
        SpinPhase::Resting => 140u8,
    };
    draw_arc(
        painter,
        center,
        r * 0.72,
        angle - 0.35,
        angle + PI * 1.15,
        Stroke::new(3.0, Color32::from_rgba_unmultiplied(232, 120, 78, arc_alpha)),
        36,
    );
    draw_arc(
        painter,
        center,
        r * 0.62,
        -angle * 0.7 + 0.8,
        -angle * 0.7 + 0.8 + PI * 0.9,
        Stroke::new(2.0, Color32::from_rgba_unmultiplied(72, 168, 196, arc_alpha)),
        28,
    );

    // Rising-sun disc (Japan)
    let sun_r = r * 0.34;
    painter.circle_filled(center, sun_r, Color32::from_rgb(224, 78, 64));
    painter.circle_stroke(center, sun_r, Stroke::new(1.5, Color32::from_rgb(255, 180, 150)));

    // Inner monogram disc
    painter.circle_filled(center, sun_r * 0.62, Color32::from_rgb(18, 28, 40));

    // JC monogram
    let font = FontId::proportional(sun_r * 0.85);
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        "JC",
        font,
        Color32::from_rgb(248, 241, 230),
    );

    // Orbiting thought beads — three nodes that ride the spin
    for (i, col) in [
        Color32::from_rgb(232, 92, 72),
        Color32::from_rgb(214, 186, 140),
        Color32::from_rgb(72, 168, 196),
    ]
    .into_iter()
    .enumerate()
    {
        let a = angle + (i as f32) * (TAU / 3.0);
        let p = polar(center, r * 0.72, a);
        painter.circle_filled(p, 5.5, col);
        painter.circle_stroke(p, 5.5, Stroke::new(1.0, Color32::from_rgb(250, 246, 238)));
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
            .with_inner_size([520.0, 640.0])
            .with_min_inner_size([360.0, 520.0])
            .with_title("JC Time — Japan & Central"),
        ..Default::default()
    };
    eframe::run_native(
        "JC Time — Japan & Central",
        native_options,
        Box::new(|_| Ok(Box::new(ClockApp::default()))),
    )
}
