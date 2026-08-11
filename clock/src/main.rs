//! Japan & Central Time — dual egui world clock with rich custom painting.

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::{America::Chicago, Asia::Tokyo, Tz};
use eframe::{
    egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2},
    App, Frame, NativeOptions,
};
use std::f32::consts::{PI, TAU};
use std::time::Instant;

const BG_TOP: Color32 = Color32::from_rgb(2, 10, 26);
const BG_BOT: Color32 = Color32::from_rgb(4, 22, 48);
const PANEL: Color32 = Color32::from_rgb(8, 28, 64);
const PANEL_HI: Color32 = Color32::from_rgb(14, 42, 88);
const ACCENT: Color32 = Color32::from_rgb(90, 168, 232);
const ACCENT_SOFT: Color32 = Color32::from_rgb(42, 106, 184);
const TEXT: Color32 = Color32::from_rgb(242, 247, 255);
const MUTED: Color32 = Color32::from_rgb(122, 168, 216);
const WARM: Color32 = Color32::from_rgb(224, 184, 90);
const RING: Color32 = Color32::from_rgb(26, 74, 136);

struct ZoneCard {
    title: &'static str,
    city: &'static str,
    badge: &'static str,
    tz: Tz,
    accent: Color32,
}

impl ZoneCard {
    fn now(&self) -> DateTime<Tz> {
        Utc::now().with_timezone(&self.tz)
    }
}

struct ClockApp {
    zones: [ZoneCard; 2],
    started: Instant,
}

impl Default for ClockApp {
    fn default() -> Self {
        Self {
            zones: [
                ZoneCard {
                    title: "JAPAN",
                    city: "Tokyo",
                    badge: "JST · UTC+9",
                    tz: Tokyo,
                    accent: ACCENT,
                },
                ZoneCard {
                    title: "CENTRAL",
                    city: "Chicago",
                    badge: "CT · US Central",
                    tz: Chicago,
                    accent: WARM,
                },
            ],
            started: Instant::now(),
        }
    }
}

impl App for ClockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        let t = self.started.elapsed().as_secs_f32();
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let full = ui.max_rect();
                paint_backdrop(ui, full, t);

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(full), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(18.0);
                        ui.label(
                            RichText::new("JC TIME")
                                .font(FontId::proportional(34.0))
                                .color(TEXT)
                                .strong(),
                        );
                        ui.label(
                            RichText::new("Japan  ·  Central")
                                .font(FontId::proportional(16.0))
                                .color(MUTED),
                        );
                        ui.add_space(10.0);
                    });

                    let avail = ui.available_rect_before_wrap();
                    let gap = 20.0_f32;
                    let card_w = ((avail.width() - gap) * 0.5).max(280.0);
                    let card_h = (avail.height() - 24.0).max(360.0);
                    let left = Rect::from_min_size(
                        Pos2::new(
                            avail.left() + (avail.width() - card_w * 2.0 - gap) * 0.5,
                            avail.top(),
                        ),
                        Vec2::new(card_w, card_h),
                    );
                    let right = Rect::from_min_size(
                        Pos2::new(left.right() + gap, left.top()),
                        Vec2::new(card_w, card_h),
                    );

                    paint_zone_card(ui, left, &self.zones[0], t);
                    paint_zone_card(ui, right, &self.zones[1], t);
                });
            });

        // Smooth second-hand motion.
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn paint_backdrop(ui: &egui::Ui, rect: Rect, t: f32) {
    let painter = ui.painter();
    // Vertical wash
    let steps = 24;
    for i in 0..steps {
        let f = i as f32 / (steps - 1) as f32;
        let y0 = rect.top() + rect.height() * f;
        let y1 = rect.top() + rect.height() * ((i + 1) as f32 / steps as f32);
        let c = lerp_color(BG_TOP, BG_BOT, f);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1)),
            0.0,
            c,
        );
    }

    // Soft drifting orbs
    for (i, (ax, ay, r)) in [(0.18, 0.22, 120.0), (0.82, 0.18, 90.0), (0.55, 0.78, 140.0)]
        .iter()
        .enumerate()
    {
        let pulse = 0.85 + 0.15 * (t * (0.4 + i as f32 * 0.1) + i as f32).sin();
        let center = Pos2::new(rect.left() + rect.width() * ax, rect.top() + rect.height() * *ay);
        painter.circle_filled(center, r * pulse, Color32::from_rgba_unmultiplied(20, 60, 120, 40));
    }
}

fn paint_zone_card(ui: &egui::Ui, rect: Rect, zone: &ZoneCard, t: f32) {
    let painter = ui.painter_at(rect);
    let response = ui.interact(rect, ui.id().with(zone.title), Sense::hover());
    let hover = response.hovered();

    let shadow = rect.translate(Vec2::new(0.0, 6.0));
    painter.rect_filled(shadow, 22.0, Color32::from_rgba_unmultiplied(0, 0, 0, 70));
    painter.rect_filled(rect, 22.0, if hover { PANEL_HI } else { PANEL });
    painter.rect_stroke(rect, 22.0, Stroke::new(1.5_f32, RING));

    let now = zone.now();
    let digital = now.format("%H:%M:%S").to_string();
    let date = now.format("%A · %b %d, %Y").to_string();
    let offset = now.format("%Z").to_string();

    // Header
    painter.text(
        Pos2::new(rect.center().x, rect.top() + 28.0),
        egui::Align2::CENTER_CENTER,
        zone.title,
        FontId::proportional(18.0),
        zone.accent,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.top() + 54.0),
        egui::Align2::CENTER_CENTER,
        zone.city,
        FontId::proportional(28.0),
        TEXT,
    );

    // Analog clock
    let clock_c = Pos2::new(rect.center().x, rect.top() + rect.height() * 0.46);
    let radius = (rect.width().min(rect.height()) * 0.28).clamp(70.0, 130.0);
    paint_analog_clock(&painter, clock_c, radius, &now, zone.accent, t);

    // Digital readout
    painter.text(
        Pos2::new(rect.center().x, clock_c.y + radius + 36.0),
        egui::Align2::CENTER_CENTER,
        digital,
        FontId::monospace(34.0),
        TEXT,
    );
    painter.text(
        Pos2::new(rect.center().x, clock_c.y + radius + 68.0),
        egui::Align2::CENTER_CENTER,
        date,
        FontId::proportional(15.0),
        MUTED,
    );

    // Offset chip
    let chip_c = Pos2::new(rect.center().x, rect.bottom() - 36.0);
    let chip = Rect::from_center_size(chip_c, Vec2::new(160.0, 28.0));
    painter.rect_filled(chip, 14.0, ACCENT_SOFT);
    painter.text(
        chip_c,
        egui::Align2::CENTER_CENTER,
        if offset.is_empty() {
            zone.badge
        } else {
            zone.badge
        },
        FontId::proportional(13.0),
        TEXT,
    );
}

fn paint_analog_clock(
    painter: &egui::Painter,
    c: Pos2,
    r: f32,
    now: &DateTime<Tz>,
    accent: Color32,
    t: f32,
) {
    // Glow rings
    let glow = 0.92 + 0.08 * (t * 1.3).sin();
    painter.circle_filled(c, r + 16.0, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 28));
    painter.circle_filled(c, r + 6.0, Color32::from_rgb(6, 18, 40));
    painter.circle_stroke(c, r, Stroke::new(3.0_f32, accent.gamma_multiply(glow)));
    painter.circle_filled(c, r - 4.0, Color32::from_rgb(2, 12, 28));

    // Minute ticks
    for i in 0..60 {
        let ang = (i as f32 / 60.0) * TAU - PI / 2.0;
        let major = i % 5 == 0;
        let inner = if major { r - 16.0 } else { r - 9.0 };
        let outer = r - 4.0;
        let col = if major { accent } else { RING };
        painter.line_segment(
            [polar(c, ang, inner), polar(c, ang, outer)],
            Stroke::new(if major { 2.4_f32 } else { 1.0_f32 }, col),
        );
    }

    // Hour numerals
    for h in 1..=12 {
        let ang = (h as f32 / 12.0) * TAU - PI / 2.0;
        let p = polar(c, ang, r - 28.0);
        painter.text(
            p,
            egui::Align2::CENTER_CENTER,
            h.to_string(),
            FontId::proportional(13.0),
            MUTED,
        );
    }

    let hour = now.hour() % 12;
    let minute = now.minute();
    let second = now.second();
    let nanos = now.nanosecond();
    let sec_f = second as f32 + nanos as f32 / 1_000_000_000.0;
    let min_f = minute as f32 + sec_f / 60.0;
    let hour_f = hour as f32 + min_f / 60.0;

    let hour_ang = (hour_f / 12.0) * TAU - PI / 2.0;
    let min_ang = (min_f / 60.0) * TAU - PI / 2.0;
    let sec_ang = (sec_f / 60.0) * TAU - PI / 2.0;

    // Hands — thick hub to fine tip look via layered strokes
    painter.line_segment(
        [c, polar(c, hour_ang, r * 0.48)],
        Stroke::new(5.0_f32, TEXT),
    );
    painter.line_segment(
        [c, polar(c, min_ang, r * 0.68)],
        Stroke::new(3.2_f32, ACCENT),
    );
    painter.line_segment(
        [c, polar(c, sec_ang, r * 0.78)],
        Stroke::new(1.6_f32, accent),
    );
    painter.circle_filled(c, 6.0, accent);
    painter.circle_filled(c, 2.5, TEXT);
}

fn polar(c: Pos2, ang: f32, r: f32) -> Pos2 {
    Pos2::new(c.x + ang.cos() * r, c.y + ang.sin() * r)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("JC Time — Japan & Central"),
        ..Default::default()
    };
    eframe::run_native(
        "JC Time — Japan & Central",
        options,
        Box::new(|_cc| Ok(Box::new(ClockApp::default()))),
    )
}
