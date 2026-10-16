//! The window: the Stage CL front panel along the bottom, the touchscreen on
//! the left and the stage visualizer in the middle.

mod panel;
mod screens;

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{self, Color32, Key, RichText, Ui};
use serde::{Deserialize, Serialize};

use crate::colour::Rgb;
use crate::engine::{Cue, Desk, Look, PlayRef, Prefs, RecordChoice, Show, Slot};
use crate::fixtures::{FixtureDef, Patched};
use crate::output::{Network, Output};
use crate::visualizer::Visualizer;

pub(crate) const PANEL: Color32 = Color32::from_rgb(28, 30, 36);
pub(crate) const KEY: Color32 = Color32::from_rgb(52, 55, 64);
pub(crate) const KEY_HOVER: Color32 = Color32::from_rgb(66, 70, 82);
pub(crate) const LED_RED: Color32 = Color32::from_rgb(255, 48, 40);
pub(crate) const LED_BLUE: Color32 = Color32::from_rgb(60, 140, 255);
pub(crate) const LED_GREEN: Color32 = Color32::from_rgb(60, 220, 90);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(255, 196, 64);
pub(crate) const SCREEN: Color32 = Color32::from_rgb(16, 18, 24);

/// Console settings kept between shows (Setup and Preferences).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub prefs: Prefs,
    /// Seconds between autosaves; 0 is off.
    pub autosave_secs: u32,
    pub network: Network,
    pub artnet_target: Ipv4Addr,
    pub artnet_net: u8,
    pub artnet_subnet: u8,
    pub artnet_universe: u8,
    pub sacn_universe: u16,
    pub sacn_priority: u8,
    pub sacn_unicast: Option<Ipv4Addr>,
    pub audio_sensitivity: f32,
    pub audio_dynamics: f32,
    pub passcode: String,
    pub haze: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            prefs: Prefs::default(),
            autosave_secs: 30,
            network: Network::ArtNet,
            artnet_target: Ipv4Addr::BROADCAST,
            artnet_net: 0,
            artnet_subnet: 0,
            artnet_universe: 0,
            sacn_universe: 1,
            sacn_priority: 100,
            sacn_unicast: None,
            audio_sensitivity: 0.5,
            audio_dynamics: 0.5,
            passcode: String::new(),
            haze: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Screen {
    Home,
    Colour,
    Scenes,
    Chases,
    Cuelist,
    Monitor,
    Shows,
    Patch,
    Setup,
    Autopilot,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColourTab {
    Swatches,
    Picker,
    Sliders,
    Patterns,
    Chases,
}

/// Where the fixture editor saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditTarget {
    /// A fixture in My Fixtures (None: a new one).
    Library(Option<usize>),
    /// The fixture patched to this channel.
    Patch(usize),
}

pub(crate) struct Editor {
    pub def: FixtureDef,
    pub target: EditTarget,
    pub apply_all: bool,
    pub save_copy: bool,
}

pub(crate) struct Wizard {
    pub fixture: usize,
    pub quantity: usize,
    pub start_channel: usize,
    pub start_dmx: u16,
    /// Gap between fixtures; 0 packs them at their footprint.
    pub offset: u16,
}

pub struct App {
    desk: Arc<Mutex<Desk>>,
    output: Output,
    epoch: Instant,
    settings: Settings,
    saved_settings: Settings,
    settings_path: PathBuf,
    show_path: PathBuf,
    saved_show: Show,
    last_save: f64,
    status: String,
    visualizer: Visualizer,

    // Touchscreen
    screen: Screen,
    prev_screen: Screen,
    tab: usize,
    colour_tab: ColourTab,
    pattern_page: usize,
    chase_page: usize,
    picker: Rgb,
    sel_slot: Option<Slot>,
    sel_step: Option<usize>,
    step_clip: Option<Look>,
    sel_cue: Option<usize>,
    cue_clip: Option<Cue>,
    sel_play: Option<PlayRef>,
    patch_sel: usize,
    patch_clip: Option<Patched>,
    lib_sel: Option<usize>,
    wizard: Wizard,
    editor: Option<Editor>,
    show_name: String,
    confirm_new: bool,
    confirm_reset: bool,

    // Front panel
    record_dialog: Option<(Slot, Vec<RecordChoice>)>,
    shift_latched: bool,
    fan_latched: bool,
    flash_was_down: [bool; crate::engine::STRIPS],
    rate_click: Option<f64>,
    dbo_button: bool,
    locked: bool,
    lock_entry: String,

    // Setup
    artnet_ip_text: String,
    sacn_ip_text: String,
    #[cfg(feature = "enttec")]
    serial_ports: Vec<String>,
    #[cfg(feature = "enttec")]
    serial_choice: String,
    #[cfg(feature = "audio")]
    mic: Option<crate::audio::Mic>,
    #[cfg(feature = "audio")]
    tuning: crate::audio::Tuning,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub(crate) fn show_name(path: &Path) -> String {
    let file = path
        .file_name()
        .map_or(String::new(), |f| f.to_string_lossy().into_owned());
    file.strip_suffix(".show.json")
        .or_else(|| file.strip_suffix(".json"))
        .unwrap_or(&file)
        .to_string()
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, show_path: PathBuf, data_dir: PathBuf) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let settings_path = data_dir.join("settings.json");
        let settings: Settings = read_json(&settings_path).unwrap_or_default();
        let epoch = Instant::now();
        let desk = Arc::new(Mutex::new(Desk::new(
            Show::default(),
            settings.prefs.clone(),
        )));
        let output = Output::start(desk.clone(), epoch);
        let mut app = Self {
            desk,
            output,
            epoch,
            saved_settings: settings.clone(),
            artnet_ip_text: settings.artnet_target.to_string(),
            sacn_ip_text: settings
                .sacn_unicast
                .map_or(String::new(), |ip| ip.to_string()),
            visualizer: Visualizer {
                haze: settings.haze,
            },
            settings,
            settings_path,
            show_name: show_name(&show_path),
            show_path,
            saved_show: Show::default(),
            last_save: 0.0,
            status: String::new(),
            screen: Screen::Home,
            prev_screen: Screen::Home,
            tab: 0,
            colour_tab: ColourTab::Swatches,
            pattern_page: 0,
            chase_page: 0,
            picker: Rgb::new(1.0, 0.0, 0.0),
            sel_slot: None,
            sel_step: None,
            step_clip: None,
            sel_cue: None,
            cue_clip: None,
            sel_play: None,
            patch_sel: 0,
            patch_clip: None,
            lib_sel: None,
            wizard: Wizard {
                fixture: 0,
                quantity: 1,
                start_channel: 0,
                start_dmx: 1,
                offset: 0,
            },
            editor: None,
            confirm_new: false,
            confirm_reset: false,
            record_dialog: None,
            shift_latched: false,
            fan_latched: false,
            flash_was_down: [false; crate::engine::STRIPS],
            rate_click: None,
            dbo_button: false,
            locked: false,
            lock_entry: String::new(),
            #[cfg(feature = "enttec")]
            serial_ports: crate::enttec::list_ports(),
            #[cfg(feature = "enttec")]
            serial_choice: String::new(),
            #[cfg(feature = "audio")]
            mic: None,
            #[cfg(feature = "audio")]
            tuning: Arc::new(Mutex::new([0.5, 0.5])),
        };
        if app.show_path.exists() {
            app.load_show(&app.show_path.clone());
        } else {
            app.set_show(crate::demo::demo_show());
            app.status = format!("Demo show loaded — saves to {}", app.show_path.display());
        }
        app.push_output_config();
        app
    }

    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    // ---- shows and settings ---------------------------------------------------------------

    fn set_show(&mut self, show: Show) {
        let mut d = self.desk.lock().unwrap();
        *d = Desk::new(show, self.settings.prefs.clone());
        self.saved_show = d.show.clone();
        self.sel_slot = None;
        self.sel_cue = None;
        self.sel_step = None;
    }

    pub(crate) fn save_show(&mut self) {
        let show = self.desk.lock().unwrap().show.clone();
        match write_json(&self.show_path, &show) {
            Ok(()) => {
                self.saved_show = show;
                self.status = format!("Saved {}", self.show_path.display());
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
        self.last_save = self.now();
    }

    pub(crate) fn load_show(&mut self, path: &Path) {
        match read_json::<Show>(path) {
            Ok(show) => {
                self.show_path = path.to_path_buf();
                self.show_name = show_name(path);
                self.set_show(show);
                self.status = format!("Loaded {}", path.display());
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }

    /// Show files next to the current one.
    pub(crate) fn show_files(&self) -> Vec<PathBuf> {
        let dir = self.show_dir();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| {
                        p.file_name()
                            .is_some_and(|n| n.to_string_lossy().ends_with(".show.json"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    pub(crate) fn show_dir(&self) -> PathBuf {
        self.show_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }

    fn save_settings_if_changed(&mut self) {
        if self.settings != self.saved_settings {
            if let Err(e) = write_json(&self.settings_path, &self.settings) {
                self.status = format!("Settings not saved: {e}");
            }
            self.saved_settings = self.settings.clone();
            self.push_output_config();
        }
    }

    fn push_output_config(&self) {
        let s = &self.settings;
        let mut cfg = self.output.config.lock().unwrap();
        cfg.network = s.network;
        cfg.artnet_target = s.artnet_target;
        cfg.artnet_universe =
            crate::artnet::port_address(s.artnet_net, s.artnet_subnet, s.artnet_universe);
        cfg.sacn_universe = s.sacn_universe.clamp(1, 63999);
        cfg.sacn_priority = s.sacn_priority.min(200);
        cfg.sacn_unicast = s.sacn_unicast;
        #[cfg(feature = "audio")]
        {
            *self.tuning.lock().unwrap() = [s.audio_sensitivity, s.audio_dynamics];
        }
    }

    // ---- top-level frame ------------------------------------------------------------------

    fn keyboard(&mut self, ui: &Ui, d: &mut Desk) {
        let typing = ui.ctx().egui_wants_keyboard_input();
        let (shift_held, b_held) = ui.input(|i| (i.modifiers.shift, i.key_down(Key::B)));
        d.shift = self.shift_latched || (shift_held && !typing);
        d.fan = self.fan_latched;
        d.dbo = self.dbo_button || (b_held && !typing);
        if typing || self.locked {
            return;
        }
        ui.input(|i| {
            if i.key_pressed(Key::Space) {
                if d.record {
                    d.record_cue();
                } else {
                    d.cue_go();
                }
            }
            if i.modifiers.command && i.key_pressed(Key::Z) {
                d.undo();
            } else if i.key_pressed(Key::R) {
                d.press_record();
            }
            if i.key_pressed(Key::Escape) {
                d.record = false;
                self.record_dialog = None;
            }
            if i.key_pressed(Key::S) && !i.modifiers.command {
                d.press_snapshot();
            }
            if i.key_pressed(Key::T) {
                d.press_sync();
            }
            if i.key_pressed(Key::M) {
                d.beat();
            }
            if i.key_pressed(Key::Tab) {
                d.press_page();
            }
            let keys = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
            ];
            for (n, k) in keys.into_iter().enumerate() {
                if i.key_pressed(k) {
                    let slot = if n < 4 {
                        Slot::Scene(n)
                    } else {
                        Slot::Chase(n - 4)
                    };
                    self.press_playback(d, slot);
                }
            }
        });
    }

    /// A Scene/Chase/fader-slot button: record into it in Record mode, else play it.
    pub(crate) fn press_playback(&mut self, d: &mut Desk, slot: Slot) {
        if d.record {
            let options = d.record_options(slot);
            if options.len() == 1 {
                d.record_to(slot, options[0]);
            } else {
                self.record_dialog = Some((slot, options));
            }
        } else {
            d.toggle_slot(slot);
        }
    }

    fn record_dialog_ui(&mut self, ui: &mut Ui, d: &mut Desk) {
        let Some((slot, options)) = self.record_dialog.clone() else {
            return;
        };
        let existing = d.show.item(slot).map(|i| i.name().to_string());
        let modal = egui::Modal::new(egui::Id::unique("record")).show(ui.ctx(), |ui| {
            ui.heading(format!("Record to {}", slot.label()));
            if let Some(name) = &existing {
                ui.label(RichText::new(format!("Replaces “{name}”")).weak());
            }
            if matches!(slot, Slot::Chase(_)) {
                ui.label(
                    RichText::new(
                        "Auto builds a chase with a step per included channel. \
                         Manual adds the current look as the next step.",
                    )
                    .small()
                    .weak(),
                );
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for c in &options {
                    if ui.button(c.name()).clicked() {
                        d.record_to(slot, *c);
                        self.record_dialog = None;
                    }
                }
                if ui.button("Cancel").clicked() {
                    d.record = false;
                    self.record_dialog = None;
                }
            });
        });
        if modal.should_close() {
            self.record_dialog = None;
        }
    }

    fn lock_ui(&mut self, ui: &mut Ui) {
        if !self.locked {
            return;
        }
        egui::Modal::new(egui::Id::unique("lock")).show(ui.ctx(), |ui| {
            ui.heading("Console locked");
            ui.label("Enter the passcode to unlock.");
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.lock_entry)
                    .password(true)
                    .desired_width(120.0),
            );
            r.request_focus();
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if (ui.button("Unlock").clicked() || enter) && self.lock_entry == self.settings.passcode
            {
                self.locked = false;
                self.lock_entry.clear();
            }
        });
    }

    pub(crate) fn lock(&mut self) {
        if self.settings.passcode.is_empty() {
            self.status = "Set a passcode in Setup first".into();
            self.screen = Screen::Setup;
            self.tab = 0;
        } else {
            self.locked = true;
            self.lock_entry.clear();
        }
    }

    fn top_bar(&mut self, ui: &mut Ui, d: &Desk) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("STAGE-LITE").strong().color(ACCENT));
            ui.label(
                RichText::new("HSI LED CONSOLE · 24 FIXTURES · 512 CH")
                    .small()
                    .weak(),
            );
            ui.separator();
            ui.label(RichText::new(&self.show_name).strong());
            if ui.button("Save").clicked() {
                self.save_show();
            }
            ui.separator();
            let net = self.output.status.lock().unwrap().network.clone();
            let colour = if net.contains("error") {
                LED_RED
            } else if self.settings.network == Network::Off {
                Color32::GRAY
            } else {
                LED_GREEN
            };
            ui.label(RichText::new("•").size(18.0).color(colour));
            ui.label(RichText::new(net).small());
            ui.separator();
            let msg = if d.message.is_empty() {
                &self.status
            } else {
                &d.message
            };
            ui.label(RichText::new(msg).weak());
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let t = self.now();
        let desk = self.desk.clone();
        let mut d = desk.lock().unwrap();
        if d.prefs != self.settings.prefs {
            d.prefs = self.settings.prefs.clone();
        }
        if self.screen != Screen::Autopilot && d.autopilot.on {
            d.autopilot_off();
        }

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui, &d));
        egui::Panel::bottom("front")
            .resizable(false)
            .show(ui, |ui| self.front_panel(ui, &mut d, t));
        egui::Panel::left("screen")
            .default_size(640.0)
            .min_size(520.0)
            .resizable(true)
            .frame(egui::Frame::new().fill(SCREEN).inner_margin(8.0))
            .show(ui, |ui| self.touchscreen(ui, &mut d, t));
        egui::CentralPanel::default_margins().show(ui, |ui| {
            let out = d.out;
            if let Some(ch) = self.visualizer.show(ui, &d.show, &out, &d.prog.selected, t) {
                d.press_select(ch, panel::ctrl_click(ui), false);
            }
        });
        self.keyboard(ui, &mut d);
        self.record_dialog_ui(ui, &mut d);
        self.lock_ui(ui);

        // Autosave once the show has changed and the interval has passed.
        let interval = self.settings.autosave_secs as f64;
        let show_changed =
            interval > 0.0 && t - self.last_save >= interval && d.show != self.saved_show;
        drop(d);
        if show_changed {
            self.save_show();
        }
        self.settings.haze = self.visualizer.haze;
        self.save_settings_if_changed();
        ui.ctx().request_repaint();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let changed = self.desk.lock().unwrap().show != self.saved_show;
        if changed {
            self.save_show();
        }
        self.save_settings_if_changed();
    }
}
