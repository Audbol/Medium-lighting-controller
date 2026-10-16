//! The Colour window's built-in content: three pages of swatches, a library of
//! named colours, ten pages of patterns and twelve pages of chases. Colour
//! chases carry colour only, so the faders keep control of intensity; the
//! Intensity page carries levels only, so colour comes from elsewhere.

use crate::colour::{Hs, Rgb};
use crate::engine::{CHANNELS, Cell, Chase, Crossfade, Direction, Look};

pub const BUTTONS_PER_PAGE: usize = 18;
pub const SWATCH_PAGES: usize = 3;
pub const SWATCHES: usize = SWATCH_PAGES * BUTTONS_PER_PAGE;

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

const RED: Rgb = rgb(255, 0, 0);
const ORANGE: Rgb = rgb(255, 96, 0);
const AMBER: Rgb = rgb(255, 150, 0);
const YELLOW: Rgb = rgb(255, 230, 0);
const LIME: Rgb = rgb(150, 255, 0);
const GREEN: Rgb = rgb(0, 255, 0);
const TEAL: Rgb = rgb(0, 255, 140);
const CYAN: Rgb = rgb(0, 255, 255);
const SKY: Rgb = rgb(0, 140, 255);
const BLUE: Rgb = rgb(0, 0, 255);
const PURPLE: Rgb = rgb(110, 0, 255);
const MAGENTA: Rgb = rgb(255, 0, 255);
const PINK: Rgb = rgb(255, 80, 160);
const LAVENDER: Rgb = rgb(190, 150, 255);
const WHITE: Rgb = rgb(255, 255, 255);
const WARM: Rgb = rgb(255, 205, 150);
const CONGO: Rgb = rgb(50, 0, 160);
const STEEL: Rgb = rgb(170, 210, 255);

pub const DEFAULT_SWATCHES: [Rgb; SWATCHES] = [
    // Page 1: the console's familiar colours.
    RED,
    ORANGE,
    AMBER,
    YELLOW,
    LIME,
    GREEN,
    TEAL,
    CYAN,
    SKY,
    BLUE,
    CONGO,
    PURPLE,
    MAGENTA,
    PINK,
    LAVENDER,
    STEEL,
    WARM,
    WHITE,
    // Page 2: saturated in-betweens.
    rgb(255, 0, 64),
    rgb(255, 40, 0),
    rgb(255, 120, 0),
    rgb(255, 190, 0),
    rgb(200, 255, 0),
    rgb(80, 255, 0),
    rgb(0, 255, 80),
    rgb(0, 255, 200),
    rgb(0, 200, 255),
    rgb(0, 90, 255),
    rgb(60, 0, 255),
    rgb(160, 0, 255),
    rgb(210, 0, 255),
    rgb(255, 0, 200),
    rgb(255, 0, 120),
    rgb(255, 60, 60),
    rgb(120, 255, 120),
    rgb(90, 90, 255),
    // Page 3: tints, pastels and whites.
    rgb(255, 160, 160),
    rgb(255, 190, 140),
    rgb(255, 225, 150),
    rgb(255, 250, 180),
    rgb(210, 255, 170),
    rgb(170, 255, 200),
    rgb(170, 255, 255),
    rgb(170, 210, 255),
    rgb(190, 180, 255),
    rgb(230, 180, 255),
    rgb(255, 180, 230),
    rgb(255, 210, 220),
    rgb(255, 170, 90),
    rgb(255, 215, 170),
    rgb(255, 238, 215),
    rgb(240, 245, 255),
    rgb(215, 230, 255),
    rgb(190, 215, 255),
];

/// A named colour in the Colour window's library.
pub struct Named {
    pub group: &'static str,
    pub name: &'static str,
    pub rgb: Rgb,
}

const fn named(group: &'static str, name: &'static str, rgb: Rgb) -> Named {
    Named { group, name, rgb }
}

/// Theatre and party colours by name, approximate on RGB LEDs.
pub const LIBRARY: &[Named] = &[
    named("Reds", "Primary Red", rgb(255, 0, 0)),
    named("Reds", "Fire Red", rgb(255, 25, 0)),
    named("Reds", "Blood Red", rgb(200, 0, 10)),
    named("Reds", "Crimson", rgb(230, 0, 50)),
    named("Reds", "Scarlet", rgb(255, 36, 0)),
    named("Reds", "Cherry", rgb(255, 0, 70)),
    named("Reds", "Rose Red", rgb(255, 30, 90)),
    named("Reds", "Coral", rgb(255, 90, 70)),
    named("Reds", "Salmon", rgb(255, 120, 100)),
    named("Oranges & Ambers", "Orange", rgb(255, 96, 0)),
    named("Oranges & Ambers", "Deep Orange", rgb(255, 60, 0)),
    named("Oranges & Ambers", "Tangerine", rgb(255, 110, 20)),
    named("Oranges & Ambers", "Amber", rgb(255, 150, 0)),
    named("Oranges & Ambers", "Deep Amber", rgb(255, 120, 0)),
    named("Oranges & Ambers", "Bastard Amber", rgb(255, 190, 140)),
    named("Oranges & Ambers", "Straw", rgb(255, 220, 140)),
    named("Oranges & Ambers", "Peach", rgb(255, 170, 120)),
    named("Oranges & Ambers", "Apricot", rgb(255, 150, 80)),
    named("Oranges & Ambers", "Candlelight", rgb(255, 160, 60)),
    named("Yellows", "Yellow", rgb(255, 230, 0)),
    named("Yellows", "Lemon", rgb(255, 255, 60)),
    named("Yellows", "Gold", rgb(255, 200, 0)),
    named("Yellows", "Sunflower", rgb(255, 210, 30)),
    named("Yellows", "Pale Yellow", rgb(255, 245, 160)),
    named("Yellows", "Chartreuse", rgb(210, 255, 0)),
    named("Greens", "Lime", rgb(150, 255, 0)),
    named("Greens", "Primary Green", rgb(0, 255, 0)),
    named("Greens", "Emerald", rgb(0, 200, 80)),
    named("Greens", "Forest Green", rgb(20, 140, 30)),
    named("Greens", "Moss Green", rgb(110, 160, 40)),
    named("Greens", "Mint", rgb(120, 255, 170)),
    named("Greens", "Jade", rgb(0, 180, 110)),
    named("Greens", "Pale Green", rgb(180, 255, 180)),
    named("Greens", "Spring Green", rgb(0, 255, 127)),
    named("Cyans & Teals", "Cyan", rgb(0, 255, 255)),
    named("Cyans & Teals", "Teal", rgb(0, 255, 140)),
    named("Cyans & Teals", "Turquoise", rgb(30, 230, 210)),
    named("Cyans & Teals", "Aqua", rgb(80, 255, 230)),
    named("Cyans & Teals", "Peacock", rgb(0, 180, 200)),
    named("Cyans & Teals", "Pale Cyan", rgb(170, 255, 255)),
    named("Cyans & Teals", "Ice", rgb(200, 240, 255)),
    named("Blues", "Primary Blue", rgb(0, 0, 255)),
    named("Blues", "Congo Blue", rgb(50, 0, 160)),
    named("Blues", "Royal Blue", rgb(40, 60, 255)),
    named("Blues", "Navy", rgb(0, 20, 140)),
    named("Blues", "Sky Blue", rgb(0, 140, 255)),
    named("Blues", "Azure", rgb(0, 110, 255)),
    named("Blues", "Steel Blue", rgb(170, 210, 255)),
    named("Blues", "Moonlight", rgb(150, 180, 255)),
    named("Blues", "Cobalt", rgb(0, 70, 220)),
    named("Blues", "Indigo", rgb(75, 0, 200)),
    named("Purples", "Purple", rgb(110, 0, 255)),
    named("Purples", "Violet", rgb(140, 0, 255)),
    named("Purples", "Lavender", rgb(190, 150, 255)),
    named("Purples", "Lilac", rgb(210, 170, 255)),
    named("Purples", "Plum", rgb(150, 0, 150)),
    named("Purples", "Amethyst", rgb(160, 80, 255)),
    named("Purples", "Ultraviolet", rgb(90, 0, 255)),
    named("Purples", "Orchid", rgb(220, 90, 255)),
    named("Pinks", "Magenta", rgb(255, 0, 255)),
    named("Pinks", "Hot Pink", rgb(255, 20, 150)),
    named("Pinks", "Pink", rgb(255, 80, 160)),
    named("Pinks", "Rose", rgb(255, 120, 170)),
    named("Pinks", "Flamingo", rgb(255, 130, 140)),
    named("Pinks", "Bubblegum", rgb(255, 150, 210)),
    named("Pinks", "Blush", rgb(255, 190, 200)),
    named("Pinks", "Fuchsia", rgb(240, 0, 200)),
    named("Pinks", "Raspberry", rgb(230, 0, 110)),
    named("Whites", "White", rgb(255, 255, 255)),
    named("Whites", "Tungsten 3200K", rgb(255, 180, 107)),
    named("Whites", "Warm White 2700K", rgb(255, 166, 87)),
    named("Whites", "Halogen 3000K", rgb(255, 177, 110)),
    named("Whites", "Neutral 4000K", rgb(255, 206, 166)),
    named("Whites", "Daylight 5600K", rgb(255, 238, 227)),
    named("Whites", "Cool 6500K", rgb(255, 249, 253)),
    named("Whites", "Overcast 7500K", rgb(230, 235, 255)),
    named("Whites", "Blue Sky 10000K", rgb(200, 215, 255)),
    named("Whites", "Ivory", rgb(255, 245, 225)),
    named("Whites", "Cream", rgb(255, 235, 200)),
    named("Whites", "Champagne", rgb(255, 225, 180)),
    named("Whites", "Pearl", rgb(240, 240, 255)),
    named("Whites", "Snow", rgb(250, 250, 255)),
    named("Whites", "Silver", rgb(220, 225, 235)),
];

pub fn library_groups() -> Vec<&'static str> {
    let mut groups: Vec<&str> = Vec::new();
    for c in LIBRARY {
        if !groups.contains(&c.group) {
            groups.push(c.group);
        }
    }
    groups
}

/// Colour pairs for the two-colour patterns and chases (two pages of 18).
pub const PAIRS: [(Rgb, Rgb); 36] = [
    (RED, BLUE),
    (RED, WHITE),
    (BLUE, WHITE),
    (RED, GREEN),
    (GREEN, BLUE),
    (MAGENTA, CYAN),
    (AMBER, BLUE),
    (RED, AMBER),
    (MAGENTA, BLUE),
    (CYAN, BLUE),
    (GREEN, YELLOW),
    (ORANGE, PURPLE),
    (PINK, WHITE),
    (LAVENDER, TEAL),
    (RED, YELLOW),
    (BLUE, YELLOW),
    (GREEN, MAGENTA),
    (CYAN, WHITE),
    (AMBER, CONGO),
    (TEAL, MAGENTA),
    (WARM, STEEL),
    (RED, CYAN),
    (ORANGE, BLUE),
    (YELLOW, PURPLE),
    (LIME, MAGENTA),
    (PINK, CYAN),
    (GREEN, WHITE),
    (PURPLE, WHITE),
    (CONGO, WHITE),
    (RED, PURPLE),
    (AMBER, WHITE),
    (SKY, PINK),
    (TEAL, AMBER),
    (LAVENDER, AMBER),
    (LIME, BLUE),
    (MAGENTA, YELLOW),
];

/// Three-colour sets for the Thirds patterns.
pub const TRIPLES: [[Rgb; 3]; 18] = [
    [RED, GREEN, BLUE],
    [CYAN, MAGENTA, YELLOW],
    [RED, WHITE, BLUE],
    [RED, AMBER, YELLOW],
    [GREEN, WHITE, RED],
    [PURPLE, PINK, WHITE],
    [TEAL, BLUE, PURPLE],
    [ORANGE, MAGENTA, PURPLE],
    [CYAN, LIME, PINK],
    [AMBER, WARM, STEEL],
    [CONGO, MAGENTA, CYAN],
    [RED, YELLOW, GREEN],
    [BLUE, WHITE, CYAN],
    [PINK, LAVENDER, SKY],
    [GREEN, LIME, TEAL],
    [ORANGE, TEAL, WHITE],
    [MAGENTA, BLUE, AMBER],
    [WARM, AMBER, RED],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// One colour on odd channels, the other on even (pairs 0-35).
    OddEven(usize),
    /// First half one colour, second half the other.
    Half(usize),
    /// One colour at both ends crossfading to the other in the centre.
    Crossfade(usize),
    /// One colour fading to the other from end to end.
    Gradient(usize),
    /// Three colours repeating.
    Thirds(usize),
    /// Rainbows and palettes.
    Special(usize),
}

pub const PATTERN_PAGES: [&str; 10] = [
    "Odds & Evens 1",
    "Odds & Evens 2",
    "Half & Half 1",
    "Half & Half 2",
    "Crossfade 1",
    "Crossfade 2",
    "Gradient 1",
    "Gradient 2",
    "Thirds",
    "Specials",
];

pub const PATTERN_SPECIALS: [&str; BUTTONS_PER_PAGE] = [
    "Rainbow",
    "Pastel",
    "Warm Rainbow",
    "Cool Rainbow",
    "Fire",
    "Ocean",
    "Forest",
    "Sunset",
    "Ice",
    "Candy",
    "Neon",
    "Aurora",
    "Lava",
    "Spring",
    "Autumn",
    "Tricolour",
    "Mirror Rainbow",
    "Random",
];

pub fn pattern_page(page: usize) -> Vec<Pattern> {
    let base = (page % 2) * BUTTONS_PER_PAGE;
    (0..BUTTONS_PER_PAGE)
        .map(|i| match page {
            0 | 1 => Pattern::OddEven(base + i),
            2 | 3 => Pattern::Half(base + i),
            4 | 5 => Pattern::Crossfade(base + i),
            6 | 7 => Pattern::Gradient(base + i),
            8 => Pattern::Thirds(i),
            _ => Pattern::Special(i),
        })
        .collect()
}

pub fn pattern_name(p: Pattern) -> String {
    match p {
        Pattern::Special(i) => PATTERN_SPECIALS[i % BUTTONS_PER_PAGE].to_string(),
        _ => String::new(),
    }
}

/// A palette for the special patterns and chases.
fn palette(i: usize) -> &'static [Rgb] {
    const FIRE: [Rgb; 4] = [RED, ORANGE, AMBER, YELLOW];
    const OCEAN: [Rgb; 4] = [BLUE, SKY, CYAN, TEAL];
    const FOREST: [Rgb; 4] = [GREEN, LIME, TEAL, rgb(20, 140, 30)];
    const SUNSET: [Rgb; 4] = [ORANGE, PINK, MAGENTA, PURPLE];
    const ICE: [Rgb; 4] = [WHITE, STEEL, CYAN, SKY];
    const CANDY: [Rgb; 3] = [PINK, CYAN, YELLOW];
    const NEON: [Rgb; 4] = [MAGENTA, LIME, CYAN, rgb(255, 20, 150)];
    const AURORA: [Rgb; 4] = [GREEN, TEAL, PURPLE, CYAN];
    const LAVA: [Rgb; 4] = [RED, ORANGE, RED, MAGENTA];
    const SPRING: [Rgb; 4] = [LIME, PINK, YELLOW, rgb(170, 255, 200)];
    const AUTUMN: [Rgb; 4] = [ORANGE, rgb(200, 0, 10), AMBER, rgb(110, 160, 40)];
    const TRICOLOUR: [Rgb; 3] = [RED, WHITE, BLUE];
    match i {
        4 => &FIRE,
        5 => &OCEAN,
        6 => &FOREST,
        7 => &SUNSET,
        8 => &ICE,
        9 => &CANDY,
        10 => &NEON,
        11 => &AURORA,
        12 => &LAVA,
        13 => &SPRING,
        14 => &AUTUMN,
        _ => &TRICOLOUR,
    }
}

/// Colour of each of `n` selected channels, in selection order.
pub fn pattern_colours(p: Pattern, n: usize) -> Vec<Rgb> {
    // 0 at the first channel, 1 at the last.
    let along = |j: usize| {
        if n < 2 {
            0.0
        } else {
            j as f32 / (n - 1) as f32
        }
    };
    (0..n)
        .map(|j| match p {
            Pattern::OddEven(i) => {
                let (a, b) = PAIRS[i % PAIRS.len()];
                // Channel numbers count from 1: the first is odd.
                if j % 2 == 0 { a } else { b }
            }
            Pattern::Half(i) => {
                let (a, b) = PAIRS[i % PAIRS.len()];
                if j < n.div_ceil(2) { a } else { b }
            }
            Pattern::Crossfade(i) => {
                let (a, b) = PAIRS[i % PAIRS.len()];
                a.lerp(b, 1.0 - (along(j) * 2.0 - 1.0).abs())
            }
            Pattern::Gradient(i) => {
                let (a, b) = PAIRS[i % PAIRS.len()];
                a.lerp(b, along(j))
            }
            Pattern::Thirds(i) => TRIPLES[i % TRIPLES.len()][j % 3],
            Pattern::Special(i) => match i % BUTTONS_PER_PAGE {
                0 => Hs::new(j as f32 / n.max(1) as f32, 1.0).rgb(),
                1 => Hs::new(j as f32 / n.max(1) as f32, 0.4).rgb(),
                2 => Hs::new(-0.05 + along(j) * 0.25, 1.0).rgb(),
                3 => Hs::new(0.4 + along(j) * 0.4, 1.0).rgb(),
                16 => Hs::new((along(j) * 2.0 - 1.0).abs(), 1.0).rgb(),
                17 => Hs::new((noise(j, 7) * 24.0).floor() / 24.0, 1.0).rgb(),
                15 => TRICOLOUR_SPLIT[(along(j) * 2.999) as usize],
                k => wave(palette(k), along(j) * 0.999),
            },
        })
        .collect()
}

const TRICOLOUR_SPLIT: [Rgb; 3] = [RED, WHITE, BLUE];

/// Colours for a pattern button preview.
pub fn pattern_preview(p: Pattern) -> Vec<Rgb> {
    pattern_colours(p, 6)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourChase {
    /// Rainbow, fire, water, twinkle and friends.
    Special(usize),
    /// More effects: wipes, ripples, themes.
    Special2(usize),
    /// The pair swaps between odd and even channels (pairs 0-35).
    OddEven(usize),
    /// Halves of the two colours crossfading along the selection.
    Crossfade(usize),
    /// Halves of the two colours snapping along the selection.
    Snap(usize),
    /// The two colours flowing along the selection as a smooth wave.
    Wave(usize),
    /// The second colour sparkling over the first.
    Sparkle(usize),
    /// Intensity effects with no colour: comets, pulses, flicker.
    Intensity(usize),
}

pub const CHASE_PAGES: [&str; 12] = [
    "Specials",
    "Specials 2",
    "Odds & Evens 1",
    "Odds & Evens 2",
    "Crossfade 1",
    "Crossfade 2",
    "Snap 1",
    "Snap 2",
    "Wave 1",
    "Wave 2",
    "Sparkle",
    "Intensity",
];

/// The page of intensity effects.
pub const INTENSITY_PAGE: usize = 11;

pub const SPECIALS: [&str; BUTTONS_PER_PAGE] = [
    "Odd/Even",
    "Crossfade",
    "Rainbow",
    "Rainbow Fade",
    "Fire",
    "Water",
    "Twinkle",
    "Sparkle",
    "Police",
    "Ice",
    "Forest",
    "Sunset",
    "Candy",
    "Disco",
    "RGB",
    "Pastel",
    "Lava",
    "Aurora",
];

pub const SPECIALS_2: [&str; BUTTONS_PER_PAGE] = [
    "Rainbow Wave",
    "Colour Wipe",
    "Spectrum Split",
    "Complements",
    "Ping Pong",
    "Breathe",
    "Ripple",
    "RGB Rotate",
    "Thunder",
    "Halloween",
    "Christmas",
    "Stars & Stripes",
    "Tropical",
    "Deep Sea",
    "Galaxy",
    "Sunrise",
    "Quarter Hues",
    "Two-tone",
];

pub const INTENSITY: [&str; BUTTONS_PER_PAGE] = [
    "Comet",
    "Knight Rider",
    "Chase",
    "Chase Pairs",
    "Every Third",
    "Odd/Even",
    "Pulse",
    "Breathe",
    "Wave",
    "Flicker",
    "Twinkle",
    "Lightning",
    "Fill & Empty",
    "Centre Out",
    "Outside In",
    "Heartbeat",
    "Strobe",
    "Random",
];

pub fn chase_page(page: usize) -> Vec<ColourChase> {
    let base = (page % 2) * BUTTONS_PER_PAGE;
    (0..BUTTONS_PER_PAGE)
        .map(|i| match page {
            0 => ColourChase::Special(i),
            1 => ColourChase::Special2(i),
            2 | 3 => ColourChase::OddEven(base + i),
            4 | 5 => ColourChase::Crossfade(base + i),
            6 | 7 => ColourChase::Snap(base + i),
            8 | 9 => ColourChase::Wave(base + i),
            10 => ColourChase::Sparkle(i),
            _ => ColourChase::Intensity(i),
        })
        .collect()
}

/// Every built-in chase, in the order Autopilot plays them.
pub fn all_chases() -> Vec<ColourChase> {
    (0..CHASE_PAGES.len()).flat_map(chase_page).collect()
}

pub fn chase_name(c: ColourChase) -> String {
    match c {
        ColourChase::Special(i) => SPECIALS[i % BUTTONS_PER_PAGE].to_string(),
        ColourChase::Special2(i) => SPECIALS_2[i % BUTTONS_PER_PAGE].to_string(),
        ColourChase::OddEven(i) => format!("Odd/Even {}", i + 1),
        ColourChase::Crossfade(i) => format!("Crossfade {}", i + 1),
        ColourChase::Snap(i) => format!("Snap {}", i + 1),
        ColourChase::Wave(i) => format!("Wave {}", i + 1),
        ColourChase::Sparkle(i) => format!("Sparkle {}", i + 1),
        ColourChase::Intensity(i) => INTENSITY[i % BUTTONS_PER_PAGE].to_string(),
    }
}

/// A small deterministic hash for the "random" effects.
fn noise(a: usize, b: usize) -> f32 {
    let mut x = (a as u32).wrapping_mul(0x9e37_79b9) ^ (b as u32).wrapping_mul(0x85eb_ca6b);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

fn wave(colours: &[Rgb], x: f32) -> Rgb {
    let n = colours.len();
    let x = x.rem_euclid(1.0) * n as f32;
    let i = x as usize % n;
    colours[i].lerp(colours[(i + 1) % n], x.fract())
}

type StepColour = Box<dyn Fn(usize, usize) -> Rgb>;
type StepLevel = Box<dyn Fn(usize, usize) -> f32>;

enum Steps {
    Colour(StepColour),
    Level(StepLevel),
}

/// Builds a chase over `channels` (in selection order).
pub fn build_chase(c: ColourChase, channels: &[usize]) -> Chase {
    let n = channels.len().max(1);
    let (steps, crossfade, bpm, f): (usize, Crossfade, f32, Steps) = match c {
        ColourChase::Special(i) => {
            let (s, x, b, f) = special(i % BUTTONS_PER_PAGE, n);
            (s, x, b, Steps::Colour(f))
        }
        ColourChase::Special2(i) => {
            let (s, x, b, f) = special2(i % BUTTONS_PER_PAGE, n);
            (s, x, b, Steps::Colour(f))
        }
        ColourChase::OddEven(i) => {
            let (a, b) = PAIRS[i % PAIRS.len()];
            (
                2,
                Crossfade::Snap,
                60.0,
                Steps::Colour(Box::new(move |s, j| if (s + j) % 2 == 0 { a } else { b })),
            )
        }
        ColourChase::Crossfade(i) | ColourChase::Snap(i) => {
            let (a, b) = PAIRS[i % PAIRS.len()];
            let half = n.div_ceil(2);
            let fade = matches!(c, ColourChase::Crossfade(_));
            (
                n.max(2),
                if fade {
                    Crossfade::Full
                } else {
                    Crossfade::Snap
                },
                if fade { 60.0 } else { 120.0 },
                Steps::Colour(Box::new(move |s, j| if (j + s) % n < half { a } else { b })),
            )
        }
        ColourChase::Wave(i) => {
            let (a, b) = PAIRS[i % PAIRS.len()];
            let nf = n as f32;
            (
                12,
                Crossfade::Full,
                120.0,
                Steps::Colour(Box::new(move |s, j| {
                    let x = j as f32 / nf + s as f32 / 12.0;
                    a.lerp(b, 0.5 + 0.5 * (x * std::f32::consts::TAU).sin())
                })),
            )
        }
        ColourChase::Sparkle(i) => {
            let (a, b) = PAIRS[i % PAIRS.len()];
            (
                8,
                Crossfade::Half,
                180.0,
                Steps::Colour(Box::new(
                    move |s, j| if noise(s + 31 * i, j) > 0.75 { b } else { a },
                )),
            )
        }
        ColourChase::Intensity(i) => {
            let (s, x, b, f) = intensity(i % BUTTONS_PER_PAGE, n);
            (s, x, b, Steps::Level(f))
        }
    };
    let steps = (0..steps.max(1))
        .map(|s| {
            let mut look: Look = [Cell::default(); CHANNELS];
            for (j, &ch) in channels.iter().enumerate() {
                if ch < CHANNELS {
                    match &f {
                        Steps::Colour(f) => look[ch].colour = Some(f(s, j).hs()),
                        Steps::Level(f) => look[ch].level = Some(f(s, j).clamp(0.0, 1.0)),
                    }
                }
            }
            look
        })
        .collect();
    Chase {
        name: chase_name(c),
        steps,
        bpm,
        direction: Direction::Forward,
        mirror: false,
        crossfade,
        intensity_only: false,
        audio: false,
    }
}

/// The step that bounces 0, 1, … n-1, n-2, … 1 over 2(n-1) steps.
fn bounce(s: usize, n: usize) -> usize {
    if n < 2 {
        return 0;
    }
    let period = 2 * (n - 1);
    let s = s % period;
    if s < n { s } else { period - s }
}

fn special2(i: usize, n: usize) -> (usize, Crossfade, f32, StepColour) {
    let nf = n as f32;
    let centre = (n as f32 - 1.0) / 2.0;
    let dist = move |j: usize| (j as f32 - centre).abs() / (centre.max(0.5) * 2.0);
    match i {
        // Rainbow wave: a slow rainbow flowing along.
        0 => (
            24,
            Crossfade::Full,
            120.0,
            Box::new(move |s, j| Hs::new(j as f32 / nf + s as f32 / 24.0, 1.0).rgb()),
        ),
        // Colour wipe: each colour fills the selection in turn.
        1 => (
            n * 6,
            Crossfade::Snap,
            240.0,
            Box::new(move |s, j| {
                let (pos, c) = (s % n, s / n);
                let hue = if j <= pos { c } else { c + 5 };
                Hs::new((hue % 6) as f32 / 6.0, 1.0).rgb()
            }),
        ),
        // Spectrum split: mirrored rainbows from the centre.
        2 => (
            12,
            Crossfade::Full,
            90.0,
            Box::new(move |s, j| Hs::new(dist(j) + s as f32 / 12.0, 1.0).rgb()),
        ),
        // Complements: everything flips between a colour and its opposite.
        3 => (
            12,
            Crossfade::Snap,
            180.0,
            Box::new(|s, _| {
                let base = (s / 2) as f32 / 6.0;
                let h = if s % 2 == 0 { base } else { base + 0.5 };
                Hs::new(h, 1.0).rgb()
            }),
        ),
        // Ping pong: a magenta dot bouncing over cyan.
        4 => (
            (2 * n.saturating_sub(1)).max(2),
            Crossfade::Half,
            240.0,
            Box::new(move |s, j| if j == bounce(s, n) { MAGENTA } else { CYAN }),
        ),
        // Breathe: a drifting colour between full and pastel.
        5 => (
            12,
            Crossfade::Full,
            40.0,
            Box::new(|s, _| Hs::new(s as f32 / 12.0, if s % 2 == 0 { 1.0 } else { 0.3 }).rgb()),
        ),
        // Ripple: rainbow rings moving out from the centre.
        6 => (
            12,
            Crossfade::Full,
            120.0,
            Box::new(move |s, j| Hs::new(dist(j) - s as f32 / 12.0, 1.0).rgb()),
        ),
        // RGB rotate.
        7 => (
            3,
            Crossfade::Snap,
            120.0,
            Box::new(|s, j| [RED, GREEN, BLUE][(s + j) % 3]),
        ),
        // Thunder: white flashes over deep blue.
        8 => (
            12,
            Crossfade::Snap,
            300.0,
            Box::new(|s, j| if noise(s + 7, j) > 0.9 { WHITE } else { CONGO }),
        ),
        // Halloween
        9 => (
            8,
            Crossfade::Full,
            60.0,
            Box::new(move |s, j| {
                wave(
                    &[ORANGE, PURPLE, LIME, PURPLE],
                    j as f32 / nf + s as f32 / 8.0,
                )
            }),
        ),
        // Christmas
        10 => (
            3,
            Crossfade::Half,
            60.0,
            Box::new(|s, j| [RED, GREEN, WHITE][(s + j) % 3]),
        ),
        // Stars & Stripes
        11 => (
            3,
            Crossfade::Half,
            60.0,
            Box::new(|s, j| [RED, WHITE, BLUE][(s + j) % 3]),
        ),
        // Tropical
        12 => (
            8,
            Crossfade::Full,
            60.0,
            Box::new(move |s, j| wave(&[CYAN, LIME, PINK, YELLOW], j as f32 / nf + s as f32 / 8.0)),
        ),
        // Deep sea
        13 => (
            8,
            Crossfade::Full,
            30.0,
            Box::new(move |s, j| wave(&[CONGO, BLUE, TEAL, SKY], j as f32 / nf + s as f32 / 8.0)),
        ),
        // Galaxy: violets with white stars.
        14 => (
            8,
            Crossfade::Half,
            120.0,
            Box::new(move |s, j| {
                if noise(s + 51, j) > 0.85 {
                    WHITE
                } else {
                    wave(&[PURPLE, MAGENTA, CONGO], j as f32 / nf + s as f32 / 16.0)
                }
            }),
        ),
        // Sunrise: everything warms up together.
        15 => (
            4,
            Crossfade::Full,
            15.0,
            Box::new(|s, _| [CONGO, PINK, AMBER, WARM][s % 4]),
        ),
        // Quarter hues: four colours stepping round the wheel.
        16 => (
            8,
            Crossfade::Snap,
            120.0,
            Box::new(|s, j| Hs::new((j % 4) as f32 / 4.0 + s as f32 / 8.0, 1.0).rgb()),
        ),
        // Two-tone: magenta and amber breathing into each other.
        _ => (
            2,
            Crossfade::Full,
            30.0,
            Box::new(|s, j| if (s + j / 2) % 2 == 0 { MAGENTA } else { AMBER }),
        ),
    }
}

fn intensity(i: usize, n: usize) -> (usize, Crossfade, f32, StepLevel) {
    let nf = n as f32;
    let centre = (n as f32 - 1.0) / 2.0;
    let half = n.div_ceil(2);
    let ring = move |j: usize| (j as f32 - centre).abs().round() as usize;
    match i {
        // Comet: a bright head with a fading tail.
        0 => (
            n.max(2),
            Crossfade::Half,
            240.0,
            Box::new(move |s, j| {
                let behind = (s % n + n - j) % n;
                (1.0 - behind as f32 / 4.0).max(0.0)
            }),
        ),
        // Knight Rider: one light bouncing end to end.
        1 => (
            (2 * n.saturating_sub(1)).max(2),
            Crossfade::Half,
            300.0,
            Box::new(move |s, j| if j == bounce(s, n) { 1.0 } else { 0.0 }),
        ),
        // Chase: one on at a time.
        2 => (
            n.max(2),
            Crossfade::Snap,
            180.0,
            Box::new(move |s, j| if j == s % n { 1.0 } else { 0.0 }),
        ),
        // Chase pairs: two neighbours on.
        3 => (
            n.max(2),
            Crossfade::Snap,
            180.0,
            Box::new(move |s, j| {
                if j == s % n || j == (s + 1) % n {
                    1.0
                } else {
                    0.0
                }
            }),
        ),
        // Every third.
        4 => (
            3,
            Crossfade::Snap,
            120.0,
            Box::new(|s, j| if (j + 3 - s % 3) % 3 == 0 { 1.0 } else { 0.0 }),
        ),
        // Odd/Even
        5 => (
            2,
            Crossfade::Snap,
            120.0,
            Box::new(|s, j| if (s + j) % 2 == 0 { 1.0 } else { 0.0 }),
        ),
        // Pulse: everything on, then dim.
        6 => (
            2,
            Crossfade::Full,
            120.0,
            Box::new(|s, _| if s % 2 == 0 { 1.0 } else { 0.1 }),
        ),
        // Breathe: a slow swell.
        7 => (
            8,
            Crossfade::Full,
            30.0,
            Box::new(|s, _| 0.5 - 0.5 * (s as f32 / 8.0 * std::f32::consts::TAU).cos()),
        ),
        // Wave: a sine of brightness travelling along.
        8 => (
            12,
            Crossfade::Full,
            120.0,
            Box::new(move |s, j| {
                0.5 + 0.5 * ((j as f32 / nf + s as f32 / 12.0) * std::f32::consts::TAU).sin()
            }),
        ),
        // Flicker: candle-like.
        9 => (
            16,
            Crossfade::Half,
            480.0,
            Box::new(|s, j| 0.45 + 0.55 * noise(s, j + 100)),
        ),
        // Twinkle: a low glow with random sparkles.
        10 => (
            12,
            Crossfade::Half,
            180.0,
            Box::new(|s, j| if noise(s + 3, j) > 0.8 { 1.0 } else { 0.2 }),
        ),
        // Lightning: dark with rare flashes.
        11 => (
            16,
            Crossfade::Snap,
            300.0,
            Box::new(|s, j| if noise(s + 17, j / 2) > 0.9 { 1.0 } else { 0.0 }),
        ),
        // Fill & empty: build up one by one, then clear.
        12 => (
            2 * n,
            Crossfade::Snap,
            240.0,
            Box::new(move |s, j| {
                let s = s % (2 * n);
                let on = if s < n { j <= s } else { j > s - n };
                if on { 1.0 } else { 0.0 }
            }),
        ),
        // Centre out
        13 => (
            half.max(2),
            Crossfade::Half,
            180.0,
            Box::new(move |s, j| if ring(j) == s % half.max(1) { 1.0 } else { 0.0 }),
        ),
        // Outside in
        14 => (
            half.max(2),
            Crossfade::Half,
            180.0,
            Box::new(move |s, j| {
                let r = half.saturating_sub(1).saturating_sub(s % half.max(1));
                if ring(j) == r { 1.0 } else { 0.0 }
            }),
        ),
        // Heartbeat: two quick beats and a rest.
        15 => (
            8,
            Crossfade::Snap,
            480.0,
            Box::new(|s, _| [1.0, 0.15, 1.0, 0.15, 0.15, 0.15, 0.15, 0.15][s % 8]),
        ),
        // Strobe: on/off, everything together.
        16 => (
            2,
            Crossfade::Snap,
            600.0,
            Box::new(|s, _| if s % 2 == 0 { 1.0 } else { 0.0 }),
        ),
        // Random dots.
        _ => (
            12,
            Crossfade::Snap,
            180.0,
            Box::new(|s, j| if noise(s + 71, j) > 0.6 { 1.0 } else { 0.0 }),
        ),
    }
}

/// Swatch colours for a chase button preview (greys for intensity effects).
pub fn chase_preview(c: ColourChase) -> Vec<Rgb> {
    let chase = build_chase(c, &[0, 1, 2, 3, 4, 5]);
    (0..6)
        .map(|j| {
            let cell = chase.steps[0][j];
            match (cell.colour, cell.level) {
                (Some(c), _) => c.rgb(),
                (None, Some(l)) => Rgb::new(0.15 + 0.85 * l, 0.15 + 0.85 * l, 0.15 + 0.85 * l),
                _ => Rgb::WHITE,
            }
        })
        .collect()
}

fn special(i: usize, n: usize) -> (usize, Crossfade, f32, StepColour) {
    let nf = n as f32;
    match i {
        // Odd/Even
        0 => (
            2,
            Crossfade::Snap,
            60.0,
            Box::new(|s, j| if (s + j) % 2 == 0 { RED } else { BLUE }),
        ),
        // Crossfade: everything between two colours.
        1 => (
            2,
            Crossfade::Full,
            30.0,
            Box::new(|s, _| if s == 0 { RED } else { BLUE }),
        ),
        // Rainbow running along the selection.
        2 => (
            12,
            Crossfade::Full,
            120.0,
            Box::new(move |s, j| Hs::new(j as f32 / nf + s as f32 / 12.0, 1.0).rgb()),
        ),
        // Rainbow fade: all together.
        3 => (
            6,
            Crossfade::Full,
            30.0,
            Box::new(|s, _| Hs::new(s as f32 / 6.0, 1.0).rgb()),
        ),
        // Fire
        4 => (
            8,
            Crossfade::Half,
            240.0,
            Box::new(|s, j| wave(&[RED, ORANGE, AMBER, RED, ORANGE], noise(s, j))),
        ),
        // Water
        5 => (
            8,
            Crossfade::Full,
            60.0,
            Box::new(move |s, j| {
                wave(
                    &[BLUE, SKY, CYAN, TEAL, BLUE],
                    j as f32 / nf + s as f32 / 8.0,
                )
            }),
        ),
        // Twinkle: white glints on deep blue.
        6 => (
            8,
            Crossfade::Half,
            120.0,
            Box::new(|s, j| if noise(s, j) > 0.8 { WHITE } else { CONGO }),
        ),
        // Sparkle: white on magenta, snappier.
        7 => (
            8,
            Crossfade::Snap,
            240.0,
            Box::new(|s, j| {
                if noise(s + 99, j) > 0.75 {
                    WHITE
                } else {
                    MAGENTA
                }
            }),
        ),
        // Police: halves flash red and blue.
        8 => (
            4,
            Crossfade::Snap,
            240.0,
            Box::new(move |s, j| {
                let left = j < n.div_ceil(2);
                if left == (s % 2 == 0) { RED } else { BLUE }
            }),
        ),
        // Ice
        9 => (
            8,
            Crossfade::Full,
            60.0,
            Box::new(move |s, j| wave(&[WHITE, STEEL, CYAN, SKY], j as f32 / nf + s as f32 / 8.0)),
        ),
        // Forest
        10 => (
            8,
            Crossfade::Full,
            45.0,
            Box::new(move |s, j| wave(&[GREEN, LIME, TEAL, GREEN], j as f32 / nf + s as f32 / 8.0)),
        ),
        // Sunset
        11 => (
            8,
            Crossfade::Full,
            30.0,
            Box::new(move |s, j| {
                wave(
                    &[ORANGE, PINK, MAGENTA, PURPLE],
                    j as f32 / nf + s as f32 / 8.0,
                )
            }),
        ),
        // Candy
        12 => (
            3,
            Crossfade::Half,
            60.0,
            Box::new(|s, j| [PINK, CYAN, YELLOW][(s + j) % 3]),
        ),
        // Disco: random saturated colours.
        13 => (
            8,
            Crossfade::Snap,
            120.0,
            Box::new(|s, j| Hs::new((noise(s, j) * 12.0).floor() / 12.0, 1.0).rgb()),
        ),
        // RGB: all together.
        14 => (
            3,
            Crossfade::Snap,
            60.0,
            Box::new(|s, _| [RED, GREEN, BLUE][s % 3]),
        ),
        // Pastel rainbow.
        15 => (
            12,
            Crossfade::Full,
            60.0,
            Box::new(move |s, j| Hs::new(j as f32 / nf + s as f32 / 12.0, 0.4).rgb()),
        ),
        // Lava
        16 => (
            8,
            Crossfade::Full,
            40.0,
            Box::new(move |s, j| {
                wave(&[RED, ORANGE, RED, MAGENTA], j as f32 / nf + s as f32 / 8.0)
            }),
        ),
        // Aurora
        _ => (
            8,
            Crossfade::Full,
            30.0,
            Box::new(move |s, j| {
                wave(&[GREEN, TEAL, PURPLE, CYAN], j as f32 / nf + s as f32 / 8.0)
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_even_pattern() {
        let c = pattern_colours(Pattern::OddEven(0), 4);
        assert_eq!(c, vec![RED, BLUE, RED, BLUE]);
    }

    #[test]
    fn half_crossfade_gradient_thirds() {
        assert_eq!(
            pattern_colours(Pattern::Half(1), 5),
            vec![RED, RED, RED, WHITE, WHITE]
        );
        let x = pattern_colours(Pattern::Crossfade(0), 5);
        assert_eq!((x[0], x[2], x[4]), (RED, BLUE, RED));
        let g = pattern_colours(Pattern::Gradient(0), 3);
        assert_eq!((g[0], g[2]), (RED, BLUE));
        assert_eq!(
            pattern_colours(Pattern::Thirds(0), 4),
            vec![RED, GREEN, BLUE, RED]
        );
    }

    #[test]
    fn every_pattern_page_fills_any_selection() {
        assert_eq!(PATTERN_PAGES.len(), 10);
        for page in 0..PATTERN_PAGES.len() {
            for p in pattern_page(page) {
                for n in [1, 2, 7, 24] {
                    assert_eq!(pattern_colours(p, n).len(), n, "{p:?}");
                }
            }
        }
        // The second pages use the second 18 pairs.
        assert_eq!(pattern_page(1)[0], Pattern::OddEven(18));
        assert_eq!(pattern_name(Pattern::Special(0)), "Rainbow");
    }

    #[test]
    fn chases_cover_only_the_selection() {
        for c in all_chases() {
            for chans in [vec![2, 3, 5], vec![7], (0..24).collect()] {
                let chase = build_chase(c, &chans);
                assert!(!chase.steps.is_empty(), "{c:?}");
                for step in &chase.steps {
                    for (ch, cell) in step.iter().enumerate() {
                        assert_eq!(!cell.is_empty(), chans.contains(&ch), "{c:?}");
                        // Colour chases leave intensity alone; intensity ones leave colour.
                        let intensity = matches!(c, ColourChase::Intensity(_));
                        assert!(cell.level.is_none() || intensity);
                        assert!(cell.colour.is_none() || !intensity);
                    }
                }
            }
        }
        assert_eq!(all_chases().len(), 12 * BUTTONS_PER_PAGE);
    }

    #[test]
    fn snap_chase_moves_one_channel_per_step() {
        let chase = build_chase(ColourChase::Snap(0), &[0, 1, 2, 3]);
        assert_eq!(chase.steps.len(), 4);
        let first = |s: usize| {
            (0..4)
                .map(|j| chase.steps[s][j].colour.unwrap().rgb().r > 0.5)
                .collect::<Vec<_>>()
        };
        assert_eq!(first(0), vec![true, true, false, false]);
        assert_eq!(first(1), vec![true, false, false, true]);
    }

    #[test]
    fn comet_and_knight_rider() {
        let comet = build_chase(ColourChase::Intensity(0), &[0, 1, 2, 3, 4, 5]);
        let levels =
            |s: usize| -> Vec<f32> { (0..6).map(|j| comet.steps[s][j].level.unwrap()).collect() };
        assert_eq!(levels(2), vec![0.5, 0.75, 1.0, 0.0, 0.0, 0.25]);
        let kr = build_chase(ColourChase::Intensity(1), &[0, 1, 2]);
        assert_eq!(kr.steps.len(), 4);
        let lit = |s: usize| (0..3).find(|&j| kr.steps[s][j].level == Some(1.0)).unwrap();
        assert_eq!((lit(0), lit(1), lit(2), lit(3)), (0, 1, 2, 1));
    }

    #[test]
    fn swatches_and_library() {
        assert_eq!(DEFAULT_SWATCHES.len(), 54);
        assert!(LIBRARY.iter().all(|c| !c.name.is_empty()));
        assert_eq!(library_groups().len(), 9);
    }
}
