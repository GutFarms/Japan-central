//! High-fidelity Borikén Taíno petroglyph / symbol drawing.
//! Motifs inspired by Puerto Rican sites (Caguana, Tibes, Jayuya Sol, Piedra Escrita)
//! rendered as respectful stylized vector carvings—not archaeological replicas.

use eframe::egui::{Color32, Painter, Pos2, Stroke, Vec2};
use std::f32::consts::{PI, TAU};

#[inline]
fn stroke(w: f32, c: Color32) -> Stroke {
    Stroke::new(w, c)
}

pub fn circle_stroke(painter: &Painter, c: Pos2, r: f32, width: f32, color: Color32) {
    let n = ((r * 1.8).clamp(24.0, 96.0)) as i32;
    for i in 0..n {
        let a0 = i as f32 / n as f32 * TAU;
        let a1 = (i + 1) as f32 / n as f32 * TAU;
        painter.line_segment(
            [c + Vec2::angled(a0) * r, c + Vec2::angled(a1) * r],
            stroke(width, color),
        );
    }
}

pub fn poly_stroke(painter: &Painter, pts: &[Pos2], width: f32, color: Color32, closed: bool) {
    if pts.len() < 2 {
        return;
    }
    for i in 0..pts.len() - 1 {
        painter.line_segment([pts[i], pts[i + 1]], stroke(width, color));
    }
    if closed {
        painter.line_segment([pts[pts.len() - 1], pts[0]], stroke(width, color));
    }
}

pub fn filled_poly(painter: &Painter, pts: &[Pos2], color: Color32) {
    // Fan fill via thin triangles approximated as short thick segments from centroid.
    if pts.len() < 3 {
        return;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for p in pts {
        cx += p.x;
        cy += p.y;
    }
    let n = pts.len() as f32;
    let center = Pos2::new(cx / n, cy / n);
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        // Dense scan fill between triangle edges
        for k in 0..8 {
            let t = k as f32 / 7.0;
            let p0 = Pos2::new(a.x + (center.x - a.x) * t, a.y + (center.y - a.y) * t);
            let p1 = Pos2::new(b.x + (center.x - b.x) * t, b.y + (center.y - b.y) * t);
            painter.line_segment([p0, p1], stroke(2.2, color));
        }
    }
}

/// Iconic Sol Taíno / Sol de Jayuya–inspired sun face with triangular rays.
pub fn paint_sol_taino(painter: &Painter, center: Pos2, radius: f32, t: f32, gold: Color32) {
    let pulse = 0.5 + 0.5 * (t * 0.8).sin();
    let r = radius * (0.98 + 0.04 * pulse);
    let ray_n = 20;
    let ink = Color32::from_rgba_unmultiplied(gold.r(), gold.g(), gold.b(), 230);
    let deep = Color32::from_rgb(36, 22, 8);
    let face = Color32::from_rgb(255, 214, 120);
    let rim = Color32::from_rgb(242, 180, 70);

    // Outer aura rings
    for i in 0..4 {
        let rr = r * (1.35 + i as f32 * 0.12) + pulse * 3.0;
        circle_stroke(
            painter,
            center,
            rr,
            1.5,
            Color32::from_rgba_unmultiplied(255, 200, 90, 35 - i as u8 * 6),
        );
    }

    // Triangular rays (classic Sol Taíno look)
    for i in 0..ray_n {
        let a = t * 0.05 + i as f32 * (TAU / ray_n as f32);
        let inner = r * 1.05;
        let outer = r * (1.55 + 0.12 * ((t * 1.3 + i as f32).sin() * 0.5 + 0.5));
        let half = 0.11;
        let tip = center + Vec2::angled(a) * outer;
        let left = center + Vec2::angled(a - half) * inner;
        let right = center + Vec2::angled(a + half) * inner;
        let pts = [left, tip, right];
        filled_poly(painter, &pts, Color32::from_rgba_unmultiplied(255, 196, 80, 200));
        poly_stroke(painter, &pts, 1.6, deep, true);
    }

    // Disc
    painter.circle_filled(center, r * 1.02, rim);
    painter.circle_filled(center, r * 0.92, face);
    circle_stroke(painter, center, r * 0.92, 3.0, deep);
    circle_stroke(painter, center, r * 0.78, 1.6, Color32::from_rgba_unmultiplied(80, 40, 10, 140));

    // Face — petroglyph style eyes / nose / mouth
    let eye_y = center.y - r * 0.18;
    let eye_dx = r * 0.28;
    let eye_r = r * 0.13;
    // almond / carved eyes
    for sign in [-1.0_f32, 1.0] {
        let e = Pos2::new(center.x + sign * eye_dx, eye_y);
        painter.circle_filled(e, eye_r, deep);
        painter.circle_filled(e + Vec2::new(-eye_r * 0.2, -eye_r * 0.15), eye_r * 0.35, face);
        // brow arcs
        for i in 0..12 {
            let a0 = PI + 0.35 + i as f32 / 12.0 * 0.9;
            let a1 = PI + 0.35 + (i + 1) as f32 / 12.0 * 0.9;
            let o = e + Vec2::new(0.0, -eye_r * 0.3);
            painter.line_segment(
                [
                    o + Vec2::angled(a0) * eye_r * 1.6,
                    o + Vec2::angled(a1) * eye_r * 1.6,
                ],
                stroke(2.4, deep),
            );
        }
    }

    // triangular nose
    let nose_top = Pos2::new(center.x, center.y - r * 0.05);
    let nose_l = Pos2::new(center.x - r * 0.1, center.y + r * 0.18);
    let nose_r = Pos2::new(center.x + r * 0.1, center.y + r * 0.18);
    poly_stroke(painter, &[nose_top, nose_l, nose_r], 2.5, deep, true);

    // crescent mouth
    let mouth_c = Pos2::new(center.x, center.y + r * 0.28);
    for i in 0..16 {
        let a0 = 0.25 * PI + i as f32 / 16.0 * 0.5 * PI;
        let a1 = 0.25 * PI + (i + 1) as f32 / 16.0 * 0.5 * PI;
        painter.line_segment(
            [
                mouth_c + Vec2::angled(a0) * r * 0.32,
                mouth_c + Vec2::angled(a1) * r * 0.32,
            ],
            stroke(3.0, deep),
        );
    }

    // cheek spirals (petroglyph flourish)
    paint_spiral(
        painter,
        Pos2::new(center.x - r * 0.55, center.y + r * 0.05),
        r * 0.12,
        2.2,
        ink,
        t * 0.2,
    );
    paint_spiral(
        painter,
        Pos2::new(center.x + r * 0.55, center.y + r * 0.05),
        r * 0.12,
        2.2,
        ink,
        -t * 0.2,
    );
}

/// Three-pointed cemí (zemí) form — sacred figure silhouette.
pub fn paint_cemi(painter: &Painter, base: Pos2, scale: f32, color: Color32, t: f32) {
    let bob = (t * 0.9).sin() * 2.0;
    let o = Pos2::new(base.x, base.y + bob);
    let s = scale;
    // classic three-pointed outline
    let pts = [
        Pos2::new(o.x, o.y - s * 1.15),            // top point
        Pos2::new(o.x + s * 0.35, o.y - s * 0.35),
        Pos2::new(o.x + s * 0.95, o.y + s * 0.15), // right point
        Pos2::new(o.x + s * 0.25, o.y + s * 0.35),
        Pos2::new(o.x, o.y + s * 0.85),            // bottom
        Pos2::new(o.x - s * 0.25, o.y + s * 0.35),
        Pos2::new(o.x - s * 0.95, o.y + s * 0.15), // left point
        Pos2::new(o.x - s * 0.35, o.y - s * 0.35),
    ];
    filled_poly(painter, &pts, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 180));
    poly_stroke(painter, &pts, 2.4, color, true);
    // carved face dots
    painter.circle_filled(Pos2::new(o.x - s * 0.18, o.y - s * 0.15), s * 0.08, color);
    painter.circle_filled(Pos2::new(o.x + s * 0.18, o.y - s * 0.15), s * 0.08, color);
    painter.circle_filled(Pos2::new(o.x, o.y + s * 0.1), s * 0.06, color);
}

/// Coquí petroglyph — stylized frog of Borikén.
pub fn paint_coqui(painter: &Painter, c: Pos2, scale: f32, color: Color32, t: f32) {
    let bounce = (t * 3.0).sin().abs() * 2.0;
    let o = Pos2::new(c.x, c.y - bounce);
    let s = scale;
    // body oval via stroked ellipse
    for i in 0..36 {
        let a0 = i as f32 / 36.0 * TAU;
        let a1 = (i + 1) as f32 / 36.0 * TAU;
        let p0 = Pos2::new(o.x + a0.cos() * s * 0.7, o.y + a0.sin() * s * 0.5);
        let p1 = Pos2::new(o.x + a1.cos() * s * 0.7, o.y + a1.sin() * s * 0.5);
        painter.line_segment([p0, p1], stroke(2.5, color));
    }
    // head
    painter.circle_filled(Pos2::new(o.x, o.y - s * 0.55), s * 0.38, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 60));
    circle_stroke(painter, Pos2::new(o.x, o.y - s * 0.55), s * 0.38, 2.4, color);
    // eyes
    painter.circle_filled(Pos2::new(o.x - s * 0.16, o.y - s * 0.7), s * 0.12, color);
    painter.circle_filled(Pos2::new(o.x + s * 0.16, o.y - s * 0.7), s * 0.12, color);
    // legs
    let legs = [
        (Vec2::new(-s * 0.85, s * 0.15), Vec2::new(-s * 1.1, s * 0.55)),
        (Vec2::new(s * 0.85, s * 0.15), Vec2::new(s * 1.1, s * 0.55)),
        (Vec2::new(-s * 0.55, s * 0.45), Vec2::new(-s * 0.7, s * 0.9)),
        (Vec2::new(s * 0.55, s * 0.45), Vec2::new(s * 0.7, s * 0.9)),
    ];
    for (a, b) in legs {
        painter.line_segment([o + a * 0.3 + a, o + b], stroke(2.6, color));
        // toe split
        let tip = o + b;
        painter.line_segment([tip, tip + Vec2::new(-4.0, 5.0)], stroke(2.0, color));
        painter.line_segment([tip, tip + Vec2::new(4.0, 5.0)], stroke(2.0, color));
    }
}

pub fn paint_spiral(painter: &Painter, c: Pos2, scale: f32, width: f32, color: Color32, spin: f32) {
    let turns = 2.4;
    let steps = 64;
    let mut prev = c + Vec2::angled(spin) * 1.0;
    for i in 1..=steps {
        let u = i as f32 / steps as f32;
        let a = spin + u * turns * TAU;
        let r = scale * u;
        let p = c + Vec2::angled(a) * r;
        painter.line_segment([prev, p], stroke(width, color));
        prev = p;
    }
}

/// Turtle (carey) petroglyph — common Caribbean Taíno motif.
pub fn paint_turtle(painter: &Painter, c: Pos2, scale: f32, color: Color32) {
    let s = scale;
    // shell oval
    for i in 0..40 {
        let a0 = i as f32 / 40.0 * TAU;
        let a1 = (i + 1) as f32 / 40.0 * TAU;
        let p0 = Pos2::new(c.x + a0.cos() * s * 0.85, c.y + a0.sin() * s * 0.6);
        let p1 = Pos2::new(c.x + a1.cos() * s * 0.85, c.y + a1.sin() * s * 0.6);
        painter.line_segment([p0, p1], stroke(2.3, color));
    }
    // shell grid
    painter.line_segment(
        [Pos2::new(c.x - s * 0.7, c.y), Pos2::new(c.x + s * 0.7, c.y)],
        stroke(1.8, color),
    );
    painter.line_segment(
        [Pos2::new(c.x, c.y - s * 0.45), Pos2::new(c.x, c.y + s * 0.45)],
        stroke(1.8, color),
    );
    // head + flippers
    painter.circle_filled(Pos2::new(c.x + s * 1.05, c.y), s * 0.22, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 80));
    circle_stroke(painter, Pos2::new(c.x + s * 1.05, c.y), s * 0.22, 2.0, color);
    for (dx, dy) in [(-0.7, -0.7), (-0.7, 0.7), (0.55, -0.75), (0.55, 0.75)] {
        painter.line_segment(
            [c, Pos2::new(c.x + dx * s, c.y + dy * s)],
            stroke(2.4, color),
        );
    }
}

/// Simple carved face petroglyph (monolith style).
pub fn paint_carved_face(painter: &Painter, c: Pos2, scale: f32, color: Color32) {
    circle_stroke(painter, c, scale, 2.5, color);
    painter.circle_filled(Pos2::new(c.x - scale * 0.28, c.y - scale * 0.1), scale * 0.12, color);
    painter.circle_filled(Pos2::new(c.x + scale * 0.28, c.y - scale * 0.1), scale * 0.12, color);
    // mouth line
    painter.line_segment(
        [
            Pos2::new(c.x - scale * 0.3, c.y + scale * 0.35),
            Pos2::new(c.x + scale * 0.3, c.y + scale * 0.35),
        ],
        stroke(2.4, color),
    );
}

/// Decorative petroglyph frieze along a horizontal band.
pub fn paint_petroglyph_frieze(painter: &Painter, y: f32, left: f32, right: f32, t: f32, color: Color32) {
    let span = right - left;
    let count = 7;
    for i in 0..count {
        let u = (i as f32 + 0.5) / count as f32;
        let x = left + span * u;
        let c = Pos2::new(x, y);
        match i % 4 {
            0 => paint_spiral(painter, c, 14.0, 1.8, color, t * 0.4 + i as f32),
            1 => paint_carved_face(painter, c, 12.0, color),
            2 => {
                // star / cross glyph
                for k in 0..4 {
                    let a = k as f32 * (PI / 2.0) + t * 0.1;
                    painter.line_segment(
                        [c + Vec2::angled(a) * 4.0, c + Vec2::angled(a) * 14.0],
                        stroke(2.0, color),
                    );
                }
                circle_stroke(painter, c, 5.0, 1.6, color);
            }
            _ => {
                // water chevrons
                for row in 0..2 {
                    let yy = c.y - 6.0 + row as f32 * 8.0;
                    painter.line_segment(
                        [Pos2::new(c.x - 12.0, yy), Pos2::new(c.x, yy - 5.0)],
                        stroke(2.0, color),
                    );
                    painter.line_segment(
                        [Pos2::new(c.x, yy - 5.0), Pos2::new(c.x + 12.0, yy)],
                        stroke(2.0, color),
                    );
                }
            }
        }
    }
}

/// Corner ornament for UI panels — miniature Sol + spirals.
pub fn paint_panel_ornament(painter: &Painter, rect: eframe::egui::Rect, color: Color32, t: f32) {
    let gold = color;
    let corners = [
        Pos2::new(rect.left() + 18.0, rect.top() + 18.0),
        Pos2::new(rect.right() - 18.0, rect.top() + 18.0),
        Pos2::new(rect.left() + 18.0, rect.bottom() - 18.0),
        Pos2::new(rect.right() - 18.0, rect.bottom() - 18.0),
    ];
    for (i, c) in corners.iter().enumerate() {
        paint_spiral(painter, *c, 10.0, 1.5, gold, t * 0.3 + i as f32);
        // small ray ticks
        for k in 0..6 {
            let a = k as f32 / 6.0 * TAU + t * 0.2;
            painter.line_segment(
                [*c + Vec2::angled(a) * 12.0, *c + Vec2::angled(a) * 16.0],
                stroke(1.4, gold),
            );
        }
    }
}
