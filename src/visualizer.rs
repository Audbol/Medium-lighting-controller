//! Stage view: fixture page 1 (channels 1-12) on the front truss, page 2
//! (13-24) on the back truss, beams through haze and pools on the floor.

use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, vec2};

use crate::engine::{CHANNELS, ChannelOut, STRIPS, Show};
use crate::fixtures::appearance;

pub struct Visualizer {
    pub haze: bool,
}

/// Light colour at `a` (0..1) for additive blending: premultiplied with zero alpha.
fn light(r: f32, g: f32, b: f32, a: f32) -> Color32 {
    let k = 255.0 * a.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied((r * k) as u8, (g * k) as u8, (b * k) as u8, 0)
}

impl Visualizer {
    /// Draws the stage; returns a fixture channel if one was clicked.
    pub fn show(
        &self,
        ui: &mut Ui,
        show: &Show,
        out: &[ChannelOut; CHANNELS],
        selected: &[bool; CHANNELS],
        time: f64,
    ) -> Option<usize> {
        let size = ui.available_size().max(vec2(200.0, 160.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter_at(rect);
        let (w, h) = (rect.width(), rect.height());
        let at = |x: f32, y: f32| rect.min + vec2(x, y);
        // Floor: u -1 (stage left) .. 1, v 0 (upstage) .. 1 (downstage).
        let floor = |u: f32, v: f32| {
            let y = h * (0.56 + 0.42 * v);
            let half = w * (0.30 + 0.2 * v);
            at(w / 2.0 + u * half, y)
        };

        p.rect_filled(
            Rect::from_min_size(rect.min, vec2(w, h * 0.56)),
            0.0,
            Color32::from_rgb(9, 10, 15),
        );
        p.rect_filled(
            Rect::from_min_max(at(0.0, h * 0.56), rect.max),
            0.0,
            Color32::from_rgb(24, 26, 34),
        );
        let grid = Stroke::new(1.0, Color32::from_white_alpha(14));
        for i in 0..=8 {
            let u = -1.0 + i as f32 / 4.0;
            p.line_segment([floor(u, 0.0), floor(u, 1.0)], grid);
        }
        for i in 0..=4 {
            let v = i as f32 / 4.0;
            p.line_segment([floor(-1.0, v), floor(1.0, v)], grid);
        }

        struct Fx {
            ch: usize,
            pos: Pos2,
            scale: f32,
            patched: bool,
            colour: (f32, f32, f32),
            level: f32,
            mover: bool,
        }
        let steel = Color32::from_rgb(46, 49, 60);
        let mut fixtures = Vec::new();
        // Back truss first so the front one draws over it.
        for (page, truss_y, x0, x1, scale, aim_v) in [
            (1usize, h * 0.09, w * 0.16, w * 0.84, 0.75f32, 0.25f32),
            (0, h * 0.24, w * 0.06, w * 0.94, 1.0, 0.7),
        ] {
            p.rect_filled(
                Rect::from_min_max(at(x0 - 12.0, truss_y - 15.0), at(x1 + 12.0, truss_y - 10.0)),
                1.0,
                steel,
            );
            p.rect_filled(
                Rect::from_min_max(at(x0 - 12.0, truss_y - 5.0), at(x1 + 12.0, truss_y - 2.0)),
                1.0,
                steel,
            );
            for i in 0..STRIPS {
                let ch = page * STRIPS + i;
                let x = x0 + (x1 - x0) * i as f32 / (STRIPS - 1) as f32;
                let pos = at(x, truss_y);
                let Some(patched) = show.patched(ch) else {
                    fixtures.push(Fx {
                        ch,
                        pos,
                        scale,
                        patched: false,
                        mover: false,
                        colour: (0.0, 0.0, 0.0),
                        level: 0.0,
                    });
                    continue;
                };
                let a = appearance(&patched.def, &out[ch]);
                let mut level = a.level;
                if let Some(hz) = a.strobe_hz
                    && (time * hz as f64).fract() >= 0.25
                {
                    level = 0.0;
                }
                let colour = (a.colour.r, a.colour.g, a.colour.b);
                let mut u = ((x - w / 2.0) / (w / 2.0)) * 0.85;
                let mut v = aim_v;
                // Moving heads aim by pan (sideways) and tilt (up/downstage).
                if let Some((pan, tilt)) = a.position {
                    u = (u + (pan - 0.5) * 2.6).clamp(-1.3, 1.3);
                    v = (aim_v + (tilt - 0.5) * 1.6).clamp(-0.15, 1.1);
                }
                if level > 0.002 {
                    let spread = (0.07 + a.zoom * 0.14) * scale;
                    self.beam(
                        &p,
                        pos + vec2(0.0, 6.0 * scale),
                        floor(u, v),
                        w * spread,
                        colour,
                        level,
                    );
                }
                fixtures.push(Fx {
                    ch,
                    pos,
                    scale,
                    mover: a.position.is_some(),
                    patched: true,
                    colour,
                    level,
                });
            }
        }

        for fx in &fixtures {
            let body = if fx.patched {
                Color32::from_rgb(62, 66, 80)
            } else {
                Color32::from_rgb(28, 30, 38)
            };
            let s = fx.scale;
            if fx.mover {
                // Yoke and round head.
                p.rect_filled(
                    Rect::from_center_size(fx.pos + vec2(0.0, -3.0 * s), vec2(24.0, 6.0) * s),
                    1.0,
                    body,
                );
                p.circle_filled(fx.pos + vec2(0.0, 6.0 * s), 11.0 * s, body);
            } else {
                p.rect_filled(
                    Rect::from_center_size(fx.pos + vec2(0.0, 3.0 * s), vec2(20.0, 20.0) * s),
                    3.0,
                    body,
                );
            }
            let lens = if fx.level > 0.002 {
                let (r, g, b) = fx.colour;
                let c = |x: f32| (40.0 + x * fx.level * 215.0) as u8;
                Color32::from_rgb(c(r), c(g), c(b))
            } else {
                Color32::from_rgb(14, 14, 16)
            };
            p.circle_filled(fx.pos + vec2(0.0, 6.0 * s), 6.0 * s, lens);
            p.text(
                fx.pos + vec2(0.0, -24.0 * s),
                egui::Align2::CENTER_CENTER,
                (fx.ch + 1).to_string(),
                egui::FontId::proportional(11.0 * s.max(0.9)),
                if selected[fx.ch] {
                    Color32::from_rgb(90, 160, 255)
                } else {
                    Color32::from_white_alpha(110)
                },
            );
        }

        // Clicking a fixture selects it, like its select button.
        response
            .interact_pointer_pos()
            .filter(|_| response.clicked())
            .and_then(|pos| {
                fixtures
                    .iter()
                    .filter(|fx| fx.patched)
                    .find(|fx| (fx.pos + vec2(0.0, 3.0)).distance(pos) < 16.0 * fx.scale)
                    .map(|fx| fx.ch)
            })
    }

    fn beam(
        &self,
        p: &egui::Painter,
        from: Pos2,
        to: Pos2,
        radius: f32,
        (r, g, b): (f32, f32, f32),
        level: f32,
    ) {
        if r.max(g).max(b) * level <= 0.002 {
            return;
        }
        let dir = (to - from).normalized();
        let n = Vec2::new(-dir.y, dir.x);

        if self.haze {
            let mut mesh = Mesh::default();
            let top = light(r, g, b, 0.4 * level);
            let bottom = light(r, g, b, 0.04 * level);
            mesh.colored_vertex(from + n * 5.0, top);
            mesh.colored_vertex(from - n * 5.0, top);
            mesh.colored_vertex(to - vec2(n.x * radius, n.y * radius * 0.35), bottom);
            mesh.colored_vertex(to + vec2(n.x * radius, n.y * radius * 0.35), bottom);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            p.add(Shape::mesh(mesh));
        }

        // Floor pool: a squashed radial gradient as a triangle fan.
        let mut mesh = Mesh::default();
        mesh.colored_vertex(to, light(r, g, b, 0.8 * level));
        let rings = [(0.6, 0.3), (1.0, 0.0)];
        let segments = 40;
        for (radius_k, alpha) in rings {
            for i in 0..segments {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                let off = vec2(
                    a.cos() * radius * radius_k,
                    a.sin() * radius * radius_k * 0.32,
                );
                mesh.colored_vertex(to + off, light(r, g, b, alpha * level));
            }
        }
        for i in 0..segments as u32 {
            let j = (i + 1) % segments as u32;
            mesh.add_triangle(0, 1 + i, 1 + j);
            let (a, b2) = (1 + i, 1 + j);
            let (c, d) = (1 + segments as u32 + i, 1 + segments as u32 + j);
            mesh.add_triangle(a, c, d);
            mesh.add_triangle(a, d, b2);
        }
        p.add(Shape::mesh(mesh));
    }
}
