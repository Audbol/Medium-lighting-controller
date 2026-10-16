//! Converts Open Fixture Library fixture files (https://open-fixture-library.org,
//! MIT licence) into console fixtures, one per DMX mode.
//!
//! Each OFL channel becomes the feature the console can drive (intensity, a
//! colour, UV, strobe, zoom, pan/tilt…), and everything else becomes an Other
//! channel held at a safe value: its default, or the start of its "no function"
//! range. The same converter builds the bundled library (`assets/fixtures.json`)
//! and imports files from the Library screen.

use std::collections::HashMap;

use serde_json::Value;

use crate::fixtures::{ChannelDef, Feature, FixtureDef};

/// The largest mode worth putting on a fader strip.
pub const MAX_CHANNELS: usize = 48;

fn dmx_value(v: &Value, shift: u32) -> Option<u32> {
    let raw = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => {
            // "50%" style values.
            let p: f64 = s.trim_end_matches('%').parse().ok()?;
            return Some((p / 100.0 * 255.0).round().clamp(0.0, 255.0) as u32);
        }
        _ => return None,
    };
    Some(((raw as u64) >> (8 * shift)).min(255) as u32)
}

fn capabilities(ch: &Value) -> Vec<&Value> {
    if let Some(c) = ch.get("capability") {
        vec![c]
    } else {
        ch.get("capabilities")
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }
}

fn range(cap: &Value, shift: u32) -> Option<(u8, u8)> {
    let r = cap.get("dmxRange")?.as_array()?;
    Some((
        dmx_value(r.first()?, shift)? as u8,
        dmx_value(r.get(1)?, shift)? as u8,
    ))
}

fn kind(cap: &Value) -> &str {
    cap.get("type").and_then(Value::as_str).unwrap_or("")
}

fn colour_feature(name: &str) -> Option<Feature> {
    Some(match name {
        "Red" => Feature::Red,
        "Green" => Feature::Green,
        "Blue" => Feature::Blue,
        "White" => Feature::White,
        "Amber" => Feature::Amber,
        "UV" => Feature::Uv,
        "Cyan" => Feature::Cyan,
        "Magenta" => Feature::Magenta,
        "Yellow" => Feature::Yellow,
        "Warm White" => Feature::WarmWhite,
        "Cold White" => Feature::CoolWhite,
        _ => return None,
    })
}

/// Maps one OFL channel. `shift` scales 16/24-bit ranges down to 8 bits.
fn map_channel(name: &str, ch: &Value, shift: u32) -> ChannelDef {
    let caps = capabilities(ch);
    let full = |c: &&Value| range(c, shift).is_none_or(|(a, b)| a == 0 && b == 255);
    let default = ch
        .get("defaultValue")
        .and_then(|v| dmx_value(v, shift))
        .map(|v| v as u8)
        .or_else(|| {
            // Park "function" channels in their no-function range.
            caps.iter()
                .find(|c| kind(c) == "NoFunction")
                .and_then(|c| range(c, shift))
                .map(|r| r.0)
        })
        .unwrap_or(0);
    let other = |default: u8| ChannelDef {
        default,
        label: name.to_string(),
        ..ChannelDef::new(Feature::Other)
    };
    let with_range = |feature: Feature, cap: &Value| {
        let (min, max) = range(cap, shift).unwrap_or((0, 255));
        ChannelDef {
            min,
            max,
            ..ChannelDef::new(feature)
        }
    };
    let Some(first) = caps.first() else {
        return other(default);
    };

    // Strobe: an Open range (the default) plus a Strobe/Pulse range for the dial.
    let strobes: Vec<&&Value> = caps
        .iter()
        .filter(|c| {
            kind(c) == "ShutterStrobe"
                && matches!(
                    c.get("shutterEffect").and_then(Value::as_str),
                    Some("Strobe" | "Pulse" | "RampUp" | "RampDown" | "RampUpDown" | "Lightning")
                )
        })
        .collect();
    if let Some(strobe) = strobes.first() {
        let open = caps
            .iter()
            .find(|c| {
                kind(c) == "ShutterStrobe"
                    && c.get("shutterEffect").and_then(Value::as_str) == Some("Open")
            })
            .and_then(|c| range(c, shift))
            .map(|r| r.0);
        // A strobe channel that's closed at 0 and has no open range can't be used safely.
        if open.is_some()
            || caps.len() == 1
            || !caps
                .iter()
                .any(|c| c.get("shutterEffect").and_then(Value::as_str) == Some("Closed"))
        {
            let mut c = with_range(Feature::Strobe, strobe);
            c.default = open.unwrap_or(0);
            return c;
        }
        return other(open.unwrap_or(default));
    }

    match kind(first) {
        "Intensity"
            if caps.len() == 1
                || full(first)
                || range(first, shift).is_some_and(|(a, b)| a == 0 && b >= 127) =>
        {
            with_range(Feature::Intensity, first)
        }
        "ColorIntensity" if caps.len() == 1 => {
            match first
                .get("color")
                .and_then(Value::as_str)
                .and_then(colour_feature)
            {
                Some(f) => with_range(f, first),
                None => other(default),
            }
        }
        "Pan" if caps.len() == 1 => ChannelDef {
            default: ch
                .get("defaultValue")
                .and_then(|v| dmx_value(v, shift))
                .map_or(128, |v| v as u8),
            ..with_range(Feature::Pan, first)
        },
        "Tilt" if caps.len() == 1 => ChannelDef {
            default: ch
                .get("defaultValue")
                .and_then(|v| dmx_value(v, shift))
                .map_or(128, |v| v as u8),
            ..with_range(Feature::Tilt, first)
        },
        "Zoom" if caps.len() == 1 => with_range(Feature::Zoom, first),
        "PanTiltSpeed" => ChannelDef {
            default,
            label: name.to_string(),
            ..ChannelDef::new(Feature::Speed)
        },
        "ColorTemperature" if caps.len() == 1 => {
            let mut c = with_range(Feature::ColourTemp, first);
            // The console's dial runs warm → cool; flip ranges that start cool.
            let start = first
                .get("colorTemperatureStart")
                .and_then(Value::as_str)
                .unwrap_or("");
            let end = first
                .get("colorTemperatureEnd")
                .and_then(Value::as_str)
                .unwrap_or("");
            let kelvin = |s: &str| s.trim_end_matches('K').parse::<f64>().ok();
            let cool_first = match (kelvin(start), kelvin(end)) {
                (Some(a), Some(b)) => a > b,
                _ => start == "cold" || start == "cool",
            };
            if cool_first {
                std::mem::swap(&mut c.min, &mut c.max);
            }
            c
        }
        _ => other(default),
    }
}

/// One fixture per mode. `manufacturer` is the display name.
pub fn convert(json: &str, manufacturer: &str) -> Result<Vec<FixtureDef>, String> {
    let fixture: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let name = fixture
        .get("name")
        .and_then(Value::as_str)
        .ok_or("not an Open Fixture Library fixture (no name)")?;
    let empty = serde_json::Map::new();
    let available = fixture
        .get("availableChannels")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    // Pixel templates like "Red $pixelKey": a bar's pixels all follow one channel.
    let templates = fixture
        .get("templateChannels")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    if available.is_empty() && templates.is_empty() {
        return Err("no availableChannels".into());
    }

    // Every name a mode can reference: channels and their fine aliases.
    let mut channels: HashMap<String, ChannelDef> = HashMap::new();
    let mut patterns: Vec<(String, String, ChannelDef)> = Vec::new();
    for (key, ch) in templates {
        if let Some((pre, post)) = key.split_once("$pixelKey") {
            patterns.push((pre.to_string(), post.to_string(), map_channel(key, ch, 0)));
        }
    }
    let template = |s: &str| {
        patterns
            .iter()
            .find(|(pre, post, _)| {
                s.len() > pre.len() + post.len()
                    && s.starts_with(pre.as_str())
                    && s.ends_with(post.as_str())
            })
            .map(|(_, _, c)| {
                let mut c = c.clone();
                if c.feature == Feature::Other {
                    c.label = s.to_string();
                }
                c
            })
    };
    for (key, ch) in available {
        let fines: Vec<&str> = ch
            .get("fineChannelAliases")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let shift = match ch.get("dmxValueResolution").and_then(Value::as_str) {
            Some("8bit") => 0,
            Some("16bit") => 1,
            Some("24bit") => 2,
            _ => fines.len() as u32,
        };
        let coarse = map_channel(key, ch, shift);
        for alias in &fines {
            let fine = if coarse.feature == Feature::Other {
                ChannelDef {
                    label: alias.to_string(),
                    ..ChannelDef::new(Feature::Other)
                }
            } else {
                ChannelDef {
                    fine: true,
                    ..ChannelDef::new(coarse.feature)
                }
            };
            channels.insert(alias.to_string(), fine);
        }
        channels.insert(key.clone(), coarse);
    }

    let modes = fixture
        .get("modes")
        .and_then(Value::as_array)
        .ok_or("no modes")?;
    let mut out = Vec::new();
    for mode in modes {
        let mode_name = mode
            .get("shortName")
            .or_else(|| mode.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("mode");
        let Some(list) = mode.get("channels").and_then(Value::as_array) else {
            continue;
        };
        let mut defs = Vec::new();
        let mut ok = true;
        for entry in list {
            match entry {
                Value::Null => defs.push(ChannelDef {
                    label: "(unused)".into(),
                    ..ChannelDef::new(Feature::Other)
                }),
                Value::String(s) => match channels.get(s).cloned().or_else(|| template(s)) {
                    Some(c) => defs.push(c),
                    // Switching channels and anything unknown: hold at 0.
                    None if !s.contains('$') => defs.push(ChannelDef {
                        label: s.clone(),
                        ..ChannelDef::new(Feature::Other)
                    }),
                    None => ok = false,
                },
                // Pixel-matrix inserts aren't supported.
                _ => ok = false,
            }
        }
        if !ok || defs.is_empty() || defs.len() > MAX_CHANNELS {
            continue;
        }
        if defs.iter().all(|c| c.feature == Feature::Other) {
            continue;
        }
        out.push(FixtureDef::new(
            manufacturer,
            &format!("{name} · {mode_name}"),
            defs,
        ));
    }
    if out.is_empty() {
        return Err(format!("{name}: no usable DMX modes"));
    }
    Ok(out)
}

/// Tidies OFL manufacturer names for the library list.
#[cfg(test)]
pub fn display_manufacturer(name: &str) -> String {
    match name {
        "cameo" => "Cameo".into(),
        "U`King" => "U'King".into(),
        other => other.into(),
    }
}

/// A display name from an OFL manufacturer key (`chauvet-dj` → `Chauvet Dj`).
pub fn manufacturer_from_key(key: &str) -> String {
    key.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::Dial;

    const PAR: &str = r#"{
      "name": "Test Par",
      "categories": ["Color Changer"],
      "availableChannels": {
        "Dimmer": { "capability": { "type": "Intensity" } },
        "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } },
        "Green": { "capability": { "type": "ColorIntensity", "color": "Green" } },
        "Blue": { "capability": { "type": "ColorIntensity", "color": "Blue" } },
        "UV": { "capability": { "type": "ColorIntensity", "color": "UV" } },
        "Strobe": { "capabilities": [
          { "dmxRange": [0, 9], "type": "ShutterStrobe", "shutterEffect": "Closed" },
          { "dmxRange": [10, 19], "type": "ShutterStrobe", "shutterEffect": "Open" },
          { "dmxRange": [20, 255], "type": "ShutterStrobe", "shutterEffect": "Strobe" }
        ]},
        "Program": { "capabilities": [
          { "dmxRange": [0, 50], "type": "NoFunction" },
          { "dmxRange": [51, 255], "type": "Effect", "effectName": "Auto" }
        ]},
        "Pan": { "fineChannelAliases": ["Pan fine"], "capability": { "type": "Pan", "angleStart": "0deg", "angleEnd": "540deg" } },
        "Speed": { "defaultValue": 0, "capability": { "type": "PanTiltSpeed", "speedStart": "fast", "speedEnd": "slow" } }
      },
      "modes": [
        { "name": "5-channel", "shortName": "5ch", "channels": ["Dimmer", "Red", "Green", "Blue", "UV"] },
        { "name": "Full", "shortName": "11ch", "channels": ["Dimmer", "Red", "Green", "Blue", "UV", "Strobe", "Program", "Pan", "Pan fine", "Speed", null] },
        { "name": "Pixels", "channels": [{ "insert": "matrixChannels" }] }
      ]
    }"#;

    #[test]
    fn converts_modes_and_features() {
        let fixtures = convert(PAR, "Tester").unwrap();
        assert_eq!(fixtures.len(), 2); // The matrix mode is skipped.
        assert_eq!(fixtures[0].name, "Test Par · 5ch");
        assert_eq!(fixtures[0].manufacturer, "Tester");
        let full = &fixtures[1];
        assert_eq!(full.summary(), "I R G B UV Strb — Pan Panf Spd —");
        let strobe = &full.channels[5];
        assert_eq!((strobe.default, strobe.min, strobe.max), (10, 20, 255));
        assert_eq!(full.channels[6].default, 0);
        assert_eq!(full.channels[6].label, "Program");
        assert_eq!(full.channels[7].default, 128);
        assert!(full.channels[8].fine);
        assert_eq!(full.dial_of(&full.channels[4]), Dial::ShiftHue);
        assert_eq!(full.dial_of(&full.channels[7]), Dial::PosHue);
    }

    #[test]
    fn pixel_templates_become_one_colour() {
        let bar = r#"{
          "name": "Bar",
          "availableChannels": { "Dimmer": { "capability": { "type": "Intensity" } } },
          "templateChannels": {
            "Red $pixelKey": { "capability": { "type": "ColorIntensity", "color": "Red" } },
            "Program $pixelKey": { "capability": { "type": "Effect", "effectName": "x" } }
          },
          "modes": [{ "name": "m", "shortName": "4ch", "channels": ["Red Master", "Red 1", "Program 1", "Dimmer"] }]
        }"#;
        let f = &convert(bar, "x").unwrap()[0];
        assert_eq!(f.summary(), "R R — I");
        assert_eq!(f.channels[2].label, "Program 1");
    }

    #[test]
    fn rejects_non_fixtures() {
        assert!(convert("{}", "x").is_err());
        assert!(convert("not json", "x").is_err());
    }

    #[test]
    fn manufacturer_names() {
        assert_eq!(manufacturer_from_key("chauvet-dj"), "Chauvet Dj");
        assert_eq!(manufacturer_from_key("shehds"), "Shehds");
    }

    /// Rebuilds `assets/fixtures.json` from an Open Fixture Library checkout:
    /// `OFL_DIR=/path/to/open-fixture-library cargo test regenerate_bundled -- --ignored`
    #[test]
    #[ignore]
    fn regenerate_bundled_library() {
        let dir = std::env::var("OFL_DIR").expect("set OFL_DIR");
        let dir = std::path::Path::new(&dir);
        let names: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("fixtures/manufacturers.json")).unwrap(),
        )
        .unwrap();
        let brands = [
            "shehds",
            "chauvet-dj",
            "american-dj",
            "eurolite",
            "stairville",
            "cameo",
            "showtec",
            "varytec",
            "uking",
            "betopper",
            "beamz",
        ];
        let wanted = [
            "Color Changer",
            "Moving Head",
            "Dimmer",
            "Strobe",
            "Blinder",
            "Pixel Bar",
            "Scanner",
        ];
        let unwanted = [
            "Laser", "Smoke", "Hazer", "Fan", "Stand", "Matrix", "Flower",
        ];
        let mut all = Vec::new();
        for brand in brands {
            let display = names
                .get(brand)
                .and_then(|m| m.get("name"))
                .and_then(Value::as_str)
                .map_or_else(|| manufacturer_from_key(brand), display_manufacturer);
            let mut files: Vec<_> = std::fs::read_dir(dir.join("fixtures").join(brand))
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect();
            files.sort();
            for f in files {
                let text = std::fs::read_to_string(&f).unwrap();
                let v: Value = serde_json::from_str(&text).unwrap();
                let cats: Vec<&str> = v["categories"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if !cats.iter().any(|c| wanted.contains(c))
                    || cats.iter().any(|c| unwanted.contains(c))
                {
                    continue;
                }
                if let Ok(fixtures) = convert(&text, &display) {
                    all.extend(fixtures);
                }
            }
        }
        let json = serde_json::to_string(&all).unwrap();
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fixtures.json"),
            json,
        )
        .unwrap();
        eprintln!("{} fixture modes", all.len());
    }
}
