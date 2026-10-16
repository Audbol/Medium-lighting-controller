//! Colour maths. The console thinks in hue/saturation (what the dials set) and
//! mixes in RGB (what the LEDs need).

use serde::{Deserialize, Serialize};

/// Hue and saturation, both 0..1. Hue 0 is red; saturation 0 is white.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hs {
    pub h: f32,
    pub s: f32,
}

pub const WHITE: Hs = Hs { h: 0.0, s: 0.0 };

/// Linear RGB, each 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const WHITE: Rgb = Rgb::new(1.0, 1.0, 1.0);

    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn to_u8(self) -> [u8; 3] {
        let c = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        [c(self.r), c(self.g), c(self.b)]
    }

    pub fn scale(self, k: f32) -> Self {
        Self::new(self.r * k, self.g * k, self.b * k)
    }

    pub fn lerp(self, other: Rgb, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self::new(l(self.r, other.r), l(self.g, other.g), l(self.b, other.b))
    }

    pub fn hs(self) -> Hs {
        rgb_to_hs(self)
    }
}

impl Hs {
    pub const fn new(h: f32, s: f32) -> Self {
        Self { h, s }
    }

    pub fn rgb(self) -> Rgb {
        hs_to_rgb(self)
    }

    /// Fades hue the short way round the wheel. A white end takes the other end's
    /// hue, so a fade from white to blue doesn't pass through red.
    pub fn lerp(self, other: Hs, t: f32) -> Hs {
        // Exact ends: a colour-temperature hue of 1.0 must not wrap to 0.
        if t >= 1.0 {
            return other;
        }
        if t <= 0.0 {
            return self;
        }
        let (mut a, mut b) = (self.h, other.h);
        if self.s < 0.01 {
            a = b;
        } else if other.s < 0.01 {
            b = a;
        }
        let mut d = b - a;
        if d > 0.5 {
            d -= 1.0;
        } else if d < -0.5 {
            d += 1.0;
        }
        Hs::new((a + d * t).rem_euclid(1.0), self.s + (other.s - self.s) * t)
    }
}

/// Full-value HSV to RGB.
pub fn hs_to_rgb(c: Hs) -> Rgb {
    let h = c.h.rem_euclid(1.0) * 6.0;
    let s = c.s.clamp(0.0, 1.0);
    let f = h.fract();
    let (p, q, t) = (1.0 - s, 1.0 - s * f, 1.0 - s * (1.0 - f));
    match h as u32 {
        0 => Rgb::new(1.0, t, p),
        1 => Rgb::new(q, 1.0, p),
        2 => Rgb::new(p, 1.0, t),
        3 => Rgb::new(p, q, 1.0),
        4 => Rgb::new(t, p, 1.0),
        _ => Rgb::new(1.0, p, q),
    }
}

/// RGB to hue/saturation; brightness is dropped (that's the fader's job).
pub fn rgb_to_hs(c: Rgb) -> Hs {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    if max <= 0.0 {
        return WHITE;
    }
    let d = max - min;
    let s = d / max;
    if d <= 1e-6 {
        return Hs::new(0.0, 0.0);
    }
    let h = if max == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    Hs::new(h / 6.0, s)
}

/// Approximate colour of a warm (0) to cool (1) white, for on-screen mimics.
pub fn white_temperature(ct: f32) -> Rgb {
    let warm = Rgb::new(1.0, 0.72, 0.42);
    let cool = Rgb::new(0.82, 0.9, 1.0);
    warm.lerp(cool, ct.clamp(0.0, 1.0))
}

/// The six colours the Hue dial steps through when pressed.
pub const HUE_STEPS: [f32; 6] = [0.0, 1.0 / 6.0, 2.0 / 6.0, 3.0 / 6.0, 4.0 / 6.0, 5.0 / 6.0];

/// The next of the six press colours after `h`.
pub fn next_hue_step(h: f32) -> f32 {
    let h = h.rem_euclid(1.0);
    HUE_STEPS
        .iter()
        .copied()
        .find(|&x| x > h + 0.01)
        .unwrap_or(HUE_STEPS[0])
}

/// Full → 50% → 0% → full, as the Saturation and Shift dials do when pressed.
pub fn next_press_level(v: f32) -> f32 {
    if v > 0.75 {
        0.5
    } else if v > 0.25 {
        0.0
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Rgb, b: Rgb) -> bool {
        (a.r - b.r).abs() < 1e-3 && (a.g - b.g).abs() < 1e-3 && (a.b - b.b).abs() < 1e-3
    }

    #[test]
    fn primaries() {
        assert!(close(Hs::new(0.0, 1.0).rgb(), Rgb::new(1.0, 0.0, 0.0)));
        assert!(close(
            Hs::new(1.0 / 3.0, 1.0).rgb(),
            Rgb::new(0.0, 1.0, 0.0)
        ));
        assert!(close(
            Hs::new(2.0 / 3.0, 1.0).rgb(),
            Rgb::new(0.0, 0.0, 1.0)
        ));
        assert!(close(WHITE.rgb(), Rgb::WHITE));
        assert!(close(Hs::new(0.0, 0.5).rgb(), Rgb::new(1.0, 0.5, 0.5)));
    }

    #[test]
    fn round_trip() {
        for i in 0..36 {
            for s in [0.25, 0.5, 1.0] {
                let c = Hs::new(i as f32 / 36.0, s);
                let back = c.rgb().hs();
                assert!((back.s - s).abs() < 1e-3);
                let dh = (back.h - c.h).abs();
                assert!(dh < 1e-3 || (1.0 - dh) < 1e-3, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn hue_fades_the_short_way() {
        let red = Hs::new(0.02, 1.0);
        let magenta = Hs::new(0.9, 1.0);
        let mid = red.lerp(magenta, 0.5);
        assert!(mid.h > 0.9 || mid.h < 0.02, "{mid:?}");
        // From white the hue jumps straight to the target.
        let blue = Hs::new(2.0 / 3.0, 1.0);
        assert!((WHITE.lerp(blue, 0.3).h - blue.h).abs() < 1e-6);
    }

    #[test]
    fn press_cycles() {
        assert_eq!(next_hue_step(0.0), 1.0 / 6.0);
        assert_eq!(next_hue_step(0.9), 0.0);
        assert_eq!(next_press_level(1.0), 0.5);
        assert_eq!(next_press_level(0.5), 0.0);
        assert_eq!(next_press_level(0.0), 1.0);
    }
}
