use serde::Serialize;

/// Where NOOP says a workout row came from, classified from its stored `source` like NOOP's
/// `WorkoutEditing.classify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkoutOrigin {
    /// Logged in NOOP by the user, live or afterwards.
    Manual,
    /// A bout NOOP's detector derived from strap heart rate.
    Detected,
    /// Imported from a WHOOP export.
    Whoop,
    /// Imported from Apple Health or Health Connect; NOOP's fallback for unrecognised sources.
    HealthImport,
    /// Imported strength session (Hevy, Liftosaur).
    Lifting,
    /// Imported GPX, TCX or FIT file.
    ActivityFile,
}

impl WorkoutOrigin {
    pub fn classify(source: &str) -> Self {
        let source = source.to_lowercase();
        // The detector writes the computed device id, which also contains "whoop".
        if source.ends_with("-noop") {
            Self::Detected
        } else if source == "manual" {
            Self::Manual
        } else if source == "lifting" {
            Self::Lifting
        } else if source == "activity-file" {
            Self::ActivityFile
        } else if source.contains("whoop") {
            Self::Whoop
        } else {
            Self::HealthImport
        }
    }
}

/// The `workout` fields a Training Day reads; zones are only checked for presence and routes are
/// never selected.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkoutRow {
    pub device_id: String,
    pub start_ts: i64,
    pub end_ts: i64,
    pub sport: String,
    pub origin: WorkoutOrigin,
    pub duration_s: Option<f64>,
    pub energy_kcal: Option<f64>,
    pub avg_hr: Option<f64>,
    pub max_hr: Option<f64>,
    pub strain: Option<f64>,
    pub distance_m: Option<f64>,
    pub steps: Option<i64>,
    pub notes: Option<String>,
    pub has_zones: bool,
}

impl WorkoutRow {
    /// Captured signals, NOOP's tiebreak between two rows of one activity.
    fn richness(&self) -> u8 {
        [
            self.avg_hr.is_some(),
            self.max_hr.is_some(),
            self.strain.is_some(),
            self.has_zones,
            self.distance_m.is_some_and(|distance| distance > 0.0),
            self.energy_kcal.is_some_and(|energy| energy > 0.0),
        ]
        .into_iter()
        .filter(|&signal| signal)
        .count() as u8
    }

    /// Case- and space-insensitive sport, so `TraditionalStrengthTraining` matches `Traditional
    /// Strength Training` and `detected` matches `Activity`.
    fn sport_key(&self) -> String {
        let sport = if self.sport == "detected" {
            "Activity"
        } else {
            &self.sport
        };
        sport
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    /// The windows overlap by more than half of the shorter session.
    fn mostly_overlaps(&self, other: &Self) -> bool {
        let overlap = self.end_ts.min(other.end_ts) - self.start_ts.max(other.start_ts);
        let shorter = (self.end_ts - self.start_ts)
            .min(other.end_ts - other.start_ts)
            .max(1);
        overlap > 0 && overlap as f64 > 0.5 * shorter as f64
    }
}

/// Selects the distinct NOOP Workouts among all namespaces' rows. The caller passes the imported
/// strap first, then computed strap, then other namespaces, each by start. Returns by start.
pub fn merge_workouts(rows: Vec<WorkoutRow>) -> Vec<WorkoutRow> {
    // A detected bout that mostly overlaps a real session is its shadow, whatever the sport.
    let is_detected = |row: &WorkoutRow| row.origin == WorkoutOrigin::Detected;
    let shadows: Vec<bool> = rows
        .iter()
        .map(|row| {
            is_detected(row)
                && rows
                    .iter()
                    .any(|real| !is_detected(real) && row.mostly_overlaps(real))
        })
        .collect();

    // The same activity recorded twice keeps the richer row; on a tie a strap-native row beats a
    // health import, then the longer session, then the earlier row.
    let mut kept: Vec<WorkoutRow> = Vec::new();
    'rows: for (row, shadow) in rows.into_iter().zip(shadows) {
        if shadow {
            continue;
        }
        for kept_row in &mut kept {
            if kept_row.sport_key() == row.sport_key() && kept_row.mostly_overlaps(&row) {
                if preferred(&row, kept_row) {
                    *kept_row = row;
                }
                continue 'rows;
            }
        }
        kept.push(row);
    }
    kept.sort_by_key(|row| (row.start_ts, row.end_ts));
    kept
}

/// Whether `candidate` replaces `kept`, NOOP's `preferred(kept, candidate)`.
fn preferred(candidate: &WorkoutRow, kept: &WorkoutRow) -> bool {
    let (candidate_richness, kept_richness) = (candidate.richness(), kept.richness());
    if candidate_richness != kept_richness {
        return candidate_richness > kept_richness;
    }
    let is_import = |row: &WorkoutRow| row.origin == WorkoutOrigin::HealthImport;
    if is_import(candidate) != is_import(kept) {
        return is_import(kept);
    }
    candidate.end_ts - candidate.start_ts > kept.end_ts - kept.start_ts
}
