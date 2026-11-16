use serde::Serialize;
use serde_json::Value;

use super::Namespace;

/// The `sleep_session` fields a Recovery Day reads; raw motion and sleep-state streams are never
/// selected.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub namespace: Namespace,
    pub start_ts: i64,
    pub end_ts: i64,
    pub efficiency: Option<f64>,
    pub resting_hr: Option<f64>,
    pub avg_hrv: Option<f64>,
    pub stages_json: Option<String>,
    pub user_edited: bool,
    pub start_ts_adjusted: Option<i64>,
    pub staging_sparse: Option<bool>,
}

impl SessionRow {
    /// The start NOOP displays: the user's adjustment when present, else the detected start.
    pub fn effective_start_ts(&self) -> i64 {
        self.start_ts_adjusted.unwrap_or(self.start_ts)
    }

    /// 0 without staging, 1 when staging covers under 95% of the session, 2 otherwise
    /// (including staging whose coverage cannot be measured).
    pub(super) fn richness(&self) -> u8 {
        let Some(stages) = self
            .stages_json
            .as_deref()
            .map(str::trim)
            .filter(|stages| !stages.is_empty() && *stages != "[]")
        else {
            return 0;
        };
        match covered_fraction(stages, self.end_ts - self.start_ts) {
            Some(fraction) if fraction < 0.95 => 1,
            _ => 2,
        }
    }
}

/// Selects one wake day's sessions: every imported session, unless the computed sessions are
/// strictly richer. Returns them ordered by effective start.
pub fn merge_sleep_sessions(sessions: Vec<SessionRow>) -> Vec<SessionRow> {
    let (imported, computed): (Vec<_>, Vec<_>) = sessions
        .into_iter()
        .partition(|session| session.namespace == Namespace::Imported);
    let day_richness =
        |sessions: &[SessionRow]| sessions.iter().map(SessionRow::richness).max().unwrap_or(0);
    let mut selected = if imported.is_empty() || day_richness(&computed) > day_richness(&imported) {
        computed
    } else {
        imported
    };
    selected.sort_by_key(|session| (session.effective_start_ts(), session.start_ts));
    selected
}

/// Seconds of `[start, end)` stage segments over the session span; `None` when either is empty.
fn covered_fraction(stages_json: &str, span_seconds: i64) -> Option<f64> {
    let covered = covered_seconds(stages_json);
    if span_seconds <= 0 || covered <= 0.0 {
        return None;
    }
    Some((covered / span_seconds as f64).min(1.0))
}

fn covered_seconds(stages_json: &str) -> f64 {
    let Ok(Value::Array(segments)) = serde_json::from_str::<Value>(stages_json) else {
        return 0.0;
    };
    let mut covered = 0.0;
    for segment in &segments {
        let Value::Object(segment) = segment else {
            return 0.0;
        };
        let bound = |key: &str| segment.get(key).and_then(Value::as_f64);
        if let (Some(start), Some(end)) = (bound("start"), bound("end"))
            && end > start
        {
            covered += end - start;
        }
    }
    covered
}

/// Minutes per sleep stage decoded from `stages_json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct StageMinutes {
    pub awake: f64,
    pub light: f64,
    pub deep: f64,
    pub rem: f64,
}

/// Decodes the stage shapes NOOP stores: a `{awake, light, deep, rem}` minutes object, or an array
/// of `{start, end, stage}` segments (Unix seconds) or `{stage, min}` entries. `None` when the JSON
/// is unreadable or records no time in bed.
pub fn stage_minutes(stages_json: &str) -> Option<StageMinutes> {
    let mut totals = StageMinutes::default();
    match serde_json::from_str::<Value>(stages_json).ok()? {
        Value::Object(minutes) => {
            let minutes_of = |key: &str| minutes.get(key).and_then(Value::as_f64).unwrap_or(0.0);
            totals = StageMinutes {
                awake: minutes_of("awake"),
                light: minutes_of("light"),
                deep: minutes_of("deep"),
                rem: minutes_of("rem"),
            };
        }
        Value::Array(entries) => {
            for entry in entries.iter().filter_map(Value::as_object) {
                let number = |key: &str| entry.get(key).and_then(Value::as_f64);
                let minutes = match (number("start"), number("end")) {
                    (Some(start), Some(end)) if end > start => (end - start) / 60.0,
                    (Some(_), Some(_)) => continue,
                    _ => match number("min") {
                        Some(minutes) => minutes,
                        None => continue,
                    },
                };
                if minutes <= 0.0 {
                    continue;
                }
                let stage = entry.get("stage").and_then(Value::as_str).unwrap_or("");
                match stage {
                    "wake" | "awake" => totals.awake += minutes,
                    "light" => totals.light += minutes,
                    "deep" => totals.deep += minutes,
                    "rem" => totals.rem += minutes,
                    _ => {}
                }
            }
        }
        _ => return None,
    }
    let in_bed = totals.awake + totals.light + totals.deep + totals.rem;
    (in_bed > 0.0).then_some(totals)
}

/// One ordered stage segment of a sleep session, in Unix seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageSegment {
    pub start: i64,
    pub end: i64,
    pub stage: &'static str,
}

/// Decodes `{start, end, stage}` segments (Unix seconds) in time order, clamped to
/// `[from, to]`. `wake` is reported as `awake`, as in `stage_minutes`. Segments that are
/// unreadable, of an unknown stage or empty after clamping are dropped. `None` when the JSON
/// holds no such segment, as for the minutes-object and `{stage, min}` shapes.
pub fn stage_segments(stages_json: &str, from: i64, to: i64) -> Option<Vec<StageSegment>> {
    let Value::Array(entries) = serde_json::from_str::<Value>(stages_json).ok()? else {
        return None;
    };
    let mut segments: Vec<StageSegment> = entries
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|entry| {
            let bound = |key: &str| entry.get(key).and_then(Value::as_f64);
            let stage = match entry.get("stage").and_then(Value::as_str)? {
                "wake" | "awake" => "awake",
                "light" => "light",
                "deep" => "deep",
                "rem" => "rem",
                _ => return None,
            };
            let start = (bound("start")?.round() as i64).max(from);
            let end = (bound("end")?.round() as i64).min(to);
            (end > start).then_some(StageSegment { start, end, stage })
        })
        .collect();
    segments.sort_by_key(|segment| (segment.start, segment.end));
    (!segments.is_empty()).then_some(segments)
}
