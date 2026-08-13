//! Cinematic island atmosphere renderer for Boriken desktop.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Ui, Vec2};
use rand::Rng;
use std::f32::consts::TAU;

#[derive(Clone)]
pub struct Particle {
    pub pos: Pos2,
    pub vel: Vec2,
    pub size: f32,
    pub phase: f32,
    pub kind: u8, // 0 mote, 1 spark, 2 petal
}

pub struct Scene {
    pub t: f32,
    pub particles: Vec<Particle>,
    pub pulse: f32,
}

impl Scene {
    pub fn new(w: f32, h: f32) -> Self {
        let mut rng = rand::thread_rng();
        let mut particles = Vec::with_capacity(140);
        for _ in 0..140 {
            particles.push(Particle {
                pos: Pos2::new(rng.gen_range(0.0..w.max(1.0)), rng.gen_range(0.0..h.max(1.0))),
                vel: Vec2::new(rng.gen_range(-12.0..12.0), rng.gen_range(-18.0..-4.0)),
                size: rng.gen_range(1.2..4.5),
                phase: rng.gen_range(0.0..TAU),
                kind: rng.gen_range(0..3),
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

        // Deep animated sky gradient bands
        let bands = 48;
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

        // Sun disc + soft rays
        let sun = Pos2::new(
            rect.left() + rect.width() * (0.78 + 0.02 * (t * 0.1).sin()),
            rect.top() + rect.height() * 0.18,
        );
        let sun_r = 42.0 + 4.0 * self.pulse;
        for i in 0..16 {
            let a = t * 0.12 + i as f32 * (TAU / 16.0);
            let len = 90.0 + 30.0 * ((t * 0.8 + i as f32).sin() * 0.5 + 0.5);
            let c = Color32::from_rgba_unmultiplied(255, 210, 120, 28);
            painter.line_segment(
                [
                    sun + Vec2::angled(a) * (sun_r + 4.0),
                    sun + Vec2::angled(a) * (sun_r + len),
                ],
                Stroke::new(3.0_f32, c),
            );
        }
        painter.circle_filled(sun, sun_r + 18.0, Color32::from_rgba_unmultiplied(255, 200, 90, 40));
        painter.circle_filled(sun, sun_r, Color32::from_rgb(255, 214, 120));
        painter.circle_filled(sun, sun_r * 0.55, Color32::from_rgb(255, 236, 180));

        // Distant island / mountain silhouette
        paint_island(&painter, rect, t);

        // Layered animated ocean
        paint_waves(&painter, rect, t);

        // Floating particles
        for p in &self.particles {
            let alpha = match p.kind {
                1 => 160,
                2 => 110,
                _ => 90,
            };
            let color = match p.kind {
                1 => Color32::from_rgba_unmultiplied(255, 220, 140, alpha),
                2 => Color32::from_rgba_unmultiplied(240, 170, 120, alpha),
                _ => Color32::from_rgba_unmultiplied(200, 255, 230, alpha),
            };
            let wobble = Pos2::new(p.pos.x + p.phase.cos() * 3.0, p.pos.y);
            painter.circle_filled(wobble, p.size * (0.85 + 0.25 * p.phase.sin()), color);
        }

        // Vignette
        let vig = Color32::from_rgba_unmultiplied(0, 20, 16, 70);
        painter.rect_filled(
            Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + 70.0)),
            0.0,
            vig,
        );
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 90.0), rect.max),
            0.0,
            Color32::from_rgba_unmultiplied(0, 25, 20, 90),
        );
    }
}

fn sky_color(u: f32, pulse: f32) -> Color32 {
    // Deep lagoon → teal → warm horizon gold (avoid purple / cream biases)
    let top = (8.0, 42.0, 48.0);
    let mid = (18.0, 110.0, 95.0);
    let horizon = (210.0 + 20.0 * pulse, 150.0, 70.0);
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
    let mut pts = Vec::new();
    let steps = 80;
    pts.push(Pos2::new(rect.left(), rect.bottom()));
    for i in 0..=steps {
        let x = rect.left() + rect.width() * (i as f32 / steps as f32);
        let nx = i as f32 / steps as f32;
        let ridge = (-((nx - 0.35) * 7.0).powi(2)).exp() * 120.0
            + (-((nx - 0.62) * 9.0).powi(2)).exp() * 70.0
            + (t * 0.2 + nx * 4.0).sin() * 4.0;
        pts.push(Pos2::new(x, base_y - ridge));
    }
    pts.push(Pos2::new(rect.right(), rect.bottom()));
    // Fill silhouette via dense vertical strips for compatibility
    for i in 0..steps {
        let a = pts[i + 1];
        let b = pts[i + 2];
        let x0 = a.x;
        let x1 = b.x;
        let y = a.y.min(b.y);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1 + 1.0, rect.bottom())),
            0.0,
            Color32::from_rgb(12, 48, 40),
        );
    }
    // Ceiba-like canopy accents
    for (cx, cy, r) in [
        (0.28, 0.50, 26.0),
        (0.33, 0.52, 18.0),
        (0.55, 0.49, 22.0),
        (0.72, 0.53, 16.0),
    ] {
        let p = Pos2::new(
            rect.left() + rect.width() * cx,
            rect.top() + rect.height() * cy + (t * 0.5).sin() * 2.0,
        );
        painter.circle_filled(p, r, Color32::from_rgb(16, 64, 48));
        painter.circle_filled(p + Vec2::new(10.0, 4.0), r * 0.7, Color32::from_rgb(20, 78, 56));
    }
}

fn paint_waves(painter: &egui::Painter, rect: Rect, t: f32) {
    let ocean_top = rect.top() + rect.height() * 0.62;
    let layers = [
        (Color32::from_rgba_unmultiplied(20, 90, 95, 180), 0.9, 18.0, 0.0),
        (Color32::from_rgba_unmultiplied(30, 130, 120, 160), 1.3, 12.0, 1.2),
        (Color32::from_rgba_unmultiplied(50, 170, 140, 130), 1.8, 8.0, 2.4),
        (Color32::from_rgba_unmultiplied(240, 200, 120, 50), 2.2, 5.0, 3.1),
    ];
    for (color, speed, amp, phase) in layers {
        let steps = 90;
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
            // foam highlights
            painter.line_segment(
                [Pos2::new(x0, y0), Pos2::new(x1, y1)],
                Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(220, 255, 240, 70)),
            );
        }
    }
}

pub fn glass_panel(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui)) {
    let available = ui.available_width().min(max_width);
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(8, 28, 24, 170))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(242, 199, 90, 90)))
        .rounding(Rounding::ZERO)
        .inner_margin(egui::Margin::symmetric(18.0, 16.0))
        .show(ui, |ui| {
            ui.set_width(available - 36.0);
            add(ui);
        });
}

pub fn brand_title(ui: &mut Ui, pulse: f32) {
    let scale = 1.0 + 0.015 * pulse;
    ui.heading(
        egui::RichText::new("BORIKÉN")
            .size(54.0 * scale)
            .color(Color32::from_rgb(255, 248, 230))
            .strong(),
    );
}
