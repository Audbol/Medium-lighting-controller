//! Fixture library and DMX rendering. A fixture type is a list of channels,
//! each with a feature (Intensity, Red, Strobe…), the dial that drives it and
//! its default/min/max values, like the Stage CL's fixture editor.

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
    /// Custom 1-4 or a name of your own (Pan, Tilt…); set by a dial or left at its default.
    Custom,
    /// Held at its default value; no control on the console.
    Other,
}

impl Feature {
    pub const ALL: [Feature; 16] = [
        Feature::Intensity,
        Feature::Red,
        Feature::Green,
        Feature::Blue,
        Feature::Amber,
        Feature::White,
        Feature::CoolWhite,
        Feature::WarmWhite,
        Feature::ColourTemp,
        Feature::Strobe,
        Feature::Zoom,
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
}

impl Dial {
    pub const ALL: [Dial; 7] = [
        Dial::Auto,
        Dial::HueSat,
        Dial::ShiftHue,
        Dial::ShiftSat,
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelDef {
    pub feature: Feature,
    /// Low byte of the previous channel with the same feature (16-bit).
    #[serde(default)]
    pub fine: bool,
    pub dial: Dial,
    pub default: u8,
    pub min: u8,
    pub max: u8,
    /// Shown instead of the feature name when set (Custom features).
    #[serde(default)]
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

    fn fine(feature: Feature) -> Self {
        Self {
            fine: true,
            ..Self::new(feature)
        }
    }

    fn other(default: u8) -> Self {
        Self {
            default,
            ..Self::new(Feature::Other)
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
            _ => Dial::None,
        }
    }

    /// The channel driven by Shift+Hue (alt 0) or Shift+Sat (alt 1), if any.
    pub fn alt_channel(&self, alt: usize) -> Option<&ChannelDef> {
        let want = if alt == 0 {
            Dial::ShiftHue
        } else {
            Dial::ShiftSat
        };
        self.channels
            .iter()
            .find(|c| !c.fine && self.dial_of(c) == want)
    }

    /// The home position of an alternate dial, 0..1.
    pub fn alt_home(&self, alt: usize) -> f32 {
        self.alt_channel(alt).map_or(0.0, |c| {
            let span = c.max as f32 - c.min as f32;
            if span.abs() < 1.0 {
                0.0
            } else {
                ((c.default as f32 - c.min as f32) / span).clamp(0.0, 1.0)
            }
        })
    }

    pub fn alt_name(&self, alt: usize) -> Option<String> {
        self.alt_channel(alt).map(|c| c.display())
    }
}

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
                _ => dial_value(dial),
            };
            let Some(v) = v else {
                return cd.default as f32 / 255.0;
            };
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
}

pub fn appearance(def: &FixtureDef, o: &ChannelOut) -> Appearance {
    let colour = match def.colour_kind() {
        ColourKind::Mixing => o.rgb,
        ColourKind::Temperature => white_temperature(o.hs.h),
        ColourKind::None => Rgb::new(1.0, 0.8, 0.55),
    };
    let alt_value = |f: Feature| {
        def.channels
            .iter()
            .find(|c| c.feature == f && !c.fine)
            .and_then(|c| match def.dial_of(c) {
                Dial::ShiftHue => Some(o.alt[0].unwrap_or(def.alt_home(0))),
                Dial::ShiftSat => Some(o.alt[1].unwrap_or(def.alt_home(1))),
                Dial::Hue => Some(o.hs.h),
                Dial::Sat => Some(o.hs.s),
                _ => None,
            })
    };
    Appearance {
        colour,
        level: o.intensity,
        strobe_hz: alt_value(Feature::Strobe)
            .filter(|&v| v > 0.05)
            .map(|v| 1.0 + v * 19.0),
        zoom: alt_value(Feature::Zoom).unwrap_or(0.5),
    }
}

/// A fixture channel's patch: the fixture type and its DMX start address(es).
/// More than one address is the Stage CL's multipatch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patched {
    pub def: FixtureDef,
    pub addresses: Vec<u16>,
}

/// The built-in Generic folder.
pub fn library() -> Vec<FixtureDef> {
    use Feature::*;
    let ch = ChannelDef::new;
    let g = |name: &str, chs: Vec<ChannelDef>| FixtureDef::new("Generic", name, chs);
    vec![
        g("Dimmer", vec![ch(Intensity)]),
        g(
            "Dimmer 16-bit",
            vec![ch(Intensity), ChannelDef::fine(Intensity)],
        ),
        g("RGB", vec![ch(Red), ch(Green), ch(Blue)]),
        g("IRGB", vec![ch(Intensity), ch(Red), ch(Green), ch(Blue)]),
        g("RGBA", vec![ch(Red), ch(Green), ch(Blue), ch(Amber)]),
        g(
            "IRGBA",
            vec![ch(Intensity), ch(Red), ch(Green), ch(Blue), ch(Amber)],
        ),
        g("RGBW", vec![ch(Red), ch(Green), ch(Blue), ch(White)]),
        g(
            "IRGBW",
            vec![ch(Intensity), ch(Red), ch(Green), ch(Blue), ch(White)],
        ),
        g(
            "RGBAW",
            vec![ch(Red), ch(Green), ch(Blue), ch(Amber), ch(White)],
        ),
        g(
            "IRGBAW",
            vec![
                ch(Intensity),
                ch(Red),
                ch(Green),
                ch(Blue),
                ch(Amber),
                ch(White),
            ],
        ),
        g(
            "RGBSZ",
            vec![
                ch(Intensity),
                ch(Red),
                ch(Green),
                ch(Blue),
                ch(Strobe),
                ch(Zoom),
            ],
        ),
        g(
            "RGB 7ch (I R G B Strobe Macro Speed)",
            vec![
                ch(Intensity),
                ch(Red),
                ch(Green),
                ch(Blue),
                ch(Strobe),
                ChannelDef::other(0),
                ChannelDef::other(0),
            ],
        ),
        g(
            "RGBW 8ch (I R G B W Strobe Macro Speed)",
            vec![
                ch(Intensity),
                ch(Red),
                ch(Green),
                ch(Blue),
                ch(White),
                ch(Strobe),
                ChannelDef::other(0),
                ChannelDef::other(0),
            ],
        ),
        g(
            "RGB 16-bit",
            vec![
                ch(Red),
                ChannelDef::fine(Red),
                ch(Green),
                ChannelDef::fine(Green),
                ch(Blue),
                ChannelDef::fine(Blue),
            ],
        ),
        g("CW/WW", vec![ch(CoolWhite), ch(WarmWhite)]),
        g("I CW/WW", vec![ch(Intensity), ch(CoolWhite), ch(WarmWhite)]),
        g("I CT", vec![ch(Intensity), ch(ColourTemp)]),
        g(
            "CMY + I",
            vec![ch(Cyan), ch(Magenta), ch(Yellow), ch(Intensity)],
        ),
        g("Strobe (I Rate)", vec![ch(Intensity), ch(Strobe)]),
    ]
}

pub fn generic(name: &str) -> FixtureDef {
    library()
        .into_iter()
        .find(|f| f.name == name)
        .expect("generic fixture")
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
            alt: [None, None],
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
}
