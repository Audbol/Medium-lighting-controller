//! The DMX clock: a background thread that runs the console ~40 times a second
//! and sends the universe over Art-Net or sACN and/or to an ENTTEC USB Pro. It
//! runs independently of the window, so the show keeps going when it's minimised.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::artnet::{ArtNetSender, Node};
use crate::engine::Desk;
use crate::sacn::SacnSender;

pub const FRAME_INTERVAL: Duration = Duration::from_millis(25);
/// How often to look for Art-Net nodes while Art-Net is on.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Setup → Networked DMX.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Network {
    Off,
    ArtNet,
    Sacn,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputConfig {
    pub network: Network,
    pub artnet_target: Ipv4Addr,
    /// 15-bit Port-Address.
    pub artnet_universe: u16,
    pub sacn_universe: u16,
    pub sacn_priority: u8,
    /// Unicast receiver; multicast when None.
    pub sacn_unicast: Option<Ipv4Addr>,
    pub enttec_port: Option<String>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            network: Network::Off,
            artnet_target: Ipv4Addr::BROADCAST,
            artnet_universe: 0,
            sacn_universe: 1,
            sacn_priority: 100,
            sacn_unicast: None,
            enttec_port: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct OutputStatus {
    pub network: String,
    pub enttec: String,
    /// The ENTTEC port failed to open; the app should forget it.
    pub enttec_failed: bool,
    pub frames: u64,
    /// Art-Net nodes that answered ArtPoll.
    pub nodes: Vec<Node>,
}

pub struct Output {
    pub config: Arc<Mutex<OutputConfig>>,
    pub status: Arc<Mutex<OutputStatus>>,
    poll_now: Arc<Mutex<bool>>,
}

impl Output {
    pub fn start(desk: Arc<Mutex<Desk>>, epoch: Instant) -> Self {
        let config = Arc::new(Mutex::new(OutputConfig::default()));
        let status = Arc::new(Mutex::new(OutputStatus::default()));
        let poll_now = Arc::new(Mutex::new(false));
        let (cfg, st, poll) = (config.clone(), status.clone(), poll_now.clone());
        thread::Builder::new()
            .name("dmx-output".into())
            .spawn(move || run(desk, cfg, st, poll, epoch))
            .expect("spawn output thread");
        Self {
            config,
            status,
            poll_now,
        }
    }

    /// Looks for Art-Net nodes now, clearing the list.
    pub fn discover(&self) {
        self.status.lock().unwrap().nodes.clear();
        *self.poll_now.lock().unwrap() = true;
    }
}

fn run(
    desk: Arc<Mutex<Desk>>,
    config: Arc<Mutex<OutputConfig>>,
    status: Arc<Mutex<OutputStatus>>,
    poll_now: Arc<Mutex<bool>>,
    epoch: Instant,
) {
    let mut artnet: Option<ArtNetSender> = None;
    let mut sacn: Option<SacnSender> = None;
    let mut last_poll: Option<Instant> = None;
    #[cfg(feature = "enttec")]
    let mut enttec: Option<(String, crate::enttec::UsbPro)> = None;
    let mut universe = [0u8; 512];
    let mut next = Instant::now();

    loop {
        universe.copy_from_slice(desk.lock().unwrap().tick(epoch.elapsed().as_secs_f64()));
        let cfg = config.lock().unwrap().clone();
        let mut st = status.lock().unwrap().clone();
        st.frames += 1;

        if cfg.network == Network::ArtNet {
            if artnet.is_none() {
                match ArtNetSender::new() {
                    Ok(s) => artnet = Some(s),
                    Err(e) => st.network = format!("Art-Net socket error: {e}"),
                }
            }
            if let Some(sender) = &mut artnet {
                let asked = std::mem::take(&mut *poll_now.lock().unwrap());
                if asked || last_poll.is_none_or(|t| t.elapsed() >= POLL_INTERVAL) {
                    let _ = sender.poll(cfg.artnet_target);
                    last_poll = Some(Instant::now());
                }
                for node in sender.receive() {
                    match st.nodes.iter_mut().find(|n| n.ip == node.ip) {
                        Some(n) => *n = node,
                        None => st.nodes.push(node),
                    }
                }
                let u = cfg.artnet_universe;
                st.network = match sender.send(cfg.artnet_target, u, &universe) {
                    Ok(()) => format!(
                        "Art-Net to {} · net {} sub {} uni {}{}",
                        cfg.artnet_target,
                        u >> 8,
                        (u >> 4) & 0xf,
                        u & 0xf,
                        if sender.listening {
                            ""
                        } else {
                            " · port 6454 busy, discovery off"
                        }
                    ),
                    Err(e) => format!("Art-Net send error: {e}"),
                };
            }
        } else {
            artnet = None;
            last_poll = None;
        }

        if cfg.network == Network::Sacn {
            if sacn.is_none() {
                match SacnSender::new() {
                    Ok(s) => sacn = Some(s),
                    Err(e) => st.network = format!("sACN socket error: {e}"),
                }
            }
            if let Some(sender) = &mut sacn {
                st.network = match sender.send(
                    cfg.sacn_universe,
                    cfg.sacn_priority,
                    cfg.sacn_unicast,
                    &universe,
                ) {
                    Ok(()) => format!(
                        "sACN universe {} priority {} to {}",
                        cfg.sacn_universe,
                        cfg.sacn_priority,
                        cfg.sacn_unicast.map_or_else(
                            || crate::sacn::multicast_group(cfg.sacn_universe).to_string(),
                            |ip| ip.to_string()
                        )
                    ),
                    Err(e) => format!("sACN send error: {e}"),
                };
            }
        } else {
            sacn = None;
        }
        if cfg.network == Network::Off {
            st.network = "Networked DMX off".into();
        }

        #[cfg(feature = "enttec")]
        {
            match &cfg.enttec_port {
                Some(path) => {
                    if enttec.as_ref().map(|(p, _)| p) != Some(path) {
                        enttec = match crate::enttec::UsbPro::open(path) {
                            Ok(dev) => Some((path.clone(), dev)),
                            Err(e) => {
                                st.enttec = format!("{path}: {e}");
                                // Don't retry every frame; the user picks the port again.
                                config.lock().unwrap().enttec_port = None;
                                st.enttec_failed = true;
                                None
                            }
                        };
                    }
                    if let Some((p, dev)) = &mut enttec {
                        st.enttec = match dev.send(&universe) {
                            Ok(()) => format!("sending on {p}"),
                            Err(e) => format!("{p}: {e}"),
                        };
                    }
                }
                None => {
                    if enttec.take().is_some() {
                        st.enttec = "off".into();
                    }
                }
            }
        }

        *status.lock().unwrap() = st;
        next += FRAME_INTERVAL;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}
