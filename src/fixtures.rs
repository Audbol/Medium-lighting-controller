//! Fixture library and DMX rendering. A fixture type is a list of channels,
//! each with a feature (Intensity, Red, Strobe…), the dial that drives it and
//! its default/min/max values, like the Stage CL's fixture editor.
//!
//! The library has a Generic folder, hand-made profiles from manufacturers'
//! manuals, and a bundled set converted from the Open Fixture Library
//! (`assets/fixtures.json`, see `ofl.rs`).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::colour::{Rgb, white_temperature};
use crate::engine::ChannelOut;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feature {
    Intensity,
    Red,
    Green,
    Blue,
    Amber,
    White,
    CoolWhite,
    WarmWhite,
    ColourTemp,
    Strobe,
    Zoom,
    Cyan,
    Magenta,
    Yellow,
    /// Custom 1-4 or a name of your own; set by a dial or left at its default.
    Custom,
    /// Held at its default value; no control on the console.
    Other,
    /// Ultraviolet LEDs: a dial (Shift+Hue by default).
    Uv,
    /// Moving-head position: the Position dials.
    Pan,
    Tilt,
    /// Pan/tilt movement speed: held at its default.
    Speed,
}

impl Feature {
    pub const ALL: [Feature; 20] = [
        Feature::Intensity,
        Feature::Red,
        Feature::Green,
        Feature::Blue,
        Feature::Amber,
        Feature::White,
        Feature::CoolWhite,
        Feature::WarmWhite,
        Feature::ColourTemp,
        Feature::Uv,
        Feature::Strobe,
        Feature::Zoom,
        Feature::Pan,
        Feature::Tilt,
        Feature::Speed,
        Feature::Cyan,
        Feature::Magenta,
        Feature::Yellow,
        Feature::Custom,
        Feature::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Feature::Intensity => "Intensity",
            Feature::Red => "Red",
            Feature::Green => "Green",
            Feature::Blue => "Blue",
            Feature::Amber => "Amber",
            Feature::White => "White",
            Feature::CoolWhite => "Cool White",
            Feature::WarmWhite => "Warm White",
            Feature::ColourTemp => "Colour Temp",
            Feature::Strobe => "Strobe",
            Feature::Zoom => "Zoom",
            Feature::Cyan => "Cyan",
            Feature::Magenta => "Magenta",
            Feature::Yellow => "Yellow",
            Feature::Custom => "Custom",
            Feature::Other => "Other",
            Feature::Uv => "UV",
            Feature::Pan => "Pan",
            Feature::Tilt => "Tilt",
            Feature::Speed => "P/T Speed",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Feature::Intensity => "I",
            Feature::Red => "R",
            Feature::Green => "G",
            Feature::Blue => "B",
            Feature::Amber => "A",
            Feature::White => "W",
            Feature::CoolWhite => "CW",
            Feature::WarmWhite => "WW",
            Feature::ColourTemp => "CT",
            Feature::Strobe => "Strb",
            Feature::Zoom => "Zoom",
            Feature::Cyan => "C",
            Feature::Magenta => "M",
            Feature::Yellow => "Y",
            Feature::Custom => "Cust",
            Feature::Other => "—",
            Feature::Uv => "UV",
            Feature::Pan => "Pan",
            Feature::Tilt => "Tilt",
            Feature::Speed => "Spd",
        }
    }

    fn is_mixing(self) -> bool {
        matches!(
            self,
            Feature::Red
                | Feature::Green
                | Feature::Blue
                | Feature::Amber
                | Feature::White
                | Feature::Cyan
                | Feature::Magenta
                | Feature::Yellow
        )
    }
}

/// Which front-panel dial drives a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dial {
    /// The library default for the feature.
    Auto,
    /// Colour mixing from the Hue & Saturation dials (R, G, B, A, W, C, M, Y).
    HueSat,
    ShiftHue,
    ShiftSat,
    /// The Hue dial on its own; only for fixtures without colour mixing.
    Hue,
    /// The Saturation dial on its own; only for fixtures without colour mixing.
    Sat,
    None,
    /// Position + Hue dial (pan by default).
    PosHue,
    /// Position + Saturation dial (tilt by default).
    PosSat,
}

impl Dial {
    pub const ALL: [Dial; 9] = [
        Dial::Auto,
        Dial::HueSat,
        Dial::ShiftHue,
        Dial::ShiftSat,
        Dial::PosHue,
        Dial::PosSat,
        Dial::Hue,
        Dial::Sat,
        Dial::None,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Dial::Auto => "Auto",
            Dial::HueSat => "Hue & Sat",
            Dial::ShiftHue => "Shift + Hue",
            Dial::ShiftSat => "Shift + Sat",
            Dial::Hue => "Hue",
            Dial::Sat => "Sat",
            Dial::None => "—",
            Dial::PosHue => "Position + Hue",
            Dial::PosSat => "Position + Sat",
        }
    }

    fn is_auto(&self) -> bool {
        *self == Dial::Auto
    }
}

fn auto() -> Dial {
    Dial::Auto
}

fn full() -> u8 {
    255
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

fn is_full(v: &u8) -> bool {
    *v == 255
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelDef {
    pub feature: Feature,
    /// Low byte of the previous channel with the same feature (16-bit).
    #[serde(default, skip_serializing_if = "is_false")]
    pub fine: bool,
    #[serde(default = "auto", skip_serializing_if = "Dial::is_auto")]
    pub dial: Dial,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub default: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min: u8,
    #[serde(default = "full", skip_serializing_if = "is_full")]
    pub max: u8,
    /// Shown instead of the feature name when set (Custom and Other channels).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

impl ChannelDef {
    pub fn new(feature: Feature) -> Self {
        Self {
            feature,
            fine: false,
            dial: Dial::Auto,
            default: 0,
            min: 0,
            max: 255,
            label: String::new(),
        }
    }

    pub fn display(&self) -> String {
        let name = if self.label.is_empty() {
            self.feature.name()
        } else {
            &self.label
        };
        if self.fine {
            format!("{name} fine")
        } else {
            name.to_string()
        }
    }
}

/// Channels from a compact spec: `I R G B W A UV Strb Zoom P Pf T Tf Spd CW WW CT
/// C M Y`, a trailing `f` for fine (`If`, `Rf`…), `-` or `-N` for an Other channel
/// at value N, and `-N:Label` to name it.
pub fn spec(s: &str) -> Vec<ChannelDef> {
    s.split_whitespace()
        .map(|t| {
            if let Some(rest) = t.strip_prefix('-') {
                let (v, label) = rest.split_once(':').unwrap_or((rest, ""));
                return ChannelDef {
                    default: v.parse().unwrap_or(0),
                    label: label.replace('_', " "),
                    ..ChannelDef::new(Feature::Other)
                };
            }
            let (t, fine) = match t.strip_suffix('f') {
                Some(base) if !base.is_empty() => (base, true),
                _ => (t, false),
            };
            let feature = match t {
                "I" => Feature::Intensity,
                "R" => Feature::Red,
                "G" => Feature::Green,
                "B" => Feature::Blue,
                "W" => Feature::White,
                "A" => Feature::Amber,
                "UV" => Feature::Uv,
                "Strb" => Feature::Strobe,
                "Zoom" => Feature::Zoom,
                "P" => Feature::Pan,
                "T" => Feature::Tilt,
                "Spd" => Feature::Speed,
                "CW" => Feature::CoolWhite,
                "WW" => Feature::WarmWhite,
                "CT" => Feature::ColourTemp,
                "C" => Feature::Cyan,
                "M" => Feature::Magenta,
                "Y" => Feature::Yellow,
                other => panic!("unknown channel spec {other}"),
            };
            let mut c = ChannelDef::new(feature);
            c.fine = fine;
            // Moving heads park centred.
            if matches!(feature, Feature::Pan | Feature::Tilt) && !fine {
                c.default = 128;
            }
            c
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixtureDef {
    pub manufacturer: String,
    pub name: String,
    pub channels: Vec<ChannelDef>,
}

/// How a fixture makes colour, which decides what the Hue/Sat dials do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourKind {
    /// RGB(AW) or CMY mixing: Hue & Saturation.
    Mixing,
    /// Cool and warm white only: the Hue dial sets colour temperature.
    Temperature,
    /// No colour (a dimmer): the dials drive whatever is assigned to them.
    None,
}

impl FixtureDef {
    pub fn new(manufacturer: &str, name: &str, channels: Vec<ChannelDef>) -> Self {
        Self {
            manufacturer: manufacturer.into(),
            name: name.into(),
            channels,
        }
    }

    pub fn footprint(&self) -> u16 {
        self.channels.len() as u16
    }

    pub fn has(&self, f: Feature) -> bool {
        self.channels.iter().any(|c| c.feature == f && !c.fine)
    }

    /// A short summary like "I R G B W A UV Strb".
    pub fn summary(&self) -> String {
        self.channels
            .iter()
            .map(|c| {
                if c.fine {
                    format!("{}f", c.feature.short())
                } else {
                    c.feature.short().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn colour_kind(&self) -> ColourKind {
        if self.channels.iter().any(|c| c.feature.is_mixing()) {
            ColourKind::Mixing
        } else if self.has(Feature::CoolWhite)
            || self.has(Feature::WarmWhite)
            || self.has(Feature::ColourTemp)
        {
            ColourKind::Temperature
        } else {
            ColourKind::None
        }
    }

    pub fn is_mover(&self) -> bool {
        self.has(Feature::Pan) || self.has(Feature::Tilt)
    }

    /// The dial a channel follows, with Auto resolved.
    pub fn dial_of(&self, c: &ChannelDef) -> Dial {
        if c.dial != Dial::Auto {
            return c.dial;
        }
        match c.feature {
            f if f.is_mixing() => Dial::HueSat,
            Feature::CoolWhite | Feature::WarmWhite | Feature::ColourTemp => {
                if self.colour_kind() == ColourKind::Mixing {
                    Dial::HueSat
                } else {
                    Dial::Hue
                }
            }
            Feature::Strobe => Dial::ShiftSat,
            Feature::Zoom => Dial::ShiftHue,
            Feature::Pan => Dial::PosHue,
            Feature::Tilt => Dial::PosSat,
            // UV takes whichever Shift dial is free.
            Feature::Uv if !self.has(Feature::Zoom) => Dial::ShiftHue,
            Feature::Uv if !self.has(Feature::Strobe) => Dial::ShiftSat,
            _ => Dial::None,
        }
    }

    /// The first channel a dial drives.
    pub fn dial_channel(&self, dial: Dial) -> Option<&ChannelDef> {
        self.channels
            .iter()
            .find(|c| !c.fine && self.dial_of(c) == dial)
    }

    /// The home position (0..1) of a dial: where its channel's default value sits.
    pub fn dial_home(&self, dial: Dial) -> f32 {
        self.dial_channel(dial).map_or(0.0, |c| {
            let span = c.max as f32 - c.min as f32;
            if span.abs() < 1.0 {
                0.0
            } else {
                ((c.default as f32 - c.min as f32) / span).clamp(0.0, 1.0)
            }
        })
    }

    /// Shift+Hue (alt 0) or Shift+Sat (alt 1).
    pub fn alt_home(&self, alt: usize) -> f32 {
        self.dial_home(ALT_DIALS[alt])
    }

    pub fn alt_name(&self, alt: usize) -> Option<String> {
        self.dial_channel(ALT_DIALS[alt]).map(ChannelDef::display)
    }

    /// Position + Hue (0, pan) or Position + Sat (1, tilt).
    pub fn pos_home(&self, i: usize) -> f32 {
        self.dial_home(POS_DIALS[i])
    }

    pub fn pos_name(&self, i: usize) -> Option<String> {
        self.dial_channel(POS_DIALS[i]).map(ChannelDef::display)
    }
}

const ALT_DIALS: [Dial; 2] = [Dial::ShiftHue, Dial::ShiftSat];
const POS_DIALS: [Dial; 2] = [Dial::PosHue, Dial::PosSat];

/// White and amber LED blending preferences (0 = only the W/A LEDs).
#[derive(Clone, Copy, Debug, Default)]
pub struct Blend {
    pub white: f32,
    pub amber: f32,
}

/// The value 0..1 of every channel of `def` for a channel output.
pub fn channel_values(def: &FixtureDef, o: &ChannelOut, blend: Blend) -> Vec<f32> {
    let kind = def.colour_kind();
    let has_intensity = def.has(Feature::Intensity);
    // Without an intensity channel the colour channels carry the level.
    let k = if has_intensity { 1.0 } else { o.intensity };

    let mut c = o.rgb;
    let mut white = 0.0;
    let mut amber = 0.0;
    if kind == ColourKind::Mixing {
        let has_white =
            def.has(Feature::White) || def.has(Feature::CoolWhite) || def.has(Feature::WarmWhite);
        let has_rgb = def.has(Feature::Red) || def.has(Feature::Green) || def.has(Feature::Blue);
        if has_white && has_rgb {
            white = c.r.min(c.g).min(c.b);
            let keep = 1.0 - blend.white;
            c = Rgb::new(c.r - white * keep, c.g - white * keep, c.b - white * keep);
        }
        if def.has(Feature::Amber) && has_rgb {
            amber = c.r.min(c.g * 2.0).clamp(0.0, 1.0);
            let keep = 1.0 - blend.amber;
            c = Rgb::new(c.r - amber * keep, c.g - amber * 0.5 * keep, c.b);
        }
    }
    let ct = o.hs.h.clamp(0.0, 1.0);

    def.channels
        .iter()
        .map(|cd| {
            let dial_value = |d: Dial| match d {
                Dial::Hue => Some(o.hs.h),
                Dial::Sat => Some(o.hs.s),
                Dial::ShiftHue => o.alt[0],
                Dial::ShiftSat => o.alt[1],
                Dial::PosHue => o.pos[0],
                Dial::PosSat => o.pos[1],
                _ => None,
            };
            let dial = def.dial_of(cd);
            let v = match cd.feature {
                Feature::Intensity => Some(o.intensity),
                Feature::Red => Some(c.r * k),
                Feature::Green => Some(c.g * k),
                Feature::Blue => Some(c.b * k),
                Feature::Amber => Some(amber * k),
                Feature::White => Some(white * k),
                Feature::Cyan => Some((1.0 - o.rgb.r) * k),
                Feature::Magenta => Some((1.0 - o.rgb.g) * k),
                Feature::Yellow => Some((1.0 - o.rgb.b) * k),
                Feature::CoolWhite | Feature::WarmWhite if kind == ColourKind::Mixing => {
                    Some(white * k)
                }
                Feature::CoolWhite => Some((2.0 * ct).min(1.0) * k),
                Feature::WarmWhite => Some((2.0 * (1.0 - ct)).min(1.0) * k),
                Feature::ColourTemp => Some(ct),
                // UV follows its dial and, without an intensity channel, the level.
                Feature::Uv => dial_value(dial).map(|v| v * k),
                _ => dial_value(dial),
            };
            let Some(v) = v else {
                return cd.default as f32 / 255.0;
            };
            // A strobe dial at zero is "open": the channel's default.
            if cd.feature == Feature::Strobe && v < 0.02 {
                return cd.default as f32 / 255.0;
            }
            let (lo, hi) = (cd.min as f32 / 255.0, cd.max as f32 / 255.0);
            lo + v.clamp(0.0, 1.0) * (hi - lo)
        })
        .collect()
}

/// Writes a patched fixture into the universe at each of its addresses.
pub fn render(p: &Patched, o: &ChannelOut, blend: Blend, dmx: &mut [u8; 512]) {
    let values = channel_values(&p.def, o, blend);
    for &addr in &p.addresses {
        let base = addr as usize;
        if base == 0 {
            continue;
        }
        for (i, (cd, v)) in p.def.channels.iter().zip(&values).enumerate() {
            let slot = base - 1 + i;
            if slot >= 512 {
                break;
            }
            let full = (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
            dmx[slot] = if cd.fine {
                (full & 0xff) as u8
            } else {
                (full >> 8) as u8
            };
        }
    }
}

/// What the fixture looks like on stage, for the visualizer and mimics.
pub struct Appearance {
    pub colour: Rgb,
    pub level: f32,
    /// Flashes per second, if strobing.
    pub strobe_hz: Option<f32>,
    /// Beam width 0 (narrow) .. 1 (wide).
    pub zoom: f32,
    /// Pan and tilt 0..1 for a moving head.
    pub position: Option<(f32, f32)>,
}

pub fn appearance(def: &FixtureDef, o: &ChannelOut) -> Appearance {
    let colour = match def.colour_kind() {
        ColourKind::Mixing => o.rgb,
        ColourKind::Temperature => white_temperature(o.hs.h),
        ColourKind::None => Rgb::new(1.0, 0.8, 0.55),
    };
    let value = |f: Feature| {
        def.channels
            .iter()
            .find(|c| c.feature == f && !c.fine)
            .and_then(|c| {
                let d = def.dial_of(c);
                match d {
                    Dial::ShiftHue => Some(o.alt[0].unwrap_or(def.dial_home(d))),
                    Dial::ShiftSat => Some(o.alt[1].unwrap_or(def.dial_home(d))),
                    Dial::PosHue => Some(o.pos[0].unwrap_or(def.dial_home(d))),
                    Dial::PosSat => Some(o.pos[1].unwrap_or(def.dial_home(d))),
                    Dial::Hue => Some(o.hs.h),
                    Dial::Sat => Some(o.hs.s),
                    _ => None,
                }
            })
    };
    let uv = value(Feature::Uv).unwrap_or(0.0);
    let mut colour = colour;
    if uv > 0.0 {
        let violet = Rgb::new(0.45, 0.1, 1.0);
        colour = Rgb::new(
            (colour.r + violet.r * uv).min(1.0),
            (colour.g + violet.g * uv).min(1.0),
            (colour.b + violet.b * uv).min(1.0),
        );
    }
    Appearance {
        colour,
        level: o.intensity,
        strobe_hz: value(Feature::Strobe)
            .filter(|&v| v > 0.05)
            .map(|v| 1.0 + v * 19.0),
        zoom: value(Feature::Zoom).unwrap_or(0.5),
        position: def.is_mover().then(|| {
            (
                value(Feature::Pan).unwrap_or(0.5),
                value(Feature::Tilt).unwrap_or(0.5),
            )
        }),
    }
}

/// A fixture channel's patch: the fixture type and its DMX start address(es).
/// More than one address is the Stage CL's multipatch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patched {
    pub def: FixtureDef,
    pub addresses: Vec<u16>,
}

/// The Generic folder.
fn generic_fixtures() -> Vec<FixtureDef> {
    let g = |name: &str, s: &str| FixtureDef::new("Generic", name, spec(s));
    vec![
        g("Dimmer", "I"),
        g("Dimmer 16-bit", "I If"),
        g("RGB", "R G B"),
        g("IRGB", "I R G B"),
        g("RGB + Strobe", "R G B Strb"),
        g("RGBA", "R G B A"),
        g("IRGBA", "I R G B A"),
        g("RGBW", "R G B W"),
        g("IRGBW", "I R G B W"),
        g("RGBAW", "R G B A W"),
        g("IRGBAW", "I R G B A W"),
        g("RGBW+UV", "R G B W UV"),
        g("RGBWA+UV (6ch)", "R G B W A UV"),
        g("IRGBWA+UV (7ch)", "I R G B W A UV"),
        g("IRGBWA+UV Strobe (8ch)", "I R G B W A UV Strb"),
        g(
            "RGBWA+UV 10ch (I RGBWAUV Strobe Macro Speed)",
            "I R G B W A UV Strb -0:Macro -0:Speed",
        ),
        g("RGBSZ", "I R G B Strb Zoom"),
        g(
            "RGB 7ch (I R G B Strobe Macro Speed)",
            "I R G B Strb -0:Macro -0:Speed",
        ),
        g(
            "RGBW 8ch (I R G B W Strobe Macro Speed)",
            "I R G B W Strb -0:Macro -0:Speed",
        ),
        g("RGB 16-bit", "R Rf G Gf B Bf"),
        g("RGBW 16-bit", "R Rf G Gf B Bf W Wf"),
        g("CW/WW", "CW WW"),
        g("I CW/WW", "I CW WW"),
        g("I CW/WW Strobe", "I CW WW Strb"),
        g("I CT", "I CT"),
        g("CMY + I", "C M Y I"),
        g("Strobe (I Rate)", "I Strb"),
        g("Blinder 2-cell (CW/WW)", "CW WW"),
        g("UV bar", "UV"),
        g("Moving head wash RGBW (9ch)", "P T Spd I Strb R G B W"),
        g(
            "Moving head wash RGBW 16-bit (14ch)",
            "P Pf T Tf Spd I Strb R G B W Zoom -0:Macro -0:Auto -0:Reset",
        ),
        g(
            "Moving head wash RGBWA+UV (13ch)",
            "P Pf T Tf Spd I Strb R G B W A UV",
        ),
        g(
            "Moving head spot (P T Spd I Strb Colour Gobo)",
            "P Pf T Tf Spd I Strb -0:Colour_wheel -0:Gobo",
        ),
    ]
}

/// Profiles typed in from manufacturers' manuals (not in the Open Fixture Library).
fn manual_fixtures() -> Vec<FixtureDef> {
    vec![FixtureDef::new(
        "Shehds",
        "7x40W Bee Eye Zoom Wash · 15ch",
        spec("P Pf T Tf Spd Zoom I Strb R G B W -0:Micro_colour -0:Colour_temperature -0:Reset"),
    )]
}

static LIBRARY: OnceLock<Vec<FixtureDef>> = OnceLock::new();

/// The built-in library: Generic, manual profiles, then the bundled Open
/// Fixture Library set, grouped by manufacturer.
pub fn library() -> &'static [FixtureDef] {
    LIBRARY.get_or_init(|| {
        let mut v = generic_fixtures();
        let bundled: Vec<FixtureDef> =
            serde_json::from_str(include_str!("../assets/fixtures.json")).unwrap_or_default();
        let mut others = manual_fixtures();
        others.extend(bundled);
        others.sort_by(|a, b| {
            (a.manufacturer.to_lowercase(), a.name.to_lowercase())
                .cmp(&(b.manufacturer.to_lowercase(), b.name.to_lowercase()))
        });
        v.extend(others);
        v
    })
}

/// A library fixture by manufacturer and name.
pub fn find(manufacturer: &str, name: &str) -> Option<FixtureDef> {
    library()
        .iter()
        .find(|f| f.manufacturer == manufacturer && f.name == name)
        .cloned()
}

#[cfg(test)]
pub fn generic(name: &str) -> FixtureDef {
    find("Generic", name).expect("generic fixture")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::{Hs, WHITE};

    fn out(intensity: f32, hs: Hs) -> ChannelOut {
        ChannelOut {
            level: intensity,
            intensity,
            hs,
            rgb: hs.rgb(),
            ..Default::default()
        }
    }

    fn dmx_of(name: &str, o: &ChannelOut, blend: Blend) -> Vec<u8> {
        let p = Patched {
            def: generic(name),
            addresses: vec![1],
        };
        let mut dmx = [0u8; 512];
        render(&p, o, blend, &mut dmx);
        dmx[..p.def.channels.len()].to_vec()
    }

    #[test]
    fn rgb_without_intensity_scales_colour() {
        let red_half = out(0.5, Hs::new(0.0, 1.0));
        assert_eq!(dmx_of("RGB", &red_half, Blend::default()), vec![128, 0, 0]);
        assert_eq!(
            dmx_of("IRGB", &red_half, Blend::default()),
            vec![128, 255, 0, 0]
        );
    }

    #[test]
    fn white_comes_from_the_white_led() {
        let white = out(1.0, WHITE);
        assert_eq!(dmx_of("RGBW", &white, Blend::default()), vec![0, 0, 0, 255]);
        let blended = Blend {
            white: 1.0,
            amber: 0.0,
        };
        assert_eq!(dmx_of("RGBW", &white, blended), vec![255, 255, 255, 255]);
        // Pink: half white plus red.
        let pink = out(1.0, Hs::new(0.0, 0.5));
        assert_eq!(
            dmx_of("RGBW", &pink, Blend::default()),
            vec![128, 0, 0, 128]
        );
    }

    #[test]
    fn amber_from_orange() {
        let amber = out(1.0, Hs::new(1.0 / 12.0, 1.0)); // (1, 0.5, 0)
        assert_eq!(dmx_of("RGBA", &amber, Blend::default()), vec![0, 0, 0, 255]);
    }

    #[test]
    fn temperature_fixture_follows_hue_dial() {
        let mut o = out(1.0, Hs::new(0.0, 0.0));
        assert_eq!(dmx_of("CW/WW", &o, Blend::default()), vec![0, 255]);
        o.hs.h = 0.5;
        assert_eq!(dmx_of("CW/WW", &o, Blend::default()), vec![255, 255]);
        o.hs.h = 1.0;
        o.intensity = 0.5;
        assert_eq!(dmx_of("CW/WW", &o, Blend::default()), vec![128, 0]);
    }

    #[test]
    fn alt_dials_drive_strobe_and_zoom() {
        let mut o = out(1.0, WHITE);
        assert_eq!(
            dmx_of("RGBSZ", &o, Blend::default()),
            vec![255, 255, 255, 255, 0, 0]
        );
        o.alt = [Some(0.5), Some(1.0)];
        assert_eq!(
            dmx_of("RGBSZ", &o, Blend::default()),
            vec![255, 255, 255, 255, 255, 128]
        );
    }

    #[test]
    fn sixteen_bit_and_multipatch() {
        let p = Patched {
            def: generic("Dimmer 16-bit"),
            addresses: vec![10, 512],
        };
        let mut dmx = [0u8; 512];
        render(&p, &out(0.5, WHITE), Blend::default(), &mut dmx);
        assert_eq!(&dmx[9..11], &[0x80, 0x00]);
        // The second copy runs off the end of the universe: only its coarse byte fits.
        assert_eq!(dmx[511], 0x80);
    }

    #[test]
    fn auto_dials() {
        let d = generic("RGBSZ");
        assert_eq!(d.alt_name(0).as_deref(), Some("Zoom"));
        assert_eq!(d.alt_name(1).as_deref(), Some("Strobe"));
        assert_eq!(generic("CW/WW").colour_kind(), ColourKind::Temperature);
        assert_eq!(generic("Dimmer").colour_kind(), ColourKind::None);
    }

    #[test]
    fn uv_takes_a_free_shift_dial() {
        let d = generic("IRGBWA+UV Strobe (8ch)");
        assert_eq!(d.alt_name(0).as_deref(), Some("UV"));
        assert_eq!(d.alt_name(1).as_deref(), Some("Strobe"));
        let mut o = out(1.0, WHITE);
        o.alt[0] = Some(1.0);
        let v = dmx_of("IRGBWA+UV Strobe (8ch)", &o, Blend::default());
        assert_eq!(v[6], 255);
        assert_eq!(v[7], 0);
    }

    #[test]
    fn position_dials_drive_pan_and_tilt() {
        let name = "Moving head wash RGBW 16-bit (14ch)";
        let d = generic(name);
        assert_eq!(d.pos_name(0).as_deref(), Some("Pan"));
        assert_eq!(d.pos_name(1).as_deref(), Some("Tilt"));
        assert!((d.pos_home(0) - 128.0 / 255.0).abs() < 1e-6);
        let mut o = out(1.0, WHITE);
        // Home: centred.
        assert_eq!(&dmx_of(name, &o, Blend::default())[..4], &[128, 0, 128, 0]);
        o.pos = [Some(0.25), Some(1.0)];
        let v = dmx_of(name, &o, Blend::default());
        assert_eq!(&v[..4], &[0x40, 0x00, 0xff, 0xff]);
        let a = appearance(&d, &o);
        assert_eq!(a.position, Some((0.25, 1.0)));
    }

    #[test]
    fn strobe_at_zero_is_open() {
        let mut def = generic("Strobe (I Rate)");
        def.channels[1].default = 10;
        def.channels[1].min = 20;
        let p = Patched {
            def,
            addresses: vec![1],
        };
        let mut o = out(1.0, WHITE);
        let mut dmx = [0u8; 512];
        render(&p, &o, Blend::default(), &mut dmx);
        assert_eq!(dmx[1], 10);
        o.alt[1] = Some(1.0);
        render(&p, &o, Blend::default(), &mut dmx);
        assert_eq!(dmx[1], 255);
    }

    #[test]
    fn spec_parses_every_token() {
        let chans = spec("I If R G B W A UV Strb Zoom P Pf T Tf Spd CW WW CT C M Y -7:Macro_mode");
        assert_eq!(chans.len(), 22);
        assert!(chans[1].fine && chans[1].feature == Feature::Intensity);
        assert_eq!(chans[10].default, 128);
        assert_eq!(chans[21].label, "Macro mode");
        assert_eq!(chans[21].default, 7);
    }

    #[test]
    fn library_has_shehds_and_bundled_fixtures() {
        let lib = library();
        assert!(lib.len() > 100, "only {} fixtures", lib.len());
        assert!(find("Shehds", "7x40W Bee Eye Zoom Wash · 15ch").is_some());
        let shehds = lib.iter().filter(|f| f.manufacturer == "Shehds").count();
        assert!(shehds >= 10, "{shehds} Shehds modes");
        // Every bundled fixture renders without panicking.
        for f in lib {
            let p = Patched {
                def: f.clone(),
                addresses: vec![1],
            };
            let mut dmx = [0u8; 512];
            render(&p, &out(1.0, WHITE), Blend::default(), &mut dmx);
        }
    }
}
