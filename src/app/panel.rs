//! The front panel: 12 HSI channel strips and the master section.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Rect, Response, RichText, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};

use super::{ACCENT, App, KEY, KEY_HOVER, LED_BLUE, LED_RED, PANEL};
use crate::colour::Rgb;
use crate::engine::{
    DEFAULT_FADE, Desk, DialKind, FaderMode, Item, Led, PAGE_NAMES, PlayRef, STRIPS, Slot,
};
use crate::fixtures::ColourKind;

const STRIP_W: f32 = 62.0;
const FADER_H: f32 = 150.0;
const DIAL: f32 = 26.0;

pub(crate) fn rgb32(c: Rgb) -> Color32 {
    let [r, g, b] = c.to_u8();
    Color32::from_rgb(r, g, b)
}

/// The colour a fixture page or scene page LED shows.
pub(crate) fn page_colour(mode: FaderMode, page: usize) -> Color32 {
    match mode {
        FaderMode::Channels => {
            if page == 0 {
                Color32::from_gray(70)
            } else {
                Color32::WHITE
            }
        }
        FaderMode::Scenes => [
            Color32::from_rgb(255, 220, 0),
            Color32::from_rgb(255, 40, 40),
            Color32::from_rgb(40, 220, 60),
            Color32::from_rgb(40, 100, 255),
            Color32::from_rgb(240, 40, 240),
            Color32::from_rgb(40, 230, 240),
        ][page % 6],
    }
}

/// A panel button with a status LED in its corner.
pub(crate) fn key(ui: &mut Ui, label: &str, led: Option<Color32>, size: Vec2) -> Response {
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if r.is_pointer_button_down_on() {
        KEY_HOVER.gamma_multiply(0.8)
    } else if r.hovered() {
        KEY_HOVER
    } else {
        KEY
    };
    p.rect_filled(rect, CornerRadius::same(4), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_black_alpha(120)),
        egui::StrokeKind::Inside,
    );
    p.circle_filled(
        rect.right_top() + vec2(-6.0, 6.0),
        2.8,
        led.unwrap_or(Color32::from_rgb(45, 22, 22)),
    );
    p.text(
        rect.center() + vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        Color32::from_gray(225),
    );
    r
}

/// A push-button encoder: drag to turn, click to press. `value` (0..1) sets the
/// pointer; `ring` colours the collar.
pub(crate) fn knob(ui: &mut Ui, value: f32, ring: Color32, size: f32) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let p = ui.painter();
    let c = rect.center();
    let radius = size / 2.0 - 1.0;
    p.circle_filled(c, radius, ring);
    p.circle_filled(
        c,
        radius - 3.0,
        if r.hovered() || r.dragged() {
            Color32::from_gray(70)
        } else {
            Color32::from_gray(50)
        },
    );
    // 300° of travel, starting at 7 o'clock.
    let a = (210.0 - 300.0 * value.clamp(0.0, 1.0)).to_radians();
    let dir = vec2(a.cos(), -a.sin());
    p.line_segment(
        [c + dir * (radius * 0.3), c + dir * (radius - 4.0)],
        Stroke::new(2.0, Color32::from_gray(230)),
    );
    r.on_hover_cursor(egui::CursorIcon::ResizeVertical)
}

/// A fader with the output level shown behind it. A gated fader (not at the
/// level it controls) has a red cap. Returns the new position while dragged.
pub(crate) fn fader(
    ui: &mut Ui,
    pos: f32,
    output: Option<f32>,
    gated: bool,
    width: f32,
    height: f32,
) -> Option<f32> {
    let (rect, r) = ui.allocate_exact_size(vec2(width, height), Sense::drag());
    let p = ui.painter();
    let cap_h = 16.0;
    let travel = height - cap_h;
    let track = Rect::from_center_size(rect.center(), vec2(8.0, travel));
    p.rect_filled(track, 3.0, Color32::from_rgb(12, 12, 14));
    if let Some(level) = output {
        let h = travel * level.clamp(0.0, 1.0);
        p.rect_filled(
            Rect::from_min_max(pos2(track.min.x, track.max.y - h), track.max),
            3.0,
            Color32::from_rgb(110, 110, 120),
        );
    }
    for i in 0..=10 {
        let y = track.max.y - travel * i as f32 / 10.0;
        let w = if i % 5 == 0 { 10.0 } else { 6.0 };
        p.line_segment(
            [pos2(rect.min.x + 4.0, y), pos2(rect.min.x + 4.0 + w, y)],
            Stroke::new(1.0, Color32::from_gray(80)),
        );
    }
    let y = track.max.y - travel * pos.clamp(0.0, 1.0);
    let cap = Rect::from_center_size(pos2(rect.center().x, y), vec2(width - 18.0, cap_h));
    let fill = if gated {
        Color32::from_rgb(150, 50, 50)
    } else if r.hovered() || r.dragged() {
        Color32::from_gray(200)
    } else {
        Color32::from_gray(170)
    };
    p.rect_filled(cap, 2.0, fill);
    p.line_segment(
        [pos2(cap.min.x + 2.0, y), pos2(cap.max.x - 2.0, y)],
        Stroke::new(1.5, Color32::from_gray(40)),
    );
    let r = r.on_hover_cursor(egui::CursorIcon::ResizeVertical);
    if r.dragged() {
        let dy = r.drag_delta().y;
        if dy != 0.0 {
            return Some((pos - dy / travel).clamp(0.0, 1.0));
        }
    }
    None
}

fn percent(v: f32) -> String {
    if v >= 0.995 {
        "FL".into()
    } else {
        format!("{:.0}", v * 100.0)
    }
}

fn led(state: Led, blink: bool, record: bool) -> Option<Color32> {
    match (record, state) {
        // In Record, empty destinations flash and used ones are lit.
        (true, Led::Off) => blink.then_some(LED_RED),
        (true, _) => Some(LED_RED),
        (false, Led::Flash) => blink.then_some(LED_RED),
        (false, Led::On) => Some(LED_RED.gamma_multiply(0.45)),
        (false, Led::Off) => None,
    }
}

/// Whether Ctrl (Cmd on macOS) was held for the click that just happened. Read
/// from the click itself, as the key may already be up by the end of the frame.
pub(crate) fn ctrl_click(ui: &Ui) -> bool {
    ui.input(|i| {
        i.modifiers.command
            || i.events.iter().any(
                |e| matches!(e, egui::Event::PointerButton { modifiers, .. } if modifiers.command),
            )
    })
}

fn small(ui: &mut Ui, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into())
            .size(10.0)
            .color(Color32::from_gray(170)),
    );
}

impl App {
    pub(crate) fn front_panel(&mut self, ui: &mut Ui, d: &mut Desk, t: f64) {
        let blink = (t * 2.4).fract() < 0.5;
        let slow = (t * 1.2).fract() < 0.6;

        // A single tap on the rate dial stops the chases, unless it becomes a double tap.
        if let Some(at) = self.rate_click
            && t - at > 0.35
        {
            d.stop_chases();
            d.message = "Chases stopped".into();
            self.rate_click = None;
        }

        egui::Frame::new()
            .fill(PANEL)
            .inner_margin(8.0)
            .corner_radius(6.0)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
                    for strip in 0..STRIPS {
                        self.strip(ui, d, strip, slow);
                    }
                    ui.separator();
                    self.masters(ui, d, t, blink, slow);
                });
            });
    }

    fn strip(&mut self, ui: &mut Ui, d: &mut Desk, strip: usize, slow: bool) {
        let ch = d.strip_channel(strip);
        ui.vertical(|ui| {
            ui.set_width(STRIP_W);
            // Header: channel and fixture, or the scene on this fader.
            let (title, sub) = match d.mode {
                FaderMode::Channels => (
                    format!("{}", ch + 1),
                    d.show
                        .patched(ch)
                        .map_or("—".to_string(), |p| p.def.name.clone()),
                ),
                FaderMode::Scenes => {
                    let slot = Slot::Fader(d.scene_page, strip);
                    (
                        slot.label(),
                        d.show
                            .item(slot)
                            .map_or("—".to_string(), |i| i.name().to_string()),
                    )
                }
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).monospace().strong());
            });
            let mut sub: String = sub.chars().take(10).collect();
            if sub.is_empty() {
                sub = " ".into();
            }
            small(ui, sub);

            // Hue and Saturation dials (Shift: the alternate features).
            let o = d.live[ch];
            let kind = d.colour_kind(ch);
            ui.horizontal(|ui| {
                for dial in [DialKind::Hue, DialKind::Sat] {
                    let (value, ring) = if d.shift {
                        let a = if dial == DialKind::Hue { 0 } else { 1 };
                        let v = o.alt[a].unwrap_or_else(|| d.alt_home(ch, a));
                        (v, Color32::from_rgb(120, 90, 200))
                    } else {
                        match (dial, kind) {
                            (DialKind::Hue, ColourKind::Temperature) => (
                                o.hs.h,
                                rgb32(crate::colour::white_temperature(o.hs.h)),
                            ),
                            (DialKind::Hue, _) => (
                                o.hs.h,
                                rgb32(crate::colour::Hs::new(o.hs.h, 1.0).rgb()),
                            ),
                            (DialKind::Sat, _) => (o.hs.s, rgb32(o.rgb)),
                        }
                    };
                    let r = knob(ui, value, ring, DIAL);
                    let tip = match (d.shift, dial) {
                        (true, DialKind::Hue) => format!(
                            "Shift+Hue: {}",
                            d.show
                                .patched(ch)
                                .and_then(|p| p.def.alt_name(0))
                                .unwrap_or("nothing assigned".into())
                        ),
                        (true, DialKind::Sat) => format!(
                            "Shift+Sat: {}",
                            d.show
                                .patched(ch)
                                .and_then(|p| p.def.alt_name(1))
                                .unwrap_or("nothing assigned".into())
                        ),
                        (false, DialKind::Hue) if kind == ColourKind::Temperature => {
                            "Colour temperature: drag to turn, click for warm/mid/cool".into()
                        }
                        (false, DialKind::Hue) => {
                            "Hue: drag to turn, click to step through six colours".into()
                        }
                        (false, DialKind::Sat) => {
                            "Saturation: drag to turn, click for full / 50% / white".into()
                        }
                    };
                    let r = r.on_hover_text(tip);
                    if r.drag_started() {
                        d.checkpoint();
                    }
                    if r.dragged() {
                        let dd = r.drag_delta();
                        let delta = (dd.x - dd.y) / 220.0;
                        if delta != 0.0 {
                            d.turn_dial(ch, dial, delta);
                        }
                    }
                    if r.clicked() {
                        d.press_dial(ch, dial);
                    }
                }
            });
            ui.horizontal(|ui| {
                small(ui, if d.shift { "Shft H" } else { "Hue" });
                ui.add_space(4.0);
                small(ui, if d.shift { "Shft S" } else { "Sat" });
            });

            // Colour mimic and Select.
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(14.0, 22.0), Sense::hover());
                let out = d.out[ch];
                let mimic = rgb32(out.rgb.scale(0.15 + 0.85 * out.intensity));
                ui.painter().circle_filled(rect.center(), 6.0, mimic);
                let sel_led = if d.record {
                    (d.record_include[ch] && slow).then_some(LED_BLUE)
                } else {
                    d.prog.selected[ch].then_some(LED_BLUE)
                };
                let r = key(ui, "SELECT", sel_led, vec2(STRIP_W - 18.0, 22.0)).on_hover_text(
                    "Click to select · Ctrl+click selects a range · double-click starts a new selection",
                );
                if r.double_clicked() {
                    d.press_select(ch, false, true);
                } else if r.clicked() {
                    d.press_select(ch, ctrl_click(ui), false);
                }
            });

            // Flash: held while the pointer is down.
            let flash_led = match d.mode {
                FaderMode::Channels => d.flash_held(strip).then_some(LED_RED),
                FaderMode::Scenes => {
                    let slot = Slot::Fader(d.scene_page, strip);
                    led(d.slot_led(slot), slow, d.record)
                }
            };
            let r = key(ui, "FLASH", flash_led, vec2(STRIP_W - 4.0, 24.0));
            let down = r.is_pointer_button_down_on();
            if down && !self.flash_was_down[strip] {
                if d.record {
                    let slot = Slot::Fader(d.scene_page, strip);
                    self.press_playback(d, slot);
                } else {
                    d.flash_down(strip);
                }
            } else if !down && self.flash_was_down[strip] {
                d.flash_up(strip);
            }
            self.flash_was_down[strip] = down;

            let output = match d.mode {
                FaderMode::Channels => d.live[ch].level,
                FaderMode::Scenes => d.scene_levels[d.scene_page][strip],
            };
            if let Some(v) = fader(
                ui,
                d.faders[strip],
                Some(output),
                d.gated[strip],
                STRIP_W - 4.0,
                FADER_H,
            ) {
                d.move_fader(strip, v);
            }
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(percent(d.fader_target(strip)))
                        .monospace()
                        .size(11.0),
                );
            });
        });
    }

    fn masters(&mut self, ui: &mut Ui, d: &mut Desk, t: f64, blink: bool, slow: bool) {
        let k = vec2(66.0, 26.0);
        // Programmer keys.
        ui.vertical(|ui| {
            small(ui, "PROGRAMMER");
            let shift_led = if self.fan_latched {
                blink.then_some(ACCENT)
            } else {
                d.shift.then_some(ACCENT)
            };
            let r = key(ui, "SHIFT", shift_led, k).on_hover_text(
                "Shift: dials drive the alternate features (zoom, strobe). \
                 Double-click to fan the dials across the selection. Holding the Shift key works too.",
            );
            if r.double_clicked() {
                self.fan_latched = true;
                self.shift_latched = false;
            } else if r.clicked() {
                if self.fan_latched {
                    self.fan_latched = false;
                } else {
                    self.shift_latched = !self.shift_latched;
                }
            }
            let r = key(ui, "CLEAR", None, k).on_hover_text(
                "Click: selection to white · double: everything to white · \
                 triple: clear all intensity and colour · Shift+Clear: reset the panel",
            );
            if r.triple_clicked() {
                d.press_clear(3);
            } else if r.double_clicked() {
                d.press_clear(2);
            } else if r.clicked() {
                let reset = d.shift;
                d.press_clear(1);
                if reset {
                    self.shift_latched = false;
                    self.fan_latched = false;
                }
            }
            if key(ui, "UNDO", d.can_undo().then_some(LED_RED.gamma_multiply(0.4)), k)
                .on_hover_text("Undo the last change (Ctrl+Z)")
                .clicked()
            {
                d.undo();
            }
            if key(ui, "RECORD", (d.record && blink).then_some(LED_RED), k)
                .on_hover_text(
                    "Record, then a Scene, Chase or fader Flash button, or the Cuelist Play button (R)",
                )
                .clicked()
            {
                d.press_record();
                if !d.record {
                    self.record_dialog = None;
                }
            }
            if key(
                ui,
                "SNAPSHOT",
                (d.snapshot_active() && slow).then_some(LED_RED),
                k,
            )
            .on_hover_text("Hold the current output while you set up the next look (S)")
            .clicked()
            {
                d.press_snapshot();
            }
        });

        // Fader mode, page and fixture master.
        ui.vertical(|ui| {
            small(ui, "FADERS");
            let scenes = d.mode == FaderMode::Scenes;
            if key(ui, "FIXTURES/\nSCENES", scenes.then_some(LED_RED), k)
                .on_hover_text("Switch the 12 faders between fixture channels and scene pages")
                .clicked()
            {
                d.toggle_fader_mode();
            }
            let page = if scenes { d.scene_page } else { d.fixture_page };
            if key(ui, "PAGE", Some(page_colour(d.mode, page)), k)
                .on_hover_text("Next page (Tab)")
                .clicked()
            {
                d.press_page();
            }
            let label = if scenes {
                format!("{} page", PAGE_NAMES[d.scene_page])
            } else if d.fixture_page == 0 {
                "Black · 1-12".into()
            } else {
                "White · 13-24".into()
            };
            small(ui, label);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if let Some(v) = fader(ui, d.fixture_master, None, false, 40.0, 92.0) {
                        d.fixture_master = v;
                    }
                    small(ui, format!("FIX {}", percent(d.fixture_master)));
                });
            });
        });

        // Scenes: fade time dial, 4 buttons, master.
        ui.vertical(|ui| {
            small(ui, "SCENES");
            ui.horizontal(|ui| {
                let r = knob(
                    ui,
                    (d.live_fade / 10.0).min(1.0),
                    Color32::from_gray(90),
                    30.0,
                )
                .on_hover_text("Fade time: drag to set · click to show · double-click: 2 s");
                if r.dragged() {
                    let dd = r.drag_delta();
                    let delta = (dd.x - dd.y) / 40.0 * (0.2 + d.live_fade * 0.15);
                    d.turn_fade(delta);
                }
                if r.double_clicked() {
                    d.live_fade = DEFAULT_FADE;
                } else if r.clicked() {
                    d.message = format!("Fade time {:.1} s", d.live_fade);
                }
                ui.label(RichText::new(format!("{:.1}s", d.live_fade)).monospace());
            });
            for row in 0..2 {
                ui.horizontal(|ui| {
                    for i in row * 2..row * 2 + 2 {
                        let slot = Slot::Scene(i);
                        self.playback_key(
                            ui,
                            d,
                            slot,
                            &format!("{}", i + 1),
                            slow,
                            vec2(32.0, 30.0),
                        );
                    }
                });
            }
            if let Some(v) = fader(ui, d.scene_master, None, false, 40.0, 60.0) {
                d.scene_master = v;
            }
            small(ui, format!("SCN {}", percent(d.scene_master)));
        });

        // Chases: rate dial, sync, 4 buttons, master.
        ui.vertical(|ui| {
            small(ui, "CHASES");
            ui.horizontal(|ui| {
                let r = knob(
                    ui,
                    (d.rate_scale / 4.0).min(1.0),
                    Color32::from_gray(90),
                    30.0,
                )
                .on_hover_text(
                    "Chase rate: drag to scale speed · click stops all chases · double-click: 100%",
                );
                if r.dragged() {
                    let dd = r.drag_delta();
                    d.turn_rate((dd.x - dd.y) / 200.0);
                }
                if r.double_clicked() {
                    d.reset_rate();
                    self.rate_click = None;
                } else if r.clicked() {
                    self.rate_click = Some(t);
                }
                let label = match d.tap_bpm {
                    Some(bpm) => format!("{:.0}bpm", bpm * d.rate_scale),
                    None => format!("{:.0}%", d.rate_scale * 100.0),
                };
                ui.label(RichText::new(label).monospace());
            });
            let beat = d
                .tap_bpm
                .is_some_and(|b| ((t * (b * d.rate_scale) as f64 / 60.0).fract()) < 0.2);
            if key(ui, "SYNC", beat.then_some(LED_RED), vec2(68.0, 22.0))
                .on_hover_text("Tap the beat to set the chase tempo (T)")
                .clicked()
            {
                d.press_sync();
            }
            for row in 0..2 {
                ui.horizontal(|ui| {
                    for i in row * 2..row * 2 + 2 {
                        let slot = Slot::Chase(i);
                        self.playback_key(
                            ui,
                            d,
                            slot,
                            &format!("{}", i + 1),
                            slow,
                            vec2(32.0, 30.0),
                        );
                    }
                });
            }
            if let Some(v) = fader(ui, d.chase_master, None, false, 40.0, 38.0) {
                d.chase_master = v;
            }
            small(ui, format!("CHS {}", percent(d.chase_master)));
        });

        // Cuelist.
        ui.vertical(|ui| {
            small(ui, "CUELIST");
            let n = d.show.cuelist.len();
            let label = match d.cue.current {
                Some(i) => format!(
                    "{}/{n} {}",
                    i + 1,
                    d.show.cuelist.get(i).map_or("", |c| c.name.as_str())
                ),
                None => format!("—/{n}"),
            };
            small(ui, label.chars().take(14).collect::<String>());
            let k = vec2(68.0, 24.0);
            if key(ui, "RELEASE", None, k)
                .on_hover_text("Stop the cuelist and reset to cue 1")
                .clicked()
            {
                d.cue_release();
            }
            if key(ui, "BACK", None, k).clicked() {
                d.cue_back();
            }
            if key(ui, "PAUSE", d.cue.paused.is_some().then_some(LED_RED), k).clicked() {
                d.cue_pause();
            }
            let play_led = if d.record {
                blink.then_some(LED_RED)
            } else if d.cue_playing() {
                slow.then_some(LED_RED)
            } else if n > 0 {
                Some(LED_RED.gamma_multiply(0.45))
            } else {
                None
            };
            if key(ui, "PLAY", play_led, vec2(68.0, 30.0))
                .on_hover_text("Go to the next cue (Space). In Record: add a cue.")
                .clicked()
            {
                if d.record {
                    d.record_cue();
                } else {
                    d.cue_go();
                }
            }
            if let Some(v) = fader(ui, d.cue_master, None, false, 40.0, 50.0) {
                d.cue_master = v;
            }
            small(ui, format!("CUE {}", percent(d.cue_master)));
        });

        // Grand Master and DBO.
        ui.vertical(|ui| {
            small(ui, "MASTER");
            if let Some(v) = fader(ui, d.grand_master, None, false, 44.0, 150.0) {
                d.grand_master = v;
            }
            small(ui, format!("GM {}", percent(d.grand_master)));
            let dbo_led = if d.dbo {
                Some(LED_RED)
            } else {
                (d.grand_master < 0.995 && blink).then_some(LED_RED)
            };
            let r = key(ui, "DBO", dbo_led, vec2(48.0, 30.0))
                .on_hover_text("Dead blackout while held (or hold B)");
            self.dbo_button = r.is_pointer_button_down_on();
        });
    }

    fn playback_key(
        &mut self,
        ui: &mut Ui,
        d: &mut Desk,
        slot: Slot,
        label: &str,
        slow: bool,
        size: Vec2,
    ) {
        let state = d.slot_led(slot);
        let r = key(ui, label, led(state, slow, d.record), size);
        let name = d.show.item(slot).map(|i| match i {
            Item::Scene(s) => format!("Scene: {}", s.name),
            Item::Chase(c) => format!("Chase: {} ({} steps)", c.name, c.steps.len()),
        });
        let r = r.on_hover_text(name.unwrap_or_else(|| format!("{} is empty", slot.label())));
        if r.clicked() {
            self.press_playback(d, slot);
        }
        r.context_menu(|ui| {
            ui.label(RichText::new(slot.label()).strong());
            if ui.button("Release").clicked() {
                d.release(PlayRef::Slot(slot));
                ui.close();
            }
            if ui.button("Edit…").clicked() {
                self.sel_slot = Some(slot);
                let chase = matches!(d.show.item(slot), Some(Item::Chase(_)));
                self.screen = if chase {
                    super::Screen::Chases
                } else {
                    super::Screen::Scenes
                };
                self.tab = 1;
                ui.close();
            }
            if ui.button("Delete (Clear + Play)").clicked() {
                d.delete_slot(slot);
                ui.close();
            }
        });
    }
}
