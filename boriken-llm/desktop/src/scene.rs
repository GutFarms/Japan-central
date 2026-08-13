//! Cinematic Borikén atmosphere with Taíno petroglyph symbolism.

use crate::symbols::{
    paint_cemi, paint_coqui, paint_panel_ornament, paint_petroglyph_frieze, paint_sol_taino,
    paint_spiral, paint_turtle,
};
use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Ui, Vec2};
use rand::Rng;
use std::f32::consts::TAU;

#[derive(Clone)]
pub struct Particle {
    pub pos: Pos2,
    pub vel: Vec2,
    pub size: f32,
    pub phase: f32,
    pub kind: u8, // 0 mote, 1 spark, 2 petal, 3 glyph-spark
}

pub struct Scene {
    pub t: f32,
    pub particles: Vec<Particle>,
    pub pulse: f32,
}

impl Scene {
    pub fn new(w: f32, h: f32) -> Self {
        let mut rng = rand::thread_rng();
        let mut particles = Vec::with_capacity(180);
        for _ in 0..180 {
            particles.push(Particle {
                pos: Pos2::new(rng.gen_range(0.0..w.max(1.0)), rng.gen_range(0.0..h.max(1.0))),
                vel: Vec2::new(rng.gen_range(-14.0..14.0), rng.gen_range(-22.0..-5.0)),
                size: rng.gen_range(1.2..5.0),
                phase: rng.gen_range(0.0..TAU),
                kind: rng.gen_range(0..4),
            });
        }
        Self {
            t: 0.0,
            particles,
            pulse: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32, rect: Rect) {
        self.t += dt;
        self.pulse = (self.t * 0.7).sin() * 0.5 + 0.5;
        let w = rect.width();
        let h = rect.height();
        for p in &mut self.particles {
            p.phase += dt * 1.7;
            p.pos.x += (p.vel.x + p.phase.sin() * 8.0) * dt;
            p.pos.y += p.vel.y * dt;
            if p.pos.y < -10.0 {
                p.pos.y = h + 10.0;
                p.pos.x = rand::thread_rng().gen_range(0.0..w.max(1.0));
            }
            if p.pos.x < -20.0 {
                p.pos.x = w + 20.0;
            }
            if p.pos.x > w + 20.0 {
                p.pos.x = -20.0;
            }
        }
    }

    pub fn paint(&self, ui: &mut Ui, rect: Rect) {
        let painter = ui.painter_at(rect);
        let t = self.t;
        let gold = Color32::from_rgb(242, 199, 90);
        let carve = Color32::from_rgba_unmultiplied(255, 230, 170, 200);
        let stone = Color32::from_rgb(28, 52, 44);

        // Higher-resolution sky gradient
        let bands = 72;
        for i in 0..bands {
            let y0 = rect.top() + rect.height() * (i as f32 / bands as f32);
            let y1 = rect.top() + rect.height() * ((i + 1) as f32 / bands as f32);
            let u = i as f32 / bands as f32;
            let sway = (t * 0.15 + u * 3.0).sin() * 0.04;
            let c = sky_color((u + sway).clamp(0.0, 1.0), self.pulse);
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.left(), y0), Pos2::new(rect.right(), y1 + 1.0)),
                0.0,
                c,
            );
        }

        // Monumental Sol Taíno (Jayuya-inspired) — brand-level sky icon
        let sol = Pos2::new(
            rect.left() + rect.width() * (0.78 + 0.015 * (t * 0.12).sin()),
            rect.top() + rect.height() * 0.20,
        );
        let sol_r = 58.0 + 6.0 * self.pulse;
        paint_sol_taino(&painter, sol, sol_r, t, gold);

        // Distant island / mountain silhouette with carved stones
        paint_island(&painter, rect, t);

        // Sacred figures on the landline
        let land_y = rect.top() + rect.height() * 0.545;
        paint_cemi(
            &painter,
            Pos2::new(rect.left() + rect.width() * 0.16, land_y - 8.0),
            48.0,
            Color32::from_rgba_unmultiplied(235, 205, 120, 230),
            t,
        );
        paint_coqui(
            &painter,
            Pos2::new(rect.left() + rect.width() * 0.40, land_y + 4.0),
            32.0,
            Color32::from_rgba_unmultiplied(200, 245, 210, 235),
            t,
        );
        paint_turtle(
            &painter,
            Pos2::new(rect.left() + rect.width() * 0.56, land_y + 16.0),
            28.0,
            Color32::from_rgba_unmultiplied(190, 230, 215, 220),
        );
        paint_cemi(
            &painter,
            Pos2::new(rect.left() + rect.width() * 0.28, land_y + 4.0),
            30.0,
            Color32::from_rgba_unmultiplied(210, 175, 95, 190),
            t + 1.3,
        );

        // Layered animated ocean
        paint_waves(&painter, rect, t);

        // Underwater / shoreline petroglyph frieze
        let frieze_y = rect.top() + rect.height() * 0.70;
        paint_petroglyph_frieze(
            &painter,
            frieze_y,
            rect.left() + 40.0,
            rect.right() - 40.0,
            t,
            Color32::from_rgba_unmultiplied(255, 220, 140, 160),
        );

        // Floating particles + tiny glyph sparks
        for p in &self.particles {
            let wobble = Pos2::new(p.pos.x + p.phase.cos() * 3.0, p.pos.y);
            match p.kind {
                3 => {
                    // mini spiral glyph sparks
                    paint_spiral(
                        &painter,
                        wobble,
                        p.size * 2.8,
                        1.4,
                        Color32::from_rgba_unmultiplied(255, 220, 140, 150),
                        p.phase,
                    );
                }
                1 => {
                    painter.circle_filled(
                        wobble,
                        p.size * (0.85 + 0.25 * p.phase.sin()),
                        Color32::from_rgba_unmultiplied(255, 220, 140, 160),
                    );
                }
                2 => {
                    painter.circle_filled(
                        wobble,
                        p.size * (0.85 + 0.25 * p.phase.sin()),
                        Color32::from_rgba_unmultiplied(240, 170, 120, 110),
                    );
                }
                _ => {
                    painter.circle_filled(
                        wobble,
                        p.size * (0.85 + 0.25 * p.phase.sin()),
                        Color32::from_rgba_unmultiplied(200, 255, 230, 90),
                    );
                }
            }
        }

        // Top petroglyph border (batey / monolith rhythm)
        paint_petroglyph_frieze(
            &painter,
            rect.top() + 28.0,
            rect.left() + 220.0,
            rect.right() - 200.0,
            t * 0.7,
            Color32::from_rgba_unmultiplied(255, 230, 160, 70),
        );

        // Soft vignette
        painter.rect_filled(
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + 70.0)),
            0.0,
            Color32::from_rgba_unmultiplied(0, 20, 16, 70),
        );
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 90.0), rect.max),
            0.0,
            Color32::from_rgba_unmultiplied(0, 25, 20, 90),
        );

        // Keep unused warnings quiet for stone/carve reserved accents
        let _ = (carve, stone);
    }
}

fn sky_color(u: f32, pulse: f32) -> Color32 {
    // Deep lagoon → teal → warm horizon gold
    let top = (6.0, 38.0, 44.0);
    let mid = (16.0, 105.0, 92.0);
    let horizon = (205.0 + 25.0 * pulse, 145.0, 65.0);
    let (r, g, b) = if u < 0.55 {
        let t = u / 0.55;
        lerp3(top, mid, t)
    } else {
        let t = (u - 0.55) / 0.45;
        lerp3(mid, horizon, t)
    };
    Color32::from_rgb(r as u8, g as u8, b as u8)
}

fn lerp3(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn paint_island(painter: &egui::Painter, rect: Rect, t: f32) {
    let base_y = rect.top() + rect.height() * 0.58;
    let steps = 180;
    // denser strips = smoother ridge
    for i in 0..steps {
        let u0 = i as f32 / steps as f32;
        let u1 = (i + 1) as f32 / steps as f32;
        let x0 = rect.left() + rect.width() * u0;
        let x1 = rect.left() + rect.width() * u1;
        let ridge = |nx: f32| {
            (-((nx - 0.35) * 7.0).powi(2)).exp() * 125.0
                + (-((nx - 0.62) * 9.0).powi(2)).exp() * 72.0
                + (-((nx - 0.48) * 14.0).powi(2)).exp() * 28.0
                + (t * 0.2 + nx * 4.0).sin() * 3.0
        };
        let y = base_y - ridge((u0 + u1) * 0.5);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1 + 1.2, rect.bottom())),
            0.0,
            Color32::from_rgb(12, 48, 40),
        );
        // rock highlight edge
        painter.line_segment(
            [Pos2::new(x0, y), Pos2::new(x1, base_y - ridge(u1))],
            Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(60, 110, 90, 90)),
        );
    }

    // Ceiba canopy clusters
    for (cx, cy, r) in [
        (0.26, 0.49, 28.0),
        (0.32, 0.51, 20.0),
        (0.54, 0.48, 24.0),
        (0.70, 0.52, 17.0),
    ] {
        let p = Pos2::new(
            rect.left() + rect.width() * cx,
            rect.top() + rect.height() * cy + (t * 0.5).sin() * 2.0,
        );
        painter.circle_filled(p, r, Color32::from_rgb(16, 64, 48));
        painter.circle_filled(p + Vec2::new(10.0, 4.0), r * 0.7, Color32::from_rgb(20, 78, 56));
    }

    // Carved stones on the ridge (Caguana-style monoliths)
    let gold = Color32::from_rgba_unmultiplied(230, 200, 120, 180);
    for (cx, face_s) in [(0.22, 10.0), (0.48, 12.0), (0.66, 9.0)] {
        let x = rect.left() + rect.width() * cx;
        let y = base_y - 40.0;
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x, y), Vec2::new(18.0, 36.0)),
            2.0,
            Color32::from_rgb(40, 58, 48),
        );
        crate::symbols::paint_carved_face(painter, Pos2::new(x, y - 4.0), face_s, gold);
    }
}

fn paint_waves(painter: &egui::Painter, rect: Rect, t: f32) {
    let ocean_top = rect.top() + rect.height() * 0.62;
    let layers = [
        (Color32::from_rgba_unmultiplied(20, 90, 95, 185), 0.9, 18.0, 0.0),
        (Color32::from_rgba_unmultiplied(30, 130, 120, 165), 1.3, 12.0, 1.2),
        (Color32::from_rgba_unmultiplied(50, 170, 140, 135), 1.8, 8.0, 2.4),
        (Color32::from_rgba_unmultiplied(240, 200, 120, 55), 2.2, 5.0, 3.1),
    ];
    for (color, speed, amp, phase) in layers {
        let steps = 120;
        for i in 0..steps {
            let u0 = i as f32 / steps as f32;
            let u1 = (i + 1) as f32 / steps as f32;
            let x0 = rect.left() + rect.width() * u0;
            let x1 = rect.left() + rect.width() * u1;
            let y0 = ocean_top + (t * speed + u0 * 8.0 + phase).sin() * amp;
            let y1 = ocean_top + (t * speed + u1 * 8.0 + phase).sin() * amp;
            let y = (y0 + y1) * 0.5;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1 + 1.5, rect.bottom())),
                0.0,
                color,
            );
            painter.line_segment(
                [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(220, 255, 240, 70)),
            );
        }
    }
}

pub fn glass_panel(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui)) {
    let available = ui.available_width().min(max_width);
    let response = egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(8, 28, 24, 175))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(242, 199, 90, 110)))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::symmetric(18.0, 16.0))
        .show(ui, |ui| {
            ui.set_width(available - 36.0);
            add(ui);
        });
    // Petroglyph corner ornaments on the finished panel
    paint_panel_ornament(
        ui.painter(),
        response.response.rect,
        Color32::from_rgba_unmultiplied(242, 199, 90, 160),
        ui.input(|i| i.time) as f32,
    );
}

pub fn brand_title(ui: &mut Ui, pulse: f32) {
    let scale = 1.0 + 0.015 * pulse;
    ui.horizontal(|ui| {
        // Mini Sol mark beside the brand
        let (resp, painter) = ui.allocate_painter(Vec2::splat(42.0), egui::Sense::hover());
        paint_sol_taino(
            &painter,
            resp.rect.center(),
            16.0 + 1.5 * pulse,
            ui.input(|i| i.time) as f32,
            Color32::from_rgb(242, 199, 90),
        );
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.heading(
                egui::RichText::new("BORIKÉN")
                    .size(52.0 * scale)
                    .color(Color32::from_rgb(255, 248, 230))
                    .strong(),
            );
        });
    });
}
