//! Art-Net 4: ArtDmx output and ArtPoll node discovery over UDP port 6454.

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

pub const ARTNET_PORT: u16 = 6454;
const ID: &[u8; 8] = b"Art-Net\0";
const OP_POLL: u16 = 0x2000;
const OP_POLL_REPLY: u16 = 0x2100;
const OP_DMX: u16 = 0x5000;
const PROTOCOL_VERSION: u16 = 14;

/// Port-Address from Net (0-127), Sub-Net (0-15) and Universe (0-15).
pub fn port_address(net: u8, subnet: u8, universe: u8) -> u16 {
    ((net as u16 & 0x7f) << 8) | ((subnet as u16 & 0xf) << 4) | (universe as u16 & 0xf)
}

/// Builds an ArtDmx packet. `universe` is the 15-bit Port-Address.
pub fn art_dmx(data: &[u8], universe: u16, sequence: u8) -> Vec<u8> {
    // The data length must be even and 2..=512.
    let mut len = data.len().clamp(2, 512);
    len += len % 2;
    let mut pkt = Vec::with_capacity(18 + len);
    pkt.extend_from_slice(ID);
    pkt.extend_from_slice(&OP_DMX.to_le_bytes());
    pkt.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    pkt.push(sequence);
    pkt.push(0); // physical port
    pkt.push((universe & 0xff) as u8); // SubUni
    pkt.push(((universe >> 8) & 0x7f) as u8); // Net
    pkt.extend_from_slice(&(len as u16).to_be_bytes());
    let n = data.len().min(len);
    pkt.extend_from_slice(&data[..n]);
    pkt.resize(18 + len, 0);
    pkt
}

pub fn art_poll() -> Vec<u8> {
    let mut pkt = Vec::with_capacity(14);
    pkt.extend_from_slice(ID);
    pkt.extend_from_slice(&OP_POLL.to_le_bytes());
    pkt.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    pkt.push(0); // Flags
    pkt.push(0); // DiagPriority
    pkt
}

/// A node that answered ArtPoll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub ip: Ipv4Addr,
    pub short_name: String,
    pub long_name: String,
    /// Port-Addresses of its outputs.
    pub outputs: Vec<u16>,
}

fn c_string(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_string()
}

/// Parses an ArtPollReply; anything else gives None.
pub fn parse_poll_reply(p: &[u8]) -> Option<Node> {
    if p.len() < 194 || &p[..8] != ID || u16::from_le_bytes([p[8], p[9]]) != OP_POLL_REPLY {
        return None;
    }
    let ip = Ipv4Addr::new(p[10], p[11], p[12], p[13]);
    let (net, sub) = (p[18] as u16 & 0x7f, p[19] as u16 & 0xf);
    let ports = (u16::from_be_bytes([p[172], p[173]]) as usize).min(4);
    let outputs = (0..ports)
        .filter(|&i| p[174 + i] & 0x80 != 0) // PortTypes: can output DMX
        .map(|i| (net << 8) | (sub << 4) | (p[190 + i] as u16 & 0xf))
        .collect();
    Some(Node {
        ip,
        short_name: c_string(&p[26..44]),
        long_name: c_string(&p[44..108]),
        outputs,
    })
}

pub struct ArtNetSender {
    socket: UdpSocket,
    sequence: u8,
    /// Bound to 6454, so node replies reach us.
    pub listening: bool,
}

impl ArtNetSender {
    pub fn new() -> io::Result<Self> {
        // Nodes expect traffic from port 6454 and reply to it; fall back to any
        // port (sending only) if another program already has it.
        let (socket, listening) = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, ARTNET_PORT)) {
            Ok(s) => (s, true),
            Err(_) => (UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?, false),
        };
        socket.set_broadcast(true)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            sequence: 0,
            listening,
        })
    }

    pub fn send(&mut self, target: Ipv4Addr, universe: u16, data: &[u8]) -> io::Result<()> {
        // 1..=255; 0 would disable sequencing at the receiver.
        self.sequence = self.sequence % 255 + 1;
        let pkt = art_dmx(data, universe, self.sequence);
        self.socket
            .send_to(&pkt, SocketAddrV4::new(target, ARTNET_PORT))?;
        Ok(())
    }

    /// Broadcasts ArtPoll (and sends it to `target` too if that's a single node).
    pub fn poll(&self, target: Ipv4Addr) -> io::Result<()> {
        let pkt = art_poll();
        self.socket
            .send_to(&pkt, SocketAddrV4::new(Ipv4Addr::BROADCAST, ARTNET_PORT))?;
        if target != Ipv4Addr::BROADCAST {
            self.socket
                .send_to(&pkt, SocketAddrV4::new(target, ARTNET_PORT))?;
        }
        Ok(())
    }

    /// Nodes that replied since the last call.
    pub fn receive(&self) -> Vec<Node> {
        let mut buf = [0u8; 1024];
        let mut nodes = Vec::new();
        while let Ok((n, _)) = self.socket.recv_from(&mut buf) {
            if let Some(node) = parse_poll_reply(&buf[..n]) {
                nodes.push(node);
            }
        }
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_dmx_layout() {
        let mut data = [0u8; 512];
        data[0] = 0xaa;
        data[511] = 0x55;
        let p = art_dmx(&data, 0x1234, 7);
        assert_eq!(&p[..8], b"Art-Net\0");
        assert_eq!(&p[8..10], &[0x00, 0x50]); // OpDmx, little-endian
        assert_eq!(&p[10..12], &[0, 14]);
        assert_eq!(p[12], 7);
        assert_eq!(p[14], 0x34); // SubUni
        assert_eq!(p[15], 0x12); // Net
        assert_eq!(&p[16..18], &[0x02, 0x00]); // 512, big-endian
        assert_eq!(p.len(), 18 + 512);
        assert_eq!(p[18], 0xaa);
        assert_eq!(p[18 + 511], 0x55);
    }

    #[test]
    fn odd_length_is_padded() {
        let p = art_dmx(&[1, 2, 3], 0, 1);
        assert_eq!(&p[16..18], &[0, 4]);
        assert_eq!(&p[18..], &[1, 2, 3, 0]);
    }

    #[test]
    fn port_addresses() {
        assert_eq!(port_address(0, 0, 0), 0);
        assert_eq!(port_address(0, 1, 2), 0x12);
        assert_eq!(port_address(3, 15, 15), 0x3ff);
    }

    #[test]
    fn poll_packet() {
        let p = art_poll();
        assert_eq!(p.len(), 14);
        assert_eq!(&p[8..10], &[0x00, 0x20]);
    }

    #[test]
    fn parses_poll_reply() {
        let mut p = vec![0u8; 239];
        p[..8].copy_from_slice(ID);
        p[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
        p[10..14].copy_from_slice(&[2, 0, 0, 7]);
        p[18] = 1; // Net
        p[19] = 2; // Sub-Net
        p[26..30].copy_from_slice(b"Node");
        p[44..53].copy_from_slice(b"Long name");
        p[173] = 2; // two ports
        p[174] = 0x80;
        p[175] = 0x80;
        p[190] = 3;
        p[191] = 4;
        let n = parse_poll_reply(&p).unwrap();
        assert_eq!(n.ip, Ipv4Addr::new(2, 0, 0, 7));
        assert_eq!(n.short_name, "Node");
        assert_eq!(n.long_name, "Long name");
        assert_eq!(n.outputs, vec![0x123, 0x124]);
        assert!(parse_poll_reply(&art_poll()).is_none());
    }

    #[test]
    fn sends_over_loopback() {
        let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = rx.local_addr().unwrap().port();
        let tx = UdpSocket::bind("127.0.0.1:0").unwrap();
        tx.send_to(&art_dmx(&[9; 512], 3, 1), ("127.0.0.1", port))
            .unwrap();
        let mut buf = [0u8; 600];
        let n = rx.recv(&mut buf).unwrap();
        assert_eq!(n, 530);
        assert_eq!(buf[14], 3);
    }
}
