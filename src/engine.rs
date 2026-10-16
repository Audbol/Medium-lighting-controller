//! The Stage CL logic, with no GUI or I/O.
//!
//! * 12 HSI channel strips on two fixture pages (24 fixture channels). Each strip
//!   has a fader, flash, select, Hue and Saturation dials (Shift for the two
//!   alternate features).
//! * The faders switch to Scene mode, where they play up to 6 pages of 12 items.
//! * 4 Scene buttons, 4 Chase buttons and a Cuelist, each with a master.
//! * Intensity is Highest Takes Precedence; colour and the alternate features
//!   are Latest Takes Precedence, crossfading over the incoming item's fade.
//! * Fader gating, Record, Clear, Undo, Snapshot, Grand Master and DBO, the
//!   fade time and chase rate dials with tap sync, audio triggering and Autopilot.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::colour::{Hs, Rgb, WHITE, next_hue_step, next_press_level};
use crate::effects::{self, ColourChase, Pattern};
use crate::fixtures::{Blend, ColourKind, FixtureDef, Patched, render};

pub const STRIPS: usize = 12;
pub const FIXTURE_PAGES: usize = 2;
pub const CHANNELS: usize = STRIPS * FIXTURE_PAGES;
pub const SCENE_PAGES: usize = 6;
pub const PLAYBACKS: usize = 4;
pub const DEFAULT_FADE: f32 = 2.0;
pub const PAGE_NAMES: [&str; SCENE_PAGES] = ["Yellow", "Red", "Green", "Blue", "Magenta", "Cyan"];
const UNDO_DEPTH: usize = 64;
/// How close a gated fader has to come to the output level to take control.
const GATE_WINDOW: f32 = 0.02;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ---- recorded content -----------------------------------------------------------------------

/// One fixture channel in a scene, chase step or cue. Empty fields aren't part
/// of the look (a chase step with only colour leaves intensity to the faders).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Hs>,
    /// Shift+Hue and Shift+Saturation features (zoom and strobe by default).
    #[serde(default)]
    pub alt: [Option<f32>; 2],
    /// Position+Hue and Position+Saturation (pan and tilt by default).
    #[serde(default, skip_serializing_if = "none2")]
    pub pos: [Option<f32>; 2],
}

fn none2(v: &[Option<f32>; 2]) -> bool {
    v.iter().all(Option::is_none)
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        self.level.is_none() && self.colour.is_none() && none2(&self.alt) && none2(&self.pos)
    }

    /// The four dial features: Shift+Hue, Shift+Sat, Position+Hue, Position+Sat.
    fn features(&self) -> [Option<f32>; 4] {
        [self.alt[0], self.alt[1], self.pos[0], self.pos[1]]
    }

    pub fn lerp(a: &Cell, b: &Cell, t: f32) -> Cell {
        let num = |x: Option<f32>, y: Option<f32>| match (x, y) {
            (None, None) => None,
            (Some(x), Some(y)) => Some(lerp(x, y, t)),
            (x, y) => y.or(x),
        };
        Cell {
            level: match (a.level, b.level) {
                (None, None) => None,
                (x, y) => Some(lerp(x.unwrap_or(0.0), y.unwrap_or(0.0), t)),
            },
            colour: match (a.colour, b.colour) {
                (Some(x), Some(y)) => Some(x.lerp(y, t)),
                (x, y) => y.or(x),
            },
            alt: [num(a.alt[0], b.alt[0]), num(a.alt[1], b.alt[1])],
            pos: [num(a.pos[0], b.pos[0]), num(a.pos[1], b.pos[1])],
        }
    }
}

pub type Look = [Cell; CHANNELS];

pub fn empty_look() -> Look {
    [Cell::default(); CHANNELS]
}

fn lerp_look(a: &Look, b: &Look, t: f32) -> Look {
    std::array::from_fn(|i| Cell::lerp(&a[i], &b[i], t))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Forward,
    Backward,
    Bounce,
    Shuffle,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Forward,
        Direction::Backward,
        Direction::Bounce,
        Direction::Shuffle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Direction::Forward => "Forwards",
            Direction::Backward => "Backwards",
            Direction::Bounce => "Bounce",
            Direction::Shuffle => "Shuffle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Crossfade {
    Snap,
    Half,
    Full,
}

impl Crossfade {
    pub const ALL: [Crossfade; 3] = [Crossfade::Snap, Crossfade::Half, Crossfade::Full];

    pub fn name(self) -> &'static str {
        match self {
            Crossfade::Snap => "Snap",
            Crossfade::Half => "Half",
            Crossfade::Full => "Full",
        }
    }

    /// The part of each step spent fading to the next.
    fn fraction(self) -> f32 {
        match self {
            Crossfade::Snap => 0.0,
            Crossfade::Half => 0.5,
            Crossfade::Full => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CueStart {
    /// Waits for Play.
    Halt,
    /// Plays automatically after the previous cue.
    Follow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub look: Look,
    /// None follows the Fade Time dial.
    pub fade: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chase {
    pub name: String,
    pub steps: Vec<Look>,
    pub bpm: f32,
    pub direction: Direction,
    pub mirror: bool,
    pub crossfade: Crossfade,
    /// Filter: play intensity only, colour comes from elsewhere.
    pub intensity_only: bool,
    /// Step on the beat from the microphone instead of the BPM.
    pub audio: bool,
}

impl Chase {
    /// The channels any step uses, in order.
    pub fn channels(&self) -> Vec<usize> {
        (0..CHANNELS)
            .filter(|&ch| self.steps.iter().any(|s| !s[ch].is_empty()))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
// Scenes carry a whole look inline; there are only ~80 of them.
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Scene(Scene),
    Chase(Chase),
}

impl Item {
    pub fn name(&self) -> &str {
        match self {
            Item::Scene(s) => &s.name,
            Item::Chase(c) => &c.name,
        }
    }

    pub fn as_chase(&self) -> Option<&Chase> {
        match self {
            Item::Chase(c) => Some(c),
            Item::Scene(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub name: String,
    pub look: Look,
    pub start: CueStart,
    /// Seconds after the previous cue finishes (negative overlaps its fade).
    pub wait: f32,
    /// None follows the Fade Time dial.
    pub fade: Option<f32>,
}

/// A playback location: the 4 Scene buttons, the 4 Chase buttons, or a fader on
/// a scene page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slot {
    Scene(usize),
    Chase(usize),
    Fader(usize, usize),
}

impl Slot {
    pub fn label(self) -> String {
        match self {
            Slot::Scene(i) => format!("S{}", i + 1),
            Slot::Chase(i) => format!("C{}", i + 1),
            Slot::Fader(p, i) => format!("{}{}", &PAGE_NAMES[p][..1], i + 1),
        }
    }

    pub fn all(pages: usize) -> Vec<Slot> {
        let mut v: Vec<Slot> = (0..PLAYBACKS).map(Slot::Scene).collect();
        v.extend((0..PLAYBACKS).map(Slot::Chase));
        for p in 0..pages.min(SCENE_PAGES) {
            v.extend((0..STRIPS).map(|i| Slot::Fader(p, i)));
        }
        v
    }
}

/// Everything saved in a show file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Show {
    pub patch: Vec<Option<Patched>>,
    pub my_fixtures: Vec<FixtureDef>,
    pub scenes: Vec<Option<Item>>,
    pub chases: Vec<Option<Item>>,
    pub faders: Vec<Vec<Option<Item>>>,
    pub cuelist: Vec<Cue>,
    pub swatches: Vec<Rgb>,
}

impl Default for Show {
    fn default() -> Self {
        let mut s = Show {
            patch: Vec::new(),
            my_fixtures: Vec::new(),
            scenes: Vec::new(),
            chases: Vec::new(),
            faders: Vec::new(),
            cuelist: Vec::new(),
            swatches: Vec::new(),
        };
        s.normalise();
        s
    }
}

impl Show {
    /// Pads or trims every list to the console's size (older or hand-edited files).
    pub fn normalise(&mut self) {
        self.patch.resize(CHANNELS, None);
        self.scenes.resize(PLAYBACKS, None);
        self.chases.resize(PLAYBACKS, None);
        self.faders.resize(SCENE_PAGES, Vec::new());
        for page in &mut self.faders {
            page.resize(STRIPS, None);
        }
        // Older shows had one page of swatches: keep theirs, add the new pages.
        if self.swatches.len() != effects::SWATCHES {
            let n = self.swatches.len().min(effects::SWATCHES);
            self.swatches.truncate(n);
            self.swatches
                .extend_from_slice(&effects::DEFAULT_SWATCHES[n..]);
        }
    }

    pub fn item(&self, slot: Slot) -> Option<&Item> {
        match slot {
            Slot::Scene(i) => self.scenes.get(i)?.as_ref(),
            Slot::Chase(i) => self.chases.get(i)?.as_ref(),
            Slot::Fader(p, i) => self.faders.get(p)?.get(i)?.as_ref(),
        }
    }

    pub fn item_mut(&mut self, slot: Slot) -> Option<&mut Item> {
        self.slot_mut(slot)?.as_mut()
    }

    fn slot_mut(&mut self, slot: Slot) -> Option<&mut Option<Item>> {
        match slot {
            Slot::Scene(i) => self.scenes.get_mut(i),
            Slot::Chase(i) => self.chases.get_mut(i),
            Slot::Fader(p, i) => self.faders.get_mut(p)?.get_mut(i),
        }
    }

    pub fn set_item(&mut self, slot: Slot, item: Option<Item>) {
        if let Some(s) = self.slot_mut(slot) {
            *s = item;
        }
    }

    pub fn patched(&self, ch: usize) -> Option<&Patched> {
        self.patch.get(ch)?.as_ref()
    }
}

/// Defaults and behaviour from Setup → Preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Fade time for new cues; None follows the Fade Time dial.
    pub cue_fade: Option<f32>,
    pub cue_start: CueStart,
    pub cue_follow: f32,
    pub chase_bpm: f32,
    pub chase_direction: Direction,
    pub chase_crossfade: Crossfade,
    /// Scene pages available to the faders (1-6).
    pub scene_pages: usize,
    /// Slow start for LEDs that turn on too quickly.
    pub led_curve: bool,
    pub white_blend: f32,
    pub amber_blend: f32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            cue_fade: None,
            cue_start: CueStart::Halt,
            cue_follow: 0.0,
            chase_bpm: 120.0,
            chase_direction: Direction::Forward,
            chase_crossfade: Crossfade::Snap,
            scene_pages: 4,
            led_curve: false,
            white_blend: 0.0,
            amber_blend: 0.0,
        }
    }
}

// ---- live state -----------------------------------------------------------------------------

/// The mixed result for one fixture channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelOut {
    /// Intensity before the Grand Master.
    pub level: f32,
    /// Intensity sent to the fixture: after Grand Master, DBO and the LED curve.
    pub intensity: f32,
    pub hs: Hs,
    pub rgb: Rgb,
    pub alt: [Option<f32>; 2],
    pub pos: [Option<f32>; 2],
}

impl Default for ChannelOut {
    fn default() -> Self {
        Self {
            level: 0.0,
            intensity: 0.0,
            hs: WHITE,
            rgb: Rgb::WHITE,
            alt: [None, None],
            pos: [None, None],
        }
    }
}

impl ChannelOut {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let num = |x: Option<f32>, y: Option<f32>| match (x, y) {
            (Some(x), Some(y)) => Some(lerp(x, y, t)),
            (x, y) => y.or(x),
        };
        Self {
            level: lerp(a.level, b.level, t),
            intensity: lerp(a.intensity, b.intensity, t),
            hs: a.hs.lerp(b.hs, t),
            rgb: a.rgb.lerp(b.rgb, t),
            alt: [num(a.alt[0], b.alt[0]), num(a.alt[1], b.alt[1])],
            pos: [num(a.pos[0], b.pos[0]), num(a.pos[1], b.pos[1])],
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Release {
    start: f64,
    fade: f32,
    from: f32,
}

#[derive(Clone, Debug, Default)]
struct ChaseClock {
    step: usize,
    next: usize,
    step_start: f64,
    backwards: bool,
}

/// A playing scene, chase or cuelist.
#[derive(Clone, Debug)]
pub struct Play {
    pub stamp: u64,
    start: f64,
    fade: f32,
    release: Option<Release>,
    clock: ChaseClock,
}

impl Play {
    fn new(stamp: u64, t: f64, fade: f32, chase: Option<&Chase>, rng: &mut Rng) -> Self {
        let mut clock = ChaseClock {
            step_start: t,
            ..Default::default()
        };
        if let Some(c) = chase {
            let n = c.steps.len();
            if c.direction == Direction::Backward && n > 0 {
                clock.step = n - 1;
            }
            (clock.next, clock.backwards) = next_step(
                clock.step,
                n,
                c.direction,
                c.direction == Direction::Backward,
                rng,
            );
        }
        Self {
            stamp,
            start: t,
            fade,
            release: None,
            clock,
        }
    }

    fn fade_in(&self, t: f64) -> f32 {
        if self.fade <= 0.0 {
            1.0
        } else {
            ((t - self.start) / self.fade as f64).clamp(0.0, 1.0) as f32
        }
    }

    fn envelope(&self, t: f64) -> f32 {
        match self.release {
            None => self.fade_in(t),
            Some(r) => r.from * (1.0 - release_progress(r, t)),
        }
    }

    fn release(&mut self, t: f64, fade: f32) {
        if self.release.is_none() {
            self.release = Some(Release {
                start: t,
                fade,
                from: self.envelope(t),
            });
        }
    }

    fn finished(&self, t: f64) -> bool {
        self.release.is_some_and(|r| release_progress(r, t) >= 1.0)
    }

    fn advance(&mut self, chase: &Chase, rng: &mut Rng) {
        let n = chase.steps.len();
        if n == 0 {
            return;
        }
        self.clock.step = self.clock.next % n;
        (self.clock.next, self.clock.backwards) = next_step(
            self.clock.step,
            n,
            chase.direction,
            self.clock.backwards,
            rng,
        );
    }

    /// Steps a chase on the clock (or on beats for an audio chase).
    fn run(&mut self, chase: &Chase, t: f64, step_seconds: f64, beats: u32, rng: &mut Rng) {
        if chase.audio {
            for _ in 0..beats {
                self.advance(chase, rng);
                self.clock.step_start = t;
            }
            return;
        }
        if t - self.clock.step_start > step_seconds * 16.0 {
            self.clock.step_start = t; // After a long stall don't race to catch up.
        }
        while t - self.clock.step_start >= step_seconds {
            self.clock.step_start += step_seconds;
            self.advance(chase, rng);
        }
    }
}

fn release_progress(r: Release, t: f64) -> f32 {
    if r.fade <= 0.0 {
        1.0
    } else {
        ((t - r.start) / r.fade as f64).clamp(0.0, 1.0) as f32
    }
}

fn next_step(
    step: usize,
    n: usize,
    dir: Direction,
    backwards: bool,
    rng: &mut Rng,
) -> (usize, bool) {
    if n <= 1 {
        return (0, backwards);
    }
    match dir {
        Direction::Forward => ((step + 1) % n, false),
        Direction::Backward => ((step + n - 1) % n, true),
        Direction::Bounce => {
            if backwards {
                if step == 0 {
                    (1, false)
                } else {
                    (step - 1, true)
                }
            } else if step + 1 >= n {
                (n - 2, true)
            } else {
                (step + 1, false)
            }
        }
        Direction::Shuffle => {
            let mut x = rng.below(n - 1);
            if x >= step {
                x += 1;
            }
            (x, false)
        }
    }
}

fn step_seconds(bpm: f32, tap_bpm: Option<f32>, rate: f32) -> f64 {
    60.0 / (tap_bpm.unwrap_or(bpm) * rate).clamp(1.0, 2000.0) as f64
}

/// The look of a chase at time `t`, with crossfade, mirror and filter applied.
fn chase_look(chase: &Chase, clock: &ChaseClock, t: f64, step_seconds: f64) -> Look {
    let n = chase.steps.len();
    if n == 0 {
        return empty_look();
    }
    let f = chase.crossfade.fraction();
    let p = ((t - clock.step_start) / step_seconds).clamp(0.0, 1.0) as f32;
    let s = if f <= 0.0 {
        0.0
    } else {
        ((p - (1.0 - f)) / f).clamp(0.0, 1.0)
    };
    let mut look = lerp_look(
        &chase.steps[clock.step % n],
        &chase.steps[clock.next % n],
        s,
    );
    if chase.mirror {
        let chans = chase.channels();
        let m = chans.len();
        for j in m.div_ceil(2)..m {
            look[chans[j]] = look[chans[m - 1 - j]];
        }
    }
    if chase.intensity_only {
        for c in look.iter_mut() {
            c.colour = None;
            c.alt = [None, None];
            c.pos = [None, None];
        }
    }
    look
}

/// xorshift64 for shuffled chases.
#[derive(Clone, Debug)]
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

/// The fixture-channel controls: levels set by the faders, colour and features
/// set by the dials and the Colour window, the selection, and colour chases
/// applied from the Colour window.
#[derive(Clone, Debug, Default)]
pub struct Programmer {
    pub level: [f32; CHANNELS],
    pub colour: [Option<Hs>; CHANNELS],
    colour_stamp: [u64; CHANNELS],
    /// Dial features per channel: Shift+Hue, Shift+Sat, Position+Hue, Position+Sat.
    pub alt: [[Option<f32>; 4]; CHANNELS],
    alt_stamp: [[u64; 4]; CHANNELS],
    pub selected: [bool; CHANNELS],
    /// Colour was set on this selection, so the next select starts a new one.
    selection_used: bool,
    anchor: Option<usize>,
    pub chases: Vec<(Chase, Play)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaderMode {
    Channels,
    Scenes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialKind {
    Hue,
    Sat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flash {
    Channel(usize),
    Slot(Slot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Led {
    Off,
    On,
    Flash,
}

/// Ways to record into a playback location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordChoice {
    Scene,
    /// Copy the chase that's playing (the latest one).
    CopyChase,
    /// Build a chase that walks the look across its channels.
    AutoChase,
    /// Append the look as the next step.
    ChaseStep,
}

impl RecordChoice {
    pub fn name(self) -> &'static str {
        match self {
            RecordChoice::Scene => "Scene",
            RecordChoice::CopyChase => "Copy playing chase",
            RecordChoice::AutoChase => "Auto chase",
            RecordChoice::ChaseStep => "Manual (add step)",
        }
    }
}

/// Something contributing to the output, for the Monitor window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayRef {
    Slot(Slot),
    Cuelist,
    Programmer(usize),
}

#[derive(Clone, Debug, Default)]
pub struct Cuelist {
    pub current: Option<usize>,
    from: Option<Look>,
    start: f64,
    fade: f32,
    pub paused: Option<f64>,
    stamp: u64,
    release: Option<Release>,
    /// Started from idle: colour fades in over LTP like a scene.
    from_idle: bool,
}

#[derive(Clone, Debug)]
pub struct Autopilot {
    pub on: bool,
    pub playing: bool,
    pub index: usize,
    /// Seconds per chase.
    pub duration: f32,
    pub repeat: bool,
    pub audio: bool,
    pub bpm: f32,
    started: f64,
    chase: Option<Chase>,
    play: Option<Play>,
}

impl Default for Autopilot {
    fn default() -> Self {
        Self {
            on: false,
            playing: false,
            index: 0,
            duration: 30.0,
            repeat: false,
            audio: false,
            bpm: 120.0,
            started: 0.0,
            chase: None,
            play: None,
        }
    }
}

impl Autopilot {
    pub fn chase_name(&self) -> String {
        effects::chase_name(effects::all_chases()[self.index])
    }
}

pub struct Desk {
    pub show: Show,
    pub prefs: Prefs,
    pub prog: Programmer,

    pub mode: FaderMode,
    pub fixture_page: usize,
    pub scene_page: usize,
    /// Physical fader positions.
    pub faders: [f32; STRIPS],
    pub gated: [bool; STRIPS],
    flash: [Option<Flash>; STRIPS],
    /// Scene-mode fader levels per page.
    pub scene_levels: [[f32; STRIPS]; SCENE_PAGES],

    pub fixture_master: f32,
    pub scene_master: f32,
    pub chase_master: f32,
    pub cue_master: f32,
    pub grand_master: f32,
    pub dbo: bool,
    pub live_fade: f32,
    /// Chase rate dial: scales every chase's speed.
    pub rate_scale: f32,
    /// Tempo from the Sync button, overriding each chase's own BPM.
    pub tap_bpm: Option<f32>,
    taps: Vec<f64>,
    pub shift: bool,
    /// Position mode: the dials drive pan (Hue) and tilt (Saturation).
    pub position: bool,
    /// Double-tapped Shift: dials fan across the selection.
    pub fan: bool,
    pub record: bool,
    pub record_include: [bool; CHANNELS],

    snapshot: Option<[ChannelOut; CHANNELS]>,
    snapshot_release: Option<(f64, f32, [ChannelOut; CHANNELS])>,

    plays: HashMap<Slot, Play>,
    pub cue: Cuelist,
    pub autopilot: Autopilot,

    /// Live mix before the Grand Master, ignoring Snapshot (what Record captures).
    pub live: [ChannelOut; CHANNELS],
    /// What the fixtures get.
    pub out: [ChannelOut; CHANNELS],
    pub dmx: [u8; 512],

    stamp: u64,
    now: f64,
    pending_beats: u32,
    rng: Rng,
    undo: Vec<(Programmer, Show)>,
    /// Last message for the touchscreen status line.
    pub message: String,
}

impl Default for Desk {
    fn default() -> Self {
        Self::new(Show::default(), Prefs::default())
    }
}

impl Desk {
    pub fn new(mut show: Show, prefs: Prefs) -> Self {
        show.normalise();
        Self {
            show,
            prefs,
            prog: Programmer::default(),
            mode: FaderMode::Channels,
            fixture_page: 0,
            scene_page: 0,
            faders: [0.0; STRIPS],
            gated: [false; STRIPS],
            flash: [None; STRIPS],
            scene_levels: [[0.0; STRIPS]; SCENE_PAGES],
            fixture_master: 1.0,
            scene_master: 1.0,
            chase_master: 1.0,
            cue_master: 1.0,
            grand_master: 1.0,
            dbo: false,
            live_fade: DEFAULT_FADE,
            rate_scale: 1.0,
            tap_bpm: None,
            taps: Vec::new(),
            shift: false,
            position: false,
            fan: false,
            record: false,
            record_include: [false; CHANNELS],
            snapshot: None,
            snapshot_release: None,
            plays: HashMap::new(),
            cue: Cuelist::default(),
            autopilot: Autopilot::default(),
            live: [ChannelOut::default(); CHANNELS],
            out: [ChannelOut::default(); CHANNELS],
            dmx: [0; 512],
            stamp: 0,
            now: 0.0,
            pending_beats: 0,
            rng: Rng(0x2545_f491_4f6c_dd1d),
            undo: Vec::new(),
            message: String::new(),
        }
    }

    fn next_stamp(&mut self) -> u64 {
        self.stamp += 1;
        self.stamp
    }

    // ---- patch info -----------------------------------------------------------------------

    pub fn colour_kind(&self, ch: usize) -> ColourKind {
        self.show
            .patched(ch)
            .map_or(ColourKind::Mixing, |p| p.def.colour_kind())
    }

    pub fn alt_home(&self, ch: usize, alt: usize) -> f32 {
        self.show.patched(ch).map_or(0.0, |p| p.def.alt_home(alt))
    }

    pub fn pos_home(&self, ch: usize, i: usize) -> f32 {
        self.show.patched(ch).map_or(0.5, |p| p.def.pos_home(i))
    }

    /// Home value of dial feature `k` (0-1 Shift, 2-3 Position).
    fn feature_home(&self, ch: usize, k: usize) -> f32 {
        if k < 2 {
            self.alt_home(ch, k)
        } else {
            self.pos_home(ch, k - 2)
        }
    }

    /// Which dial feature a dial drives now: Position wins over Shift.
    fn dial_feature(&self, dial: DialKind) -> Option<usize> {
        let k = if dial == DialKind::Hue { 0 } else { 1 };
        if self.position {
            Some(2 + k)
        } else if self.shift {
            Some(k)
        } else {
            None
        }
    }

    pub fn patched_channels(&self) -> Vec<usize> {
        (0..CHANNELS)
            .filter(|&c| self.show.patched(c).is_some())
            .collect()
    }

    /// The fixture channel a strip controls on the current fixture page.
    pub fn strip_channel(&self, strip: usize) -> usize {
        self.fixture_page * STRIPS + strip
    }

    // ---- undo ----------------------------------------------------------------------------

    /// Saves the programmer and show so the next change can be undone.
    pub fn checkpoint(&mut self) {
        self.undo.push((self.prog.clone(), self.show.clone()));
        if self.undo.len() > UNDO_DEPTH {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some((prog, show)) = self.undo.pop() {
            self.prog = prog;
            self.show = show;
            self.regate();
            self.message = "Undone".into();
        }
    }

    // ---- selection and dials ---------------------------------------------------------------

    pub fn selected_channels(&self) -> Vec<usize> {
        (0..CHANNELS).filter(|&c| self.prog.selected[c]).collect()
    }

    /// A select button. `range` (hold one and press another) selects everything
    /// between; `double` (double press) starts a new selection.
    pub fn press_select(&mut self, ch: usize, range: bool, double: bool) {
        if self.record {
            self.record_include[ch] = !self.record_include[ch];
            return;
        }
        let p = &mut self.prog;
        if double {
            p.selected = [false; CHANNELS];
            p.selected[ch] = true;
            p.selection_used = false;
        } else if let (true, Some(a)) = (range, p.anchor) {
            for c in a.min(ch)..=a.max(ch) {
                p.selected[c] = true;
            }
            p.selection_used = false;
        } else if p.selection_used {
            p.selected = [false; CHANNELS];
            p.selected[ch] = true;
            p.selection_used = false;
        } else {
            p.selected[ch] = !p.selected[ch];
        }
        p.anchor = Some(ch);
    }

    pub fn clear_selection(&mut self) {
        self.prog.selected = [false; CHANNELS];
        self.prog.selection_used = false;
    }

    /// The channels a dial on `ch` changes, with fan weights.
    fn dial_targets(&self, ch: usize, fan: bool) -> Vec<(usize, f32)> {
        if !self.prog.selected[ch] {
            return vec![(ch, 1.0)];
        }
        let sel = self.selected_channels();
        let n = sel.len();
        if !fan || n < 2 {
            return sel.into_iter().map(|c| (c, 1.0)).collect();
        }
        // Fan: the ends stay put and the change grows towards the dial turned.
        let k = sel.iter().position(|&c| c == ch).unwrap_or(0);
        sel.iter()
            .enumerate()
            .map(|(j, &c)| {
                let w = if j <= k {
                    if k == 0 { 1.0 } else { j as f32 / k as f32 }
                } else if k == n - 1 {
                    1.0
                } else {
                    (n - 1 - j) as f32 / (n - 1 - k) as f32
                };
                (c, w)
            })
            .collect()
    }

    fn base_colour(&self, ch: usize) -> Hs {
        self.prog.colour[ch].unwrap_or(self.live[ch].hs)
    }

    fn base_alt(&self, ch: usize, k: usize) -> f32 {
        let live = &self.live[ch];
        let live = if k < 2 { live.alt[k] } else { live.pos[k - 2] };
        self.prog.alt[ch][k]
            .or(live)
            .unwrap_or_else(|| self.feature_home(ch, k))
    }

    fn set_colour(&mut self, ch: usize, c: Hs, stamp: u64) {
        self.prog.colour[ch] = Some(c);
        self.prog.colour_stamp[ch] = stamp;
    }

    fn set_alt(&mut self, ch: usize, a: usize, v: f32, stamp: u64) {
        self.prog.alt[ch][a] = Some(v.clamp(0.0, 1.0));
        self.prog.alt_stamp[ch][a] = stamp;
    }

    /// Turning a dial: `delta` is in turns (1.0 = full range). With Shift it
    /// drives the strip's alternate feature instead, in Position mode pan/tilt.
    pub fn turn_dial(&mut self, ch: usize, dial: DialKind, delta: f32) {
        let targets = self.dial_targets(ch, self.fan);
        let stamp = self.next_stamp();
        if let Some(a) = self.dial_feature(dial) {
            for &(c, w) in &targets {
                let v = self.base_alt(c, a) + delta * w;
                self.set_alt(c, a, v, stamp);
            }
        } else {
            for &(c, w) in &targets {
                let mut hs = self.base_colour(c);
                match dial {
                    DialKind::Hue if self.colour_kind(c) == ColourKind::Temperature => {
                        hs.h = (hs.h + delta * w).clamp(0.0, 1.0);
                    }
                    DialKind::Hue => hs.h = (hs.h + delta * w).rem_euclid(1.0),
                    DialKind::Sat => hs.s = (hs.s + delta * w).clamp(0.0, 1.0),
                }
                self.set_colour(c, hs, stamp);
            }
            let chans: Vec<usize> = targets.iter().map(|t| t.0).collect();
            self.take_from_prog_chases(&chans);
        }
        if self.prog.selected[ch] {
            self.prog.selection_used = true;
        }
    }

    /// Pressing a dial: Hue steps through six colours at full saturation,
    /// Saturation (and Shift + either) goes full → 50% → 0%. In Position mode a
    /// press sends the fixture back to its home position.
    pub fn press_dial(&mut self, ch: usize, dial: DialKind) {
        self.checkpoint();
        let targets = self.dial_targets(ch, false);
        let stamp = self.next_stamp();
        if let Some(a) = self.dial_feature(dial) {
            let v = next_press_level(self.base_alt(ch, a));
            for &(c, _) in &targets {
                let v = if a >= 2 { self.feature_home(c, a) } else { v };
                self.set_alt(c, a, v, stamp);
            }
        } else {
            let cur = self.base_colour(ch);
            for &(c, _) in &targets {
                let mut hs = self.base_colour(c);
                match dial {
                    DialKind::Hue if self.colour_kind(c) == ColourKind::Temperature => {
                        hs.h = next_press_level(cur.h);
                    }
                    DialKind::Hue => {
                        hs.h = next_hue_step(cur.h);
                        hs.s = 1.0;
                    }
                    DialKind::Sat => hs.s = next_press_level(cur.s),
                }
                self.set_colour(c, hs, stamp);
            }
            let chans: Vec<usize> = targets.iter().map(|t| t.0).collect();
            self.take_from_prog_chases(&chans);
        }
        if self.prog.selected[ch] {
            self.prog.selection_used = true;
        }
    }

    /// Stops Colour-window chases on these channels.
    fn take_from_prog_chases(&mut self, chans: &[usize]) {
        for (chase, _) in &mut self.prog.chases {
            for step in &mut chase.steps {
                for &ch in chans {
                    step[ch] = Cell::default();
                }
            }
        }
        self.prog
            .chases
            .retain(|(c, _)| c.steps.iter().any(|s| s.iter().any(|x| !x.is_empty())));
    }

    // ---- Colour window -------------------------------------------------------------------

    pub fn apply_colour(&mut self, rgb: Rgb) -> bool {
        self.apply_colours(&[rgb])
    }

    pub fn apply_pattern(&mut self, p: Pattern) -> bool {
        let n = self.selected_channels().len();
        self.apply_colours(&effects::pattern_colours(p, n))
    }

    /// Colours the selection in order, repeating `colours` if it's shorter.
    fn apply_colours(&mut self, colours: &[Rgb]) -> bool {
        let sel = self.selected_channels();
        if sel.is_empty() || colours.is_empty() {
            self.message = "Select fixtures first".into();
            return false;
        }
        self.checkpoint();
        let stamp = self.next_stamp();
        for (j, &ch) in sel.iter().enumerate() {
            self.set_colour(ch, colours[j % colours.len()].hs(), stamp);
        }
        self.take_from_prog_chases(&sel);
        self.prog.selection_used = true;
        true
    }

    /// Sets the selection's colour live (picker and RGB sliders) without an undo step.
    pub fn drag_colour(&mut self, rgb: Rgb) {
        let sel = self.selected_channels();
        let stamp = self.next_stamp();
        for &ch in &sel {
            self.set_colour(ch, rgb.hs(), stamp);
        }
        self.take_from_prog_chases(&sel);
        if !sel.is_empty() {
            self.prog.selection_used = true;
        }
    }

    pub fn apply_chase(&mut self, c: ColourChase) -> bool {
        let sel = self.selected_channels();
        if sel.is_empty() {
            self.message = "Select fixtures first".into();
            return false;
        }
        self.checkpoint();
        self.take_from_prog_chases(&sel);
        let chase = effects::build_chase(c, &sel);
        let stamp = self.next_stamp();
        let play = Play::new(stamp, self.now, self.live_fade, Some(&chase), &mut self.rng);
        self.prog.chases.push((chase, play));
        self.prog.selection_used = true;
        true
    }

    // ---- faders and flash buttons ----------------------------------------------------------

    /// The level a strip's fader controls in the current mode and page.
    pub fn fader_target(&self, strip: usize) -> f32 {
        match self.mode {
            FaderMode::Channels => self.prog.level[self.strip_channel(strip)],
            FaderMode::Scenes => self.scene_levels[self.scene_page][strip],
        }
    }

    /// Moves a fader. A gated fader takes control once it reaches or crosses
    /// the level it controls.
    pub fn move_fader(&mut self, strip: usize, pos: f32) {
        let pos = pos.clamp(0.0, 1.0);
        let old = self.faders[strip];
        self.faders[strip] = pos;
        if self.gated[strip] {
            let target = self.fader_target(strip);
            let crossed = (old - target).signum() != (pos - target).signum();
            if crossed || (pos - target).abs() <= GATE_WINDOW {
                self.gated[strip] = false;
            } else {
                return;
            }
        }
        match self.mode {
            FaderMode::Channels => {
                let ch = self.strip_channel(strip);
                self.prog.level[ch] = pos;
            }
            FaderMode::Scenes => self.scene_levels[self.scene_page][strip] = pos,
        }
    }

    fn regate(&mut self) {
        for s in 0..STRIPS {
            self.gated[s] = (self.faders[s] - self.fader_target(s)).abs() > GATE_WINDOW;
        }
    }

    /// The Fixtures / Scenes button.
    pub fn toggle_fader_mode(&mut self) {
        self.mode = match self.mode {
            FaderMode::Channels => FaderMode::Scenes,
            FaderMode::Scenes => FaderMode::Channels,
        };
        self.flash = [None; STRIPS];
        self.regate();
    }

    pub fn press_page(&mut self) {
        match self.mode {
            FaderMode::Channels => self.fixture_page = (self.fixture_page + 1) % FIXTURE_PAGES,
            FaderMode::Scenes => {
                self.scene_page =
                    (self.scene_page + 1) % self.prefs.scene_pages.clamp(1, SCENE_PAGES)
            }
        }
        self.regate();
    }

    pub fn flash_down(&mut self, strip: usize) {
        self.flash[strip] = Some(match self.mode {
            FaderMode::Channels => Flash::Channel(self.strip_channel(strip)),
            FaderMode::Scenes => Flash::Slot(Slot::Fader(self.scene_page, strip)),
        });
    }

    pub fn flash_up(&mut self, strip: usize) {
        self.flash[strip] = None;
    }

    pub fn flash_held(&self, strip: usize) -> bool {
        self.flash[strip].is_some()
    }

    fn slot_master(&self, slot: Slot) -> f32 {
        match slot {
            Slot::Scene(_) => self.scene_master,
            Slot::Chase(_) => self.chase_master,
            Slot::Fader(p, i) => {
                let flashed = self.flash.contains(&Some(Flash::Slot(slot)));
                if flashed {
                    1.0
                } else {
                    self.scene_levels[p][i]
                }
            }
        }
    }

    // ---- playbacks -------------------------------------------------------------------------

    /// A Scene or Chase button: play the item, or release it if it's playing.
    pub fn toggle_slot(&mut self, slot: Slot) {
        let Some(item) = self.show.item(slot) else {
            self.message = format!("{} is empty", slot.label());
            return;
        };
        let fade = match item {
            Item::Scene(s) => s.fade.unwrap_or(self.live_fade),
            Item::Chase(_) => self.live_fade,
        };
        let t = self.now;
        if let Some(p) = self.plays.get_mut(&slot)
            && p.release.is_none()
        {
            p.release(t, fade);
            return;
        }
        self.start_slot(slot, fade);
    }

    /// Plays an item again from the start (making it the latest for colour).
    pub fn replay_slot(&mut self, slot: Slot) {
        let fade = match self.show.item(slot) {
            Some(Item::Scene(s)) => s.fade.unwrap_or(self.live_fade),
            Some(Item::Chase(_)) => self.live_fade,
            None => return,
        };
        self.start_slot(slot, fade);
    }

    fn start_slot(&mut self, slot: Slot, fade: f32) {
        let stamp = self.next_stamp();
        let chase = self.show.item(slot).and_then(Item::as_chase);
        let play = Play::new(stamp, self.now, fade, chase, &mut self.rng);
        self.plays.insert(slot, play);
    }

    pub fn slot_playing(&self, slot: Slot) -> bool {
        self.plays.get(&slot).is_some_and(|p| !p.finished(self.now))
    }

    pub fn slot_led(&self, slot: Slot) -> Led {
        if self.slot_playing(slot) {
            Led::Flash
        } else if self.show.item(slot).is_some() {
            Led::On
        } else {
            Led::Off
        }
    }

    pub fn delete_slot(&mut self, slot: Slot) {
        if self.show.item(slot).is_none() {
            return;
        }
        self.checkpoint();
        self.show.set_item(slot, None);
        self.plays.remove(&slot);
        self.message = format!("Deleted {}", slot.label());
    }

    /// Fader slots play while their fader is up or their flash button is held.
    fn sync_fader_plays(&mut self) {
        for p in 0..SCENE_PAGES {
            for i in 0..STRIPS {
                let slot = Slot::Fader(p, i);
                let want = self.show.item(slot).is_some()
                    && (self.scene_levels[p][i] > 0.001
                        || self.flash.contains(&Some(Flash::Slot(slot))));
                let has = self.plays.contains_key(&slot);
                if want && !has {
                    self.start_slot(slot, 0.0);
                } else if !want && has {
                    self.plays.remove(&slot);
                }
            }
        }
        let t = self.now;
        let show = &self.show;
        self.plays
            .retain(|&s, p| show.item(s).is_some() && !p.finished(t));
        self.prog.chases.retain(|(_, p)| !p.finished(t));
    }

    /// Everything contributing to the output, newest first.
    pub fn playing(&self) -> Vec<(PlayRef, String)> {
        let mut v: Vec<(u64, PlayRef, String)> = self
            .plays
            .iter()
            .map(|(&slot, p)| {
                let name = self.show.item(slot).map_or("", Item::name);
                (
                    p.stamp,
                    PlayRef::Slot(slot),
                    format!("{} {name}", slot.label()),
                )
            })
            .collect();
        for (i, (c, p)) in self.prog.chases.iter().enumerate() {
            v.push((
                p.stamp,
                PlayRef::Programmer(i),
                format!("Colour {}", c.name),
            ));
        }
        if let Some(i) = self.cue.current {
            let name = self.show.cuelist.get(i).map_or("", |c| c.name.as_str());
            v.push((
                self.cue.stamp,
                PlayRef::Cuelist,
                format!("Cuelist {} {name}", i + 1),
            ));
        }
        v.sort_by_key(|x| std::cmp::Reverse(x.0));
        v.into_iter().map(|(_, r, s)| (r, s)).collect()
    }

    pub fn release(&mut self, r: PlayRef) {
        let (t, fade) = (self.now, self.live_fade);
        match r {
            PlayRef::Slot(s) => {
                if let Slot::Fader(p, i) = s {
                    // A fader slot plays while its fader is up: pull it down.
                    self.scene_levels[p][i] = 0.0;
                    self.regate();
                }
                if let Some(p) = self.plays.get_mut(&s) {
                    p.release(t, fade);
                }
            }
            PlayRef::Cuelist => self.cue_release(),
            PlayRef::Programmer(i) => {
                if let Some((_, p)) = self.prog.chases.get_mut(i) {
                    p.release(t, fade);
                }
            }
        }
    }

    pub fn release_all(&mut self) {
        for (r, _) in self.playing() {
            self.release(r);
        }
    }

    /// Single tap on the chase rate dial: stop every chase.
    pub fn stop_chases(&mut self) {
        let (t, fade) = (self.now, self.live_fade);
        let chases: Vec<Slot> = self
            .plays
            .keys()
            .copied()
            .filter(|&s| matches!(self.show.item(s), Some(Item::Chase(_))))
            .collect();
        for s in chases {
            self.release(PlayRef::Slot(s));
        }
        for (_, p) in &mut self.prog.chases {
            p.release(t, fade);
        }
    }

    /// The chase most recently started, from any playback or the Colour window.
    pub fn latest_chase(&self) -> Option<Chase> {
        let mut best: Option<(u64, &Chase)> = None;
        for (&slot, p) in &self.plays {
            if p.release.is_some() {
                continue;
            }
            if let Some(Item::Chase(c)) = self.show.item(slot)
                && best.is_none_or(|(s, _)| p.stamp > s)
            {
                best = Some((p.stamp, c));
            }
        }
        for (c, p) in &self.prog.chases {
            if p.release.is_none() && best.is_none_or(|(s, _)| p.stamp > s) {
                best = Some((p.stamp, c));
            }
        }
        best.map(|(_, c)| c.clone())
    }

    pub fn step_seconds_for(&self, chase: &Chase) -> f64 {
        step_seconds(chase.bpm, self.tap_bpm, self.rate_scale)
    }

    /// The Sync button: tap the beat to set every chase's tempo.
    pub fn press_sync(&mut self) {
        let t = self.now;
        if self.taps.last().is_some_and(|&last| t - last > 2.0) {
            self.taps.clear();
        }
        self.taps.push(t);
        if self.taps.len() > 4 {
            self.taps.remove(0);
        }
        if self.taps.len() >= 2 {
            let n = self.taps.len();
            let interval = (self.taps[n - 1] - self.taps[0]) / (n - 1) as f64;
            if interval > 0.0 {
                self.tap_bpm = Some((60.0 / interval).clamp(10.0, 600.0) as f32);
                self.rate_scale = 1.0;
            }
        }
        for p in self.plays.values_mut() {
            p.clock.step_start = t;
        }
        for (_, p) in &mut self.prog.chases {
            p.clock.step_start = t;
        }
    }

    pub fn turn_rate(&mut self, delta: f32) {
        self.rate_scale = (self.rate_scale * (1.0 + delta)).clamp(0.1, 10.0);
    }

    /// Double tap on the chase rate dial.
    pub fn reset_rate(&mut self) {
        self.rate_scale = 1.0;
        self.tap_bpm = None;
        self.taps.clear();
    }

    pub fn turn_fade(&mut self, delta: f32) {
        self.live_fade = (self.live_fade + delta).clamp(0.0, 600.0);
    }

    /// A beat from the microphone (or the keyboard).
    pub fn beat(&mut self) {
        self.pending_beats += 1;
    }

    // ---- cuelist ---------------------------------------------------------------------------

    fn cue_progress(&self, t: f64) -> f32 {
        let t = self.cue.paused.unwrap_or(t);
        if self.cue.fade <= 0.0 {
            1.0
        } else {
            ((t - self.cue.start) / self.cue.fade as f64).clamp(0.0, 1.0) as f32
        }
    }

    fn cue_look(&self, t: f64) -> Look {
        let Some(cue) = self.cue.current.and_then(|i| self.show.cuelist.get(i)) else {
            return empty_look();
        };
        match &self.cue.from {
            Some(from) => lerp_look(from, &cue.look, self.cue_progress(t)),
            None => cue.look,
        }
    }

    fn cue_go_to(&mut self, i: usize, fade: Option<f32>) {
        if i >= self.show.cuelist.len() {
            return;
        }
        let t = self.now;
        let idle = self.cue.current.is_none() || self.cue.release.is_some();
        let from = if idle { None } else { Some(self.cue_look(t)) };
        let fade = fade.unwrap_or_else(|| self.show.cuelist[i].fade.unwrap_or(self.live_fade));
        self.cue = Cuelist {
            current: Some(i),
            // From idle, intensity fades up from zero and colour fades in by LTP.
            from: Some(from.unwrap_or_else(|| {
                let mut l = self.show.cuelist[i].look;
                for c in l.iter_mut() {
                    c.level = c.level.map(|_| 0.0);
                }
                l
            })),
            start: t,
            fade,
            paused: None,
            stamp: self.next_stamp(),
            release: None,
            from_idle: idle,
        };
    }

    /// Play (Go): the next cue.
    pub fn cue_go(&mut self) {
        if self.show.cuelist.is_empty() {
            self.message = "The cuelist is empty".into();
            return;
        }
        if self.cue.paused.is_some() {
            self.cue_pause();
            return;
        }
        let next = match self.cue.current {
            Some(i) if self.cue.release.is_none() => (i + 1) % self.show.cuelist.len(),
            _ => 0,
        };
        self.cue_go_to(next, None);
    }

    pub fn cue_back(&mut self) {
        if let Some(i) = self.cue.current
            && i > 0
        {
            self.cue_go_to(i - 1, None);
        }
    }

    /// Next or previous cue with no fade (<< and >> in the Cuelist window).
    pub fn cue_skip(&mut self, forward: bool) {
        let n = self.show.cuelist.len();
        if n == 0 {
            return;
        }
        let i = match (self.cue.current, forward) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(n - 1),
            (Some(i), false) => i.saturating_sub(1),
        };
        self.cue_go_to(i, Some(0.0));
    }

    /// Crossfades to a cue in its own fade time.
    pub fn cue_jump(&mut self, i: usize) {
        self.cue_go_to(i, None);
    }

    pub fn cue_pause(&mut self) {
        if self.cue.current.is_none() {
            return;
        }
        match self.cue.paused.take() {
            Some(at) => self.cue.start += self.now - at,
            None => self.cue.paused = Some(self.now),
        }
    }

    /// Release: stop sending anything and reset to cue 1.
    pub fn cue_release(&mut self) {
        if self.cue.current.is_some() && self.cue.release.is_none() {
            self.cue.paused = None;
            let from = 1.0;
            self.cue.release = Some(Release {
                start: self.now,
                fade: self.live_fade,
                from,
            });
        }
    }

    pub fn cue_playing(&self) -> bool {
        self.cue.current.is_some() && self.cue.release.is_none()
    }

    fn advance_cuelist(&mut self) {
        let t = self.now;
        if let Some(r) = self.cue.release
            && release_progress(r, t) >= 1.0
        {
            self.cue = Cuelist::default();
            return;
        }
        let Some(i) = self.cue.current else { return };
        if self.cue.paused.is_some() || self.cue.release.is_some() {
            return;
        }
        if let Some(next) = self.show.cuelist.get(i + 1)
            && next.start == CueStart::Follow
            && t >= self.cue.start + self.cue.fade as f64 + next.wait as f64
        {
            self.cue_go_to(i + 1, None);
        }
    }

    pub fn delete_cue(&mut self, i: usize) {
        if i >= self.show.cuelist.len() {
            return;
        }
        self.checkpoint();
        self.show.cuelist.remove(i);
        match self.cue.current {
            Some(c) if c == i => self.cue = Cuelist::default(),
            Some(c) if c > i => self.cue.current = Some(c - 1),
            _ => {}
        }
    }

    /// Update: the cue takes the current look of its channels (and any newly lit ones).
    pub fn update_cue(&mut self, i: usize) {
        if i >= self.show.cuelist.len() {
            return;
        }
        self.checkpoint();
        let old = self.show.cuelist[i].look;
        let include: [bool; CHANNELS] =
            std::array::from_fn(|ch| !old[ch].is_empty() || self.live[ch].level > 0.004);
        self.show.cuelist[i].look = self.capture(&include);
        self.message = format!("Updated cue {}", i + 1);
    }

    // ---- record ----------------------------------------------------------------------------

    /// Record: channels above 0% are included; select buttons toggle them.
    pub fn press_record(&mut self) {
        if self.record {
            self.record = false;
            return;
        }
        self.record = true;
        self.record_include = std::array::from_fn(|ch| self.live[ch].level > 0.004);
    }

    fn capture(&self, include: &[bool; CHANNELS]) -> Look {
        std::array::from_fn(|ch| {
            if include[ch] {
                let o = &self.live[ch];
                Cell {
                    level: Some(o.level),
                    colour: Some(o.hs),
                    alt: o.alt,
                    pos: o.pos,
                }
            } else {
                Cell::default()
            }
        })
    }

    /// What Record can do with a location.
    pub fn record_options(&self, slot: Slot) -> Vec<RecordChoice> {
        let mut v = match slot {
            Slot::Chase(_) => vec![RecordChoice::AutoChase, RecordChoice::ChaseStep],
            _ => vec![RecordChoice::Scene],
        };
        if self.latest_chase().is_some() {
            v.push(RecordChoice::CopyChase);
        }
        v
    }

    fn count_items(&self, chase: bool) -> usize {
        Slot::all(SCENE_PAGES)
            .into_iter()
            .filter_map(|s| self.show.item(s))
            .filter(|i| matches!(i, Item::Chase(_)) == chase)
            .count()
    }

    fn new_chase(&self, steps: Vec<Look>) -> Chase {
        Chase {
            name: format!("Chase {}", self.count_items(true) + 1),
            steps,
            bpm: self.prefs.chase_bpm,
            direction: self.prefs.chase_direction,
            mirror: false,
            crossfade: self.prefs.chase_crossfade,
            intensity_only: false,
            audio: false,
        }
    }

    pub fn record_to(&mut self, slot: Slot, choice: RecordChoice) {
        let look = self.capture(&self.record_include);
        let included: Vec<usize> = (0..CHANNELS).filter(|&c| self.record_include[c]).collect();
        if included.is_empty() && choice != RecordChoice::CopyChase {
            self.message = "Nothing to record: no channels are included".into();
            self.record = false;
            return;
        }
        self.checkpoint();
        let item = match choice {
            RecordChoice::Scene => Item::Scene(Scene {
                name: format!("Scene {}", self.count_items(false) + 1),
                look,
                fade: None,
            }),
            RecordChoice::CopyChase => match self.latest_chase() {
                Some(c) => Item::Chase(c),
                None => return,
            },
            RecordChoice::AutoChase => {
                // Each step shifts the look one channel along the included channels.
                let n = included.len();
                let steps = (0..n)
                    .map(|k| {
                        let mut step = empty_look();
                        for (j, &ch) in included.iter().enumerate() {
                            step[ch] = look[included[(j + n - k) % n]];
                        }
                        step
                    })
                    .collect();
                Item::Chase(self.new_chase(steps))
            }
            RecordChoice::ChaseStep => match self.show.item(slot) {
                Some(Item::Chase(c)) => {
                    let mut c = c.clone();
                    c.steps.push(look);
                    Item::Chase(c)
                }
                _ => Item::Chase(self.new_chase(vec![look])),
            },
        };
        self.message = format!("Recorded {} to {}", item.name(), slot.label());
        self.show.set_item(slot, Some(item));
        // A playing item picks up the change; restart its clock if it's a chase.
        if let Some(p) = self.plays.get_mut(&slot) {
            p.clock = ChaseClock {
                step_start: self.now,
                next: 1,
                ..Default::default()
            };
        }
        self.record = false;
    }

    pub fn record_cue(&mut self) {
        if !self.record_include.contains(&true) {
            self.message = "Nothing to record: no channels are included".into();
            self.record = false;
            return;
        }
        self.checkpoint();
        let n = self.show.cuelist.len() + 1;
        self.show.cuelist.push(Cue {
            name: format!("Cue {n}"),
            look: self.capture(&self.record_include),
            start: self.prefs.cue_start,
            wait: self.prefs.cue_follow,
            fade: self.prefs.cue_fade,
        });
        self.message = format!("Recorded cue {n}");
        self.record = false;
    }

    // ---- clear, reset, snapshot -----------------------------------------------------------

    /// Clear pressed `presses` times in a row: 1 whites the selection, 2 whites
    /// everything, 3 clears all intensity and colour.
    pub fn press_clear(&mut self, presses: u32) {
        if self.shift {
            self.reset_panel();
            return;
        }
        let stamp = self.next_stamp();
        match presses {
            1 => {
                let sel = self.selected_channels();
                if sel.is_empty() {
                    return;
                }
                self.checkpoint();
                for &ch in &sel {
                    self.set_colour(ch, WHITE, stamp);
                }
                self.take_from_prog_chases(&sel);
            }
            2 => {
                let all: Vec<usize> = (0..CHANNELS).collect();
                for &ch in &all {
                    self.set_colour(ch, WHITE, stamp);
                }
                self.take_from_prog_chases(&all);
            }
            _ => {
                let selected = self.prog.selected;
                self.prog = Programmer {
                    selected,
                    ..Default::default()
                };
                self.regate();
                self.message = "Cleared all intensity and colour".into();
            }
        }
    }

    /// Shift + Clear: fade and rate back to default, release everything,
    /// un-gate the faders and return them to channel mode.
    pub fn reset_panel(&mut self) {
        self.live_fade = DEFAULT_FADE;
        self.reset_rate();
        self.release_all();
        self.mode = FaderMode::Channels;
        self.fixture_page = 0;
        self.gated = [false; STRIPS];
        self.record = false;
        self.fan = false;
        self.shift = false;
        self.message = "Panel reset".into();
    }

    /// Snapshot: freeze the output until pressed again, then crossfade to live.
    pub fn press_snapshot(&mut self) {
        match self.snapshot.take() {
            Some(frozen) => {
                self.snapshot_release = Some((self.now, self.live_fade, frozen));
            }
            None => {
                let current: [ChannelOut; CHANNELS] = match self.snapshot_release {
                    Some(_) => self.pre_master(),
                    None => self.live,
                };
                self.snapshot = Some(current);
                self.snapshot_release = None;
            }
        }
    }

    pub fn snapshot_active(&self) -> bool {
        self.snapshot.is_some()
    }

    /// Output before Grand Master, with Snapshot applied.
    fn pre_master(&self) -> [ChannelOut; CHANNELS] {
        if let Some(frozen) = self.snapshot {
            return frozen;
        }
        if let Some((start, fade, frozen)) = self.snapshot_release {
            let p = if fade <= 0.0 {
                1.0
            } else {
                ((self.now - start) / fade as f64).clamp(0.0, 1.0) as f32
            };
            return std::array::from_fn(|i| ChannelOut::lerp(&frozen[i], &self.live[i], p));
        }
        self.live
    }

    // ---- autopilot -------------------------------------------------------------------------

    pub fn autopilot_play_pause(&mut self) {
        let a = &mut self.autopilot;
        a.on = true;
        a.playing = !a.playing;
        if a.playing && a.chase.is_none() {
            self.autopilot_load(self.autopilot.index);
        }
    }

    pub fn autopilot_step(&mut self, forward: bool) {
        let n = effects::all_chases().len();
        let i = if forward {
            (self.autopilot.index + 1) % n
        } else {
            (self.autopilot.index + n - 1) % n
        };
        self.autopilot_load(i);
    }

    fn autopilot_load(&mut self, i: usize) {
        let all = effects::all_chases();
        let i = i % all.len();
        let chase = effects::build_chase(all[i], &self.patched_channels());
        let stamp = self.next_stamp();
        let play = Play::new(stamp, self.now, 0.0, Some(&chase), &mut self.rng);
        let a = &mut self.autopilot;
        a.index = i;
        a.started = self.now;
        a.chase = Some(chase);
        a.play = Some(play);
    }

    pub fn autopilot_off(&mut self) {
        self.autopilot.on = false;
        self.autopilot.playing = false;
    }

    fn advance_autopilot(&mut self, beats: u32) {
        let t = self.now;
        if !self.autopilot.on || !self.autopilot.playing {
            return;
        }
        if self.autopilot.chase.is_none()
            || (!self.autopilot.repeat
                && t - self.autopilot.started >= self.autopilot.duration as f64)
        {
            let next = if self.autopilot.chase.is_none() {
                self.autopilot.index
            } else {
                self.autopilot.index + 1
            };
            self.autopilot_load(next);
        }
        let a = &mut self.autopilot;
        if let (Some(chase), Some(play)) = (&a.chase, &mut a.play) {
            let mut chase = chase.clone();
            chase.audio = a.audio;
            play.run(
                &chase,
                t,
                60.0 / a.bpm.max(1.0) as f64,
                beats,
                &mut self.rng,
            );
        }
    }

    // ---- the mix ---------------------------------------------------------------------------

    /// Runs the console for one frame and renders the DMX universe.
    pub fn tick(&mut self, t: f64) -> &[u8; 512] {
        self.now = t;
        let beats = std::mem::take(&mut self.pending_beats);
        self.sync_fader_plays();

        // Step the chases.
        let (tap, rate) = (self.tap_bpm, self.rate_scale);
        for (&slot, play) in self.plays.iter_mut() {
            if let Some(Item::Chase(c)) = self.show.item(slot) {
                play.run(c, t, step_seconds(c.bpm, tap, rate), beats, &mut self.rng);
            }
        }
        for (c, play) in self.prog.chases.iter_mut() {
            play.run(c, t, step_seconds(c.bpm, tap, rate), beats, &mut self.rng);
        }
        self.advance_cuelist();
        self.advance_autopilot(beats);

        self.live = self.mix(t);
        let pre = if self.autopilot.on {
            self.autopilot_mix(t)
        } else {
            self.pre_master()
        };
        if let Some((start, fade, _)) = self.snapshot_release
            && t - start >= fade as f64
        {
            self.snapshot_release = None;
        }

        let gm = if self.dbo { 0.0 } else { self.grand_master };
        for (ch, mut o) in pre.into_iter().enumerate() {
            let i = (o.level * gm).clamp(0.0, 1.0);
            o.intensity = if self.prefs.led_curve { i * i } else { i };
            self.out[ch] = o;
        }

        self.dmx = [0; 512];
        let blend = Blend {
            white: self.prefs.white_blend,
            amber: self.prefs.amber_blend,
        };
        for (ch, p) in self.show.patch.iter().enumerate() {
            if let Some(p) = p {
                render(p, &self.out[ch], blend, &mut self.dmx);
            }
        }
        &self.dmx
    }

    fn autopilot_mix(&self, t: f64) -> [ChannelOut; CHANNELS] {
        let a = &self.autopilot;
        let look = match (&a.chase, &a.play) {
            (Some(c), Some(p)) => chase_look(c, &p.clock, t, 60.0 / a.bpm.max(1.0) as f64),
            _ => empty_look(),
        };
        std::array::from_fn(|ch| {
            let hs = look[ch].colour.unwrap_or(WHITE);
            ChannelOut {
                // Intensity chases bring their own levels; colour chases run at full.
                level: if self.show.patched(ch).is_some() {
                    look[ch].level.unwrap_or(1.0)
                } else {
                    0.0
                },
                hs,
                rgb: hs.rgb(),
                ..Default::default()
            }
        })
    }

    /// HTP intensity, LTP colour and features.
    fn mix(&self, t: f64) -> [ChannelOut; CHANNELS] {
        struct Source {
            stamp: u64,
            /// Fade-in progress, for the LTP colour crossfade.
            fade: f32,
            scale: f32,
            look: Look,
        }
        let mut sources: Vec<Source> = Vec::new();
        for (&slot, play) in &self.plays {
            let Some(item) = self.show.item(slot) else {
                continue;
            };
            let look = match item {
                Item::Scene(s) => s.look,
                Item::Chase(c) => chase_look(c, &play.clock, t, self.step_seconds_for(c)),
            };
            sources.push(Source {
                stamp: play.stamp,
                fade: play.fade_in(t),
                scale: play.envelope(t) * self.slot_master(slot),
                look,
            });
        }
        for (c, play) in &self.prog.chases {
            sources.push(Source {
                stamp: play.stamp,
                fade: play.fade_in(t),
                scale: play.envelope(t),
                look: chase_look(c, &play.clock, t, self.step_seconds_for(c)),
            });
        }
        if self.cue.current.is_some() {
            let env = self
                .cue
                .release
                .map_or(1.0, |r| 1.0 - release_progress(r, t));
            sources.push(Source {
                stamp: self.cue.stamp,
                fade: if self.cue.from_idle {
                    self.cue_progress(t)
                } else {
                    1.0
                },
                scale: env * self.cue_master,
                look: self.cue_look(t),
            });
        }

        std::array::from_fn(|ch| {
            let flashed = self.flash.contains(&Some(Flash::Channel(ch)));
            let mut level = if flashed { 1.0 } else { self.prog.level[ch] } * self.fixture_master;
            let mut colours: Vec<(u64, f32, Hs)> = Vec::new();
            let mut alts: [Vec<(u64, f32, f32)>; 4] = Default::default();
            if let Some(c) = self.prog.colour[ch] {
                colours.push((self.prog.colour_stamp[ch], 1.0, c));
            }
            for (a, list) in alts.iter_mut().enumerate() {
                if let Some(v) = self.prog.alt[ch][a] {
                    list.push((self.prog.alt_stamp[ch][a], 1.0, v));
                }
            }
            for s in &sources {
                let cell = &s.look[ch];
                if let Some(l) = cell.level {
                    level = level.max(l * s.scale);
                }
                if let Some(c) = cell.colour {
                    colours.push((s.stamp, s.fade, c));
                }
                for (list, v) in alts.iter_mut().zip(cell.features()) {
                    if let Some(v) = v {
                        list.push((s.stamp, s.fade, v));
                    }
                }
            }
            colours.sort_by_key(|x| std::cmp::Reverse(x.0));
            let (hs, rgb) = match colours.as_slice() {
                [] => (WHITE, Rgb::WHITE),
                [top, rest @ ..] => {
                    let base = rest.first().map_or(WHITE, |c| c.2);
                    (base.lerp(top.2, top.1), base.rgb().lerp(top.2.rgb(), top.1))
                }
            };
            let feature: [Option<f32>; 4] = std::array::from_fn(|a| {
                alts[a].sort_by_key(|x| std::cmp::Reverse(x.0));
                match alts[a].as_slice() {
                    [] => None,
                    [top, rest @ ..] => {
                        let base = rest.first().map_or(top.2, |x| x.2);
                        Some(lerp(base, top.2, top.1))
                    }
                }
            });
            ChannelOut {
                level: level.clamp(0.0, 1.0),
                intensity: 0.0,
                hs,
                rgb,
                alt: [feature[0], feature[1]],
                pos: [feature[2], feature[3]],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{Patched, generic};

    const BLUE: Hs = Hs {
        h: 2.0 / 3.0,
        s: 1.0,
    };
    const RED: Hs = Hs { h: 0.0, s: 1.0 };

    fn desk() -> Desk {
        let mut show = Show::default();
        for ch in 0..CHANNELS {
            show.patch[ch] = Some(Patched {
                def: generic("RGB"),
                addresses: vec![ch as u16 * 3 + 1],
            });
        }
        let mut d = Desk::new(show, Prefs::default());
        d.tick(0.0);
        d
    }

    fn select(d: &mut Desk, chans: &[usize]) {
        d.clear_selection();
        for &c in chans {
            d.press_select(c, false, false);
        }
    }

    #[test]
    fn faders_and_colour_reach_dmx() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        select(&mut d, &[0]);
        d.press_dial(0, DialKind::Hue); // red → yellow
        d.press_dial(0, DialKind::Hue); // → green
        d.tick(0.1);
        assert_eq!(&d.dmx[0..3], &[0, 255, 0]);
        d.grand_master = 0.5;
        d.tick(0.2);
        assert_eq!(&d.dmx[0..3], &[0, 128, 0]);
        d.dbo = true;
        d.tick(0.3);
        assert_eq!(&d.dmx[0..3], &[0, 0, 0]);
    }

    #[test]
    fn dial_on_a_selected_channel_moves_the_selection() {
        let mut d = desk();
        select(&mut d, &[1, 2, 3]);
        d.turn_dial(2, DialKind::Sat, 1.0);
        d.turn_dial(2, DialKind::Hue, 2.0 / 3.0);
        for ch in [1, 2, 3] {
            let c = d.prog.colour[ch].unwrap();
            assert!((c.h - BLUE.h).abs() < 1e-4 && c.s == 1.0);
        }
        assert_eq!(d.prog.colour[0], None);
        // Having set colour, the next select starts a new selection.
        d.press_select(5, false, false);
        assert_eq!(d.selected_channels(), vec![5]);
    }

    #[test]
    fn fan_spreads_from_the_far_end() {
        let mut d = desk();
        select(&mut d, &[0, 1, 2, 3, 4]);
        d.fan = true;
        d.turn_dial(0, DialKind::Sat, 1.0);
        let sats: Vec<f32> = (0..5).map(|c| d.prog.colour[c].unwrap().s).collect();
        assert_eq!(sats, vec![1.0, 0.75, 0.5, 0.25, 0.0]);
        // From the middle the ends stay put.
        select(&mut d, &[0, 1, 2, 3, 4]);
        for c in 0..5 {
            d.prog.colour[c] = Some(WHITE);
        }
        d.turn_dial(2, DialKind::Sat, 1.0);
        let sats: Vec<f32> = (0..5).map(|c| d.prog.colour[c].unwrap().s).collect();
        assert_eq!(sats, vec![0.0, 0.5, 1.0, 0.5, 0.0]);
    }

    #[test]
    fn select_range_and_double_press() {
        let mut d = desk();
        d.press_select(2, false, false);
        d.press_select(6, true, false);
        assert_eq!(d.selected_channels(), vec![2, 3, 4, 5, 6]);
        d.press_select(9, false, true);
        assert_eq!(d.selected_channels(), vec![9]);
    }

    #[test]
    fn fader_gating() {
        let mut d = desk();
        d.move_fader(0, 0.5);
        assert_eq!(d.prog.level[0], 0.5);
        d.press_clear(3); // clears intensity: fader at 50% is now gated
        assert!(d.gated[0]);
        d.move_fader(0, 0.6);
        assert_eq!(d.prog.level[0], 0.0);
        d.move_fader(0, 0.01); // comes down to the output level and takes control
        assert!(!d.gated[0]);
        d.move_fader(0, 0.3);
        assert_eq!(d.prog.level[0], 0.3);
    }

    #[test]
    fn pages_switch_strips_and_gate() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        d.press_page();
        assert_eq!(d.strip_channel(0), 12);
        assert!(d.gated[0]);
        d.move_fader(0, 0.5);
        assert_eq!(d.prog.level[12], 0.0);
        assert_eq!(d.prog.level[0], 1.0);
    }

    fn record_scene(d: &mut Desk, slot: Slot) {
        d.press_record();
        d.record_to(slot, RecordChoice::Scene);
    }

    #[test]
    fn record_and_play_scene_with_fade() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        select(&mut d, &[0]);
        d.press_dial(0, DialKind::Sat);
        d.prog.colour[0] = Some(BLUE);
        d.tick(0.1);
        record_scene(&mut d, Slot::Scene(0));
        assert!(matches!(d.show.item(Slot::Scene(0)), Some(Item::Scene(_))));
        d.press_clear(3);
        d.tick(0.2);
        assert_eq!(&d.dmx[0..3], &[0, 0, 0]);

        d.toggle_slot(Slot::Scene(0)); // 2 s fade
        d.tick(1.2);
        assert_eq!(d.dmx[2], 128);
        d.tick(2.2);
        assert_eq!(&d.dmx[0..3], &[0, 0, 255]);
        d.scene_master = 0.5;
        d.tick(2.3);
        assert_eq!(d.dmx[2], 128);
        d.toggle_slot(Slot::Scene(0)); // release
        d.tick(5.0);
        assert_eq!(d.dmx[2], 0);
        assert!(!d.slot_playing(Slot::Scene(0)));
    }

    #[test]
    fn colour_is_latest_takes_precedence() {
        let mut d = desk();
        let scene = |c: Hs| {
            let mut look = empty_look();
            look[0] = Cell {
                level: Some(1.0),
                colour: Some(c),
                ..Default::default()
            };
            Some(Item::Scene(Scene {
                name: "x".into(),
                look,
                fade: Some(0.0),
            }))
        };
        d.show.scenes[0] = scene(RED);
        d.show.scenes[1] = scene(BLUE);
        d.toggle_slot(Slot::Scene(0));
        d.tick(0.1);
        d.toggle_slot(Slot::Scene(1));
        d.tick(0.2);
        assert_eq!(&d.dmx[0..3], &[0, 0, 255]); // blue, not purple
        d.toggle_slot(Slot::Scene(0)); // replaying red makes it the latest
        d.toggle_slot(Slot::Scene(0));
        d.tick(0.3);
        assert_eq!(&d.dmx[0..3], &[255, 0, 0]);
        // A dial on the channel overrides both.
        d.turn_dial(0, DialKind::Sat, -1.0);
        d.tick(0.4);
        assert_eq!(&d.dmx[0..3], &[255, 255, 255]);
    }

    #[test]
    fn intensity_is_highest_takes_precedence() {
        let mut d = desk();
        let mut look = empty_look();
        look[0].level = Some(0.4);
        d.show.scenes[0] = Some(Item::Scene(Scene {
            name: "x".into(),
            look,
            fade: Some(0.0),
        }));
        d.toggle_slot(Slot::Scene(0));
        d.move_fader(0, 0.2);
        d.tick(0.1);
        assert!((d.out[0].level - 0.4).abs() < 1e-6);
        d.move_fader(0, 0.9);
        d.tick(0.2);
        assert!((d.out[0].level - 0.9).abs() < 1e-6);
    }

    #[test]
    fn scene_mode_faders_and_flash() {
        let mut d = desk();
        d.move_fader(3, 1.0);
        d.tick(0.1);
        d.press_record();
        d.record_to(Slot::Fader(0, 5), RecordChoice::Scene);
        d.move_fader(3, 0.0);
        d.toggle_fader_mode();
        d.tick(0.2);
        assert_eq!(d.out[3].level, 0.0);
        d.move_fader(5, 0.5);
        d.tick(0.3);
        assert!((d.out[3].level - 0.5).abs() < 1e-6);
        d.move_fader(5, 0.0);
        d.flash_down(5);
        d.tick(0.4);
        assert_eq!(d.out[3].level, 1.0);
        d.flash_up(5);
        d.tick(0.5);
        assert_eq!(d.out[3].level, 0.0);
        // Other scene pages
        d.press_page();
        assert_eq!(d.scene_page, 1);
        d.prefs.scene_pages = 2;
        d.press_page();
        assert_eq!(d.scene_page, 0);
    }

    #[test]
    fn auto_chase_walks_the_look() {
        let mut d = desk();
        // Channel 0 lit, 1-3 at a low glow.
        d.move_fader(0, 1.0);
        for s in 1..4 {
            d.move_fader(s, 0.1);
        }
        d.tick(0.1);
        d.press_record();
        d.record_to(Slot::Chase(0), RecordChoice::AutoChase);
        let Some(Item::Chase(c)) = d.show.item(Slot::Chase(0)) else {
            panic!()
        };
        assert_eq!(c.steps.len(), 4);
        assert_eq!(c.steps[1][1].level, Some(1.0));
        assert_eq!(c.steps[1][0].level, Some(0.1));
        let mut c = c.clone();
        c.bpm = 60.0;
        d.show.chases[0] = Some(Item::Chase(c));
        d.press_clear(3);
        d.live_fade = 0.0;
        d.toggle_slot(Slot::Chase(0));
        d.tick(0.2);
        assert_eq!(d.out[0].level, 1.0);
        d.tick(1.25);
        assert_eq!(d.out[1].level, 1.0);
        assert!((d.out[0].level - 0.1).abs() < 1e-6);
        d.chase_master = 0.0;
        d.tick(1.3);
        assert_eq!(d.out[1].level, 0.0);
    }

    #[test]
    fn chase_steps_manually_and_directions() {
        let mut d = desk();
        let mut rng = Rng(1);
        assert_eq!(
            next_step(2, 3, Direction::Bounce, false, &mut rng),
            (1, true)
        );
        assert_eq!(
            next_step(0, 3, Direction::Bounce, true, &mut rng),
            (1, false)
        );
        assert_eq!(
            next_step(0, 3, Direction::Backward, true, &mut rng),
            (2, true)
        );
        for _ in 0..20 {
            let (n, _) = next_step(1, 4, Direction::Shuffle, false, &mut rng);
            assert!(n != 1 && n < 4);
        }
        for s in 0..3 {
            d.press_clear(3);
            d.move_fader(s, 1.0);
            d.tick(0.1 * s as f64);
            d.press_record();
            d.record_to(Slot::Chase(1), RecordChoice::ChaseStep);
            d.move_fader(s, 0.0);
        }
        let Some(Item::Chase(c)) = d.show.item(Slot::Chase(1)) else {
            panic!()
        };
        assert_eq!(c.steps.len(), 3);
        assert_eq!(c.steps[2][2].level, Some(1.0));
    }

    #[test]
    fn audio_chase_steps_on_beats() {
        let mut d = desk();
        let steps = (0..2)
            .map(|k| {
                let mut l = empty_look();
                l[k].level = Some(1.0);
                l
            })
            .collect();
        let mut c = d.new_chase(steps);
        c.audio = true;
        d.show.chases[0] = Some(Item::Chase(c));
        d.live_fade = 0.0;
        d.toggle_slot(Slot::Chase(0));
        d.tick(0.1);
        d.tick(10.0);
        assert_eq!(d.out[0].level, 1.0);
        d.beat();
        d.tick(10.1);
        assert_eq!(d.out[1].level, 1.0);
        assert_eq!(d.out[0].level, 0.0);
    }

    #[test]
    fn tap_sync_and_rate() {
        let mut d = desk();
        for i in 0..4 {
            d.tick(i as f64 * 0.5);
            d.press_sync();
        }
        assert_eq!(d.tap_bpm, Some(120.0));
        let c = d.new_chase(vec![empty_look()]);
        assert!((d.step_seconds_for(&c) - 0.5).abs() < 1e-9);
        d.turn_rate(1.0);
        assert!((d.step_seconds_for(&c) - 0.25).abs() < 1e-9);
        d.reset_rate();
        assert_eq!(d.tap_bpm, None);
    }

    #[test]
    fn cuelist_go_back_follow_release() {
        let mut d = desk();
        d.live_fade = 0.0;
        for ch in 0..3 {
            d.press_clear(3);
            d.move_fader(ch, 0.0);
            d.prog.level[ch] = 1.0;
            d.tick(ch as f64);
            d.press_record();
            d.record_cue();
        }
        d.press_clear(3);
        d.show.cuelist[1].fade = Some(1.0);
        d.show.cuelist[2].start = CueStart::Follow;
        d.show.cuelist[2].wait = 1.0;
        d.tick(10.0);
        d.cue_go();
        d.tick(10.1);
        assert_eq!(d.out[0].level, 1.0);
        d.cue_go(); // cue 2 over 1 s
        d.tick(10.6);
        assert!((d.out[1].level - 0.5).abs() < 0.01);
        assert!((d.out[0].level - 0.5).abs() < 0.01);
        d.cue_pause();
        d.tick(20.0);
        assert!((d.out[1].level - 0.5).abs() < 0.01);
        d.cue_pause(); // resume
        d.tick(20.6);
        assert_eq!(d.out[1].level, 1.0);
        // Cue 3 follows 1 s after cue 2 completes.
        d.tick(21.0);
        assert_eq!(d.cue.current, Some(1));
        d.tick(21.6);
        assert_eq!(d.cue.current, Some(2));
        d.cue_back();
        assert_eq!(d.cue.current, Some(1));
        d.cue_release();
        d.tick(21.7);
        assert_eq!(d.cue.current, None);
        d.cue_go();
        assert_eq!(d.cue.current, Some(0));
    }

    #[test]
    fn colour_window_chase_runs_on_selection() {
        let mut d = desk();
        select(&mut d, &[0, 1]);
        d.live_fade = 0.0;
        assert!(d.apply_chase(ColourChase::Special(14))); // RGB, 60 bpm
        d.move_fader(0, 1.0);
        d.tick(0.1);
        assert_eq!(&d.dmx[0..3], &[255, 0, 0]);
        d.tick(1.1);
        assert_eq!(&d.dmx[0..3], &[0, 255, 0]);
        // Turning a dial on a channel takes it out of the chase.
        d.clear_selection();
        d.turn_dial(0, DialKind::Sat, -1.0);
        d.tick(2.1);
        assert_eq!(&d.dmx[0..3], &[255, 255, 255]);
        assert_eq!(d.prog.chases.len(), 1);
        // Recording now offers to copy the chase.
        d.press_record();
        assert!(
            d.record_options(Slot::Scene(2))
                .contains(&RecordChoice::CopyChase)
        );
        d.record_to(Slot::Scene(2), RecordChoice::CopyChase);
        assert!(matches!(d.show.item(Slot::Scene(2)), Some(Item::Chase(_))));
    }

    #[test]
    fn patterns_colour_the_selection_in_order() {
        let mut d = desk();
        select(&mut d, &[4, 5, 6, 7]);
        assert!(d.apply_pattern(Pattern::OddEven(0)));
        d.tick(0.1);
        assert_eq!(d.out[4].rgb.to_u8(), [255, 0, 0]);
        assert_eq!(d.out[5].rgb.to_u8(), [0, 0, 255]);
        d.clear_selection();
        assert!(!d.apply_pattern(Pattern::Special(0)));
    }

    #[test]
    fn clear_presses() {
        let mut d = desk();
        for c in 0..3 {
            d.prog.colour[c] = Some(BLUE);
        }
        select(&mut d, &[0]);
        d.press_clear(1);
        assert_eq!(d.prog.colour[0], Some(WHITE));
        assert_eq!(d.prog.colour[1], Some(BLUE));
        d.press_clear(2);
        assert_eq!(d.prog.colour[1], Some(WHITE));
        d.prog.level[1] = 1.0;
        d.press_clear(3);
        assert_eq!(d.prog.colour[1], None);
        assert_eq!(d.prog.level[1], 0.0);
    }

    #[test]
    fn undo_restores_programmer_and_show() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        d.tick(0.1);
        record_scene(&mut d, Slot::Scene(1));
        assert!(d.show.scenes[1].is_some());
        d.undo();
        assert!(d.show.scenes[1].is_none());
        select(&mut d, &[2]);
        d.apply_colour(Rgb::new(1.0, 0.0, 0.0));
        assert!(d.prog.colour[2].is_some());
        d.undo();
        assert!(d.prog.colour[2].is_none());
    }

    #[test]
    fn snapshot_holds_output() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        d.tick(0.1);
        d.press_snapshot();
        d.move_fader(0, 0.0);
        d.tick(0.2);
        assert_eq!(d.out[0].level, 1.0);
        d.grand_master = 0.5;
        d.tick(0.3);
        assert_eq!(d.out[0].intensity, 0.5);
        d.press_snapshot(); // crossfade to live over 2 s
        d.tick(1.3);
        assert!((d.out[0].level - 0.5).abs() < 0.01);
        d.tick(3.0);
        assert_eq!(d.out[0].level, 0.0);
    }

    #[test]
    fn shift_drives_alternate_features() {
        let mut show = Show::default();
        show.patch[0] = Some(Patched {
            def: generic("RGBSZ"),
            addresses: vec![1],
        });
        let mut d = Desk::new(show, Prefs::default());
        d.move_fader(0, 1.0);
        d.shift = true;
        d.press_dial(0, DialKind::Sat); // strobe 0 → full
        d.turn_dial(0, DialKind::Hue, 0.5); // zoom
        d.tick(0.1);
        assert_eq!(d.dmx[4], 255);
        assert_eq!(d.dmx[5], 128);
    }

    fn movers() -> Desk {
        let mut show = Show::default();
        for ch in 0..4 {
            show.patch[ch] = Some(Patched {
                def: generic("Moving head wash RGBW (9ch)"),
                addresses: vec![ch as u16 * 9 + 1],
            });
        }
        let mut d = Desk::new(show, Prefs::default());
        d.tick(0.0);
        d
    }

    #[test]
    fn position_mode_moves_pan_and_tilt() {
        let mut d = movers();
        d.move_fader(0, 1.0);
        d.tick(0.1);
        assert_eq!(&d.dmx[0..2], &[128, 128]); // parked centre
        d.position = true;
        d.turn_dial(0, DialKind::Hue, 0.25); // pan
        d.turn_dial(0, DialKind::Sat, -0.5); // tilt
        d.tick(0.2);
        assert_eq!(d.dmx[0], 192);
        assert_eq!(d.dmx[1], 0);
        // Colour still comes from the colour dials.
        d.position = false;
        d.turn_dial(0, DialKind::Sat, 1.0);
        d.tick(0.3);
        assert_eq!(&d.dmx[0..2], &[192, 0]);
        // Pressing a dial in Position mode re-homes it.
        d.position = true;
        d.press_dial(0, DialKind::Hue);
        d.tick(0.4);
        assert_eq!(d.dmx[0], 128);
    }

    #[test]
    fn position_fans_and_records() {
        let mut d = movers();
        select(&mut d, &[0, 1, 2, 3]);
        d.position = true;
        d.fan = true;
        for ch in 0..4 {
            d.prog.alt[ch][2] = Some(0.0);
        }
        d.turn_dial(0, DialKind::Hue, 0.9);
        let pans: Vec<f32> = (0..4).map(|c| d.prog.alt[c][2].unwrap()).collect();
        assert!((pans[0] - 0.9).abs() < 1e-6 && (pans[1] - 0.6).abs() < 1e-6);
        assert_eq!(pans[3], 0.0);
        d.fan = false;
        d.position = false;
        for s in 0..4 {
            d.move_fader(s, 1.0);
        }
        d.tick(0.1);
        record_scene(&mut d, Slot::Scene(0));
        let Some(Item::Scene(scene)) = d.show.item(Slot::Scene(0)) else {
            panic!()
        };
        assert!((scene.look[0].pos[0].unwrap() - 0.9).abs() < 1e-6);
        // Played back over a cleared programmer, the scene sets the position.
        d.press_clear(3);
        d.live_fade = 0.0;
        if let Some(Item::Scene(s)) = d.show.scenes[0].as_mut() {
            s.fade = Some(0.0);
        }
        d.toggle_slot(Slot::Scene(0));
        d.tick(0.2);
        assert!((229..=230).contains(&d.dmx[0]), "{}", d.dmx[0]);
    }

    #[test]
    fn old_show_files_without_position_still_load() {
        let json = r#"{"level":1.0,"colour":{"h":0.5,"s":1.0},"alt":[null,0.5]}"#;
        let c: Cell = serde_json::from_str(json).unwrap();
        assert_eq!(c.pos, [None, None]);
        assert_eq!(c.alt[1], Some(0.5));
        // And empty position isn't written out.
        assert!(!serde_json::to_string(&c).unwrap().contains("pos"));
    }

    #[test]
    fn temperature_fixture_hue_dial() {
        let mut show = Show::default();
        show.patch[0] = Some(Patched {
            def: generic("CW/WW"),
            addresses: vec![1],
        });
        let mut d = Desk::new(show, Prefs::default());
        d.move_fader(0, 1.0);
        d.turn_dial(0, DialKind::Hue, 2.0); // clamps at fully cool
        d.tick(0.1);
        assert_eq!(&d.dmx[0..2], &[255, 0]);
    }

    #[test]
    fn autopilot_lights_every_patched_fixture() {
        let mut d = desk();
        d.autopilot_play_pause();
        d.tick(0.1);
        assert!(d.out.iter().all(|o| o.level == 1.0));
        let first = d.autopilot.index;
        d.autopilot.duration = 1.0;
        d.tick(1.2);
        assert_eq!(d.autopilot.index, first + 1);
        d.autopilot_off();
        d.tick(1.3);
        assert!(d.out.iter().all(|o| o.level == 0.0));
    }

    #[test]
    fn monitor_lists_newest_first_and_releases() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        d.tick(0.1);
        record_scene(&mut d, Slot::Scene(0));
        record_scene(&mut d, Slot::Scene(1));
        d.toggle_slot(Slot::Scene(0));
        d.toggle_slot(Slot::Scene(1));
        let list = d.playing();
        assert_eq!(list[0].0, PlayRef::Slot(Slot::Scene(1)));
        assert_eq!(list.len(), 2);
        d.release_all();
        d.tick(10.0);
        assert!(d.playing().is_empty());
    }

    #[test]
    fn show_round_trips_through_json() {
        let mut d = desk();
        d.move_fader(0, 1.0);
        d.tick(0.1);
        record_scene(&mut d, Slot::Fader(2, 3));
        d.press_record();
        d.record_cue();
        let json = serde_json::to_string(&d.show).unwrap();
        let back: Show = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d.show);
    }
}
