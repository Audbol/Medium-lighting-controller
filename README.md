# Stage-Lite

A desktop lighting console, written in Rust, that works like a **Jands Stage CL** (Chroma-Q): 24 HSI fixture channels with hue and saturation dials, scenes, chases, a cuelist and Autopilot. It outputs **Art-Net**, **sACN** or an **ENTTEC DMX USB Pro**, and has a stage visualizer.

## What it copies from the Stage CL

| | Stage CL | Stage-Lite |
|---|---|---|
| Fixture channels | 12 HSI strips × 2 pages (Black 1-12, White 13-24) | same |
| Per strip | fader, flash, select, Hue and Saturation dials, colour mimic | same; drag a dial to turn it, click to press it |
| Shift | Shift + dials drive two more features (zoom, strobe…) | same; double-click SHIFT to fan across the selection |
| Scene mode | 12 faders play up to 6 pages of scenes or chases | same (pages: Yellow, Red, Green, Blue, Magenta, Cyan) |
| Playbacks | 4 Scene buttons, 4 Chase buttons, a Cuelist, each with a master | same |
| Precedence | intensity HTP, colour LTP | same, with LTP colour crossfades |
| Programming | Record, Clear (1/2/3 presses), Undo, Snapshot, fader gating | same |
| Colour window | 18 swatches, picker, RGB sliders, 3 pattern pages, 4 colour-chase pages | same |
| Chases | BPM, direction, mirror, crossfade, intensity-only filter, audio | same, plus Auto chase from a look |
| Cuelist | Play, Back, Pause, Release; Halt/Follow, wait, fade | same, plus Jump, skip, copy/insert/update |
| Masters | GM, DBO, fade time dial, chase rate dial with tap sync | same |
| Fixture library | Intensity, R G B A W, CW/WW, CT, CMY, Strobe, Zoom, custom, 16-bit, multipatch | same, with a fixture editor |
| Output | 1 universe: DMX, Art-Net, sACN | Art-Net (with node discovery), sACN, ENTTEC USB Pro |
| Autopilot, Lock | yes | yes |

## Using it

The window has the **touchscreen** on the left, the **stage** in the middle and the **front panel** along the bottom.

1. **Patch** (Patch window): give each fixture channel a fixture type and DMX address. The **Wizard** patches a row of identical fixtures; **&** (multipatch) puts more fixtures on one channel. Build your own fixture types in **Library**.
2. **Make a look**: raise faders, then turn the **Hue** and **Saturation** dials. Select several fixtures (SELECT, Ctrl+click for a range, or click them on stage) and any one of their dials changes them all. Clicking Hue steps through six colours; clicking Saturation goes full → 50% → white. With **SHIFT**, the dials set zoom and strobe.
3. **Colour window**: apply swatches, a picked colour, two-colour patterns (odds & evens, half & half, crossfade, rainbow) or colour chases (rainbow, fire, water, twinkle, police…) to the selection.
4. **Record**: press **RECORD** (lit channels are included; SELECT toggles them), then:
   * a **Scene** button: stores a scene.
   * a **Chase** button: **Auto** builds a chase that walks the look along the included channels; **Manual** adds the look as a step.
   * a **FLASH** button: stores a scene on that fader in the current scene page.
   * the Cuelist **PLAY** button: adds a cue.

   While a chase is playing, recording can copy it to another button instead.
5. **Play**: Scene and Chase buttons toggle (the LED flashes while playing) and their faders are masters. **FIXTURES/SCENES** turns the 12 faders into scene submasters; **PAGE** changes the page. The cuelist has PLAY / BACK / PAUSE / RELEASE.
6. **Clear**: click to set the selection to white, double-click for everything, triple-click to clear all intensity and colour. **SHIFT + CLEAR** resets the panel. **UNDO** reverts the last change.

A fader with a **red cap** is *gated*: its position doesn't match the level it controls (after a page change or Clear). Move it to the output level, shown behind the fader, to take control.

**SNAPSHOT** freezes the stage while you set up the next look, and crossfades to it when pressed again. **DBO** is a blackout while held. The **fade time** dial sets the live fade (double-click: 2 s). The **chase rate** dial scales chase speed (click: stop chases, double-click: 100%), and **SYNC** taps the tempo.

Right-click a Scene or Chase button to release, edit or delete it. **Autopilot** (top of the touchscreen) plays every built-in colour chase on the whole rig for unattended use, and **Lock** protects the console with a passcode.

Keys: `Space` cuelist go · `R` record · `Esc` cancel · `Ctrl+Z` undo · `S` snapshot · `T` sync · `M` beat · `Tab` page · `1`-`4` scenes · `5`-`8` chases · hold `B` blackout · hold `Shift` for Shift.

## Output

Set it up in **Setup**:

* **Art-Net** (the default): send to broadcast `255.255.255.255`, `2.255.255.255`, `10.255.255.255` or a node's own IP, on any Net / Sub-Net / Universe (universe 1 is 0:0:0). Frames go out at 40 Hz from UDP port 6454. The console polls for nodes every 3 seconds (ArtPoll). The nodes that answer are listed with their outputs, and **Use** points the console at one.
* **sACN** (E1.31): universe 1-63999 and priority 0-200, multicast to `239.255.x.y` or unicast to one receiver.
* **ENTTEC DMX USB Pro** (and compatibles): pick the serial port and press *Connect*. It can run alongside Art-Net or sACN.

Output runs on its own thread, so playback continues when the window is minimised.

## Shows

Shows are JSON files (`*.show.json`) holding the patch, your fixture types, swatches, scenes, chases and cuelist. Without a path argument the show is `default.show.json` in your user data folder: `%APPDATA%\Stage-Lite` (Windows), `~/Library/Application Support/Stage-Lite` (macOS) or `~/.local/share/stage-lite` (Linux). You can also pass a path: `stage-lite myshow.show.json`. The **Shows** window saves under a new name and loads other shows from the same folder. Changes are autosaved (every 30 s by default) and on exit. Console settings (network, preferences, passcode) are kept in `settings.json` in the same folder.

On first run a demo show is loaded. It has a mixed rig of IRGBW pars, RGBSZ washes, CW/WW fixtures, RGB battens, RGBAW, a dimmer and a strobe, plus a few scenes, chases and a cuelist.

## Download

Every push is built by GitHub Actions ([Build workflow](../../actions/workflows/build.yml)): open a run and download your system's package from **Artifacts**. Tagged versions (`v*`) are also published under [Releases](../../releases).

| Package | For |
|---|---|
| `stage-lite-windows-x86_64.zip` | Windows 10/11 (Intel/AMD) |
| `stage-lite-windows-arm64.zip` | Windows on ARM |
| `stage-lite-macos-universal.zip` | macOS 11+, Apple silicon and Intel |
| `stage-lite-linux-x86_64.tar.gz` | Linux PCs |
| `stage-lite-raspberry-pi-arm64.tar.gz` | Raspberry Pi OS 64-bit |
| `stage-lite-raspberry-pi-armhf.tar.gz` | Raspberry Pi OS 32-bit |

* **Windows**: unzip and run `stage-lite.exe`. SmartScreen may warn because the exe isn't signed: *More info → Run anyway*.
* **macOS**: unzip and move `Stage-Lite.app` to Applications. It isn't notarized, so the first time right-click it → *Open*.
* **Linux / Raspberry Pi**: `tar xzf stage-lite-*.tar.gz`, then `./stage-lite`, or `./install.sh` to install it with a menu entry.

## Build from source

Needs a stable Rust toolchain (edition 2024).

```sh
cargo run --release                     # Art-Net, sACN and ENTTEC
cargo run --release --features audio    # plus the microphone for audio triggering
cargo test
```

On Linux, `--features audio` needs the ALSA headers (`libasound2-dev` / `alsa-lib-devel`).

## Layout

| File | |
|---|---|
| `src/engine.rs` | The console: programmer, HTP/LTP mix, fader gating, scenes, chases, cuelist, record, undo, snapshot, Autopilot. No GUI or I/O; unit tested |
| `src/fixtures.rs` | Fixture library, colour mixing to R G B A W / CW WW / CMY, DMX rendering |
| `src/colour.rs` | Hue/saturation ↔ RGB |
| `src/effects.rs` | Swatches, patterns and colour chases |
| `src/app.rs`, `src/app/panel.rs`, `src/app/screens.rs` | Window, front panel, touchscreen windows |
| `src/visualizer.rs` | Stage view |
| `src/output.rs` | 40 Hz output thread |
| `src/artnet.rs`, `src/sacn.rs`, `src/enttec.rs` | Art-Net (ArtDmx, ArtPoll), E1.31, ENTTEC USB Pro |
| `src/beat.rs`, `src/audio.rs` | Beat detection, microphone input |
| `src/demo.rs` | First-run demo show |

Stage-Lite is an independent project and is not affiliated with Jands or Chroma-Q.
