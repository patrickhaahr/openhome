use super::{Observation, Packet};
use serde::{Deserialize, Serialize};

pub fn frame(payload: &[u8]) -> Packet {
    let mut packet = [0; 20];
    packet[..payload.len()].copy_from_slice(payload);
    packet[19] = packet[..19].iter().fold(0, |a, b| a ^ b);
    packet
}
fn valid(packet: &Packet, command: u8) -> bool {
    packet[0] == 0xaa && packet[1] == command && packet.iter().fold(0, |a, b| a ^ b) == 0
}
/// Stored privately; never serialize this type in capability responses or logs.
#[derive(Clone, Deserialize, Serialize)]
pub struct TvSyncProfile {
    video: Packet,
    brightness: Packet,
}
impl TvSyncProfile {
    pub fn capture(replies: &[Packet]) -> Option<Self> {
        let video = *replies.iter().find(|p| valid(p, 5) && p[2] == 0)?;
        let brightness = *replies.iter().find(|p| valid(p, 4) && p[2] <= 100)?;
        Some(Self { video, brightness })
    }
    pub fn validate(self) -> Option<Self> {
        Self::capture(&[self.video, self.brightness])
    }
    pub fn writes(&self, brightness: Option<u8>) -> Vec<Packet> {
        let mut video = self.video;
        video[0] = 0x33;
        vec![
            frame(&[0x33, 1, 1]),
            frame(&video[..19]),
            frame(&[0x33, 4, brightness.unwrap_or(self.brightness[2])]),
        ]
    }
}
pub fn observation(replies: &[Packet]) -> Option<Observation> {
    let power = replies.iter().find(|p| valid(p, 1) && p[2] <= 1)?;
    let brightness = replies.iter().find(|p| valid(p, 4) && p[2] <= 100)?;
    let mode = replies.iter().find(|p| valid(p, 5))?;
    Some(Observation {
        power: power[2] == 1,
        brightness: brightness[2],
        mode: if mode[2] == 0 {
            super::ObservedMode::TvSync
        } else if mode[2] == 0x15 {
            super::ObservedMode::Color
        } else {
            super::ObservedMode::Other
        },
        observed_at: chrono::Utc::now(),
    })
}
