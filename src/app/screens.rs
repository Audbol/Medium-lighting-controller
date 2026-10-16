//! The touchscreen windows.

use std::net::Ipv4Addr;

use eframe::egui::{
    self, Align2, Color32, FontId, Mesh, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2,
};

use super::panel::rgb32;
use super::{ACCENT, App, ColourTab, EditTarget, Editor, LED_GREEN, LED_RED, Screen, Settings};
use crate::colour::{Hs, Rgb};
use crate::effects::{
    self, BUTTONS_PER_PAGE, CHASE_PAGES, INTENSITY_PAGE, PATTERN_PAGES, SWATCH_PAGES, chase_page,
    chase_preview, pattern_page, pattern_preview,
};
use crate::engine::{
    CHANNELS, Cell, ChannelOut, Crossfade, CueStart, Desk, Direction, Item, Look, PlayRef,
    SCENE_PAGES, STRIPS, Show, Slot,
};
use crate::fixtures::{
    Blend, ChannelDef, Dial, Feature, FixtureDef, Patched, channel_values, library,
};
use crate::output::Network;

const SCREENS: [(Screen, &str); 9] = [
    (Screen::Home, "Home"),
    (Screen::Colour, "Colour"),
    (Screen::Scenes, "Scenes"),
    (Screen::Chases, "Chases"),
    (Screen::Cuelist, "Cuelist"),
    (Screen::Monitor, "Monitor"),
    (Screen::Shows, "Shows"),
    (Screen::Patch, "Patch"),
    (Screen::Setup, "Setup"),
];

/// A row of tab buttons; returns the one clicked (even if already current).
fn tabs(ui: &mut Ui, current: usize, names: &[&str]) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        for (i, name) in names.iter().enumerate() {
            if ui
                .selectable_label(i == current, RichText::new(*name).size(14.0))
                .clicked()
            {
                clicked = Some(i);
            }
        }
    });
    ui.add_space(4.0);
    clicked
}

/// A button showing colour stripes.
fn colour_button(ui: &mut Ui, colours: &[Color32], label: &str, size: Vec2) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let n = colours.len().max(1);
    let w = rect.width() / n as f32;
    for (i, c) in colours.iter().enumerate() {
        p.rect_filled(
            Rect::from_min_size(
                rect.min + vec2(w * i as f32, 0.0),
                vec2(w + 0.5, rect.height()),
            ),
            0.0,
            *c,
        );
    }
    let stroke = if r.hovered() {
        Stroke::new(2.0, Color32::WHITE)
    } else {
        Stroke::new(1.0, Color32::from_black_alpha(160))
    };
    p.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Inside);
    if !label.is_empty() {
        let text_rect = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 15.0), rect.max);
        p.rect_filled(text_rect, 0.0, Color32::from_black_alpha(150));
        p.text(
            text_rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.0),
            Color32::WHITE,
        );
    }
    r
}

fn cell_out(cell: &Cell) -> ChannelOut {
    let hs = cell.colour.unwrap_or(crate::colour::WHITE);
    let level = cell.level.unwrap_or(0.0);
    ChannelOut {
        level,
        intensity: level,
        hs,
        rgb: hs.rgb(),
        alt: cell.alt,
        pos: cell.pos,
    }
}

/// Feature values 0-255 for the detailed preview tooltip.
fn cell_detail(d: &Desk, ch: usize, cell: &Cell) -> String {
    let mut s = format!("Fixture {}", ch + 1);
    let Some(p) = d.show.patched(ch) else {
        return s + " (not patched)";
    };
    s += &format!(" · {}\n", p.def.name);
    let blend = Blend {
        white: d.prefs.white_blend,
        amber: d.prefs.amber_blend,
    };
    let values = channel_values(&p.def, &cell_out(cell), blend);
    for (cd, v) in p.def.channels.iter().zip(values) {
        if cd.fine || cd.feature == Feature::Other {
            continue;
        }
        let home = match p.def.dial_of(cd) {
            Dial::ShiftHue => cell.alt[0].is_none(),
            Dial::ShiftSat => cell.alt[1].is_none(),
            _ => false,
        };
        s += &format!(
            "{} {}{}\n",
            cd.display(),
            (v * 255.0).round() as u8,
            if home { " (home)" } else { "" }
        );
    }
    s.trim_end().to_string()
}

/// The 24 fixture icons for a scene, step or cue: colour × intensity, X when
/// the fixture isn't part of it. Hover for values.
fn look_icons(ui: &mut Ui, d: &Desk, look: &Look) {
    let size = 15.0;
    let (rect, r) =
        ui.allocate_exact_size(vec2(size * CHANNELS as f32 + 6.0, size), Sense::hover());
    let p = ui.painter();
    for (ch, cell) in look.iter().enumerate() {
        let x = rect.min.x + ch as f32 * size + if ch >= STRIPS { 6.0 } else { 0.0 };
        let c = Rect::from_min_size(pos2(x, rect.min.y), vec2(size - 2.0, size - 2.0));
        if cell.is_empty() {
            p.rect_filled(c, 2.0, Color32::from_gray(30));
            let s = Stroke::new(1.0, Color32::from_gray(70));
            p.line_segment([c.left_top(), c.right_bottom()], s);
            p.line_segment([c.right_top(), c.left_bottom()], s);
        } else {
            let o = cell_out(cell);
            let level = cell.level.unwrap_or(1.0);
            p.rect_filled(c, 2.0, rgb32(o.rgb.scale(0.2 + 0.8 * level)));
        }
    }
    if let Some(pos) = r.hover_pos() {
        let mut x = pos.x - rect.min.x;
        if x >= size * STRIPS as f32 {
            x -= 6.0;
        }
        let ch = ((x / size) as usize).min(CHANNELS - 1);
        r.on_hover_text(cell_detail(d, ch, &look[ch]));
    }
}

fn all_fixtures(show: &Show) -> Vec<FixtureDef> {
    let mut v = library().to_vec();
    v.extend(show.my_fixtures.iter().cloned());
    v
}

/// "Maker · Name · 10ch" without repeating what the name already says.
fn fixture_label(f: &FixtureDef) -> String {
    let mut s = if f.manufacturer == "Generic" {
        f.name.clone()
    } else {
        format!("{} · {}", f.manufacturer, f.name)
    };
    let ch = format!("{}ch", f.footprint());
    if !f.name.ends_with(&ch) {
        s += &format!(" · {ch}");
    }
    s
}

/// Search state for the fixture browser.
#[derive(Default)]
pub(crate) struct Browser {
    pub query: String,
    pub maker: String,
}

/// A searchable list of fixtures; returns the index clicked.
fn fixture_browser(
    ui: &mut Ui,
    id: &str,
    fixtures: &[FixtureDef],
    b: &mut Browser,
    selected: Option<usize>,
) -> Option<usize> {
    let mut makers: Vec<&str> = Vec::new();
    for f in fixtures {
        if !makers.contains(&f.manufacturer.as_str()) {
            makers.push(&f.manufacturer);
        }
    }
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt((id, "maker"))
            .width(130.0)
            .selected_text(if b.maker.is_empty() {
                "All makers"
            } else {
                &b.maker
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut b.maker, String::new(), "All makers");
                for m in &makers {
                    ui.selectable_value(&mut b.maker, m.to_string(), *m);
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut b.query)
                .hint_text("Search: name, RGBW, 7x18, moving…")
                .desired_width(220.0),
        );
        if !b.query.is_empty() && ui.small_button("x").clicked() {
            b.query.clear();
        }
    });
    let words: Vec<String> = b
        .query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let matches: Vec<usize> = fixtures
        .iter()
        .enumerate()
        .filter(|(_, f)| b.maker.is_empty() || f.manufacturer == b.maker)
        .filter(|(_, f)| {
            let hay = format!(
                "{} {} {}{}",
                f.manufacturer,
                f.name,
                f.summary(),
                if f.is_mover() { " moving head" } else { "" }
            )
            .to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    ui.label(
        RichText::new(format!("{} of {} fixtures", matches.len(), fixtures.len()))
            .small()
            .weak(),
    );
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt((id, "list"))
        .max_height(230.0)
        .min_scrolled_height(230.0)
        .auto_shrink([false, true])
        .show_rows(ui, 18.0, matches.len(), |ui, rows| {
            for &i in &matches[rows] {
                let f = &fixtures[i];
                let text = RichText::new(if f.manufacturer == "Generic" {
                    format!("Generic · {}", fixture_label(f))
                } else {
                    fixture_label(f)
                });
                if ui
                    .selectable_label(selected == Some(i), text)
                    .on_hover_text(f.summary())
                    .clicked()
                {
                    clicked = Some(i);
                }
            }
        });
    clicked
}

/// The lowest address with `footprint` free channels, ignoring channel `skip`.
fn free_address(show: &Show, footprint: u16, skip: Option<usize>) -> u16 {
    let mut used = [false; 513];
    for (ch, p) in show.patch.iter().enumerate() {
        if Some(ch) == skip {
            continue;
        }
        if let Some(p) = p {
            for &a in &p.addresses {
                for i in 0..p.def.footprint() {
                    if let Some(u) = used.get_mut((a + i) as usize) {
                        *u = true;
                    }
                }
            }
        }
    }
    (1..=512u16)
        .find(|&a| a + footprint <= 513 && (a..a + footprint).all(|x| !used[x as usize]))
        .unwrap_or(1)
}

/// Channels whose DMX addresses overlap another's.
fn conflicts(show: &Show) -> [bool; CHANNELS] {
    let mut owner: Vec<Option<usize>> = vec![None; 513];
    let mut bad = [false; CHANNELS];
    for (ch, p) in show.patch.iter().enumerate() {
        let Some(p) = p else { continue };
        for &a in &p.addresses {
            for i in 0..p.def.footprint() {
                let slot = (a + i) as usize;
                if slot > 512 {
                    bad[ch] = true;
                    continue;
                }
                match owner[slot] {
                    Some(o) if o != ch => {
                        bad[ch] = true;
                        bad[o] = true;
                    }
                    _ => owner[slot] = Some(ch),
                }
            }
        }
    }
    bad
}

fn playing_dot(ui: &mut Ui, on: bool) {
    ui.label(RichText::new(if on { "▶" } else { " " }).color(LED_GREEN));
}

impl App {
    pub(crate) fn go(&mut self, s: Screen) {
        if s != self.screen {
            self.prev_screen = self.screen;
            self.screen = s;
            self.tab = 0;
        }
    }

    pub(crate) fn touchscreen(&mut self, ui: &mut Ui, d: &mut Desk, t: f64) {
        ui.horizontal_wrapped(|ui| {
            for (s, name) in SCREENS {
                let r = ui.selectable_label(self.screen == s, RichText::new(name).size(14.0));
                if s == Screen::Home && r.double_clicked() {
                    let prev = self.prev_screen;
                    self.go(prev);
                } else if r.clicked() {
                    self.go(s);
                }
            }
            ui.separator();
            let ap = if d.autopilot.on {
                "Autopilot •"
            } else {
                "Autopilot"
            };
            if ui
                .selectable_label(self.screen == Screen::Autopilot, ap)
                .clicked()
            {
                self.go(Screen::Autopilot);
            }
            if ui
                .selectable_label(self.screen == Screen::Help, "Help")
                .clicked()
            {
                self.go(Screen::Help);
            }
            if ui
                .button("Lock")
                .on_hover_text("Lock the console")
                .clicked()
            {
                self.lock();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(("screen", self.screen as u8))
            .auto_shrink(false)
            .show(ui, |ui| match self.screen {
                Screen::Home => self.home(ui, d),
                Screen::Colour => self.colour(ui, d),
                Screen::Scenes => self.scenes(ui, d),
                Screen::Chases => self.chases(ui, d),
                Screen::Cuelist => self.cuelist(ui, d, t),
                Screen::Monitor => self.monitor(ui, d),
                Screen::Shows => self.shows(ui, d),
                Screen::Patch => self.patch(ui, d),
                Screen::Setup => self.setup(ui, d),
                Screen::Autopilot => self.autopilot(ui, d),
                Screen::Help => help(ui),
            });
    }

    // ---- Home -----------------------------------------------------------------------------

    fn home(&mut self, ui: &mut Ui, d: &mut Desk) {
        ui.add_space(8.0);
        let big = vec2(140.0, 64.0);
        for row in [
            [
                Screen::Colour,
                Screen::Scenes,
                Screen::Chases,
                Screen::Cuelist,
            ],
            [Screen::Monitor, Screen::Shows, Screen::Patch, Screen::Setup],
        ] {
            ui.horizontal(|ui| {
                for s in row {
                    let name = SCREENS.iter().find(|x| x.0 == s).unwrap().1;
                    if ui
                        .add_sized(big, egui::Button::new(RichText::new(name).size(18.0)))
                        .clicked()
                    {
                        self.go(s);
                    }
                }
            });
        }
        ui.add_space(12.0);
        let patched = d.patched_channels().len();
        if patched == 0 {
            ui.label(
                RichText::new("Nothing is patched yet: open Patch to add your fixtures.")
                    .color(ACCENT),
            );
        }
        ui.label(format!(
            "{patched} of 24 fixture channels patched · {} selected · fade {:.1} s",
            d.selected_channels().len(),
            d.live_fade
        ));
        ui.add_space(6.0);
        ui.label(RichText::new("Playing").strong());
        let playing = d.playing();
        if playing.is_empty() {
            ui.label(RichText::new("Nothing").weak());
        }
        for (_, name) in playing.iter().take(8) {
            ui.label(name);
        }
    }

    // ---- Colour ---------------------------------------------------------------------------

    fn colour(&mut self, ui: &mut Ui, d: &mut Desk) {
        let current = match self.colour_tab {
            ColourTab::Swatches => 0,
            ColourTab::Picker | ColourTab::Sliders => 1,
            ColourTab::Library => 2,
            ColourTab::Patterns => 3,
            ColourTab::Chases => 4,
        };
        if let Some(i) = tabs(
            ui,
            current,
            &["Swatches", "Colours", "Library", "Patterns", "Chases"],
        ) {
            // Pressing the current tab again moves on, like the console's buttons.
            self.colour_tab = match (i, self.colour_tab) {
                (0, ColourTab::Swatches) => {
                    self.swatch_page = (self.swatch_page + 1) % SWATCH_PAGES;
                    ColourTab::Swatches
                }
                (0, _) => ColourTab::Swatches,
                (1, ColourTab::Picker) => ColourTab::Sliders,
                (1, ColourTab::Sliders) => ColourTab::Picker,
                (1, _) => ColourTab::Picker,
                (2, _) => ColourTab::Library,
                (3, ColourTab::Patterns) => {
                    self.pattern_page = (self.pattern_page + 1) % PATTERN_PAGES.len();
                    ColourTab::Patterns
                }
                (3, _) => ColourTab::Patterns,
                (4, ColourTab::Chases) => {
                    self.chase_page = (self.chase_page + 1) % CHASE_PAGES.len();
                    ColourTab::Chases
                }
                _ => ColourTab::Chases,
            };
        }
        // Page buttons.
        let pages: Option<(Vec<String>, &mut usize)> = match self.colour_tab {
            ColourTab::Swatches => Some((
                (1..=SWATCH_PAGES).map(|p| format!("Page {p}")).collect(),
                &mut self.swatch_page,
            )),
            ColourTab::Patterns => Some((
                PATTERN_PAGES.iter().map(|s| s.to_string()).collect(),
                &mut self.pattern_page,
            )),
            ColourTab::Chases => Some((
                CHASE_PAGES.iter().map(|s| s.to_string()).collect(),
                &mut self.chase_page,
            )),
            _ => None,
        };
        if let Some((names, page)) = pages {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (i, name) in names.iter().enumerate() {
                    if ui
                        .selectable_label(*page == i, RichText::new(name).size(11.0))
                        .clicked()
                    {
                        *page = i;
                    }
                }
            });
            ui.add_space(2.0);
        }
        let n = d.selected_channels().len();
        ui.horizontal(|ui| {
            if n == 0 {
                ui.label(
                    RichText::new(
                        "Select fixtures with their SELECT buttons (or click them on stage).",
                    )
                    .color(ACCENT),
                );
            } else {
                ui.label(format!(
                    "{n} fixture{} selected",
                    if n == 1 { "" } else { "s" }
                ));
                if ui.small_button("Deselect").clicked() {
                    d.clear_selection();
                }
            }
        });
        ui.add_space(6.0);
        let size = vec2(92.0, 58.0);
        match self.colour_tab {
            ColourTab::Swatches => {
                let first = self.swatch_page * BUTTONS_PER_PAGE;
                let swatches: Vec<Rgb> = d.show.swatches[first..first + BUTTONS_PER_PAGE].to_vec();
                for (row, chunk) in swatches.chunks(6).enumerate() {
                    ui.horizontal(|ui| {
                        for (j, c) in chunk.iter().enumerate() {
                            let i = first + row * 6 + j;
                            let r = colour_button(ui, &[rgb32(*c)], "", size)
                                .on_hover_text("Click to apply · right-click to replace");
                            if r.clicked() {
                                d.apply_colour(*c);
                            }
                            r.context_menu(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Picker colour:");
                                    let (rect, _) =
                                        ui.allocate_exact_size(vec2(24.0, 14.0), Sense::hover());
                                    ui.painter().rect_filled(rect, 2.0, rgb32(self.picker));
                                });
                                if ui.button("Replace this swatch with it").clicked() {
                                    d.checkpoint();
                                    d.show.swatches[i] = self.picker;
                                    ui.close();
                                }
                                if ui.button("Use as picker colour").clicked() {
                                    self.picker = *c;
                                    ui.close();
                                }
                                if ui.button("Restore the default").clicked() {
                                    d.checkpoint();
                                    d.show.swatches[i] = effects::DEFAULT_SWATCHES[i];
                                    ui.close();
                                }
                            });
                        }
                    });
                }
                ui.label(
                    RichText::new(
                        "To change a swatch, choose a colour in Colours or Library, then right-click the swatch.",
                    )
                    .small()
                    .weak(),
                );
            }
            ColourTab::Library => {
                for group in effects::library_groups() {
                    ui.label(RichText::new(group).strong());
                    ui.horizontal_wrapped(|ui| {
                        for c in effects::LIBRARY.iter().filter(|c| c.group == group) {
                            let r = colour_button(ui, &[rgb32(c.rgb)], c.name, vec2(92.0, 40.0))
                                .on_hover_text(format!(
                                    "{} · click to apply, right-click to use as the picker colour",
                                    c.name
                                ));
                            if r.clicked() {
                                d.apply_colour(c.rgb);
                                self.picker = c.rgb;
                            }
                            r.context_menu(|ui| {
                                if ui.button("Use as picker colour").clicked() {
                                    self.picker = c.rgb;
                                    ui.close();
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);
                }
            }
            ColourTab::Picker => {
                let w = ui.available_width().min(560.0);
                let (rect, r) = ui.allocate_exact_size(vec2(w, 200.0), Sense::click_and_drag());
                let mut mesh = Mesh::default();
                let (cols, rows) = (36u32, 6u32);
                for y in 0..=rows {
                    for x in 0..=cols {
                        let hs = Hs::new(x as f32 / cols as f32, 1.0 - y as f32 / rows as f32);
                        let pos = rect.min
                            + vec2(
                                rect.width() * x as f32 / cols as f32,
                                rect.height() * y as f32 / rows as f32,
                            );
                        mesh.colored_vertex(pos, rgb32(hs.rgb()));
                    }
                }
                for y in 0..rows {
                    for x in 0..cols {
                        let i = y * (cols + 1) + x;
                        mesh.add_triangle(i, i + 1, i + cols + 2);
                        mesh.add_triangle(i, i + cols + 2, i + cols + 1);
                    }
                }
                ui.painter().add(egui::Shape::mesh(mesh));
                let cur = self.picker.hs();
                let marker = rect.min + vec2(cur.h * rect.width(), (1.0 - cur.s) * rect.height());
                ui.painter()
                    .circle_stroke(marker, 7.0, Stroke::new(2.0, Color32::BLACK));
                ui.painter()
                    .circle_stroke(marker, 5.0, Stroke::new(2.0, Color32::WHITE));
                if r.drag_started() || r.clicked() {
                    d.checkpoint();
                }
                if (r.dragged() || r.clicked())
                    && let Some(pos) = r.interact_pointer_pos()
                {
                    let rel = pos - rect.min;
                    let hs = Hs::new(
                        (rel.x / rect.width()).clamp(0.0, 0.999),
                        1.0 - (rel.y / rect.height()).clamp(0.0, 1.0),
                    );
                    self.picker = hs.rgb();
                    d.drag_colour(self.picker);
                }
                ui.label(
                    RichText::new(
                        "Drag to colour the selection. Press Colours again for RGB sliders.",
                    )
                    .small()
                    .weak(),
                );
            }
            ColourTab::Sliders => {
                let mut v = self.picker.to_u8();
                let mut changed = false;
                let mut started = false;
                for (i, name) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).monospace());
                        ui.spacing_mut().slider_width = 360.0;
                        let r = ui.add(egui::Slider::new(&mut v[i], 0..=255));
                        started |= r.drag_started();
                        changed |= r.changed();
                    });
                }
                if started {
                    d.checkpoint();
                }
                if changed {
                    self.picker = Rgb::from_u8(v[0], v[1], v[2]);
                    d.drag_colour(self.picker);
                }
                let (rect, _) = ui.allocate_exact_size(vec2(120.0, 30.0), Sense::hover());
                ui.painter().rect_filled(rect, 4.0, rgb32(self.picker));
            }
            ColourTab::Patterns => {
                let patterns = pattern_page(self.pattern_page);
                for row in patterns.chunks(6) {
                    ui.horizontal(|ui| {
                        for p in row {
                            let colours: Vec<Color32> =
                                pattern_preview(*p).into_iter().map(rgb32).collect();
                            let name = effects::pattern_name(*p);
                            if colour_button(ui, &colours, &name, size).clicked() {
                                d.apply_pattern(*p);
                            }
                        }
                    });
                }
                ui.label(
                    RichText::new(match self.pattern_page {
                        0 | 1 => "One colour on odd channels, the other on even.",
                        2 | 3 => "One colour on the first half of the selection, the other on the second.",
                        4 | 5 => "One colour at both ends crossfading to the other in the centre.",
                        6 | 7 => "One colour fading to the other from the first fixture to the last.",
                        8 => "Three colours repeating along the selection.",
                        _ => "Rainbows and palettes spread across the selection.",
                    })
                    .small()
                    .weak(),
                );
            }
            ColourTab::Chases => {
                let chases = chase_page(self.chase_page);
                let named = matches!(self.chase_page, 0 | 1) || self.chase_page == INTENSITY_PAGE;
                for row in chases.chunks(6) {
                    ui.horizontal(|ui| {
                        for c in row {
                            let colours: Vec<Color32> =
                                chase_preview(*c).into_iter().map(rgb32).collect();
                            let label = if named {
                                effects::chase_name(*c)
                            } else {
                                String::new()
                            };
                            if colour_button(ui, &colours, &label, size)
                                .on_hover_text(effects::chase_name(*c))
                                .clicked()
                            {
                                d.apply_chase(*c);
                            }
                        }
                    });
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(if self.chase_page == INTENSITY_PAGE {
                        "Intensity effects bring their own levels (highest takes precedence): \
                         pull the selection's faders down, colour stays as it is."
                    } else {
                        "Colour chases leave intensity to the faders."
                    })
                    .small()
                    .weak(),
                );
                ui.add_space(6.0);
                if !d.prog.chases.is_empty() {
                    ui.label(RichText::new("Running chases").strong());
                    let mut release = None;
                    for (i, (c, _)) in d.prog.chases.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{} on {} fixtures", c.name, c.channels().len()));
                            if ui.small_button("Stop").clicked() {
                                release = Some(i);
                            }
                        });
                    }
                    if let Some(i) = release {
                        d.release(PlayRef::Programmer(i));
                    }
                    ui.label(
                        RichText::new(
                            "Record + a Scene or Chase button copies the latest chase there.",
                        )
                        .small()
                        .weak(),
                    );
                }
            }
        }
    }

    // ---- Scenes ---------------------------------------------------------------------------

    fn scenes(&mut self, ui: &mut Ui, d: &mut Desk) {
        if let Some(i) = tabs(ui, self.tab, &["Control", "Scenes", "Preview"]) {
            self.tab = i;
        }
        let pages = d.prefs.scene_pages;
        let slots: Vec<Slot> = Slot::all(pages)
            .into_iter()
            .filter(|&s| matches!(d.show.item(s), Some(Item::Scene(_))))
            .collect();
        match self.tab {
            0 => {
                ui.label(RichText::new("Scenes playing").strong());
                let playing: Vec<Slot> = slots
                    .iter()
                    .copied()
                    .filter(|&s| d.slot_playing(s))
                    .collect();
                if playing.is_empty() {
                    ui.label(RichText::new("None").weak());
                }
                for s in &playing {
                    let name = d.show.item(*s).map_or("", Item::name).to_string();
                    if ui
                        .selectable_label(
                            self.sel_slot == Some(*s),
                            format!("{} {name}", s.label()),
                        )
                        .clicked()
                    {
                        self.sel_slot = Some(*s);
                    }
                }
                ui.horizontal(|ui| {
                    if let Some(s) = self.sel_slot {
                        if ui.button("Release").clicked() {
                            d.release(PlayRef::Slot(s));
                        }
                        if ui.button("Replay").clicked() {
                            d.replay_slot(s);
                        }
                    }
                });
            }
            1 => {
                if slots.is_empty() {
                    ui.label(
                        RichText::new(
                            "No scenes yet. Set a look, press RECORD, then a Scene button.",
                        )
                        .weak(),
                    );
                }
                egui::Grid::new("scenes")
                    .striped(true)
                    .num_columns(5)
                    .show(ui, |ui| {
                        ui.label("#");
                        ui.label("Name");
                        ui.label("Fade");
                        ui.label("");
                        ui.label("");
                        ui.end_row();
                        for s in &slots {
                            let playing = d.slot_playing(*s);
                            if ui
                                .selectable_label(self.sel_slot == Some(*s), s.label())
                                .clicked()
                            {
                                self.sel_slot = Some(*s);
                            }
                            let fader_slot = matches!(s, Slot::Fader(..));
                            if let Some(Item::Scene(scene)) = d.show.item_mut(*s) {
                                ui.add(
                                    egui::TextEdit::singleline(&mut scene.name)
                                        .desired_width(150.0),
                                );
                                if fader_slot {
                                    ui.label(RichText::new("fader").weak())
                                        .on_hover_text("Fader scenes fade as you move the fader");
                                } else {
                                    ui.horizontal(|ui| {
                                        let mut follow = scene.fade.is_none();
                                        if ui
                                            .checkbox(&mut follow, "dial")
                                            .on_hover_text("Follow the Fade Time dial")
                                            .changed()
                                        {
                                            scene.fade = if follow { None } else { Some(2.0) };
                                        }
                                        if let Some(f) = &mut scene.fade {
                                            ui.add(
                                                egui::DragValue::new(f)
                                                    .range(0.0..=600.0)
                                                    .speed(0.1)
                                                    .suffix(" s"),
                                            );
                                        }
                                    });
                                }
                            }
                            if ui
                                .button(if playing { "Release" } else { "Play" })
                                .clicked()
                            {
                                d.toggle_slot(*s);
                            }
                            if ui.button("Delete").clicked() {
                                d.delete_slot(*s);
                            }
                            ui.end_row();
                        }
                    });
            }
            _ => {
                ui.label(
                    RichText::new("Each square is a fixture (1-12 | 13-24). X: not in the scene. Hover for values.")
                        .small()
                        .weak(),
                );
                for s in &slots {
                    if let Some(Item::Scene(scene)) = d.show.item(*s) {
                        let look = scene.look;
                        let name = scene.name.clone();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:>4}", s.label())).monospace());
                            look_icons(ui, d, &look);
                            ui.label(name);
                        });
                    }
                }
            }
        }
    }

    // ---- Chases ---------------------------------------------------------------------------

    fn chases(&mut self, ui: &mut Ui, d: &mut Desk) {
        if let Some(i) = tabs(ui, self.tab, &["Control", "Chases", "Steps"]) {
            self.tab = i;
        }
        let slots: Vec<Slot> = Slot::all(SCENE_PAGES)
            .into_iter()
            .filter(|&s| matches!(d.show.item(s), Some(Item::Chase(_))))
            .collect();
        // Keep the selection on a chase: the latest playing one, else the first.
        if !self.sel_slot.is_some_and(|s| slots.contains(&s)) {
            self.sel_slot = slots
                .iter()
                .copied()
                .find(|&s| d.slot_playing(s))
                .or(slots.first().copied());
            self.sel_step = None;
        }
        if slots.is_empty() {
            ui.label(
                RichText::new(
                    "No chases yet. Set a look, press RECORD, then a Chase button (Auto or Manual), \
                     or apply a colour chase from the Colour window and record it.",
                )
                .weak(),
            );
            return;
        }
        let sel = self.sel_slot.unwrap();
        match self.tab {
            0 => {
                ui.horizontal(|ui| {
                    let i = slots.iter().position(|&s| s == sel).unwrap_or(0);
                    if ui.button("◀").clicked() {
                        self.sel_slot = Some(slots[(i + slots.len() - 1) % slots.len()]);
                    }
                    ui.label(RichText::new(sel.label()).strong().monospace());
                    if ui.button("▶").clicked() {
                        self.sel_slot = Some(slots[(i + 1) % slots.len()]);
                    }
                    let playing = d.slot_playing(sel);
                    playing_dot(ui, playing);
                    if ui
                        .button(if playing { "Release" } else { "Play" })
                        .clicked()
                    {
                        d.toggle_slot(sel);
                    }
                });
                let tap = d.tap_bpm;
                let rate = d.rate_scale;
                if let Some(Item::Chase(c)) = d.show.item_mut(sel) {
                    egui::Grid::new("chase_ctl")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Name");
                            ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(200.0));
                            ui.end_row();
                            ui.label("BPM");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::DragValue::new(&mut c.bpm)
                                        .range(1.0..=600.0)
                                        .speed(0.5),
                                );
                                let eff = tap.unwrap_or(c.bpm) * rate;
                                if (eff - c.bpm).abs() > 0.5 {
                                    ui.label(
                                        RichText::new(format!(
                                            "playing at {eff:.0} (rate dial / sync)"
                                        ))
                                        .small()
                                        .weak(),
                                    );
                                }
                            });
                            ui.end_row();
                            ui.label("Direction");
                            ui.horizontal(|ui| {
                                for dir in Direction::ALL {
                                    ui.selectable_value(&mut c.direction, dir, dir.name());
                                }
                            });
                            ui.end_row();
                            ui.label("Mirror");
                            ui.checkbox(&mut c.mirror, "Split into two mirrored halves");
                            ui.end_row();
                            ui.label("Crossfade");
                            ui.horizontal(|ui| {
                                for x in Crossfade::ALL {
                                    ui.selectable_value(&mut c.crossfade, x, x.name());
                                }
                            });
                            ui.end_row();
                            ui.label("Filter");
                            ui.checkbox(
                                &mut c.intensity_only,
                                "Intensity only (colour from other playbacks)",
                            );
                            ui.end_row();
                            ui.label("Audio");
                            ui.checkbox(&mut c.audio, "Step on the beat (mic, or M key)");
                            ui.end_row();
                        });
                }
            }
            1 => {
                egui::Grid::new("chase_list")
                    .striped(true)
                    .num_columns(6)
                    .show(ui, |ui| {
                        for h in ["#", "Name", "BPM", "Steps", "", ""] {
                            ui.label(h);
                        }
                        ui.end_row();
                        for s in &slots {
                            let playing = d.slot_playing(*s);
                            ui.horizontal(|ui| {
                                playing_dot(ui, playing);
                                if ui.selectable_label(sel == *s, s.label()).clicked() {
                                    self.sel_slot = Some(*s);
                                    self.sel_step = None;
                                }
                            });
                            if let Some(Item::Chase(c)) = d.show.item_mut(*s) {
                                ui.add(
                                    egui::TextEdit::singleline(&mut c.name).desired_width(150.0),
                                );
                                ui.add(egui::DragValue::new(&mut c.bpm).range(1.0..=600.0));
                                ui.label(c.steps.len().to_string());
                            }
                            if ui
                                .button(if playing { "Release" } else { "Play" })
                                .clicked()
                            {
                                d.toggle_slot(*s);
                            }
                            if ui.button("Delete").clicked() {
                                d.delete_slot(*s);
                            }
                            ui.end_row();
                        }
                    });
                ui.label(
                    RichText::new(
                        "Chases can live on the Chase buttons (C), the Scene buttons (S) or scene-page faders.",
                    )
                    .small()
                    .weak(),
                );
            }
            _ => self.chase_steps(ui, d, sel),
        }
    }

    fn chase_steps(&mut self, ui: &mut Ui, d: &mut Desk, sel: Slot) {
        let Some(Item::Chase(c)) = d.show.item(sel) else {
            return;
        };
        let steps = c.steps.clone();
        ui.label(RichText::new(format!("{} · {}", sel.label(), c.name)).strong());
        for (i, step) in steps.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.sel_step == Some(i), format!("{:>3}", i + 1))
                    .clicked()
                {
                    self.sel_step = Some(i);
                }
                look_icons(ui, d, step);
            });
        }
        if ui
            .selectable_label(self.sel_step == Some(steps.len()), "New step")
            .clicked()
        {
            self.sel_step = Some(steps.len());
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let s = self.sel_step.filter(|&i| i <= steps.len());
            let on_step = s.filter(|&i| i < steps.len());
            if ui
                .add_enabled(on_step.is_some(), egui::Button::new("Copy"))
                .clicked()
            {
                self.step_clip = on_step.map(|i| steps[i]);
            }
            let clip = self.step_clip;
            let mut edit = |f: &mut dyn FnMut(&mut Vec<Look>)| {
                d.checkpoint();
                if let Some(Item::Chase(c)) = d.show.item_mut(sel) {
                    f(&mut c.steps);
                }
            };
            if ui
                .add_enabled(clip.is_some() && s.is_some(), egui::Button::new("Insert"))
                .on_hover_text("Paste before the selected step")
                .clicked()
            {
                let (i, l) = (s.unwrap(), clip.unwrap());
                edit(&mut |v| v.insert(i.min(v.len()), l));
            }
            if ui
                .add_enabled(clip.is_some() && s.is_some(), egui::Button::new("Paste"))
                .on_hover_text("Replace the selected step, or add at New step")
                .clicked()
            {
                let (i, l) = (s.unwrap(), clip.unwrap());
                edit(&mut |v| {
                    if i < v.len() { v[i] = l } else { v.push(l) }
                });
            }
            if ui
                .add_enabled(on_step.is_some(), egui::Button::new("Move up"))
                .clicked()
            {
                let i = on_step.unwrap();
                if i > 0 {
                    edit(&mut |v| v.swap(i, i - 1));
                    self.sel_step = Some(i - 1);
                }
            }
            if ui
                .add_enabled(on_step.is_some(), egui::Button::new("Move down"))
                .clicked()
            {
                let i = on_step.unwrap();
                if i + 1 < steps.len() {
                    edit(&mut |v| v.swap(i, i + 1));
                    self.sel_step = Some(i + 1);
                }
            }
            if ui
                .add_enabled(
                    on_step.is_some() && steps.len() > 1,
                    egui::Button::new("Delete"),
                )
                .clicked()
            {
                let i = on_step.unwrap();
                edit(&mut |v| {
                    v.remove(i);
                });
            }
        });
        ui.label(
            RichText::new(
                "Record + this Chase button → Manual adds the current look as a new step.",
            )
            .small()
            .weak(),
        );
    }

    // ---- Cuelist --------------------------------------------------------------------------

    fn cuelist(&mut self, ui: &mut Ui, d: &mut Desk, _t: f64) {
        if let Some(i) = tabs(ui, self.tab, &["Control", "Cuelist", "Preview"]) {
            self.tab = i;
        }
        let n = d.show.cuelist.len();
        if n == 0 {
            ui.label(
                RichText::new(
                    "No cues yet. Set a look, press RECORD, then the Cuelist PLAY button.",
                )
                .weak(),
            );
            return;
        }
        if self.sel_cue.is_some_and(|i| i >= n) {
            self.sel_cue = None;
        }
        let current = d.cue.current;
        match self.tab {
            0 => {
                for i in 0..n {
                    let cue = &d.show.cuelist[i];
                    let bg = match current {
                        Some(c) if c == i => Color32::from_rgb(120, 110, 20),
                        Some(c) if i < c => Color32::from_rgb(30, 90, 40),
                        _ => Color32::TRANSPARENT,
                    };
                    let text = format!(
                        "{:>3}  {:<20} {}",
                        i + 1,
                        cue.name,
                        match cue.fade {
                            Some(f) => format!("{f:.1} s"),
                            None => format!("dial {:.1} s", d.live_fade),
                        }
                    );
                    egui::Frame::new().fill(bg).show(ui, |ui| {
                        if ui
                            .selectable_label(
                                self.sel_cue == Some(i),
                                RichText::new(text).monospace(),
                            )
                            .clicked()
                        {
                            self.sel_cue = Some(i);
                        }
                    });
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Release").clicked() {
                        d.cue_release();
                    }
                    if ui
                        .button("<<")
                        .on_hover_text("Previous cue, no fade")
                        .clicked()
                    {
                        d.cue_skip(false);
                    }
                    if ui.button(">>").on_hover_text("Next cue, no fade").clicked() {
                        d.cue_skip(true);
                    }
                    if ui
                        .add_enabled(self.sel_cue.is_some(), egui::Button::new("Jump"))
                        .on_hover_text("Crossfade to the selected cue")
                        .clicked()
                    {
                        d.cue_jump(self.sel_cue.unwrap());
                    }
                    ui.separator();
                    if ui.button("Back").clicked() {
                        d.cue_back();
                    }
                    let pause = if d.cue.paused.is_some() {
                        "Resume"
                    } else {
                        "Pause"
                    };
                    if ui.button(pause).clicked() {
                        d.cue_pause();
                    }
                    if ui.button(RichText::new("Play ▶").strong()).clicked() {
                        d.cue_go();
                    }
                });
            }
            1 => {
                let live_fade = d.live_fade;
                egui::Grid::new("cues")
                    .striped(true)
                    .num_columns(5)
                    .show(ui, |ui| {
                        for h in ["#", "Name", "Start", "Wait", "Fade"] {
                            ui.label(h);
                        }
                        ui.end_row();
                        for i in 0..n {
                            let cue = &mut d.show.cuelist[i];
                            let num = RichText::new(format!("{}", i + 1)).monospace();
                            let num = match current {
                                Some(c) if c == i => num.color(ACCENT),
                                Some(c) if i < c => num.color(LED_GREEN),
                                _ => num,
                            };
                            if ui.selectable_label(self.sel_cue == Some(i), num).clicked() {
                                self.sel_cue = Some(i);
                            }
                            ui.add(egui::TextEdit::singleline(&mut cue.name).desired_width(140.0));
                            egui::ComboBox::from_id_salt(("start", i))
                                .width(70.0)
                                .selected_text(match cue.start {
                                    CueStart::Halt => "Stop",
                                    CueStart::Follow => "Play",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut cue.start, CueStart::Halt, "Stop")
                                        .on_hover_text("Wait for Play");
                                    ui.selectable_value(&mut cue.start, CueStart::Follow, "Play")
                                        .on_hover_text("Play automatically after the previous cue");
                                });
                            ui.add_enabled(
                                cue.start == CueStart::Follow,
                                egui::DragValue::new(&mut cue.wait)
                                    .range(-600.0..=600.0)
                                    .speed(0.1)
                                    .suffix(" s"),
                            );
                            ui.horizontal(|ui| {
                                let mut follow = cue.fade.is_none();
                                if ui
                                    .checkbox(&mut follow, "dial")
                                    .on_hover_text(format!(
                                        "Follow the Fade Time dial (now {live_fade:.1} s)"
                                    ))
                                    .changed()
                                {
                                    cue.fade = if follow { None } else { Some(live_fade) };
                                }
                                if let Some(f) = &mut cue.fade {
                                    ui.add(
                                        egui::DragValue::new(f)
                                            .range(0.0..=600.0)
                                            .speed(0.1)
                                            .suffix(" s"),
                                    );
                                }
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let s = self.sel_cue;
                    if ui
                        .add_enabled(s.is_some(), egui::Button::new("Copy"))
                        .clicked()
                    {
                        self.cue_clip = s.map(|i| d.show.cuelist[i].clone());
                    }
                    let clip = self.cue_clip.clone();
                    if ui
                        .add_enabled(clip.is_some() && s.is_some(), egui::Button::new("Paste"))
                        .on_hover_text("Replace the selected cue")
                        .clicked()
                    {
                        d.checkpoint();
                        d.show.cuelist[s.unwrap()] = clip.clone().unwrap();
                    }
                    if ui
                        .add_enabled(clip.is_some(), egui::Button::new("Insert"))
                        .on_hover_text("Insert before the selected cue (or at the end)")
                        .clicked()
                    {
                        d.checkpoint();
                        let at = s.unwrap_or(n);
                        let mut c = clip.unwrap();
                        c.name = format!("{} copy", c.name);
                        d.show.cuelist.insert(at, c);
                    }
                    if ui
                        .add_enabled(s.is_some(), egui::Button::new("Update"))
                        .on_hover_text("Store the current look into the selected cue")
                        .clicked()
                    {
                        d.update_cue(s.unwrap());
                    }
                    if ui
                        .add_enabled(s.is_some(), egui::Button::new("Delete"))
                        .clicked()
                    {
                        d.delete_cue(s.unwrap());
                    }
                });
            }
            _ => {
                for i in 0..n {
                    let look = d.show.cuelist[i].look;
                    let name = d.show.cuelist[i].name.clone();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:>3}", i + 1)).monospace());
                        look_icons(ui, d, &look);
                        ui.label(name);
                    });
                }
            }
        }
    }

    // ---- Monitor --------------------------------------------------------------------------

    fn monitor(&mut self, ui: &mut Ui, d: &mut Desk) {
        if let Some(i) = tabs(ui, self.tab, &["Monitor", "Mimic", "DMX"]) {
            self.tab = i;
        }
        match self.tab {
            0 => {
                ui.label(RichText::new("Controls").strong());
                egui::Grid::new("controls").num_columns(4).show(ui, |ui| {
                    let pc = |v: f32| format!("{:.0}%", v * 100.0);
                    ui.label("Grand Master");
                    ui.label(pc(d.grand_master));
                    ui.label("Fixtures");
                    ui.label(pc(d.fixture_master));
                    ui.end_row();
                    ui.label("Scenes");
                    ui.label(pc(d.scene_master));
                    ui.label("Chases");
                    ui.label(pc(d.chase_master));
                    ui.end_row();
                    ui.label("Cuelist");
                    ui.label(pc(d.cue_master));
                    ui.label("Fade time");
                    ui.label(format!("{:.1} s", d.live_fade));
                    ui.end_row();
                    ui.label("Chase rate");
                    ui.label(match d.tap_bpm {
                        Some(b) => format!("{:.0} bpm (sync)", b * d.rate_scale),
                        None => pc(d.rate_scale),
                    });
                    ui.label("Snapshot");
                    ui.label(if d.snapshot_active() { "On" } else { "Off" });
                    ui.end_row();
                });
                ui.add_space(8.0);
                ui.label(RichText::new("Play list (latest first)").strong());
                let list = d.playing();
                if list.is_empty() {
                    ui.label(RichText::new("Nothing is playing").weak());
                }
                for (r, name) in &list {
                    if ui
                        .selectable_label(self.sel_play == Some(*r), name)
                        .clicked()
                    {
                        self.sel_play = Some(*r);
                    }
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.sel_play.is_some(), egui::Button::new("Release"))
                        .clicked()
                    {
                        d.release(self.sel_play.take().unwrap());
                    }
                    if ui.button("Release All").clicked() {
                        d.release_all();
                    }
                });
                ui.label(
                    RichText::new(
                        "Colour is Latest Takes Precedence: the last thing played sets a fixture's colour. \
                         Intensity is Highest Takes Precedence.",
                    )
                    .small()
                    .weak(),
                );
            }
            1 => self.mimic(ui, d),
            _ => dmx_grid(ui, &d.dmx),
        }
    }

    fn mimic(&mut self, ui: &mut Ui, d: &Desk) {
        ui.label(
            RichText::new(
                "Top: colour output · bar: output level, red line: fader setting · bottom: colour setting",
            )
            .small()
            .weak(),
        );
        for page in 0..2 {
            ui.horizontal(|ui| {
                for i in 0..STRIPS {
                    let ch = page * STRIPS + i;
                    let (rect, r) = ui.allocate_exact_size(vec2(38.0, 118.0), Sense::hover());
                    let p = ui.painter();
                    let o = d.out[ch];
                    let out_box = Rect::from_min_size(rect.min + vec2(4.0, 0.0), vec2(30.0, 14.0));
                    p.rect_filled(out_box, 2.0, rgb32(o.rgb.scale(o.intensity)));
                    let bar = Rect::from_min_size(rect.min + vec2(12.0, 18.0), vec2(14.0, 70.0));
                    p.rect_filled(bar, 1.0, Color32::from_gray(25));
                    let h = bar.height() * o.level;
                    p.rect_filled(
                        Rect::from_min_max(pos2(bar.min.x, bar.max.y - h), bar.max),
                        1.0,
                        Color32::from_gray(110),
                    );
                    let fy = bar.max.y - bar.height() * d.prog.level[ch];
                    p.line_segment(
                        [pos2(bar.min.x - 3.0, fy), pos2(bar.max.x + 3.0, fy)],
                        Stroke::new(2.0, LED_RED),
                    );
                    let set = d.prog.colour[ch].unwrap_or(o.hs);
                    let set_box = Rect::from_min_size(rect.min + vec2(4.0, 92.0), vec2(30.0, 12.0));
                    p.rect_filled(set_box, 2.0, rgb32(set.rgb()));
                    p.text(
                        rect.min + vec2(19.0, 112.0),
                        Align2::CENTER_CENTER,
                        (ch + 1).to_string(),
                        FontId::monospace(10.0),
                        if d.prog.selected[ch] {
                            ACCENT
                        } else {
                            Color32::from_gray(160)
                        },
                    );
                    let cell = Cell {
                        level: Some(o.level),
                        colour: Some(o.hs),
                        alt: o.alt,
                        pos: o.pos,
                    };
                    r.on_hover_text(cell_detail(d, ch, &cell));
                }
            });
            ui.add_space(6.0);
        }
    }

    // ---- Shows ----------------------------------------------------------------------------

    fn shows(&mut self, ui: &mut Ui, d: &mut Desk) {
        if let Some(i) = tabs(ui, self.tab, &["Save", "Load"]) {
            self.tab = i;
        }
        ui.label(
            RichText::new(format!("Folder: {}", self.show_dir().display()))
                .small()
                .weak(),
        );
        ui.add_space(4.0);
        match self.tab {
            0 => {
                ui.horizontal(|ui| {
                    ui.label("Show name");
                    ui.add(egui::TextEdit::singleline(&mut self.show_name).desired_width(220.0));
                    if ui.button("Save").clicked() {
                        let name: String = self
                            .show_name
                            .chars()
                            .filter(|c| {
                                !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
                            })
                            .collect();
                        if name.trim().is_empty() {
                            self.status = "Give the show a name".into();
                        } else {
                            self.show_path =
                                self.show_dir().join(format!("{}.show.json", name.trim()));
                            self.show_name = name.trim().to_string();
                            // Release the desk lock's borrow before saving.
                            let show = d.show.clone();
                            match super::write_json(&self.show_path, &show) {
                                Ok(()) => {
                                    self.saved_show = show;
                                    self.status = format!("Saved {}", self.show_path.display());
                                }
                                Err(e) => self.status = format!("Save failed: {e}"),
                            }
                        }
                    }
                });
                ui.label(
                    RichText::new(format!(
                        "Changes are autosaved every {} s (Setup).",
                        self.settings.autosave_secs
                    ))
                    .small()
                    .weak(),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if self.confirm_new {
                        ui.label(
                            RichText::new("Start an empty show? Unsaved changes are lost.")
                                .color(LED_RED),
                        );
                        if ui.button("New show").clicked() {
                            *d = Desk::new(Show::default(), self.settings.prefs.clone());
                            self.show_name = "untitled".into();
                            self.show_path = self.show_dir().join("untitled.show.json");
                            self.saved_show = d.show.clone();
                            self.confirm_new = false;
                            self.go(Screen::Patch);
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_new = false;
                        }
                    } else {
                        if ui.button("New show…").clicked() {
                            self.confirm_new = true;
                        }
                        if ui.button("Load the demo show").clicked() {
                            *d = Desk::new(crate::demo::demo_show(), self.settings.prefs.clone());
                            self.status = "Demo show loaded (not saved yet)".into();
                        }
                    }
                });
            }
            _ => {
                let files = self.show_files();
                if files.is_empty() {
                    ui.label(RichText::new("No .show.json files in this folder").weak());
                }
                let mut load = None;
                for f in &files {
                    ui.horizontal(|ui| {
                        let current = *f == self.show_path;
                        if ui
                            .selectable_label(current, super::show_name(f))
                            .on_hover_text(f.display().to_string())
                            .clicked()
                        {
                            load = Some(f.clone());
                        }
                    });
                }
                if let Some(f) = load {
                    match super::read_json::<Show>(&f) {
                        Ok(show) => {
                            *d = Desk::new(show, self.settings.prefs.clone());
                            self.saved_show = d.show.clone();
                            self.show_name = super::show_name(&f);
                            self.show_path = f;
                            self.status = format!("Loaded {}", self.show_path.display());
                        }
                        Err(e) => self.status = format!("Load failed: {e}"),
                    }
                }
            }
        }
    }

    // ---- Patch ----------------------------------------------------------------------------

    fn patch(&mut self, ui: &mut Ui, d: &mut Desk) {
        if self.editor.is_some() {
            self.fixture_editor(ui, d);
            return;
        }
        if let Some(i) = tabs(ui, self.tab, &["Patch", "Wizard", "Library"]) {
            self.tab = i;
        }
        match self.tab {
            0 => self.patch_list(ui, d),
            1 => self.patch_wizard(ui, d),
            _ => self.patch_library(ui, d),
        }
    }

    fn patch_list(&mut self, ui: &mut Ui, d: &mut Desk) {
        let bad = conflicts(&d.show);
        ui.label(
            RichText::new("Fixtures 1-12 are on the Black page, 13-24 on the White page.")
                .small()
                .weak(),
        );
        egui::Grid::new("patch")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                for h in ["#", "Fixture", "DMX", "&"] {
                    ui.label(h);
                }
                ui.end_row();
                for (ch, &bad) in bad.iter().enumerate() {
                    let num = RichText::new(format!("{}", ch + 1)).monospace();
                    if ui.selectable_label(self.patch_sel == ch, num).clicked() {
                        self.patch_sel = ch;
                    }
                    match &mut d.show.patch[ch] {
                        Some(p) => {
                            let name = RichText::new(fixture_label(&p.def));
                            ui.label(if bad { name.color(LED_RED) } else { name })
                                .on_hover_text(if bad {
                                    "Overlaps another fixture or runs past 512"
                                } else {
                                    ""
                                });
                            ui.add(egui::DragValue::new(&mut p.addresses[0]).range(1..=512));
                            let extra = p.addresses.len() - 1;
                            ui.label(if extra > 0 {
                                format!("+{extra}")
                            } else {
                                String::new()
                            });
                        }
                        None => {
                            ui.label(RichText::new("—").weak());
                            ui.label("");
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(8.0);
        let ch = self.patch_sel;
        ui.label(RichText::new(format!("Fixture channel {}", ch + 1)).strong());
        let fixtures = all_fixtures(&d.show);
        ui.horizontal(|ui| {
            let current = d.show.patch[ch]
                .as_ref()
                .map_or("Not patched".into(), |p| p.def.name.clone());
            ui.label(RichText::new(current).strong());
            if ui
                .selectable_label(self.browse_patch, "Choose fixture…")
                .clicked()
            {
                self.browse_patch = !self.browse_patch;
            }
            let patched = d.show.patch[ch].is_some();
            if ui.add_enabled(patched, egui::Button::new("Copy")).clicked() {
                self.patch_clip = d.show.patch[ch].clone();
            }
            if ui
                .add_enabled(self.patch_clip.is_some(), egui::Button::new("Paste"))
                .on_hover_text("Patch the copied fixture here at the next free address")
                .clicked()
            {
                d.checkpoint();
                let mut p = self.patch_clip.clone().unwrap();
                p.addresses = vec![free_address(&d.show, p.def.footprint(), Some(ch))];
                d.show.patch[ch] = Some(p);
            }
            if ui.add_enabled(patched, egui::Button::new("Edit")).clicked() {
                self.editor = Some(Editor {
                    def: d.show.patch[ch].as_ref().unwrap().def.clone(),
                    target: EditTarget::Patch(ch),
                    apply_all: false,
                    save_copy: false,
                });
            }
            if ui
                .add_enabled(patched, egui::Button::new("Unpatch"))
                .clicked()
            {
                d.checkpoint();
                d.show.patch[ch] = None;
            }
        });
        if self.browse_patch {
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(Color32::from_gray(24))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    if let Some(i) =
                        fixture_browser(ui, "patch", &fixtures, &mut self.browser, None)
                    {
                        d.checkpoint();
                        let def = fixtures[i].clone();
                        let addr = free_address(&d.show, def.footprint(), Some(ch));
                        d.show.patch[ch] = Some(Patched {
                            def,
                            addresses: vec![addr],
                        });
                        self.browse_patch = false;
                    }
                });
        }
        if let Some(p) = &mut d.show.patch[ch] {
            ui.add_space(4.0);
            ui.label(
                RichText::new("Multipatch: more fixtures of this type controlled by this channel")
                    .small()
                    .weak(),
            );
            let mut remove = None;
            for (i, a) in p.addresses.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(if i == 0 {
                        "Address".to_string()
                    } else {
                        format!("& {i}")
                    });
                    ui.add(egui::DragValue::new(a).range(1..=512));
                    if i > 0 && ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                p.addresses.remove(i);
            }
            let footprint = p.def.footprint();
            if ui.button("Add fixture to this channel").clicked() {
                let next = p.addresses.iter().max().copied().unwrap_or(1) + footprint;
                p.addresses.push(next.min(512));
            }
        }
    }

    fn patch_wizard(&mut self, ui: &mut Ui, d: &mut Desk) {
        let fixtures = all_fixtures(&d.show);
        let w = &mut self.wizard;
        w.fixture = w.fixture.min(fixtures.len() - 1);
        egui::Grid::new("wizard")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Fixture");
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        fixtures[w.fixture].manufacturer, fixtures[w.fixture].name
                    ))
                    .strong(),
                );
                ui.end_row();
                ui.label("Quantity");
                ui.add(egui::DragValue::new(&mut w.quantity).range(1..=CHANNELS));
                ui.end_row();
                ui.label("First fixture channel");
                let mut first = w.start_channel + 1;
                ui.add(egui::DragValue::new(&mut first).range(1..=CHANNELS));
                w.start_channel = first - 1;
                ui.end_row();
                ui.label("Start DMX channel");
                ui.add(egui::DragValue::new(&mut w.start_dmx).range(1..=512));
                ui.end_row();
                ui.label("DMX offset");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut w.offset).range(0..=512));
                    ui.label(
                        RichText::new(format!(
                            "0 packs them ({} channels each)",
                            fixtures[w.fixture].footprint()
                        ))
                        .small()
                        .weak(),
                    );
                });
                ui.end_row();
            });
        if let Some(i) =
            fixture_browser(ui, "wizard", &fixtures, &mut self.browser, Some(w.fixture))
        {
            w.fixture = i;
        }
        ui.add_space(4.0);
        let def = &fixtures[w.fixture];
        let step = if w.offset == 0 {
            def.footprint()
        } else {
            w.offset
        };
        let last = w.start_channel + w.quantity - 1;
        let end =
            w.start_dmx as u32 + step as u32 * (w.quantity as u32 - 1) + def.footprint() as u32 - 1;
        ui.label(format!(
            "Patches fixture channels {}-{} at DMX {}-{}",
            w.start_channel + 1,
            last.min(CHANNELS - 1) + 1,
            w.start_dmx,
            end
        ));
        if last >= CHANNELS || end > 512 {
            ui.label(
                RichText::new("Doesn't fit: reduce the quantity or move the start.").color(LED_RED),
            );
        } else if ui.button("Patch").clicked() {
            d.checkpoint();
            for k in 0..w.quantity {
                d.show.patch[w.start_channel + k] = Some(Patched {
                    def: def.clone(),
                    addresses: vec![w.start_dmx + step * k as u16],
                });
            }
            self.status = format!("Patched {} × {}", w.quantity, def.name);
            w.start_channel = (last + 1).min(CHANNELS - 1);
            w.start_dmx = (end as u16 + 1).min(512);
        }
    }

    fn patch_library(&mut self, ui: &mut Ui, d: &mut Desk) {
        let generic = library();
        let fixtures = all_fixtures(&d.show);
        ui.label(
            RichText::new(format!(
                "{} built-in fixture modes (Generic, manufacturers' manuals and the Open Fixture \
                 Library) and {} in My Fixtures, saved with the show.",
                generic.len(),
                fixtures.len() - generic.len()
            ))
            .small()
            .weak(),
        );
        if let Some(i) = fixture_browser(ui, "library", &fixtures, &mut self.browser, self.lib_sel)
        {
            self.lib_sel = Some(i);
        }
        if let Some(f) = self.lib_sel.and_then(|i| fixtures.get(i)) {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} · {}", f.manufacturer, f.name)).strong());
            ui.horizontal_wrapped(|ui| {
                for (n, c) in f.channels.iter().enumerate() {
                    ui.label(
                        RichText::new(format!("{} {}", n + 1, c.display()))
                            .small()
                            .monospace(),
                    );
                }
            });
        }
        ui.add_space(6.0);
        let sel = self.lib_sel.filter(|&i| i < fixtures.len());
        let mine = sel.and_then(|i| i.checked_sub(generic.len()));
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.editor = Some(Editor {
                    def: FixtureDef::new(
                        "My Fixtures",
                        "New fixture",
                        vec![ChannelDef::new(Feature::Intensity)],
                    ),
                    target: EditTarget::Library(None),
                    apply_all: false,
                    save_copy: false,
                });
            }
            if ui
                .add_enabled(sel.is_some(), egui::Button::new("Copy"))
                .clicked()
            {
                let mut def = fixtures[sel.unwrap()].clone();
                def.manufacturer = "My Fixtures".into();
                def.name = format!("{} copy", def.name);
                d.checkpoint();
                d.show.my_fixtures.push(def);
                self.lib_sel = Some(generic.len() + d.show.my_fixtures.len() - 1);
            }
            if ui
                .add_enabled(mine.is_some(), egui::Button::new("Edit"))
                .clicked()
            {
                let i = mine.unwrap();
                self.editor = Some(Editor {
                    def: d.show.my_fixtures[i].clone(),
                    target: EditTarget::Library(Some(i)),
                    apply_all: false,
                    save_copy: false,
                });
            }
            if ui
                .add_enabled(mine.is_some(), egui::Button::new("Delete"))
                .clicked()
            {
                d.checkpoint();
                d.show.my_fixtures.remove(mine.unwrap());
                self.lib_sel = None;
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Import from the Open Fixture Library").strong());
        ui.label(
            RichText::new(
                "Download a fixture's .json from open-fixture-library.org (or its GitHub \
                 repository) and give its path here. Each DMX mode becomes a fixture in My Fixtures.",
            )
            .small()
            .weak(),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.import_path)
                    .hint_text("/path/to/fixture.json")
                    .desired_width(300.0),
            );
            if ui.button("Import").clicked() {
                self.status = match self.import_fixture(d) {
                    Ok(n) => format!("Imported {n} fixture mode(s) into My Fixtures"),
                    Err(e) => format!("Import failed: {e}"),
                };
            }
        });
    }

    fn import_fixture(&mut self, d: &mut Desk) -> Result<usize, String> {
        let path = std::path::PathBuf::from(self.import_path.trim());
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        // OFL keeps fixtures in a folder named after the manufacturer.
        let maker = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|m| crate::ofl::manufacturer_from_key(&m.to_string_lossy()))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "Imported".into());
        let fixtures = crate::ofl::convert(&text, &maker)?;
        let n = fixtures.len();
        d.checkpoint();
        d.show.my_fixtures.extend(fixtures);
        self.lib_sel = Some(library().len() + d.show.my_fixtures.len() - 1);
        Ok(n)
    }

    fn fixture_editor(&mut self, ui: &mut Ui, d: &mut Desk) {
        let Some(ed) = &mut self.editor else { return };
        ui.heading("Edit fixture");
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.add(egui::TextEdit::singleline(&mut ed.def.name).desired_width(200.0));
        });
        ui.add_space(4.0);
        let mut remove = None;
        let mut swap = None;
        let n = ed.def.channels.len();
        egui::Grid::new("editor")
            .striped(true)
            .num_columns(8)
            .show(ui, |ui| {
                for h in [
                    "Ch", "Feature", "Fine", "Name", "Dial", "Dflt", "Min", "Max",
                ] {
                    ui.label(h);
                }
                ui.end_row();
                for (i, c) in ed.def.channels.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}", i + 1));
                        if ui.small_button("Up").clicked() && i > 0 {
                            swap = Some(i);
                        }
                        if ui.small_button("x").clicked() {
                            remove = Some(i);
                        }
                    });
                    egui::ComboBox::from_id_salt(("feature", i))
                        .width(100.0)
                        .selected_text(c.feature.name())
                        .show_ui(ui, |ui| {
                            for f in Feature::ALL {
                                ui.selectable_value(&mut c.feature, f, f.name());
                            }
                        });
                    ui.checkbox(&mut c.fine, "");
                    ui.add(
                        egui::TextEdit::singleline(&mut c.label)
                            .hint_text("own name")
                            .desired_width(70.0),
                    );
                    egui::ComboBox::from_id_salt(("dial", i))
                        .width(90.0)
                        .selected_text(c.dial.name())
                        .show_ui(ui, |ui| {
                            for x in Dial::ALL {
                                ui.selectable_value(&mut c.dial, x, x.name());
                            }
                        });
                    ui.add(egui::DragValue::new(&mut c.default));
                    ui.add(egui::DragValue::new(&mut c.min));
                    ui.add(egui::DragValue::new(&mut c.max));
                    ui.end_row();
                }
            });
        if let Some(i) = swap {
            ed.def.channels.swap(i, i - 1);
        }
        if let Some(i) = remove
            && n > 1
        {
            ed.def.channels.remove(i);
        }
        if ui.button("Add channel").clicked() {
            ed.def.channels.push(ChannelDef::new(Feature::Other));
        }
        ui.label(
            RichText::new(
                "Dial: Hue & Sat mixes R G B A W C M Y. Shift+Sat is the default for Strobe, \
                 Shift+Hue for Zoom. Hue / Sat alone only on fixtures without colour mixing. \
                 Other channels sit at their default value.",
            )
            .small()
            .weak(),
        );
        if let EditTarget::Patch(_) = ed.target {
            ui.checkbox(
                &mut ed.apply_all,
                "Save to all patched fixtures of this type",
            );
            ui.checkbox(&mut ed.save_copy, "Save a copy to My Fixtures");
        }
        let (mut save, mut cancel) = (false, false);
        ui.horizontal(|ui| {
            save = ui.button(RichText::new("Save").strong()).clicked();
            cancel = ui.button("Cancel").clicked();
        });
        if save {
            d.checkpoint();
            match ed.target {
                EditTarget::Library(Some(i)) => d.show.my_fixtures[i] = ed.def.clone(),
                EditTarget::Library(None) => d.show.my_fixtures.push(ed.def.clone()),
                EditTarget::Patch(ch) => {
                    let old = d.show.patch[ch].as_ref().map(|p| p.def.clone());
                    for (c, p) in d.show.patch.iter_mut().enumerate() {
                        if let Some(p) = p
                            && (c == ch || (ed.apply_all && Some(&p.def) == old.as_ref()))
                        {
                            p.def = ed.def.clone();
                        }
                    }
                    if ed.save_copy {
                        let mut copy = ed.def.clone();
                        copy.manufacturer = "My Fixtures".into();
                        d.show.my_fixtures.push(copy);
                    }
                }
            }
        }
        if save || cancel {
            self.editor = None;
        }
    }

    // ---- Setup ----------------------------------------------------------------------------

    fn setup(&mut self, ui: &mut Ui, d: &mut Desk) {
        if let Some(i) = tabs(ui, self.tab, &["Setup", "Preferences", "About"]) {
            self.tab = i;
        }
        match self.tab {
            0 => self.setup_main(ui, d),
            1 => self.preferences(ui),
            _ => self.about(ui, d),
        }
    }

    fn setup_main(&mut self, ui: &mut Ui, d: &mut Desk) {
        let s = &mut self.settings;
        egui::Grid::new("setup")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Autosave interval");
                egui::ComboBox::from_id_salt("autosave")
                    .selected_text(match s.autosave_secs {
                        0 => "Off".into(),
                        x if x < 60 => format!("{x} s"),
                        x => format!("{} min", x / 60),
                    })
                    .show_ui(ui, |ui| {
                        for (v, name) in [
                            (0, "Off"),
                            (10, "10 s"),
                            (30, "30 s"),
                            (60, "1 min"),
                            (300, "5 min"),
                        ] {
                            ui.selectable_value(&mut s.autosave_secs, v, name);
                        }
                    });
                ui.end_row();

                ui.label("Networked DMX");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut s.network, Network::Off, "Off");
                    ui.selectable_value(&mut s.network, Network::ArtNet, "Art-Net");
                    ui.selectable_value(&mut s.network, Network::Sacn, "sACN");
                });
                ui.end_row();
            });

        match s.network {
            Network::ArtNet => {
                ui.add_space(6.0);
                ui.label(RichText::new("Art-Net").strong());
                egui::Grid::new("artnet")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Send to");
                        ui.horizontal(|ui| {
                            let valid = self.artnet_ip_text.parse::<Ipv4Addr>();
                            let edit = egui::TextEdit::singleline(&mut self.artnet_ip_text)
                                .desired_width(130.0)
                                .text_color(if valid.is_ok() {
                                    Color32::WHITE
                                } else {
                                    LED_RED
                                });
                            if ui.add(edit).changed()
                                && let Ok(ip) = self.artnet_ip_text.parse()
                            {
                                s.artnet_target = ip;
                            }
                            for (label, ip) in [
                                ("Broadcast", "255.255.255.255"),
                                ("2.x", "2.255.255.255"),
                                ("10.x", "10.255.255.255"),
                            ] {
                                if ui.small_button(label).clicked() {
                                    self.artnet_ip_text = ip.into();
                                    s.artnet_target = ip.parse().unwrap();
                                }
                            }
                        });
                        ui.end_row();
                        ui.label("Net");
                        ui.add(egui::DragValue::new(&mut s.artnet_net).range(0..=127));
                        ui.end_row();
                        ui.label("Sub-Net");
                        ui.add(egui::DragValue::new(&mut s.artnet_subnet).range(0..=15));
                        ui.end_row();
                        ui.label("Universe");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut s.artnet_universe).range(0..=15));
                            let pa = crate::artnet::port_address(
                                s.artnet_net,
                                s.artnet_subnet,
                                s.artnet_universe,
                            );
                            ui.label(
                                RichText::new(format!("Port-Address {pa} (universe 1 is 0)"))
                                    .small()
                                    .weak(),
                            );
                        });
                        ui.end_row();
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Nodes").strong());
                    if ui
                        .button("Discover")
                        .on_hover_text("Send ArtPoll now")
                        .clicked()
                    {
                        self.output.discover();
                    }
                });
                let nodes = self.output.status.lock().unwrap().nodes.clone();
                if nodes.is_empty() {
                    ui.label(RichText::new("No Art-Net nodes have answered yet.").weak());
                }
                for n in nodes {
                    ui.horizontal(|ui| {
                        let outs = n
                            .outputs
                            .iter()
                            .map(|u| format!("{}:{}:{}", u >> 8, (u >> 4) & 0xf, u & 0xf))
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(RichText::new(n.ip.to_string()).monospace());
                        ui.label(&n.short_name).on_hover_text(&n.long_name);
                        ui.label(RichText::new(outs).small().weak());
                        if ui
                            .small_button("Use")
                            .on_hover_text("Send to this node on its first output")
                            .clicked()
                        {
                            s.artnet_target = n.ip;
                            self.artnet_ip_text = n.ip.to_string();
                            if let Some(&u) = n.outputs.first() {
                                s.artnet_net = (u >> 8) as u8;
                                s.artnet_subnet = ((u >> 4) & 0xf) as u8;
                                s.artnet_universe = (u & 0xf) as u8;
                            }
                        }
                    });
                }
            }
            Network::Sacn => {
                ui.add_space(6.0);
                ui.label(RichText::new("Streaming ACN").strong());
                egui::Grid::new("sacn")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Universe");
                        ui.add(egui::DragValue::new(&mut s.sacn_universe).range(1..=63999));
                        ui.end_row();
                        ui.label("Priority");
                        ui.add(egui::DragValue::new(&mut s.sacn_priority).range(0..=200));
                        ui.end_row();
                        ui.label("Unicast to");
                        ui.horizontal(|ui| {
                            let edit = egui::TextEdit::singleline(&mut self.sacn_ip_text)
                                .hint_text("blank = multicast")
                                .desired_width(130.0);
                            if ui.add(edit).changed() {
                                s.sacn_unicast = self.sacn_ip_text.trim().parse().ok();
                            }
                            ui.label(
                                RichText::new(match s.sacn_unicast {
                                    Some(ip) => format!("unicast {ip}"),
                                    None => format!(
                                        "multicast {}",
                                        crate::sacn::multicast_group(s.sacn_universe)
                                    ),
                                })
                                .small()
                                .weak(),
                            );
                        });
                        ui.end_row();
                    });
            }
            Network::Off => {}
        }
        let st = self.output.status.lock().unwrap().clone();
        ui.label(RichText::new(&st.network).small().weak());

        #[cfg(feature = "enttec")]
        {
            ui.add_space(8.0);
            ui.label(RichText::new("DMX output (ENTTEC DMX USB Pro)").strong());
            if st.enttec_failed {
                self.output.status.lock().unwrap().enttec_failed = false;
            }
            let mut cfg = self.output.config.lock().unwrap();
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("serial")
                    .width(150.0)
                    .selected_text(if self.serial_choice.is_empty() {
                        "Port…"
                    } else {
                        &self.serial_choice
                    })
                    .show_ui(ui, |ui| {
                        for p in &self.serial_ports {
                            ui.selectable_value(&mut self.serial_choice, p.clone(), p);
                        }
                    });
                if ui.small_button("Rescan").clicked() {
                    self.serial_ports = crate::enttec::list_ports();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.serial_choice)
                        .hint_text("/dev/ttyUSB0 or COM3")
                        .desired_width(120.0),
                );
                if cfg.enttec_port.is_some() {
                    if ui.button("Disconnect").clicked() {
                        cfg.enttec_port = None;
                    }
                } else if ui.button("Connect").clicked() && !self.serial_choice.is_empty() {
                    cfg.enttec_port = Some(self.serial_choice.clone());
                }
            });
            ui.label(RichText::new(&st.enttec).small().weak());
        }

        let s = &mut self.settings;
        ui.add_space(8.0);
        ui.label(RichText::new("Audio").strong());
        #[cfg(feature = "audio")]
        {
            let mut on = self.mic.is_some();
            if ui.checkbox(&mut on, "Microphone").changed() {
                if on {
                    match crate::audio::Mic::start(self.desk.clone(), self.tuning.clone()) {
                        Ok(m) => self.mic = Some(m),
                        Err(e) => self.status = format!("Mic: {e}"),
                    }
                } else {
                    self.mic = None;
                }
            }
        }
        #[cfg(not(feature = "audio"))]
        ui.label(
            RichText::new("Built without the microphone (--features audio); press M for a beat.")
                .small()
                .weak(),
        );
        ui.horizontal(|ui| {
            ui.label("Sensitivity");
            ui.add(egui::Slider::new(&mut s.audio_sensitivity, 0.0..=1.0).show_value(false));
            ui.label("Dynamics");
            ui.add(egui::Slider::new(&mut s.audio_dynamics, 0.0..=1.0).show_value(false));
        });

        ui.add_space(8.0);
        ui.label(RichText::new("Locking").strong());
        ui.horizontal(|ui| {
            ui.label("Passcode");
            ui.add(
                egui::TextEdit::singleline(&mut s.passcode)
                    .password(true)
                    .desired_width(80.0),
            );
        });
        let has_code = !s.passcode.is_empty();
        if ui
            .add_enabled(has_code, egui::Button::new("Lock now"))
            .clicked()
        {
            self.lock();
        }

        ui.add_space(8.0);
        if self.confirm_reset {
            ui.label(
                RichText::new(
                    "Reset all settings and start an empty show? Saved show files are kept.",
                )
                .color(LED_RED),
            );
            ui.horizontal(|ui| {
                if ui.button("Reset console").clicked() {
                    self.settings = Settings::default();
                    self.artnet_ip_text = self.settings.artnet_target.to_string();
                    self.sacn_ip_text.clear();
                    *d = Desk::new(Show::default(), self.settings.prefs.clone());
                    self.confirm_reset = false;
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_reset = false;
                }
            });
        } else if ui.button("Reset console…").clicked() {
            self.confirm_reset = true;
        }
    }

    fn preferences(&mut self, ui: &mut Ui) {
        let p = &mut self.settings.prefs;
        egui::Grid::new("prefs")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Cuelist fade time");
                ui.horizontal(|ui| {
                    let mut follow = p.cue_fade.is_none();
                    if ui.checkbox(&mut follow, "Follow the dial").changed() {
                        p.cue_fade = if follow { None } else { Some(2.0) };
                    }
                    if let Some(f) = &mut p.cue_fade {
                        ui.add(
                            egui::DragValue::new(f)
                                .range(0.0..=600.0)
                                .speed(0.1)
                                .suffix(" s"),
                        );
                    }
                });
                ui.end_row();
                ui.label("Cue start action");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut p.cue_start, CueStart::Halt, "Wait for Play");
                    ui.selectable_value(&mut p.cue_start, CueStart::Follow, "Follow");
                });
                ui.end_row();
                ui.label("Cue follow time");
                ui.add(
                    egui::DragValue::new(&mut p.cue_follow)
                        .range(-600.0..=600.0)
                        .speed(0.1)
                        .suffix(" s"),
                );
                ui.end_row();
                ui.label("Chase speed");
                ui.add(
                    egui::DragValue::new(&mut p.chase_bpm)
                        .range(1.0..=600.0)
                        .suffix(" bpm"),
                );
                ui.end_row();
                ui.label("Chase direction");
                ui.horizontal(|ui| {
                    for dir in Direction::ALL {
                        ui.selectable_value(&mut p.chase_direction, dir, dir.name());
                    }
                });
                ui.end_row();
                ui.label("Chase crossfade");
                ui.horizontal(|ui| {
                    for x in Crossfade::ALL {
                        ui.selectable_value(&mut p.chase_crossfade, x, x.name());
                    }
                });
                ui.end_row();
                ui.label("Scene pages");
                ui.add(egui::DragValue::new(&mut p.scene_pages).range(1..=SCENE_PAGES));
                ui.end_row();
                ui.label("LED intensity curve");
                ui.checkbox(&mut p.led_curve, "Slow start for fast-switching LEDs");
                ui.end_row();
                ui.label("White LED blend");
                ui.add(
                    egui::Slider::new(&mut p.white_blend, 0.0..=1.0)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                );
                ui.end_row();
                ui.label("Amber LED blend");
                ui.add(
                    egui::Slider::new(&mut p.amber_blend, 0.0..=1.0)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                );
                ui.end_row();
            });
        ui.label(
            RichText::new(
                "Blend: at 0% white (or amber) comes only from the W (A) LEDs; at 100% the RGB LEDs join in at full.",
            )
            .small()
            .weak(),
        );
    }

    fn about(&mut self, ui: &mut Ui, d: &Desk) {
        let st = self.output.status.lock().unwrap().clone();
        egui::Grid::new("about").num_columns(2).show(ui, |ui| {
            ui.label("Software version");
            ui.label(env!("CARGO_PKG_VERSION"));
            ui.end_row();
            ui.label("Fixture library");
            ui.label(format!(
                "{} generic, {} in My Fixtures",
                library().len(),
                d.show.my_fixtures.len()
            ));
            ui.end_row();
            ui.label("Outputs");
            let mut outs = vec!["Art-Net", "sACN"];
            if cfg!(feature = "enttec") {
                outs.push("ENTTEC USB Pro");
            }
            ui.label(outs.join(", "));
            ui.end_row();
            ui.label("Microphone");
            ui.label(if cfg!(feature = "audio") {
                "built in"
            } else {
                "not built"
            });
            ui.end_row();
            ui.label("DMX frames sent");
            ui.label(st.frames.to_string());
            ui.end_row();
            ui.label("Show file");
            ui.label(self.show_path.display().to_string());
            ui.end_row();
            ui.label("Settings");
            ui.label(self.settings_path.display().to_string());
            ui.end_row();
        });
    }

    // ---- Autopilot ------------------------------------------------------------------------

    fn autopilot(&mut self, ui: &mut Ui, d: &mut Desk) {
        ui.heading("Autopilot");
        ui.label(
            RichText::new(
                "Plays every built-in colour chase on all patched fixtures, one after another. \
                 Leaving this window turns Autopilot off.",
            )
            .small()
            .weak(),
        );
        ui.add_space(8.0);
        if d.patched_channels().is_empty() {
            ui.label(RichText::new("Patch some fixtures first.").color(ACCENT));
            return;
        }
        ui.label(
            RichText::new(format!(
                "{} · {}/{}",
                d.autopilot.chase_name(),
                d.autopilot.index + 1,
                effects::all_chases().len()
            ))
            .size(20.0)
            .strong(),
        );
        ui.add_space(6.0);
        let big = vec2(110.0, 44.0);
        ui.horizontal(|ui| {
            if ui.add_sized(big, egui::Button::new("⏮ Previous")).clicked() {
                d.autopilot_step(false);
            }
            let label = if d.autopilot.playing {
                "⏸ Pause"
            } else {
                "▶ Play"
            };
            if ui
                .add_sized(big, egui::Button::new(RichText::new(label).strong()))
                .clicked()
            {
                d.autopilot_play_pause();
            }
            if ui.add_sized(big, egui::Button::new("Next ⏭")).clicked() {
                d.autopilot_step(true);
            }
        });
        ui.add_space(8.0);
        let a = &mut d.autopilot;
        egui::Grid::new("autopilot")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Duration");
                ui.add(
                    egui::DragValue::new(&mut a.duration)
                        .range(2.0..=3600.0)
                        .suffix(" s"),
                );
                ui.end_row();
                ui.label("Repeat");
                ui.checkbox(&mut a.repeat, "Stay on this chase");
                ui.end_row();
                ui.label("Audio");
                ui.checkbox(&mut a.audio, "Step on the beat (mic, or M key)");
                ui.end_row();
                ui.label("BPM");
                ui.add_enabled(
                    !a.audio,
                    egui::DragValue::new(&mut a.bpm).range(1.0..=600.0),
                );
                ui.end_row();
            });
        ui.add_space(8.0);
        if ui.button("Lock").clicked() {
            self.lock();
        }
    }
}

fn dmx_grid(ui: &mut Ui, dmx: &[u8; 512]) {
    let cols = 24;
    let rows = 512usize.div_ceil(cols);
    let cell = vec2(24.0, 15.0);
    let (rect, r) = ui.allocate_exact_size(
        vec2(cell.x * cols as f32 + 30.0, cell.y * rows as f32),
        Sense::hover(),
    );
    let p = ui.painter();
    for row in 0..rows {
        p.text(
            rect.min + vec2(0.0, cell.y * (row as f32 + 0.5)),
            Align2::LEFT_CENTER,
            format!("{}", row * cols + 1),
            FontId::monospace(9.0),
            Color32::from_gray(120),
        );
        for col in 0..cols {
            let i = row * cols + col;
            if i >= 512 {
                break;
            }
            let v = dmx[i];
            let c = Rect::from_min_size(
                rect.min + vec2(30.0 + cell.x * col as f32, cell.y * row as f32),
                cell - vec2(1.0, 1.0),
            );
            p.rect_filled(c, 1.0, Color32::from_rgb(20, 22, 28));
            if v > 0 {
                let h = c.height() * v as f32 / 255.0;
                p.rect_filled(
                    Rect::from_min_max(pos2(c.min.x, c.max.y - h), c.max),
                    1.0,
                    Color32::from_rgb(60, 70, 110),
                );
                p.text(
                    c.center(),
                    Align2::CENTER_CENTER,
                    v.to_string(),
                    FontId::monospace(9.0),
                    Color32::WHITE,
                );
            }
        }
    }
    if let Some(pos) = r.hover_pos() {
        let rel = pos - rect.min - vec2(30.0, 0.0);
        if rel.x >= 0.0 {
            let i = (rel.y / cell.y) as usize * cols + (rel.x / cell.x) as usize;
            if i < 512 {
                r.on_hover_text(format!("DMX {} = {}", i + 1, dmx[i]));
            }
        }
    }
}

fn help(ui: &mut Ui) {
    ui.heading("Quickstart");
    ui.label(HELP);
}

const HELP: &str = "\
PATCH: open Patch and give each of the 24 fixture channels a fixture type and DMX address \
(the Wizard patches a row of the same fixture at once). Channels 1-12 are the Black page, \
13-24 the White page; PAGE switches the 12 strips between them. The library has about 630 \
fixture modes (Generic, Shehds, Chauvet DJ, ADJ, Cameo, Eurolite, Stairville, Showtec and \
more): search it by name, maker or channels (\"7x18\", \"RGBWA\", \"moving\"). Library → \
Import adds any fixture file from the Open Fixture Library.

FIXTURE STRIPS
• Fader: intensity. A red fader cap is gated: move it to the output level to take control.
• Flash: full while held.
• Select: click to select, Ctrl+click for a range, double-click to start a new selection. \
Turning the dials of any selected fixture changes them all. Click fixtures on stage too.
• Hue dial: drag to turn, click to step through six colours (saturation goes to full). \
On cool/warm white fixtures it sets colour temperature.
• Saturation dial: drag to turn, click for full / 50% / white.
• SHIFT + dials: the alternate features (Shift+Hue: zoom or UV, Shift+Sat: strobe by default). \
Double-click SHIFT to fan the dials across the selection.
• POSITION + dials: moving heads. Hue pans, Saturation tilts, clicking a dial re-homes it. \
Fan works here too, for spreading a row of movers.

CLEAR: click whites the selection, double-click whites everything, triple-click clears all \
intensity and colour. SHIFT + CLEAR resets the panel. UNDO reverts the last change.

COLOUR WINDOW: three pages of swatches, a colour picker and RGB sliders, a library of 80+ \
named colours, ten pages of patterns (odds & evens, halves, crossfades, gradients, thirds, \
rainbows and palettes) and twelve pages of chases, applied to the selection. Colour chases \
leave intensity to the faders; the Intensity page (comet, Knight Rider, pulse, flicker…) \
brings its own levels, so pull those faders down.

RECORD: press RECORD (channels above 0% are included; select buttons toggle them), then:
• a Scene button → a scene
• a Chase button → Auto (a step per channel, the look walks along) or Manual (adds a step)
• a Flash button → a scene on that fader in the current scene page
• Cuelist PLAY → a new cue
While a chase is playing you can copy it into a Scene or Chase button.

PLAYBACK: Scene and Chase buttons toggle their items (LED flashes while playing); their \
faders are masters. FIXTURES/SCENES turns the 12 faders into scene submasters on up to 6 \
pages. The fade time dial sets the live fade; the chase rate dial scales chase speed (click: \
stop chases, double-click: 100%); SYNC taps the tempo. Cuelist: PLAY (go), BACK, PAUSE, \
RELEASE. Right-click a playback button to edit or delete it.

Colour is Latest Takes Precedence, intensity Highest Takes Precedence.

SNAPSHOT freezes the output while you build the next look; press again to crossfade to it. \
Grand Master scales everything; DBO is a blackout while held.

KEYS: Space cuelist Go · R record · Esc cancel · Ctrl+Z undo · S snapshot · T sync · \
M beat (audio chases) · P position · Tab page · 1-4 scenes · 5-8 chases · hold B blackout · \
hold Shift for Shift.";
