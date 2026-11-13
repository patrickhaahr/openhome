use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_router,
};
use serde::Deserialize;

use super::OpenHomeTools;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecoveryOnDayRequest {
    /// The Europe/Copenhagen calendar day, written YYYY-MM-DD. The Sleep Night reported is the
    /// one that wakes on this day.
    pub day: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SleepRecentRequest {
    /// Number of Copenhagen wake days to read, from 1 through 14.
    pub n: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorkoutsOnDayRequest {
    /// The Europe/Copenhagen Training Day, written YYYY-MM-DD.
    pub day: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NoopWorkoutDetailRequest {
    /// The NOOP Workout's `source` device namespace, exactly as `workouts_on_day` lists it.
    pub source: String,
    /// The NOOP Workout's RFC 3339 `start`, as `workouts_on_day` lists it.
    pub start: String,
    /// The NOOP Workout's `sport` as `workouts_on_day` lists it, in any case. Needed only when
    /// another workout from the same source starts at the same instant.
    pub sport: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExerciseHistoryRequest {
    /// Exercise name, resolved case-insensitively; ambiguous names are an error.
    pub exercise_name: String,
    /// First Europe/Copenhagen calendar day, YYYY-MM-DD, included.
    pub from_day: String,
    /// Last Europe/Copenhagen calendar day, YYYY-MM-DD, included (at most 90 days total).
    pub to_day: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrendMetric {
    BodyWeight,
    SleepDuration,
    RestingHeartRate,
    Hrv,
}

impl TrendMetric {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::BodyWeight => "body_weight",
            Self::SleepDuration => "sleep_duration",
            Self::RestingHeartRate => "resting_heart_rate",
            Self::Hrv => "hrv",
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MetricTrendRequest {
    /// One of `body_weight`, `sleep_duration`, `resting_heart_rate`, or `hrv`.
    pub metric: TrendMetric,
    /// First Europe/Copenhagen calendar day, YYYY-MM-DD, included.
    pub from_day: String,
    /// Last Europe/Copenhagen calendar day, YYYY-MM-DD, included (at most 90 days total).
    pub to_day: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TrainingContextRequest {
    /// First Europe/Copenhagen calendar day, YYYY-MM-DD, included.
    pub from_day: String,
    /// Last Europe/Copenhagen calendar day, YYYY-MM-DD, included (at most 90 days total).
    pub to_day: String,
}

const RECOVERY_ON_DAY: &str = "Sleep and recovery for one Europe/Copenhagen calendar day, from \
the NOOP strap mirror. `sleep` is the Sleep Night that wakes on `day`: NOOP's daily totals plus the \
sessions ending that day; each session has `stage_min` totals and, when staged, `stages`: its \
awake/light/deep/rem segments in time order. Every value has a unit (sleep in minutes, efficiency as a 0-1 fraction, \
resting heart rate in beats/min, HRV as RMSSD in ms, skin temperature as a deviation in degC from the user's baseline rather than an \
absolute temperature, respiratory rate in breaths/min, SpO2 in percent, steps as a count) and names the NOOP namespace it was selected \
from. `derived_scores` are 0-100 recovery and strain scores from NOOP and a rest score OpenHome derives \
from NOOP's sleep data (source openhome-derived): model outputs, not \
measurements or diagnoses. `journal` lists the Journal Entries the user logged in NOOP against \
`day` (felt recovered, alcohol, illness, stress, supplements, ...), one per question with its yes/no \
answer, numeric value, notes and source; entries logged on `day` often explain the Sleep Night waking \
the next day. Null means no value. Before treating a null, a night without sessions or an empty \
journal as real, check `noop.freshness` (`unconfirmed`: complete recovery coverage for this day has \
not arrived yet) and `noop.coverage` (`covered`: an empty result is a recorded absence; `unknown`: \
rows may be missing; `coverage.journal` is the journal's).";

const SLEEP_RECENT: &str = "The most recent 1-14 Europe/Copenhagen Sleep Nights, newest wake day \
first. Each entry includes NOOP sleep duration and stages in minutes, efficiency as a 0-1 \
fraction, session start and end timestamps with offsets, ordered stage segments (`stages`), \
per-value source, and freshness and \
coverage. Missing nights remain in the list with null values; check `noop.coverage` to distinguish \
recorded gaps from unsynced data.";

const WORKOUTS_ON_DAY: &str = "Workouts on one Europe/Copenhagen Training Day. `workouts` are the \
Workouts logged in OpenHome with that date: ordered Exercises with their exact Sets (reps, \
added_weight_kg (null for bodyweight), hold_duration_s for Timed Holds, RPE 1-10). \
`noop_workouts` are the separate NOOP Workouts whose start falls on that local day, with start and \
end timestamps, NOOP's recorded duration in minutes, stored average and max heart rate in \
beats/min, NOOP's derived 0-100 strain score, energy in kcal and origin (manual, detected, ...). \
The two lists are never paired: a NOOP Workout on the same day is context, not proof it is the \
same session. Null means no value. A missing NOOP Workout never means the user did not train; \
check `noop.freshness` (`unconfirmed`: complete workout coverage for this day has not arrived yet) \
and `noop.coverage.workouts` (`covered`: NOOP recorded no other workout that day; `unknown`: \
rows may be missing). For one NOOP Workout's heart rate from the strap samples, its time in \
Heart Rate Zones, its Heart Rate Recovery, its heart rate series and its average pace, call \
noop_workout_detail with its `source`, `start` and `sport` as listed here.";

const NOOP_WORKOUT_DETAIL: &str = "One NOOP Workout in detail. Pass its `source`, `start` and \
`sport` exactly as workouts_on_day lists them; `start` is matched as an instant, so any UTC \
offset names the same workout. `sport` may be left out when no other workout from that source \
starts at the same instant; otherwise leaving it out is a 409 error naming the sports. Returns the workout's start and end, sport, origin, source, NOOP's recorded \
duration in minutes, distance in m, energy in kcal, steps and NOOP's derived 0-100 strain score, \
whether a GPS route was captured (`has_route`; the route's coordinates are private and are not \
returned here), plus \
the average pace in seconds per km (null without a positive distance). `hr` is the average, min \
and max heart rate in beats/min, always from the strap's samples whatever source recorded the \
workout: `hr.basis` is `strap_samples`, or `workout_row` when the strap has no samples in the \
workout and the values are NOOP's stored average and max (min is then null), or null when \
neither exists. `hr_series` is the shape of the session: `points` of mean strap heart rate \
per `bucket_s` bucket (15 s, widened to keep at most 300 points), each `t_s` seconds from the \
start; empty buckets are omitted, so a gap in the data stays a gap. `hr_series` is null without \
strap samples (`no_strap_samples`). `hr_zones` is the time in Heart Rate Zones 1-5, computed as the NOOP app does: \
inclusive lower edges at 50/60/70/80/90 % of the Profile's Max Heart Rate (echoed as \
`hrmax_used`, each zone with lower_bpm and upper_bpm), minutes and percent per zone (unrounded), \
and time below Zone 1 in `below_zone_min`, left out of the percentages. `hr_zones` is null when \
the Profile has no Max Heart Rate (`hr_max_not_configured`) or the strap has no samples in the \
workout (`no_strap_samples`). `hr_recovery` is Heart \
Rate Recovery as the NOOP app computes it: `end_hr_bpm`, the highest heart rate in the final 30 \
s, and `at_1_min`, `at_2_min` and `at_5_min`, each the median heart rate within 15 s of that \
time after the end with `drop_bpm` from `end_hr_bpm`, or null without 3 samples there. It is \
null when the profile has no Max Heart Rate, without strap samples, when the last 5 minutes \
lacked 2 minutes of continuous effort at 70% of Max Heart Rate, or with under 3 samples in the \
final 30 s. `unavailable` names why any null block is null. `noop` holds the freshness and \
coverage of the workouts on the Europe/Copenhagen Training Day the workout starts on. A \
workout that workouts_on_day does not list, including a row NOOP merged into another, is a 404 \
error.";

const EXERCISE_HISTORY: &str = "Exact Set history for one Exercise over 1-90 inclusive \
Europe/Copenhagen calendar days. Resolves exercise_name case-insensitively; an unknown name or \
ambiguous match is an error. Returns dated Workouts in chronological order, repeated Exercise \
entries in logged order, and Sets by set_number. Each Set includes reps, added_weight_kg (null \
for bodyweight), hold_duration_s for Timed Holds, RPE 1-10, and notes; missing values are null. \
An Exercise with no performances has workouts: []. Both from_day and to_day are included.";

const METRIC_TREND: &str = "Daily Body Weight, sleep duration, resting heart rate, or HRV over \
1-90 inclusive Europe/Copenhagen calendar days. metric is exactly body_weight, sleep_duration, \
resting_heart_rate, or hrv. Each day has a value or null, unit, source, and relevant NOOP \
freshness and coverage. Weekly summaries use Monday-Sunday ISO weeks, showing the mean of \
observed days and counts; a missing day is never zero.";

const TRAINING_CONTEXT: &str = "Compact view of a training block over 1-90 inclusive \
Europe/Copenhagen calendar days, one entry per day, oldest first, including days with nothing \
logged. Each day summarizes the Workouts logged in OpenHome that date (per Exercise entry: Set \
count, best reps, best added_weight_kg (null for bodyweight), best hold_duration_s, best RPE, \
volume as reps (or hold seconds) times added weight) beside the separate NOOP Workouts starting that local day, which are never paired \
with a logged Workout. It also has daily Body Weight in kg and the Recovery Day's sleep duration \
in minutes, resting heart rate in beats/min, and HRV as RMSSD in ms, each with its source. Null \
means no value; before treating a gap as real, check the day's `noop.recovery` (for sleep, resting \
heart rate and HRV) or `noop.workouts` (for NOOP Workouts) freshness and coverage. Use exercise_history or workouts_on_day for exact Sets and recovery_on_day for sleep \
sessions.";

#[tool_router(router = training_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = TRAINING_CONTEXT,
        annotations(
            title = "Training Context",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn training_context(
        &self,
        Parameters(TrainingContextRequest { from_day, to_day }): Parameters<TrainingContextRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "training", "context", &from_day, &to_day])
            .await
    }

    #[tool(
        description = METRIC_TREND,
        annotations(
            title = "Metric trend",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn metric_trend(
        &self,
        Parameters(MetricTrendRequest {
            metric,
            from_day,
            to_day,
        }): Parameters<MetricTrendRequest>,
    ) -> CallToolResult {
        self.api
            .get(&[
                "api",
                "training",
                "trends",
                metric.as_str(),
                &from_day,
                &to_day,
            ])
            .await
    }

    #[tool(
        description = SLEEP_RECENT,
        annotations(
            title = "Recent Sleep Nights",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn sleep_recent(
        &self,
        Parameters(SleepRecentRequest { n }): Parameters<SleepRecentRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "training", "sleep", "recent", &n.to_string()])
            .await
    }

    #[tool(
        description = EXERCISE_HISTORY,
        annotations(
            title = "Exercise Set history",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn exercise_history(
        &self,
        Parameters(ExerciseHistoryRequest {
            exercise_name,
            from_day,
            to_day,
        }): Parameters<ExerciseHistoryRequest>,
    ) -> CallToolResult {
        self.api
            .get(&[
                "api",
                "training",
                "exercises",
                &exercise_name,
                "history",
                &from_day,
                &to_day,
            ])
            .await
    }

    #[tool(
        description = RECOVERY_ON_DAY,
        annotations(
            title = "Recovery on a day",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn recovery_on_day(
        &self,
        Parameters(RecoveryOnDayRequest { day }): Parameters<RecoveryOnDayRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "training", "days", &day, "recovery"])
            .await
    }

    #[tool(
        description = WORKOUTS_ON_DAY,
        annotations(
            title = "Workouts on a day",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn workouts_on_day(
        &self,
        Parameters(WorkoutsOnDayRequest { day }): Parameters<WorkoutsOnDayRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "training", "days", &day, "workouts"])
            .await
    }

    #[tool(
        description = NOOP_WORKOUT_DETAIL,
        annotations(
            title = "NOOP Workout detail",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn noop_workout_detail(
        &self,
        Parameters(NoopWorkoutDetailRequest {
            source,
            start,
            sport,
        }): Parameters<NoopWorkoutDetailRequest>,
    ) -> CallToolResult {
        let mut segments = vec!["api", "training", "noop-workouts", &source, &start];
        segments.extend(sport.as_deref());
        self.api.get(&segments).await
    }
}
