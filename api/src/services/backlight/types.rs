use super::effects::EffectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

use super::effects;
use super::routine::{BedtimeSettings, ControlSource};

pub type Packet = [u8; 20];
pub struct Exchange {
    /// A newly established link requires current-target recovery, even if reads succeed.
    pub reconnected: bool,
    pub writes_accepted: bool,
    pub replies: Vec<Packet>,
}
#[derive(Debug)]
pub enum TransportError {
    Unavailable,
    Superseded,
}
/// Checked after connecting and before each write, including every scene-upload frame.
pub type CurrentOperation = dyn Fn() -> bool + Send + Sync;
pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = std::result::Result<Exchange, TransportError>> + Send + 'a>>;
/// One complete GATT operation: ordered writes followed by correlated status queries.
pub trait GattTransport: Send + Sync {
    /// Verify a private backup belongs to the configured controller.
    fn matches_device(&self, identity: &str) -> bool;
    /// Check `current` after connection setup and before every write.
    /// Keep the operation lock until completion/cancellation; never interleave scene uploads.
    fn exchange<'a>(
        &'a self,
        writes: &'a [Packet],
        queries: &'a [Vec<u8>],
        current: &'a CurrentOperation,
    ) -> TransportFuture<'a>;
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Selection {
    Off {},
    TvSync {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        brightness: Option<Brightness>,
    },
    Color {
        rgb: Rgb,
        brightness: Brightness,
    },
    Effect {
        effect: EffectId,
        brightness: Brightness,
    },
}
/// Controller brightness command value (0–100); not photometric calibration.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub struct Brightness(u8);
impl TryFrom<u8> for Brightness {
    type Error = &'static str;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err("Brightness must be between 0 and 100")
        }
    }
}
impl From<Brightness> for u8 {
    fn from(value: Brightness) -> Self {
        value.0
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub effects: &'static [effects::Effect],
    pub setup: Setup,
    pub reachability: Reachability,
    pub intended: Selection,
    pub acknowledged: Option<Selection>,
    pub observed: Option<Observation>,
    pub stale: bool,
    pub command: Command,
    pub recovery: Recovery,
    pub bedtime: BedtimeSettings,
    pub control_source: ControlSource,
    pub override_expires_at: Option<DateTime<Utc>>,
    pub next_transition: Option<DateTime<Utc>>,
}
/// Whether a validated private TV Sync profile is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Setup {
    Incomplete,
    Ready,
}
/// Device reachability, independent of a command acknowledgment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reachability {
    Unconfigured,
    Unknown,
    Reachable,
    Unreachable,
}
/// Outcome of the most recent intended-target dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Command {
    Idle,
    Pending,
    Acknowledged,
    Failed,
}
/// Only the mode categories supported by controller readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservedMode {
    TvSync,
    Color,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Recovery {
    Idle,
    Pending,
    Retrying,
    Recovered,
    ProfileRequired,
}
#[derive(Clone, Serialize)]
pub struct Observation {
    pub power: bool,
    pub brightness: u8,
    pub mode: ObservedMode,
    pub observed_at: DateTime<Utc>,
}
#[derive(Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProfileSource {
    Capture,
    Backup,
}
