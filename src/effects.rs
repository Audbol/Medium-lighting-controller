//! The Colour window's built-in content: 18 swatches, three pages of two-colour
//! patterns (plus Rainbow) and four pages of colour chases. Chases built here
//! carry colour only, so the faders keep control of intensity.

use crate::colour::{Hs, Rgb};
use crate::engine::{CHANNELS, Cell, Chase, Crossfade, Direction, Look};

pub const SWATCHES: usize = 18;
pub const BUTTONS_PER_PAGE: usize = 18;

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
    RED, ORANGE, AMBER, YELLOW, LIME, GREEN, TEAL, CYAN, SKY, BLUE, CONGO, PURPLE, MAGENTA, PINK,
    LAVENDER, STEEL, WARM, WHITE,
];

/// The colour pairs used by the pattern pages and chase pages 2-4.
pub const PAIRS: [(Rgb, Rgb); BUTTONS_PER_PAGE] = [
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
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// Page 1: one colour on odd channels, the other on even.
    OddEven(usize),
    /// Page 2: first half one colour, second half the other.
    Half(usize),
    /// Page 3: one colour at both ends crossfading to the other in the centre.
    Crossfade(usize),
    Rainbow,
}

pub const PATTERN_PAGES: [&str; 3] = ["Odds & Evens", "Half & Half", "Crossfade"];

pub fn pattern_page(page: usize) -> Vec<Pattern> {
    let mut v: Vec<Pattern> = (0..BUTTONS_PER_PAGE)
        .map(|i| match page {
            0 => Pattern::OddEven(i),
            1 => Pattern::Half(i),
            _ => Pattern::Crossfade(i),
        })
        .collect();
    if page == 2 {
        v.push(Pattern::Rainbow);
    }
    v
}

/// Colour of each of `n` selected channels, in selection order.
pub fn pattern_colours(p: Pattern, n: usize) -> Vec<Rgb> {
    (0..n)
        .map(|j| match p {
            Pattern::OddEven(i) => {
                // Channel numbers count from 1: the first is odd.
                if j % 2 == 0 { PAIRS[i].0 } else { PAIRS[i].1 }
            }
            Pattern::Half(i) => {
                if j < n.div_ceil(2) {
                    PAIRS[i].0
                } else {
                    PAIRS[i].1
                }
            }
            Pattern::Crossfade(i) => {
                let t = if n < 2 {
                    0.0
                } else {
                    1.0 - ((j as f32 / (n - 1) as f32) * 2.0 - 1.0).abs()
                };
                PAIRS[i].0.lerp(PAIRS[i].1, t)
            }
            Pattern::Rainbow => Hs::new(j as f32 / n.max(1) as f32, 1.0).rgb(),
        })
        .collect()
}

/// Two-colour preview for a pattern or chase button.
pub fn pattern_preview(p: Pattern) -> Vec<Rgb> {
    pattern_colours(p, 6)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourChase {
    /// Page 1: rainbow, fire, water, twinkle and friends.
    Special(usize),
    /// Page 2: the pair swaps between odd and even channels.
    OddEven(usize),
    /// Page 3: halves of the two colours crossfading along the selection.
    Crossfade(usize),
    /// Page 4: halves of the two colours snapping along the selection.
    Snap(usize),
}

pub const CHASE_PAGES: [&str; 4] = ["Specials", "Odds & Evens", "Crossfade", "Snap"];

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

pub fn chase_page(page: usize) -> Vec<ColourChase> {
    (0..BUTTONS_PER_PAGE)
        .map(|i| match page {
            0 => ColourChase::Special(i),
            1 => ColourChase::OddEven(i),
            2 => ColourChase::Crossfade(i),
            _ => ColourChase::Snap(i),
        })
        .collect()
}

/// Every built-in chase, in the order Autopilot plays them.
pub fn all_chases() -> Vec<ColourChase> {
    (0..CHASE_PAGES.len()).flat_map(chase_page).collect()
}

pub fn chase_name(c: ColourChase) -> String {
    match c {
        ColourChase::Special(i) => SPECIALS[i].to_string(),
        ColourChase::OddEven(i) => format!("Odd/Even {}", i + 1),
        ColourChase::Crossfade(i) => format!("Crossfade {}", i + 1),
        ColourChase::Snap(i) => format!("Snap {}", i + 1),
    }
}

/// A small deterministic hash for the "random" chases.
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

/// Builds a colour chase over `channels` (in selection order).
pub fn build_chase(c: ColourChase, channels: &[usize]) -> Chase {
    let n = channels.len().max(1);
    let (steps, crossfade, bpm, colour): (usize, Crossfade, f32, StepColour) = match c {
        ColourChase::Special(i) => special(i, n),
        ColourChase::OddEven(i) => {
            let (a, b) = PAIRS[i];
            (
                2,
                Crossfade::Snap,
                60.0,
                Box::new(move |s, j| if (s + j) % 2 == 0 { a } else { b }),
            )
        }
        ColourChase::Crossfade(i) | ColourChase::Snap(i) => {
            let (a, b) = PAIRS[i];
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
                Box::new(move |s, j| if (j + s) % n < half { a } else { b }),
            )
        }
    };
    let steps = (0..steps)
        .map(|s| {
            let mut look: Look = [Cell::default(); CHANNELS];
            for (j, &ch) in channels.iter().enumerate() {
                if ch < CHANNELS {
                    look[ch].colour = Some(colour(s, j).hs());
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

type StepColour = Box<dyn Fn(usize, usize) -> Rgb>;

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

/// Swatch colours for a chase button preview.
pub fn chase_preview(c: ColourChase) -> Vec<Rgb> {
    let chase = build_chase(c, &[0, 1, 2, 3, 4, 5]);
    (0..6)
        .map(|j| chase.steps[0][j].colour.map_or(Rgb::WHITE, |c| c.rgb()))
        .collect()
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
    fn half_and_crossfade_patterns() {
        assert_eq!(
            pattern_colours(Pattern::Half(1), 5),
            vec![RED, RED, RED, WHITE, WHITE]
        );
        let x = pattern_colours(Pattern::Crossfade(0), 5);
        assert_eq!(x[0], RED);
        assert_eq!(x[4], RED);
        assert_eq!(x[2], BLUE);
    }

    #[test]
    fn chases_cover_only_the_selection() {
        for c in all_chases() {
            let chase = build_chase(c, &[2, 3, 5]);
            assert!(!chase.steps.is_empty(), "{c:?}");
            for step in &chase.steps {
                for (ch, cell) in step.iter().enumerate() {
                    assert_eq!(cell.colour.is_some(), [2, 3, 5].contains(&ch), "{c:?}");
                    assert!(cell.level.is_none());
                }
            }
        }
        assert_eq!(all_chases().len(), 72);
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
}
