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
/// Native picture-inactivity settings, independent of the saved TV Sync profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct InactivitySettings {
    pub enabled: bool,
    pub mode: InactivityMode,
    pub low_brightness_seconds: u16,
    pub same_tone_seconds: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InactivityMode {
    LowBrightness,
    SameTone,
}
impl InactivitySettings {
    #[must_use]
    pub fn capture(replies: &[Packet]) -> Option<Self> {
        let p = replies
            .iter()
            .find(|p| valid(p, 0xa9) && p[2..4] == [0x0a, 6])?;
        let enabled = match p[4] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let mode = match p[5] {
            1 => InactivityMode::LowBrightness,
            2 => InactivityMode::SameTone,
            _ => return None,
        };
        let low_brightness_seconds = u16::from_le_bytes([p[6], p[7]]);
        let same_tone_seconds = u16::from_le_bytes([p[8], p[9]]);
        if !(5..=300).contains(&low_brightness_seconds)
            || !(120..=1800).contains(&same_tone_seconds)
        {
            return None;
        }
        Some(Self {
            enabled,
            mode,
            low_brightness_seconds,
            same_tone_seconds,
        })
    }
    #[must_use]
    pub(super) fn hold_same_tone(self, seconds: u16) -> Self {
        Self {
            enabled: true,
            mode: InactivityMode::SameTone,
            same_tone_seconds: seconds,
            ..self
        }
    }
    #[must_use]
    pub(super) fn write(self) -> Packet {
        let low = self.low_brightness_seconds.to_le_bytes();
        let same = self.same_tone_seconds.to_le_bytes();
        frame(&[
            0x33,
            0xa9,
            0x0a,
            6,
            u8::from(self.enabled),
            match self.mode {
                InactivityMode::LowBrightness => 1,
                InactivityMode::SameTone => 2,
            },
            low[0],
            low[1],
            same[0],
            same[1],
        ])
    }
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
