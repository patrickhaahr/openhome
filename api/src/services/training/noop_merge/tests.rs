use super::sleep::StageSegment;
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
    assert_eq!(
        merged.disturbances,
        Sourced {
            value: None,
            source: None
        }
    );

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
    use Namespace::{Computed, Imported};

    let full = r#"[{"start":0,"end":100,"stage":"deep"}]"#;
    let partial = r#"[{"start":0,"end":50,"stage":"deep"}]"#;
    let pick = |sessions| {
        merge_sleep_sessions(sessions)
            .iter()
            .map(|session| session.namespace)
            .collect::<Vec<_>>()
    };

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
    assert_eq!(rank(Some(r#"{"deep":30}"#)), 2);
    assert_eq!(rank(Some(r#"[{"start":0,"end":50,"stage":"deep"},1]"#)), 2);
}

#[test]
fn stage_segments_are_ordered_clamped_and_named_like_stage_minutes() {
    let segments = r#"[{"start":600,"end":900,"stage":"wake"},{"start":0,"end":600,"stage":"deep"},
        {"start":900,"end":900,"stage":"rem"},{"start":900,"end":2000,"stage":"light"},
        {"start":10,"end":20,"stage":"mystery"},"noise"]"#;
    let segment = |start, end, stage| StageSegment { start, end, stage };
    assert_eq!(
        stage_segments(segments, 100, 1000),
        Some(vec![
            segment(100, 600, "deep"),
            segment(600, 900, "awake"),
            segment(900, 1000, "light"),
        ])
    );
    assert_eq!(stage_segments(segments, 5000, 6000), None);
    assert_eq!(
        stage_segments(r#"[{"stage":"light","min":12.5}]"#, 0, 100),
        None
    );
    assert_eq!(
        stage_segments(r#"{"awake":1,"light":2,"deep":3,"rem":4}"#, 0, 100),
        None
    );
    assert_eq!(stage_segments("not json", 0, 100), None);
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

fn workout(
    namespace: Namespace,
    start_ts: i64,
    end_ts: i64,
    sport: &str,
    source: &str,
) -> WorkoutRow {
    WorkoutRow {
        device_id: namespace.device_id().to_owned(),
        start_ts,
        end_ts,
        sport: sport.to_owned(),
        origin: WorkoutOrigin::classify(source),
        duration_s: Some((end_ts - start_ts) as f64),
        energy_kcal: None,
        avg_hr: None,
        max_hr: None,
        strain: None,
        distance_m: None,
        steps: None,
        notes: None,
        has_zones: false,
    }
}

#[test]
fn workout_origin_follows_noop_classification() {
    for (source, origin) in [
        ("my-whoop-noop", WorkoutOrigin::Detected),
        ("manual", WorkoutOrigin::Manual),
        ("whoop", WorkoutOrigin::Whoop),
        ("lifting", WorkoutOrigin::Lifting),
        ("activity-file", WorkoutOrigin::ActivityFile),
        ("apple_health", WorkoutOrigin::HealthImport),
        ("health-connect", WorkoutOrigin::HealthImport),
    ] {
        assert_eq!(WorkoutOrigin::classify(source), origin, "{source}");
    }
}

#[test]
fn distinct_workouts_stay_distinct() {
    let rows = vec![
        workout(Namespace::Imported, 1_000, 2_000, "Calisthenics", "manual"),
        workout(Namespace::Imported, 2_000, 3_000, "Calisthenics", "manual"),
        workout(Namespace::Imported, 1_000, 2_000, "Running", "manual"),
        workout(Namespace::Imported, 2_500, 3_500, "Calisthenics", "manual"),
    ];
    assert_eq!(merge_workouts(rows).len(), 4);
}

#[test]
fn one_activity_recorded_twice_keeps_the_richer_row() {
    let thin = workout(
        Namespace::Imported,
        1_000,
        5_000,
        "Traditional Strength Training",
        "health-connect",
    );
    let rich = WorkoutRow {
        avg_hr: Some(81.0),
        max_hr: Some(134.0),
        ..workout(
            Namespace::Imported,
            1_100,
            4_900,
            "TraditionalStrengthTraining",
            "manual",
        )
    };
    assert_eq!(
        merge_workouts(vec![thin.clone(), rich.clone()]),
        std::slice::from_ref(&rich)
    );
    assert_eq!(merge_workouts(vec![rich.clone(), thin.clone()]), [rich]);

    let manual = workout(
        Namespace::Imported,
        1_100,
        4_900,
        "Traditional Strength Training",
        "manual",
    );
    assert_eq!(
        merge_workouts(vec![thin, manual.clone()]),
        std::slice::from_ref(&manual)
    );
    let longer = workout(
        Namespace::Computed,
        1_000,
        5_000,
        "Traditional Strength Training",
        "manual",
    );
    assert_eq!(
        merge_workouts(vec![manual.clone(), longer.clone()]),
        [longer]
    );
    let copy = WorkoutRow {
        device_id: COMPUTED_DEVICE_ID.to_owned(),
        ..manual.clone()
    };
    assert_eq!(merge_workouts(vec![manual.clone(), copy]), [manual]);
}

#[test]
fn a_detected_bout_shadowing_a_real_session_is_dropped() {
    let real = workout(Namespace::Imported, 1_000, 4_000, "Calisthenics", "manual");
    let shadow = WorkoutRow {
        avg_hr: Some(120.0),
        strain: Some(9.0),
        ..workout(Namespace::Computed, 800, 4_200, "detected", "my-whoop-noop")
    };
    let separate = workout(
        Namespace::Computed,
        10_000,
        11_000,
        "detected",
        "my-whoop-noop",
    );
    assert_eq!(
        merge_workouts(vec![real.clone(), shadow.clone(), separate.clone()]),
        [real, separate]
    );
    assert_eq!(merge_workouts(vec![shadow.clone()]), [shadow]);
}

#[test]
fn workouts_are_ordered_by_start() {
    let late = workout(Namespace::Imported, 5_000, 6_000, "Running", "manual");
    let early = workout(
        Namespace::Computed,
        1_000,
        2_000,
        "detected",
        "my-whoop-noop",
    );
    assert_eq!(
        merge_workouts(vec![late.clone(), early.clone()]),
        [early, late]
    );
}
