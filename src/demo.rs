//! The show loaded on first run: a mixed LED rig patched across both fixture
//! pages (Shehds RGBWA+UV pars and 7x40W moving heads up front), with a few
//! scenes, chases and a short cuelist to play with.

use crate::colour::{Hs, Rgb};
use crate::effects::{self, ColourChase, Pattern};
use crate::engine::{CHANNELS, Cell, Cue, CueStart, Item, Look, Scene, Show, empty_look};
use crate::fixtures::{Patched, find};

/// (manufacturer, fixture type, channels it's patched to)
const RIG: [(&str, &str, std::ops::Range<usize>); 7] = [
    ("Shehds", "LED Par 18x18W · 10ch", 0..8),
    ("Shehds", "7x40W Bee Eye Zoom Wash · 15ch", 8..10),
    ("Generic", "I CW/WW", 10..12),
    ("Generic", "RGB", 12..20),
    ("Generic", "RGBAW", 20..22),
    ("Generic", "Dimmer", 22..23),
    ("Generic", "Strobe (I Rate)", 23..24),
];

pub fn demo_patch() -> Vec<Option<Patched>> {
    let mut patch = vec![None; CHANNELS];
    let mut address = 1u16;
    for (maker, name, chans) in RIG {
        let def = find(maker, name).expect("demo fixture in the library");
        for ch in chans {
            let n = def.footprint();
            patch[ch] = Some(Patched {
                def: def.clone(),
                addresses: vec![address],
            });
            address += n;
        }
    }
    patch
}

fn look(chans: impl IntoIterator<Item = usize>, f: impl Fn(usize, usize) -> (f32, Rgb)) -> Look {
    let mut l = empty_look();
    for (j, ch) in chans.into_iter().enumerate() {
        let (level, c) = f(j, ch);
        l[ch] = Cell {
            level: Some(level),
            colour: Some(c.hs()),
            ..Default::default()
        };
    }
    l
}

fn scene(name: &str, look: Look, fade: Option<f32>) -> Option<Item> {
    Some(Item::Scene(Scene {
        name: name.into(),
        look,
        fade,
    }))
}

fn pattern(p: Pattern, chans: std::ops::Range<usize>, level: f32) -> Look {
    let colours = effects::pattern_colours(p, chans.len());
    look(chans, |j, _| (level, colours[j]))
}

pub fn demo_show() -> Show {
    let mut show = Show {
        patch: demo_patch(),
        ..Default::default()
    };
    let warm = Rgb::new(1.0, 0.75, 0.45);
    let blue = Hs::new(0.62, 1.0).rgb();

    show.scenes[0] = scene(
        "Warm wash",
        look((0..12).chain(20..23), |_, ch| {
            (if ch == 22 { 0.8 } else { 0.85 }, warm)
        }),
        None,
    );
    show.scenes[1] = scene(
        "Blue state",
        look(0..20, |_, ch| (if ch < 8 { 0.7 } else { 0.5 }, blue)),
        Some(4.0),
    );
    show.scenes[2] = scene("Red & Blue", pattern(Pattern::OddEven(0), 0..8, 1.0), None);
    show.scenes[3] = scene("Rainbow", pattern(Pattern::Special(0), 12..20, 1.0), None);

    // C1: a dot of white running along the battens (an auto chase).
    let dot = look(12..20, |j, _| {
        if j == 0 {
            (1.0, Rgb::WHITE)
        } else {
            (0.15, Hs::new(0.75, 1.0).rgb())
        }
    });
    let steps = (0..8)
        .map(|k| {
            let mut s = empty_look();
            for j in 0..8 {
                s[12 + j] = dot[12 + (j + 8 - k) % 8];
            }
            s
        })
        .collect();
    let mut c1 = effects::build_chase(ColourChase::Special(2), &[]);
    c1.name = "Batten runner".into();
    c1.steps = steps;
    c1.bpm = 240.0;
    c1.crossfade = crate::engine::Crossfade::Half;
    show.chases[0] = Some(Item::Chase(c1));

    // C2-C4: colour chases with their own intensity.
    let with_level = |c: ColourChase, chans: Vec<usize>, level: f32| {
        let mut chase = effects::build_chase(c, &chans);
        for step in &mut chase.steps {
            for &ch in &chans {
                step[ch].level = Some(level);
            }
        }
        Some(Item::Chase(chase))
    };
    show.chases[1] = with_level(ColourChase::Special(2), (0..8).chain(12..20).collect(), 1.0);
    show.chases[2] = with_level(ColourChase::Special(4), (0..8).collect(), 0.9);
    show.chases[3] = with_level(ColourChase::Special(8), (12..20).collect(), 1.0);

    // Yellow scene page: one fader per look.
    show.faders[0][0] = scene("Front warm", look(0..8, |_, _| (1.0, warm)), None);
    show.faders[0][1] = scene(
        "Movers cyan",
        look(8..10, |_, _| (1.0, Hs::new(0.5, 1.0).rgb())),
        None,
    );
    show.faders[0][2] = scene("White light", look(10..12, |_, _| (1.0, Rgb::WHITE)), None);
    show.faders[0][3] = scene(
        "Battens magenta",
        look(12..20, |_, _| (1.0, Hs::new(0.85, 1.0).rgb())),
        None,
    );
    show.faders[0][4] = scene("Crossfade", pattern(Pattern::Crossfade(6), 0..8, 1.0), None);
    show.faders[0][11] = scene("Strobe", look(23..24, |_, _| (1.0, Rgb::WHITE)), None);

    // Moving heads: a figure-eight sweep, and a look aimed at the audience.
    let sweep = (0..8)
        .map(|k| {
            let a = k as f32 / 8.0 * std::f32::consts::TAU;
            let mut step = empty_look();
            for (j, ch) in (8..10).enumerate() {
                let dir = if j == 0 { 1.0 } else { -1.0 };
                step[ch].pos = [
                    Some(0.5 + 0.18 * a.sin() * dir),
                    Some(0.5 + 0.15 * (2.0 * a).sin()),
                ];
            }
            step
        })
        .collect();
    let mut mover_chase = effects::build_chase(ColourChase::Special(0), &[]);
    mover_chase.name = "Mover sweep".into();
    mover_chase.steps = sweep;
    mover_chase.bpm = 60.0;
    mover_chase.crossfade = crate::engine::Crossfade::Full;
    show.faders[0][5] = Some(Item::Chase(mover_chase));
    let mut audience = look(8..10, |_, _| (1.0, Rgb::new(1.0, 0.85, 0.6)));
    for (j, ch) in (8..10).enumerate() {
        audience[ch].pos = [Some(if j == 0 { 0.45 } else { 0.55 }), Some(0.85)];
        audience[ch].alt[0] = Some(1.0); // wide zoom
    }
    show.faders[0][6] = scene("Movers to audience", audience, None);
    // UV: the Shehds pars' UV LEDs are on Shift+Hue.
    let mut blacklight = look(0..8, |_, _| (1.0, Hs::new(0.72, 1.0).rgb()));
    for cell in blacklight.iter_mut().take(8) {
        cell.alt[0] = Some(1.0);
    }
    show.faders[0][7] = scene("Blacklight", blacklight, None);

    let cue = |name: &str, look: Look, fade: Option<f32>, start: CueStart, wait: f32| Cue {
        name: name.into(),
        look,
        start,
        wait,
        fade,
    };
    show.cuelist = vec![
        cue(
            "Preset",
            look(20..23, |_, _| (0.5, Rgb::new(0.4, 0.4, 1.0))),
            Some(3.0),
            CueStart::Halt,
            0.0,
        ),
        cue(
            "Walk in",
            look((0..8).chain(20..23), |_, _| (0.8, warm)),
            Some(5.0),
            CueStart::Halt,
            0.0,
        ),
        cue(
            "Sunset",
            pattern(Pattern::Crossfade(11), 0..8, 1.0),
            Some(8.0),
            CueStart::Halt,
            0.0,
        ),
        cue(
            "Night",
            look(0..20, |_, ch| (if ch < 8 { 0.4 } else { 0.2 }, blue)),
            Some(6.0),
            CueStart::Follow,
            4.0,
        ),
        cue("Blackout", empty_look(), Some(3.0), CueStart::Halt, 0.0),
    ];
    show
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Slot;

    #[test]
    fn rig_fits_one_universe_without_overlap() {
        let patch = demo_patch();
        let mut used = [false; 512];
        for p in patch.iter().flatten() {
            for &a in &p.addresses {
                for i in 0..p.def.footprint() {
                    let slot = (a + i - 1) as usize;
                    assert!(slot < 512);
                    assert!(!used[slot], "overlap at {}", slot + 1);
                    used[slot] = true;
                }
            }
        }
        assert!(patch.iter().all(Option::is_some));
    }

    #[test]
    fn demo_show_plays() {
        let mut d = crate::engine::Desk::new(demo_show(), Default::default());
        d.toggle_slot(Slot::Chase(1));
        d.cue_go();
        d.tick(0.0);
        d.tick(10.0);
        assert!(d.out.iter().any(|o| o.level > 0.5));
    }
}
