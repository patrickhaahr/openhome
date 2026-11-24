#[path = "../common/memory_noop_db.rs"]
mod memory_noop_db;
#[path = "../common/send_request.rs"]
mod send_request;
#[path = "../common/send_request_with_method.rs"]
mod send_request_with_method;
#[path = "../common/test_app.rs"]
mod test_app;
#[path = "../common/test_app_with_db.rs"]
mod test_app_with_db;
#[path = "../common/test_app_with_pools.rs"]
mod test_app_with_pools;

mod context_support;
mod mirror;
mod noop_workout_detail_support;
mod recovery_support;
mod reference_run;
mod workouts_support;

mod context_alignment;
mod context_limits;
mod metric_trend;
mod noop_workout_detail_heart_rate;
mod noop_workout_detail_lookup;
mod noop_workout_detail_recovery;
mod noop_workout_detail_zones;
mod recovery_days;
mod recovery_freshness;
mod recovery_journal;
mod recovery_precedence;
mod recovery_sleep_night;
mod sleep_recent;
mod workouts_coverage;
mod workouts_merge;
