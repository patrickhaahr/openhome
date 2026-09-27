//! Ports of NOOP's source-precedence rules, so the API selects the same values NOOP shows.
//!
//! - Daily metrics (`mergeDaily`): imported values win field by field and computed values fill
//!   imported nulls. The sleep block (total, efficiency, stages, disturbances) is taken whole from
//!   the computed row, nulls included, when the user edited that night or when the import is a bare
//!   sleep total that a scored computed night can replace.
//! - Sleep sessions (`mergeSleepRichness`): per wake day, all imported sessions are kept unless the
//!   computed sessions carry richer staging, in which case all computed sessions are kept.
//!
//! Everything here is pure; callers pass rows for a single wake day.

use serde::Serialize;
use serde_json::Value;

use super::noop_source::{COMPUTED_DEVICE_ID, IMPORTED_DEVICE_ID};

/// The NOOP device namespace a value was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Namespace {
    Imported,
    Computed,
}

impl Namespace {
    pub fn from_device_id(device_id: &str) -> Option<Self> {
        match device_id {
            IMPORTED_DEVICE_ID => Some(Self::Imported),
            COMPUTED_DEVICE_ID => Some(Self::Computed),
            _ => None,
        }
    }

    pub fn device_id(self) -> &'static str {
        match self {
            Self::Imported => IMPORTED_DEVICE_ID,
            Self::Computed => COMPUTED_DEVICE_ID,
        }
    }
}

/// A resolved value and the namespace it came from; a null value has no source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sourced<T> {
    pub value: Option<T>,
    pub source: Option<Namespace>,
}

impl<T: Copy> Sourced<T> {
    fn of(value: Option<T>, namespace: Namespace) -> Self {
        Self {
            value,
            source: value.map(|_| namespace),
        }
    }
}

/// The `daily_metric` fields a Recovery Day reads.
#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct DailyRow {
    pub total_sleep_min: Option<f64>,
    pub efficiency: Option<f64>,
    pub deep_min: Option<f64>,
    pub rem_min: Option<f64>,
    pub light_min: Option<f64>,
    pub disturbances: Option<i64>,
    pub resting_hr: Option<f64>,
    pub avg_hrv: Option<f64>,
    pub recovery: Option<f64>,
    pub strain: Option<f64>,
}

impl DailyRow {
    /// A sleep total without efficiency or stages: the shape of a strap-imported night that NOOP
    /// has not scored.
    fn is_bare_sleep_aggregate(&self) -> bool {
        self.total_sleep_min.is_some()
            && self.efficiency.is_none()
            && self.deep_min.is_none()
            && self.rem_min.is_none()
            && self.light_min.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDaily {
    pub total_sleep_min: Sourced<f64>,
    pub efficiency: Sourced<f64>,
    pub deep_min: Sourced<f64>,
    pub rem_min: Sourced<f64>,
    pub light_min: Sourced<f64>,
    pub disturbances: Sourced<i64>,
    pub resting_hr: Sourced<f64>,
    pub avg_hrv: Sourced<f64>,
    pub recovery: Sourced<f64>,
    pub strain: Sourced<f64>,
}

/// Resolves one day's imported and computed rows. `sleep_edited` is whether a computed session
/// waking on that day was edited by the user.
pub fn merge_daily(
    imported: Option<&DailyRow>,
    computed: Option<&DailyRow>,
    sleep_edited: bool,
) -> ResolvedDaily {
    fn pick<T: Copy>(
        imported: Option<&DailyRow>,
        computed: Option<&DailyRow>,
        field: fn(&DailyRow) -> Option<T>,
    ) -> Sourced<T> {
        match imported.and_then(field) {
            Some(value) => Sourced::of(Some(value), Namespace::Imported),
            None => Sourced::of(computed.and_then(field), Namespace::Computed),
        }
    }

    let mut resolved = ResolvedDaily {
        total_sleep_min: pick(imported, computed, |row| row.total_sleep_min),
        efficiency: pick(imported, computed, |row| row.efficiency),
        deep_min: pick(imported, computed, |row| row.deep_min),
        rem_min: pick(imported, computed, |row| row.rem_min),
        light_min: pick(imported, computed, |row| row.light_min),
        disturbances: pick(imported, computed, |row| row.disturbances),
        resting_hr: pick(imported, computed, |row| row.resting_hr),
        avg_hrv: pick(imported, computed, |row| row.avg_hrv),
        recovery: pick(imported, computed, |row| row.recovery),
        strain: pick(imported, computed, |row| row.strain),
    };

    if let (Some(imported), Some(computed)) = (imported, computed) {
        let computed_night_replaces_bare_import = imported.is_bare_sleep_aggregate()
            && computed.total_sleep_min.is_some()
            && !computed.is_bare_sleep_aggregate();
        if sleep_edited || computed_night_replaces_bare_import {
            let from_computed = |value| Sourced::of(value, Namespace::Computed);
            resolved.total_sleep_min = from_computed(computed.total_sleep_min);
            resolved.efficiency = from_computed(computed.efficiency);
            resolved.deep_min = from_computed(computed.deep_min);
            resolved.rem_min = from_computed(computed.rem_min);
            resolved.light_min = from_computed(computed.light_min);
            resolved.disturbances = Sourced::of(computed.disturbances, Namespace::Computed);
        }
    }
    resolved
}

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
    fn richness(&self) -> u8 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(total: Option<f64>, efficiency: Option<f64>, stages: Option<f64>) -> DailyRow {
        DailyRow {
            total_sleep_min: total,
            efficiency,
            deep_min: stages,
            rem_min: stages,
            light_min: stages,
            disturbances: Some(3),
            ..DailyRow::default()
        }
    }

    fn session(namespace: Namespace, start_ts: i64, stages_json: Option<&str>) -> SessionRow {
        SessionRow {
            namespace,
            start_ts,
            end_ts: start_ts + 100,
            efficiency: None,
            resting_hr: None,
            avg_hrv: None,
            stages_json: stages_json.map(str::to_owned),
            user_edited: false,
            start_ts_adjusted: None,
            staging_sparse: None,
        }
    }

    #[test]
    fn imported_values_win_and_computed_fills_nulls() {
        let imported = DailyRow {
            strain: Some(3.1),
            ..DailyRow::default()
        };
        let computed = DailyRow {
            strain: Some(0.0),
            recovery: Some(61.0),
            ..DailyRow::default()
        };
        let merged = merge_daily(Some(&imported), Some(&computed), false);
        assert_eq!(merged.strain, Sourced::of(Some(3.1), Namespace::Imported));
        assert_eq!(
            merged.recovery,
            Sourced::of(Some(61.0), Namespace::Computed)
        );
        assert_eq!(
            merged.avg_hrv,
            Sourced {
                value: None,
                source: None
            }
        );
    }

    #[test]
    fn a_scored_computed_night_replaces_a_bare_imported_total() {
        let imported = row(Some(400.0), None, None);
        let computed = DailyRow {
            disturbances: None,
            ..row(Some(380.0), Some(0.9), Some(60.0))
        };
        let merged = merge_daily(Some(&imported), Some(&computed), false);
        assert_eq!(merged.total_sleep_min.value, Some(380.0));
        // The whole block moves, so the imported disturbance count gives way to a computed null.
        assert_eq!(
            merged.disturbances,
            Sourced {
                value: None,
                source: None
            }
        );

        // A bare computed total is no better than a bare import.
        let bare_computed = row(Some(380.0), None, None);
        let merged = merge_daily(Some(&imported), Some(&bare_computed), false);
        assert_eq!(merged.total_sleep_min.source, Some(Namespace::Imported));
    }

    #[test]
    fn an_edited_night_takes_the_computed_sleep_block() {
        let imported = row(Some(400.0), Some(0.8), Some(50.0));
        let computed = row(Some(380.0), None, Some(60.0));
        let merged = merge_daily(Some(&imported), Some(&computed), true);
        assert_eq!(merged.total_sleep_min.value, Some(380.0));
        assert_eq!(
            merged.efficiency,
            Sourced {
                value: None,
                source: None
            }
        );
        let merged = merge_daily(Some(&imported), Some(&computed), false);
        assert_eq!(merged.efficiency.value, Some(0.8));
    }

    #[test]
    fn a_single_row_is_used_as_is() {
        let computed = row(Some(380.0), None, Some(60.0));
        let merged = merge_daily(None, Some(&computed), true);
        assert_eq!(
            merged.deep_min,
            Sourced::of(Some(60.0), Namespace::Computed)
        );
        assert_eq!(
            merged.disturbances,
            Sourced::of(Some(3), Namespace::Computed)
        );
    }

    #[test]
    fn richer_computed_sessions_replace_imported_ones() {
        let full = r#"[{"start":0,"end":100,"stage":"deep"}]"#;
        let partial = r#"[{"start":0,"end":50,"stage":"deep"}]"#;
        let pick = |sessions| {
            merge_sleep_sessions(sessions)
                .iter()
                .map(|session| session.namespace)
                .collect::<Vec<_>>()
        };
        use Namespace::{Computed, Imported};

        assert_eq!(
            pick(vec![
                session(Imported, 0, None),
                session(Computed, 0, Some(partial))
            ]),
            [Computed]
        );
        assert_eq!(
            pick(vec![
                session(Imported, 0, Some(partial)),
                session(Computed, 0, Some(full))
            ]),
            [Computed]
        );
        // Ties keep every imported session.
        assert_eq!(
            pick(vec![
                session(Imported, 0, Some(full)),
                session(Imported, 200, None),
                session(Computed, 0, Some(full)),
            ]),
            [Imported, Imported]
        );
        assert_eq!(pick(vec![session(Computed, 0, Some("[]"))]), [Computed]);
    }

    #[test]
    fn richness_follows_noop_staging_coverage() {
        let rank = |stages: Option<&str>| session(Namespace::Computed, 0, stages).richness();
        assert_eq!(rank(None), 0);
        assert_eq!(rank(Some("  ")), 0);
        assert_eq!(rank(Some("[]")), 0);
        assert_eq!(rank(Some(r#"[{"start":0,"end":94,"stage":"deep"}]"#)), 1);
        assert_eq!(rank(Some(r#"[{"start":0,"end":95,"stage":"deep"}]"#)), 2);
        // Unmeasurable coverage (minutes object, stray values) still counts as full staging.
        assert_eq!(rank(Some(r#"{"deep":30}"#)), 2);
        assert_eq!(rank(Some(r#"[{"start":0,"end":50,"stage":"deep"},1]"#)), 2);
    }

    #[test]
    fn stage_minutes_reads_every_noop_shape() {
        let segments = r#"[{"start":0,"end":600,"stage":"deep"},{"start":600,"end":900,"stage":"wake"},
            {"start":900,"end":900,"stage":"rem"},{"start":900,"end":1020,"stage":"rem"},"noise"]"#;
        assert_eq!(
            stage_minutes(segments),
            Some(StageMinutes {
                awake: 5.0,
                light: 0.0,
                deep: 10.0,
                rem: 2.0
            })
        );
        assert_eq!(
            stage_minutes(r#"[{"stage":"light","min":12.5},{"stage":"awake","min":-1}]"#),
            Some(StageMinutes {
                light: 12.5,
                ..StageMinutes::default()
            })
        );
        assert_eq!(
            stage_minutes(r#"{"awake":1,"light":2,"deep":3,"rem":4}"#),
            Some(StageMinutes {
                awake: 1.0,
                light: 2.0,
                deep: 3.0,
                rem: 4.0
            })
        );
        assert_eq!(stage_minutes("[]"), None);
        assert_eq!(stage_minutes("not json"), None);
    }
}
