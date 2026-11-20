mod common;
mod training_recovery_support;

use common::mirror::Mirror;
use common::mirror::Window;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_recovery_support::{
    COMPUTED, IMPORTED, REAL_DAYS, REAL_PUSH_AT, REAL_STARTS, SEP_20, SOURCE, absent, daily,
    measured, real_imported_days, real_mirror, sleep,
};

const OTHER_SOURCE: &str = "3a3486dd-5030-4e17-a00d-a781399890f9";

#[tokio::test]
async fn freshness_follows_the_older_namespace_push() {
    let mut mirror = real_mirror().await;
    let freshness = |body: &Value| body["noop"]["freshness"].clone();

    assert_eq!(
        freshness(&mirror.recovery_day("2026-09-25").await),
        "confirmed"
    );
    assert_eq!(
        freshness(&mirror.recovery_day("2026-09-26").await),
        "partial"
    );
    let missing = mirror.recovery_day("2026-09-27").await;
    assert_eq!(freshness(&missing), "unconfirmed");
    assert_eq!(missing["derived_scores"]["strain"], absent("score_0_100"));

    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Days("2026-09-27", "2026-09-28"),
            &[daily("2026-09-27", json!({"strain": 5.2}))],
            "2026-09-28T04:00:00.000Z",
        )
        .await;
    let half_synced = mirror.recovery_day("2026-09-27").await;
    assert_eq!(
        half_synced["derived_scores"]["strain"],
        measured(json!(5.2), "score_0_100", IMPORTED)
    );
    assert_eq!(
        half_synced["noop"]["last_push_at"],
        "2026-09-26T03:51:38+02:00"
    );
    assert_eq!(freshness(&half_synced), "unconfirmed");
    assert_eq!(half_synced["noop"]["coverage"]["daily_metrics"], "unknown");

    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Days("2026-09-27", "2026-09-28"),
            &[daily(
                "2026-09-27",
                json!({"strain": 4.0, "recovery": 58.0}),
            )],
            "2026-09-28T05:00:00.000Z",
        )
        .await;
    let synced = mirror.recovery_day("2026-09-27").await;
    assert_eq!(synced["noop"]["last_push_at"], "2026-09-26T03:51:38+02:00");
    assert_eq!(freshness(&synced), "unconfirmed");
    assert_eq!(
        synced["derived_scores"],
        json!({
            "recovery": measured(json!(58.0), "score_0_100", COMPUTED),
            "strain": measured(json!(5.2), "score_0_100", IMPORTED),
            "rest": absent("score_0_100"),
        })
    );
    assert_eq!(
        synced["noop"]["coverage"],
        json!({"daily_metrics": "covered", "sleep_sessions": "unknown", "journal": "unknown"})
    );

    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                SOURCE,
                device,
                Window::Starts(1_790_373_600, 1_790_546_400),
                &[],
                "2026-09-28T06:00:00.000Z",
            )
            .await;
    }
    let completed = mirror.recovery_day("2026-09-27").await;
    assert_eq!(
        completed["noop"]["last_push_at"],
        "2026-09-28T06:00:00+02:00"
    );
    assert_eq!(freshness(&completed), "confirmed");
}

#[tokio::test]
async fn staged_replacements_do_not_advance_recovery_freshness() {
    let mut mirror = real_mirror().await;
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push_first_part(
                SOURCE,
                device,
                REAL_DAYS,
                &[daily("2026-09-26", json!({"recovery": 75.0}))],
                2,
                "2026-09-27T04:00:00.000Z",
            )
            .await;
    }

    let body = mirror.recovery_day("2026-09-26").await;
    assert_eq!(body["noop"]["freshness"], "partial");
    assert_eq!(body["noop"]["last_push_at"], "2026-09-26T03:51:38+02:00");
    assert_eq!(body["derived_scores"]["recovery"], absent("score_0_100"));
}

#[tokio::test]
async fn confirming_recovery_requires_completed_daily_and_sleep_windows() {
    let mut mirror = real_mirror().await;
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(SOURCE, device, REAL_DAYS, &[], "2026-09-27T04:00:00.000Z")
            .await;
    }
    let daily_only = mirror.recovery_day("2026-09-26").await;
    assert_eq!(daily_only["noop"]["freshness"], "partial");
    assert_eq!(
        daily_only["noop"]["last_push_at"],
        "2026-09-26T03:51:38+02:00"
    );

    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(SOURCE, device, REAL_STARTS, &[], "2026-09-27T05:00:00.000Z")
            .await;
    }
    let completed = mirror.recovery_day("2026-09-26").await;
    assert_eq!(completed["noop"]["freshness"], "confirmed");
    assert_eq!(
        completed["noop"]["last_push_at"],
        "2026-09-27T06:00:00+02:00"
    );
}

#[tokio::test]
async fn historical_replacements_do_not_confirm_a_more_recent_day() {
    let mut mirror = real_mirror().await;
    for device in [IMPORTED, COMPUTED] {
        for window in [
            Window::Days("2026-09-19", "2026-09-21"),
            Window::Starts(SEP_20 - 86_400, SEP_20 + 86_400),
        ] {
            mirror
                .push(SOURCE, device, window, &[], "2026-09-27T04:00:00.000Z")
                .await;
        }
    }
    let body = mirror.recovery_day("2026-09-26").await;
    assert_eq!(body["noop"]["freshness"], "partial");
    assert_eq!(
        body["noop"]["coverage"],
        json!({
            "daily_metrics": "covered", "sleep_sessions": "covered", "journal": "unknown"
        })
    );
}

#[tokio::test]
async fn freshness_is_unknown_until_both_namespaces_pushed() {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            REAL_DAYS,
            &real_imported_days(),
            REAL_PUSH_AT,
        )
        .await;
    let body = mirror.recovery_day("2026-09-14").await;

    assert_eq!(body["noop"]["installation_id"], SOURCE);
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");
    assert_eq!(body["noop"]["coverage"]["daily_metrics"], "unknown");
    assert_eq!(
        body["derived_scores"]["strain"],
        measured(json!(3.1), "score_0_100", IMPORTED)
    );
}

#[tokio::test]
async fn only_applied_windows_in_both_namespaces_establish_coverage() {
    let real = real_mirror().await;
    let coverage = |body: &Value| body["noop"]["coverage"].clone();

    let no_night = real.recovery_day("2026-09-14").await;
    assert_eq!(no_night["sleep"]["sessions"], json!([]));
    assert_eq!(
        coverage(&no_night),
        json!({"daily_metrics": "covered", "sleep_sessions": "covered", "journal": "unknown"})
    );
    assert_eq!(
        coverage(&real.recovery_day("2026-09-13").await),
        json!({"daily_metrics": "covered", "sleep_sessions": "unknown", "journal": "unknown"})
    );
    assert_eq!(
        coverage(&real.recovery_day("2026-09-27").await),
        json!({"daily_metrics": "unknown", "sleep_sessions": "unknown", "journal": "unknown"})
    );

    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, REAL_DAYS, &[], REAL_PUSH_AT)
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Days("2026-09-20", "2026-09-27"),
            &[],
            REAL_PUSH_AT,
        )
        .await;
    mirror
        .push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(1_789_250_400, SEP_20),
            &[],
            REAL_PUSH_AT,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(SEP_20, 1_790_460_000),
            &[],
            REAL_PUSH_AT,
        )
        .await;
    assert_eq!(
        coverage(&mirror.recovery_day("2026-09-20").await),
        json!({"daily_metrics": "covered", "sleep_sessions": "covered", "journal": "unknown"})
    );
    assert_eq!(
        coverage(&mirror.recovery_day("2026-09-15").await)["daily_metrics"],
        "unknown"
    );

    let mut staged = Mirror::empty().await;
    staged
        .push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    staged
        .push_first_part(
            SOURCE,
            COMPUTED,
            REAL_STARTS,
            &[sleep(SEP_20 - 3_600, SEP_20 + 3_600, json!({}))],
            2,
            REAL_PUSH_AT,
        )
        .await;
    assert_eq!(
        coverage(&staged.recovery_day("2026-09-20").await)["sleep_sessions"],
        "unknown"
    );
}

#[tokio::test]
async fn reads_only_the_active_installation_and_strap_namespaces() {
    let mut mirror = Mirror::empty().await;
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                OTHER_SOURCE,
                device,
                REAL_DAYS,
                &[daily("2026-09-14", json!({"recovery": 99.0}))],
                "2026-09-20T00:00:00.000Z",
            )
            .await;
    }
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(SOURCE, device, REAL_DAYS, &[], REAL_PUSH_AT)
            .await;
    }
    mirror
        .push(
            SOURCE,
            "other-strap",
            REAL_DAYS,
            &[daily("2026-09-14", json!({"recovery": 77.0}))],
            "2026-09-27T00:00:00.000Z",
        )
        .await;
    let body = mirror.recovery_day("2026-09-14").await;

    assert_eq!(body["noop"]["installation_id"], SOURCE);
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");
    assert_eq!(body["derived_scores"]["recovery"], absent("score_0_100"));
    assert_eq!(body["noop"]["coverage"]["daily_metrics"], "covered");
}
