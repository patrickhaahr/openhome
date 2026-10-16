//! Synthetic NOOP strap data, pushed through the real ingestion protocol so the training reads see
//! exactly what a synced installation produces. Nights, workouts and journal answers come from
//! `seed/training.json`; sleep stages are generated as ordered 90-minute cycles and the daily
//! totals are computed from them, so the two always agree.

use chrono::{Days, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe::Copenhagen;
use openhome_api::services::noop_push::ingest::{self, ContentCoding};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::seed::today;

const SOURCE: &str = "00000000-0000-4000-8000-000000000a11";
const STRAP: &str = "my-whoop";
const COMPUTED: &str = "my-whoop-noop";

#[derive(Deserialize)]
struct Training {
    nights: Vec<Night>,
    naps: Vec<Nap>,
    journal: Vec<JournalDay>,
    workouts: Vec<Workout>,
}

#[derive(Deserialize)]
struct Night {
    days_ago: u64,
    bed: String,
    wake: String,
    recovery: f64,
    strain: f64,
    resting_hr: f64,
    hrv_rmssd: f64,
    skin_temp_dev_c: f64,
    resp_rate_bpm: f64,
}

#[derive(Deserialize)]
struct Nap {
    days_ago: u64,
    start: String,
    minutes: i64,
}

#[derive(Deserialize)]
struct JournalDay {
    days_ago: u64,
    entries: Vec<JournalEntry>,
}

#[derive(Deserialize)]
struct JournalEntry {
    question: String,
    answered_yes: bool,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct Workout {
    days_ago: u64,
    start: String,
    sport: String,
    minutes: i64,
    source: String,
    avg_hr: f64,
    max_hr: f64,
    kcal: f64,
    strain: f64,
    distance_m: Option<f64>,
    steps: Option<i64>,
    shape: String,
    notes: Option<String>,
}

/// Small deterministic generator, so a restart shows the same nights.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }

    fn between(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.next()
    }
}

fn day_at(day: NaiveDate, clock: &str) -> anyhow::Result<i64> {
    let time = NaiveTime::parse_from_str(clock, "%H:%M")?;
    Copenhagen
        .from_local_datetime(&day.and_time(time))
        .earliest()
        .map(|at| at.timestamp())
        .ok_or_else(|| anyhow::anyhow!("{day} {clock} does not exist in Copenhagen"))
}

fn midnight(day: NaiveDate) -> anyhow::Result<i64> {
    day_at(day, "00:00")
}

fn back(days: u64) -> NaiveDate {
    today()
        .checked_sub_days(Days::new(days))
        .expect("seed dates are recent")
}

#[derive(Clone, Copy)]
struct Segment {
    start: i64,
    end: i64,
    stage: &'static str,
}

/// Ordered stages over `[start, end)`: a brief wake to fall asleep, then 90-minute cycles of
/// light, deep, light and REM sleep. Deep sleep dominates early cycles, REM late ones, and each
/// cycle closes with a brief wake.
fn stages(start: i64, end: i64, seed: u64, wakes: bool) -> Vec<Segment> {
    let mut rng = Lcg(seed.wrapping_mul(0x9e37_79b9).wrapping_add(7));
    let mut segments: Vec<Segment> = Vec::new();
    let push = |segments: &mut Vec<Segment>, from: i64, to: i64, stage: &'static str| {
        let to = to.min(end);
        if to - from < 30 {
            return;
        }
        match segments.last_mut() {
            Some(last) if last.stage == stage && last.end == from => last.end = to,
            _ => segments.push(Segment {
                start: from,
                end: to,
                stage,
            }),
        }
    };
    let mut at = start;
    if wakes {
        let latency = (rng.between(4.0, 11.0) * 60.0) as i64;
        push(&mut segments, at, at + latency, "wake");
        at += latency;
    }
    let mut cycle = 0.0_f64;
    while at < end {
        let length = 5400.0 * rng.between(0.9, 1.1);
        let deep = length * (0.34 - 0.09 * cycle).max(0.04) * rng.between(0.85, 1.15);
        let rem = length * (0.10 + 0.075 * cycle).min(0.38) * rng.between(0.85, 1.15);
        let wake = if wakes && rng.next() < 0.85 {
            rng.between(120.0, 480.0)
        } else {
            0.0
        };
        let light = (length - deep - rem - wake).max(600.0);
        let first_light = light * rng.between(0.3, 0.45);
        for (seconds, stage) in [
            (first_light, "light"),
            (deep, "deep"),
            (light - first_light, "light"),
            (rem, "rem"),
            (wake, "wake"),
        ] {
            let seconds = (seconds / 30.0).round() as i64 * 30;
            push(&mut segments, at, at + seconds, stage);
            at = (at + seconds).min(end);
        }
        cycle += 1.0;
    }
    if let Some(last) = segments.last_mut() {
        last.end = end;
    }
    segments
}

#[derive(Default, Clone, Copy)]
struct Totals {
    light: f64,
    deep: f64,
    rem: f64,
    wake: f64,
    wakes: i64,
}

impl Totals {
    fn add(&mut self, segments: &[Segment], night: bool) {
        for (index, segment) in segments.iter().enumerate() {
            let minutes = (segment.end - segment.start) as f64 / 60.0;
            match segment.stage {
                "light" => self.light += minutes,
                "deep" => self.deep += minutes,
                "rem" => self.rem += minutes,
                _ => {
                    self.wake += minutes;
                    // The first wake is falling asleep; later ones are disturbances.
                    if night && index > 0 {
                        self.wakes += 1;
                    }
                }
            }
        }
    }

    fn asleep(&self) -> f64 {
        self.light + self.deep + self.rem
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Heart rate every ten seconds from the start to five minutes after the end, following the
/// workout's shape and then recovering towards a resting rate.
fn heart_rate(workout: &Workout, start: i64, end: i64, seed: u64) -> Vec<(i64, i64)> {
    let mut rng = Lcg(seed.wrapping_mul(0x2545_f491).wrapping_add(11));
    let resting = 66.0;
    let mut last_working = resting;
    let mut samples = Vec::new();
    let mut at = start;
    while at <= end + 330 {
        let elapsed = (at - start) as f64;
        let bpm = if at <= end {
            let ramp = (elapsed / 240.0).min(1.0);
            let remaining = (end - at) as f64;
            let target = match workout.shape.as_str() {
                // Sets of about a minute with rests between, finishing on a hard effort.
                "sets" => {
                    let phase = elapsed % 150.0;
                    let work = if phase < 65.0 { 22.0 } else { -18.0 };
                    let finisher = if remaining < 240.0 {
                        workout.max_hr - workout.avg_hr
                    } else {
                        0.0
                    };
                    workout.avg_hr + work + finisher.max(0.0) * 0.9
                }
                "steady" => {
                    workout.avg_hr
                        + 10.0 * elapsed / (end - start) as f64
                        + if remaining < 200.0 {
                            workout.max_hr - workout.avg_hr - 8.0
                        } else {
                            0.0
                        }
                }
                _ => workout.avg_hr + 6.0 * (elapsed / 400.0).sin(),
            };
            let value = resting + (target - resting) * ramp + rng.between(-3.0, 3.0);
            last_working = value;
            value
        } else {
            resting
                + (last_working - resting) * (-((at - end) as f64) / 150.0).exp()
                + rng.between(-1.5, 1.5)
        };
        samples.push((at, bpm.round() as i64));
        at += 10;
    }
    samples
}

async fn push(
    db: &SqlitePool,
    device: &str,
    stream: &str,
    window: Option<(Value, Value, &str)>,
    records: Vec<Value>,
) -> anyhow::Result<()> {
    let mut header = json!({
        "type": "batch", "protocolVersion": "1.0", "batchId": Uuid::new_v4(),
        "sourceId": SOURCE, "deviceId": device, "stream": stream,
        "recordCount": records.len(),
    });
    if let Some((lower, upper, selector)) = window {
        header["delivery"] = json!("replace_window");
        header["startCursor"] = Value::Null;
        header["endCursor"] = Value::Null;
        header["window"] = json!({
            "replacementId": Uuid::new_v4(), "selector": selector,
            "startInclusive": lower, "endExclusive": upper, "part": 1, "parts": 1,
        });
    } else {
        header["delivery"] = json!("append");
        header["startCursor"] = Value::Null;
        header["endCursor"] = json!({ "rowId": records.len(), "keySha256": "0".repeat(64) });
    }
    let mut entity = String::new();
    for line in std::iter::once(header).chain(records) {
        entity.push_str(&line.to_string());
        entity.push('\n');
    }
    ingest::apply(
        db,
        ingest::prepare(ContentCoding::Identity, entity.into_bytes())?,
    )
    .await?;
    Ok(())
}

pub async fn seed(db: &SqlitePool) -> anyhow::Result<()> {
    let training: Training = serde_json::from_str(include_str!("seed/training.json"))?;
    let now = Utc::now().timestamp();
    let oldest = training
        .nights
        .iter()
        .map(|night| night.days_ago)
        .max()
        .unwrap_or(0)
        + 8;
    let from_day = back(oldest).to_string();
    let to_day = today()
        .checked_add_days(Days::new(1))
        .expect("tomorrow exists")
        .to_string();
    let from_ts = midnight(back(oldest))?;
    let to_ts = midnight(
        today()
            .checked_add_days(Days::new(1))
            .expect("tomorrow exists"),
    )?;

    let mut daily = Vec::new();
    let mut sessions = Vec::new();
    for night in &training.nights {
        let wake_day = back(night.days_ago);
        let bed_minutes = NaiveTime::parse_from_str(&night.bed, "%H:%M")?;
        // Bedtimes after noon belong to the evening before the wake day.
        let bed_day = if bed_minutes >= NaiveTime::from_hms_opt(12, 0, 0).expect("noon") {
            wake_day
                .checked_sub_days(Days::new(1))
                .expect("previous day")
        } else {
            wake_day
        };
        let start = day_at(bed_day, &night.bed)?;
        // Keep today's night in the past even when the preview runs before the usual wake time.
        let end = day_at(wake_day, &night.wake)?.min(now - 300);
        let mut totals = Totals::default();
        let night_stages = stages(start, end, night.days_ago + 1, true);
        totals.add(&night_stages, true);
        sessions.push(session(start, end, night, &night_stages, totals));
        for nap in training
            .naps
            .iter()
            .filter(|nap| nap.days_ago == night.days_ago)
        {
            let nap_start = day_at(wake_day, &nap.start)?.min(now - 3600);
            let nap_end = nap_start + nap.minutes * 60;
            let nap_stages = stages(nap_start, nap_end, night.days_ago + 500, false);
            let mut nap_totals = Totals::default();
            nap_totals.add(&nap_stages, false);
            totals.add(&nap_stages, false);
            sessions.push(session(nap_start, nap_end, night, &nap_stages, nap_totals));
        }
        let workouts_today = training
            .workouts
            .iter()
            .filter(|w| w.days_ago == night.days_ago)
            .count();
        let in_bed = totals.asleep() + totals.wake;
        daily.push(json!({
            "type": "record", "key": { "day": wake_day.to_string() }, "data": {
                "totalSleepMin": round1(totals.asleep()), "efficiency": (totals.asleep() / in_bed * 1000.0).round() / 1000.0,
                "deepMin": round1(totals.deep), "remMin": round1(totals.rem), "lightMin": round1(totals.light),
                "disturbances": totals.wakes, "restingHr": night.resting_hr, "avgHrv": night.hrv_rmssd,
                "recovery": night.recovery, "strain": night.strain, "exerciseCount": workouts_today,
                "spo2Pct": null, "skinTempDevC": night.skin_temp_dev_c, "respRateBpm": night.resp_rate_bpm,
                "steps": null, "activeKcalEst": null, "spo2Red": null, "spo2Ir": null,
            }
        }));
    }

    let mut journal = Vec::new();
    for day in &training.journal {
        for entry in &day.entries {
            journal.push(json!({
                "type": "record", "key": { "day": back(day.days_ago).to_string(), "question": entry.question },
                "data": { "answeredYes": entry.answered_yes, "numericValue": null, "notes": entry.notes },
            }));
        }
    }

    let mut workouts = Vec::new();
    let mut hr = Vec::new();
    for (index, workout) in training.workouts.iter().enumerate() {
        let duration = workout.minutes * 60;
        let mut start = day_at(back(workout.days_ago), &workout.start)?;
        // Today's workout finishes, with its recovery minutes, before the present moment.
        start = start.min(now - duration - 340);
        let end = start + duration;
        hr.extend(heart_rate(workout, start, end, index as u64 + 1));
        workouts.push(json!({
            "type": "record", "key": { "startTs": start, "sport": workout.sport }, "data": {
                "endTs": end, "source": workout.source, "durationS": duration as f64,
                "energyKcal": workout.kcal, "avgHr": workout.avg_hr, "maxHr": workout.max_hr,
                "strain": workout.strain, "distanceM": workout.distance_m, "zonesJSON": null,
                "notes": workout.notes, "routePolyline": null, "steps": workout.steps,
            }
        }));
    }

    let days = |lower: &str, upper: &str| Some((json!(lower), json!(upper), "day"));
    let starts = || Some((json!(from_ts), json!(to_ts), "startTs"));
    // The strap carries the data; NOOP's computed namespace reports applied, empty windows.
    push(db, STRAP, "dailyMetric", days(&from_day, &to_day), daily).await?;
    push(db, STRAP, "sleepSession", starts(), sessions).await?;
    push(db, STRAP, "workout", starts(), workouts).await?;
    push(db, STRAP, "journal", days(&from_day, &to_day), journal).await?;
    push(
        db,
        COMPUTED,
        "dailyMetric",
        days(&from_day, &to_day),
        vec![],
    )
    .await?;
    push(db, COMPUTED, "sleepSession", starts(), vec![]).await?;
    push(db, COMPUTED, "workout", starts(), vec![]).await?;
    push(db, COMPUTED, "journal", days(&from_day, &to_day), vec![]).await?;
    hr.sort_unstable();
    hr.dedup_by_key(|sample| sample.0);
    let hr = hr
        .into_iter()
        .map(|(ts, bpm)| json!({ "type": "record", "key": { "ts": ts }, "data": { "bpm": bpm } }))
        .collect();
    push(db, STRAP, "hrSample", None, hr).await
}

fn session(start: i64, end: i64, night: &Night, segments: &[Segment], totals: Totals) -> Value {
    let segments: Vec<Value> = segments
        .iter()
        .map(|s| json!({ "start": s.start, "end": s.end, "stage": s.stage }))
        .collect();
    let in_bed = totals.asleep() + totals.wake;
    json!({
        "type": "record", "key": { "startTs": start }, "data": {
            "endTs": end, "efficiency": (totals.asleep() / in_bed * 1000.0).round() / 1000.0,
            "restingHr": night.resting_hr, "avgHrv": night.hrv_rmssd,
            "stagesJSON": Value::Array(segments).to_string(), "userEdited": false,
            "startTsAdjusted": null, "motionJSON": null, "sleepStateJSON": null, "stagingSparse": false,
        }
    })
}
