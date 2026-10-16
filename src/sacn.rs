//! Streaming ACN (ANSI E1.31) output: multicast to the universe's group, or
//! unicast to one receiver.

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

pub const SACN_PORT: u16 = 5568;
const PACKET_LEN: usize = 638;

/// The multicast group for a universe (1-63999): 239.255.hi.lo.
pub fn multicast_group(universe: u16) -> Ipv4Addr {
    Ipv4Addr::new(239, 255, (universe >> 8) as u8, (universe & 0xff) as u8)
}

fn flags_and_length(len: usize) -> [u8; 2] {
    (0x7000u16 | len as u16).to_be_bytes()
}

/// An E1.31 data packet for a full 512-slot universe.
pub fn data_packet(
    cid: &[u8; 16],
    source: &str,
    universe: u16,
    priority: u8,
    sequence: u8,
    data: &[u8],
) -> Vec<u8> {
    let mut p = vec![0u8; PACKET_LEN];
    // Root layer
    p[0..2].copy_from_slice(&0x0010u16.to_be_bytes());
    p[4..16].copy_from_slice(b"ASC-E1.17\0\0\0");
    p[16..18].copy_from_slice(&flags_and_length(PACKET_LEN - 16));
    p[18..22].copy_from_slice(&4u32.to_be_bytes()); // VECTOR_ROOT_E131_DATA
    p[22..38].copy_from_slice(cid);
    // Framing layer
    p[38..40].copy_from_slice(&flags_and_length(PACKET_LEN - 38));
    p[40..44].copy_from_slice(&2u32.to_be_bytes()); // VECTOR_E131_DATA_PACKET
    let name = source.as_bytes();
    let n = name.len().min(63);
    p[44..44 + n].copy_from_slice(&name[..n]);
    p[108] = priority.min(200);
    p[111] = sequence;
    p[113..115].copy_from_slice(&universe.to_be_bytes());
    // DMP layer
    p[115..117].copy_from_slice(&flags_and_length(PACKET_LEN - 115));
    p[117] = 0x02; // VECTOR_DMP_SET_PROPERTY
    p[118] = 0xa1;
    p[121..123].copy_from_slice(&1u16.to_be_bytes()); // address increment
    p[123..125].copy_from_slice(&513u16.to_be_bytes());
    // p[125] is the DMX start code, 0.
    let n = data.len().min(512);
    p[126..126 + n].copy_from_slice(&data[..n]);
    p
}

pub struct SacnSender {
    socket: UdpSocket,
    cid: [u8; 16],
    sequence: u8,
}

impl SacnSender {
    pub fn new() -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_multicast_ttl_v4(16)?;
        // A component ID that's stable for this run.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let mut cid = [0u8; 16];
        cid.copy_from_slice(&(seed ^ 0x5374_6167_652d_4c69_7465_0000_0000_0001).to_le_bytes());
        Ok(Self {
            socket,
            cid,
            sequence: 0,
        })
    }

    /// Sends to the universe's multicast group, or to `unicast` if given.
    pub fn send(
        &mut self,
        universe: u16,
        priority: u8,
        unicast: Option<Ipv4Addr>,
        data: &[u8],
    ) -> io::Result<()> {
        self.sequence = self.sequence.wrapping_add(1);
        let pkt = data_packet(
            &self.cid,
            "Stage-Lite",
            universe,
            priority,
            self.sequence,
            data,
        );
        let dest = unicast.unwrap_or_else(|| multicast_group(universe));
        self.socket
            .send_to(&pkt, SocketAddrV4::new(dest, SACN_PORT))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout() {
        let mut data = [0u8; 512];
        data[0] = 0x11;
        data[511] = 0x22;
        let p = data_packet(&[7; 16], "Test", 0x0102, 150, 9, &data);
        assert_eq!(p.len(), 638);
        assert_eq!(&p[4..13], b"ASC-E1.17");
        assert_eq!(&p[16..18], &[0x72, 0x6e]); // 622
        assert_eq!(&p[38..40], &[0x72, 0x58]); // 600
        assert_eq!(&p[115..117], &[0x72, 0x0b]); // 523
        assert_eq!(&p[44..48], b"Test");
        assert_eq!(p[108], 150);
        assert_eq!(p[111], 9);
        assert_eq!(&p[113..115], &[1, 2]);
        assert_eq!(&p[123..125], &[0x02, 0x01]);
        assert_eq!(p[125], 0);
        assert_eq!(p[126], 0x11);
        assert_eq!(p[637], 0x22);
    }

    #[test]
    fn multicast_groups() {
        assert_eq!(multicast_group(1), Ipv4Addr::new(239, 255, 0, 1));
        assert_eq!(multicast_group(63999), Ipv4Addr::new(239, 255, 249, 255));
    }
}
