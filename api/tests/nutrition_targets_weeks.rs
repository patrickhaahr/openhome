//! The weekly Calorie Target record through the Nutrition Day: freezing, stored winners and year
//! boundaries, from a real `app.db` and a real `food.db` imported from a synthetic DTU-shaped
//! workbook served by `wiremock`.

mod common;
#[path = "nutrition_support/targets.rs"]
mod support;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, day_from_today, man_of_today, monday_of, today};

// Week lifecycle: a week freezes on its first computable read that has started, evaluated
// against the requested days' calendar, and never moves again.

#[tokio::test]
async fn the_current_week_freezes_on_first_read_and_future_weeks_stay_provisional() {
    let api = Api::start().await;
    man_of_today(&api).await;
    api.weigh(&day_from_today(-20), 80.0).await;
    // Next week's first day, whatever weekday today is.
    let next_monday = day_from_today(7);
    let monday = monday_of(today()).to_string();
    let sunday = (monday_of(today()) + chrono::Duration::days(6)).to_string();

    // Reading a future week is provisional: computed from the data it already has, with
    // nothing stored.
    let future = api.targets(&next_monday).await;
    assert_eq!(future["week_fixed"], false);
    assert_eq!(future["bmr_kcal"], 1780.0);
    assert_eq!(future["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(future["calorie_target_kcal"], 2120.0);
    assert!(api.frozen_weeks().await.is_empty());

    // Every day of the current week is eligible however late in the week it is read: the ISO
    // Monday has started, so even the week's Sunday freezes the record.
    let frozen = api.targets(&sunday).await;
    assert_eq!(frozen["week_fixed"], true);
    assert_eq!(frozen["calorie_target_kcal"], 2120.0);
    assert_eq!(api.frozen_weeks().await, vec![monday.clone()]);

    // Backfilled weigh-ins and meals over the frozen record: the week never moves, while the
    // daily weight follows the weigh-ins.
    api.weigh(&day_from_today(-5), 90.0).await;
    api.eat(&day_from_today(-3), 2000.0).await;
    let again = api.targets(&sunday).await;
    assert_eq!(again["week_fixed"], true);
    assert_eq!(again["bmr_kcal"], 1780.0);
    assert_eq!(again["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(again["calorie_target_kcal"], 2120.0);
    assert_eq!(again["trend_weight_kg"], 81.0);
    assert_eq!(again["macros"]["protein"]["target_g"], 178.2);
    assert_eq!(again["macros"]["fat"]["floor_g"], 56.7);

    // The future week is still provisional and recomputes from the backfilled data instead;
    // its exact estimate depends on where the weigh-in falls in its window, so only what the
    // weekday cannot move is asserted.
    let future = api.targets(&next_monday).await;
    assert_eq!(future["week_fixed"], false);
    assert_eq!(future["expenditure_basis"]["logged_days"], 1);
    assert_eq!(future["expenditure_basis"]["prior_kcal"], 2685.0);
    assert_ne!(future["calorie_target_kcal"], 2120.0);
    assert_eq!(api.frozen_weeks().await, vec![monday]);
}

// The first in-week weigh-in gives BMR only to a read that reaches it: a Day read stops at
// its requested day, a Summary reads on to the range's later days, and the days before the
// weigh-in stay unavailable even in the frozen week.

#[tokio::test]
async fn the_first_weigh_in_bound_is_the_latest_requested_day_not_the_whole_week() {
    let api = Api::start().await;
    api.profile(json!({"height_cm": 180, "sex": "male", "birth_date": "1996-01-15"}))
        .await;
    // Nothing weighed before Monday 2026-04-06; the week's first weigh-in is the Thursday.
    api.weigh("2026-04-09", 80.0).await;

    // The Monday read stops at the Monday: its week computes nothing, so nothing is frozen.
    let monday = api.targets("2026-04-06").await;
    assert_eq!(monday["state"], "unavailable");
    assert_eq!(monday["missing"], json!(["body_weight"]));
    assert!(api.frozen_weeks().await.is_empty());

    // A Summary reading the whole week reads on to Sunday, finds the Thursday weigh-in and
    // freezes the week with it: the days of the range with a Trend Weight have their targets,
    // the days before the weigh-in stay target-less.
    let summary = api.summary("2026-04-06", "2026-04-12").await;
    let targets: Vec<Value> = summary["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| period["energy"]["calorie_target_kcal"].clone())
        .collect();
    assert_eq!(
        targets,
        vec![
            json!(null),
            json!(null),
            json!(null),
            json!(2120.0),
            json!(2120.0),
            json!(2120.0),
            json!(2120.0)
        ]
    );
    assert_eq!(api.frozen_weeks().await, vec!["2026-04-06".to_string()]);

    // The stored week does not invent the early days' Trend Weight either.
    assert_eq!(api.targets("2026-04-06").await, monday);
    let thursday = api.targets("2026-04-09").await;
    assert_eq!(thursday["state"], "available");
    assert_eq!(thursday["week_fixed"], true);
    assert_eq!(thursday["trend_weight_kg"], 80.0);
    assert_eq!(thursday["bmr_kcal"], 1780.0);
}

// The Profile is read when a record is written; a stored week keeps its targets once the
// Profile has lost every input a new week would need, and a new week gets no guessed one.

#[tokio::test]
async fn a_stored_week_reads_without_the_profile_and_future_weeks_stay_unavailable() {
    let api = Api::start().await;
    man_of_today(&api).await;
    api.weigh(&day_from_today(-20), 80.0).await;

    let before = api.targets(&day_from_today(0)).await;
    assert_eq!(before["week_fixed"], true);
    assert_eq!(before["calorie_target_kcal"], 2120.0);

    api.profile(json!({"height_cm": null, "sex": null, "birth_date": null}))
        .await;
    let after = api.targets(&day_from_today(0)).await;
    assert_eq!(after, before);

    // The next week has no row and no inputs either, so nothing is invented for it.
    let future = api.targets(&day_from_today(7)).await;
    assert_eq!(future["state"], "unavailable");
    assert_eq!(future["missing"], json!(["height", "sex", "birth_date"]));
    assert_eq!(
        future["reason"],
        "Targets need the Profile's height, sex (male or female) and birth date"
    );
    assert_eq!(
        api.frozen_weeks().await,
        vec![monday_of(today()).to_string()]
    );
}

// The stored record is the winner: a first read that committed ahead of this one stays, and
// later meals, weigh-ins and reads return its values, never a recompute relabelled as fixed.

#[tokio::test]
async fn the_stored_row_wins_over_the_computation_and_later_backfills() {
    let api = Api::start().await;
    man_of_today(&api).await;
    api.weigh(&day_from_today(-20), 80.0).await;
    let monday = monday_of(today()).to_string();
    api.seed_week(&monday, &day_from_today(-20)).await;

    let frozen = api.targets(&day_from_today(0)).await;
    assert_eq!(frozen["week_fixed"], true);
    assert_eq!(frozen["bmr_kcal"], 1600.0);
    assert_eq!(frozen["estimated_expenditure_kcal"], 3000.0);
    // The default loss of half a kilo a week against the stored 3,000.
    assert_eq!(frozen["calorie_target_kcal"], 2450.0);

    // 5,000 kcal logged and a 90 kg weigh-in would rewrite the recomputed week; the row does
    // not move and every day of the week keeps its basis.
    api.eat(&day_from_today(-3), 5000.0).await;
    api.weigh(&day_from_today(-5), 90.0).await;
    let again = api.targets(&day_from_today(0)).await;
    for key in [
        "week_fixed",
        "bmr_kcal",
        "estimated_expenditure_kcal",
        "calorie_target_kcal",
    ] {
        assert_eq!(again[key], frozen[key]);
    }
    let sunday = api
        .targets(&(monday_of(today()) + chrono::Duration::days(6)).to_string())
        .await;
    assert_eq!(sunday["estimated_expenditure_kcal"], 3000.0);
    assert_eq!(sunday["week_fixed"], true);
    assert_eq!(api.frozen_weeks().await, vec![monday]);
}

// Two readers racing for a week's first store never raise, double-write or overwrite: the
// winner is whichever read committed first, and both return its stored basis.

#[tokio::test]
async fn concurrent_first_reads_agree_on_the_stored_winner() {
    let api = Api::start_file().await;
    man_of_today(&api).await;
    api.weigh(&day_from_today(-20), 80.0).await;
    let monday = monday_of(today()).to_string();
    let sunday = (monday_of(today()) + chrono::Duration::days(6)).to_string();

    let (a, b) = tokio::join!(api.targets(&sunday), api.targets(&sunday));
    assert_eq!(a["week_fixed"], true);
    assert_eq!(a["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(a, b);
    assert_eq!(api.frozen_weeks().await, vec![monday]);
}

// A week over a calendar year's end is one Monday-to-Sunday record like any other.

#[tokio::test]
async fn new_years_week_stays_one_monday_to_sunday_record() {
    let api = Api::start().await;
    // 30 years old on Monday 2025-12-29.
    api.profile(json!({"height_cm": 180, "sex": "male", "birth_date": "1995-12-29"}))
        .await;
    api.weigh("2025-12-01", 80.0).await;

    let targets = api.targets("2026-01-02").await;
    assert_eq!(targets["week_start"], "2025-12-29");
    assert_eq!(targets["week_fixed"], true);
    assert_eq!(targets["bmr_kcal"], 1780.0);
    assert_eq!(targets["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(targets["calorie_target_kcal"], 2120.0);
    assert_eq!(api.frozen_weeks().await, vec!["2025-12-29".to_string()]);
}
