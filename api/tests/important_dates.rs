mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::{send_request_with_method, test_app_with_db};
use serde_json::{Value, json};

async fn request(app: &Router, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), path, method, Some(body), Some("test-api-key")).await
}

async fn person(app: &Router, name: &str) -> String {
    let (status, body) = request(
        app,
        Method::POST,
        "/api/household-users",
        json!({"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_owned()
}

async fn add(app: &Router, body: Value) -> (StatusCode, Value) {
    request(app, Method::POST, "/api/important-dates", body).await
}

async fn list(app: &Router, query: &str) -> Value {
    let (status, body) = request(
        app,
        Method::GET,
        &format!("/api/important-dates{query}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn milestones_follow_calendar_anniversaries_and_round_day_boundaries() {
    let (app, _) = test_app_with_db().await;
    for (index, (date, kind, on, expected)) in [
        ("2024-02-29", "since", "2025-02-27", Value::Null),
        (
            "2024-02-29",
            "since",
            "2025-02-28",
            json!({"kind":"anniversary","years":1}),
        ),
        ("2024-02-29", "since", "2028-02-28", Value::Null),
        (
            "2024-02-29",
            "since",
            "2028-02-29",
            json!({"kind":"anniversary","years":4}),
        ),
        ("2025-01-31", "since", "2025-02-28", Value::Null),
        // 248,000 days is also the 679th anniversary: the anniversary wins.
        (
            "2000-01-01",
            "since",
            "2679-01-01",
            json!({"kind":"anniversary","years":679}),
        ),
        (
            "2025-01-31",
            "since",
            "2026-01-31",
            json!({"kind":"anniversary","years":1}),
        ),
        ("2026-03-29", "since", "2026-03-29", Value::Null),
        ("2026-03-30", "since", "2026-03-29", Value::Null),
        (
            "2025-12-19",
            "since",
            "2026-03-29",
            json!({"kind":"day_count","days":100}),
        ),
        (
            "2026-07-17",
            "since",
            "2026-10-25",
            json!({"kind":"day_count","days":100}),
        ),
        (
            "1992-03-29",
            "yearly",
            "2026-03-29",
            json!({"kind":"yearly","years":34}),
        ),
        (
            "2017-10-25",
            "yearly",
            "2026-10-25",
            json!({"kind":"yearly","years":9}),
        ),
        (
            "2000-02-29",
            "yearly",
            "2026-02-28",
            json!({"kind":"yearly","years":26}),
        ),
        (
            "2026-03-29",
            "yearly",
            "2026-03-29",
            json!({"kind":"yearly","years":0}),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let (status, added) = add(&app, json!({"title":"Synthetic date","date":date,"kind":kind,"person_ids":[],"request_id":format!("case-{index}")})).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = added["important_date"]["id"].as_str().unwrap();
        let (_, read) = request(
            &app,
            Method::GET,
            &format!("/api/important-dates/{id}?on={on}"),
            Value::Null,
        )
        .await;
        assert_eq!(read["milestone"], expected, "{date} {kind} on {on}");
    }
    let (_, added) = add(&app, json!({"title":"Round days","date":"2024-01-01","kind":"since","person_ids":[],"request_id":"round"})).await;
    let id = added["important_date"]["id"].as_str().unwrap();
    for days in [
        99, 100, 101, 499, 500, 501, 999, 1000, 1001, 1999, 2000, 2001, 3000,
    ] {
        let on = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap() + chrono::Days::new(days);
        let (_, read) = request(
            &app,
            Method::GET,
            &format!("/api/important-dates/{id}?on={on}"),
            Value::Null,
        )
        .await;
        let expected = if [100, 500, 1000, 2000, 3000].contains(&days) {
            json!({"kind":"day_count","days":days})
        } else {
            Value::Null
        };
        assert_eq!(read["milestone"], expected, "day {days}");
    }
}

#[tokio::test]
async fn idle_milestones_require_show_when_idle_but_dates_tab_keeps_every_milestone() {
    let (app, _) = test_app_with_db().await;
    for (title, idle) in [
        ("Visible anniversary", true),
        ("Private anniversary", false),
    ] {
        add(&app, json!({"title":title,"date":"2024-02-14","kind":"since","person_ids":[],"show_when_idle":idle,"request_id":title})).await;
    }
    let all = list(&app, "?on=2026-02-14").await;
    assert_eq!(all["dates"].as_array().unwrap().len(), 2);
    assert!(
        all["dates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|date| date["milestone"] == json!({"kind":"anniversary","years":2}))
    );
    let idle = list(&app, "?on=2026-02-14&show_when_idle=true").await;
    assert_eq!(idle["dates"].as_array().unwrap().len(), 1);
    assert_eq!(idle["dates"][0]["title"], "Visible anniversary");
    assert_eq!(
        idle["dates"][0]["milestone"],
        json!({"kind":"anniversary","years":2})
    );
}

#[tokio::test]
async fn dates_tag_zero_one_or_two_members_and_report_figures_for_the_requested_day() {
    let (app, _) = test_app_with_db().await;
    let alex = person(&app, "Alex").await;
    let robin = person(&app, "Robin").await;
    let (status, together) = add(&app, json!({"title":" Together ","date":"2024-02-14","kind":"since","person_ids":[robin, alex],"show_when_idle":true,"request_id":"together"})).await;
    assert_eq!(status, StatusCode::CREATED, "{together}");
    assert_eq!(together["replayed"], false);
    let together = &together["important_date"];
    assert_eq!(together["title"], "Together");
    // Tags come back ordered by name, whatever order they were given in.
    assert_eq!(together["person_ids"], json!([alex, robin]));
    assert_eq!(together["show_when_idle"], true);
    let (_, birthday) = add(&app, json!({"title":"Alex's birthday","date":"1992-11-04","kind":"yearly","person_ids":[alex],"request_id":"birthday"})).await;
    assert_eq!(birthday["important_date"]["show_when_idle"], false);
    let (_, friend) = add(&app, json!({"title":"A friend's birthday","date":"2000-02-29","kind":"yearly","person_ids":[],"request_id":"friend"})).await;
    assert_eq!(friend["important_date"]["person_ids"], json!([]));

    let listed = list(&app, "?on=2026-05-26").await;
    assert_eq!(listed["on"], "2026-05-26");
    let dates = listed["dates"].as_array().unwrap();
    // Since first, then yearly dates by their next occurrence.
    assert_eq!(
        dates.iter().map(|date| &date["title"]).collect::<Vec<_>>(),
        ["Together", "Alex's birthday", "A friend's birthday"]
    );
    assert_eq!(
        dates[0]["elapsed"],
        json!({"years":2,"months":3,"days":12,"total_days":832})
    );
    assert_eq!(
        dates[0]["next"],
        json!({"date":"2027-02-14","days_until":264,"years":3})
    );
    assert_eq!(
        dates[1]["next"],
        json!({"date":"2026-11-04","days_until":162,"years":34})
    );
    // 29 February falls back to 28 February in a common year.
    assert_eq!(
        dates[2]["next"],
        json!({"date":"2027-02-28","days_until":278,"years":27})
    );

    // Before a date happens it has no elapsed figure and counts down to itself.
    let early = list(&app, "?on=2024-02-01").await;
    assert_eq!(early["dates"][0]["elapsed"], Value::Null);
    assert_eq!(
        early["dates"][0]["next"],
        json!({"date":"2024-02-14","days_until":13,"years":0})
    );

    let idle = list(&app, "?on=2026-05-26&show_when_idle=true").await;
    assert_eq!(idle["dates"].as_array().unwrap().len(), 1);
    let today = list(&app, "").await;
    assert_eq!(
        today["on"],
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Copenhagen)
            .date_naive()
            .to_string()
    );
    for query in [
        "?on=2026-02-30",
        "?on=26-05-2026",
        "?on=0000-01-01",
        "?owner=x",
    ] {
        let (status, _) = request(
            &app,
            Method::GET,
            &format!("/api/important-dates{query}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
    }

    // Removing a member removes only their tag.
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{robin}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let path = format!(
        "/api/important-dates/{}?on=2026-05-26",
        together["id"].as_str().unwrap()
    );
    let (status, read) = request(&app, Method::GET, &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["person_ids"], json!([alex]));
    assert_eq!(read["elapsed"]["total_days"], 832);
}

#[tokio::test]
async fn additions_replay_by_request_id_and_never_infer_or_duplicate_tags() {
    let (app, state) = test_app_with_db().await;
    let alex = person(&app, "Alex").await;
    let body = json!({"title":"First date","date":"2023-06-03","kind":"yearly","person_ids":[alex],"request_id":"first"});
    let (status, added) = add(&app, body.clone()).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, replayed) = add(&app, body.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["important_date"], added["important_date"]);
    let mut changed = body.clone();
    changed["person_ids"] = json!([]);
    assert_eq!(add(&app, changed).await.0, StatusCode::CONFLICT);

    let mut fresh = body.clone();
    fresh["request_id"] = json!("fresh");
    for (field, value, status) in [
        ("person_ids", json!([alex, alex]), StatusCode::BAD_REQUEST),
        (
            "person_ids",
            json!(["00000000-0000-0000-0000-000000000000"]),
            StatusCode::NOT_FOUND,
        ),
        ("date", json!("2023-02-29"), StatusCode::BAD_REQUEST),
        ("date", json!("2023-6-3"), StatusCode::BAD_REQUEST),
        ("kind", json!("monthly"), StatusCode::UNPROCESSABLE_ENTITY),
        ("title", json!("   "), StatusCode::BAD_REQUEST),
        ("title", json!("a".repeat(201)), StatusCode::BAD_REQUEST),
    ] {
        let mut invalid = fresh.clone();
        invalid[field] = value;
        assert_eq!(add(&app, invalid).await.0, status, "{field}");
    }
    // Tags are required, even when a date concerns no one.
    let mut untagged = fresh.clone();
    untagged.as_object_mut().unwrap().remove("person_ids");
    assert_eq!(
        add(&app, untagged).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Rejected additions consume no request ID.
    assert_eq!(add(&app, fresh).await.0, StatusCode::CREATED);

    let path = format!(
        "/api/important-dates/{}",
        added["important_date"]["id"].as_str().unwrap()
    );
    assert_eq!(
        request(&app, Method::DELETE, &path, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::DELETE, &path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    // A removed date cannot be recreated by an old retry.
    assert_eq!(add(&app, body).await.0, StatusCode::CONFLICT);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM important_date_people")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn edits_change_only_supplied_fields_and_replace_tags_together() {
    let (app, _) = test_app_with_db().await;
    let alex = person(&app, "Alex").await;
    let sam = person(&app, "Sam").await;
    let (_, added) = add(&app, json!({"title":"Moved in","date":"2022-08-01","kind":"since","person_ids":[alex],"request_id":"moved"})).await;
    let path = format!(
        "/api/important-dates/{}",
        added["important_date"]["id"].as_str().unwrap()
    );
    let (status, edited) = request(
        &app,
        Method::PATCH,
        &path,
        json!({"person_ids":[sam, alex],"show_when_idle":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["person_ids"], json!([alex, sam]));
    assert_eq!(edited["title"], "Moved in");
    assert_eq!(edited["date"], "2022-08-01");
    assert_eq!(edited["show_when_idle"], true);
    let (_, edited) = request(
        &app,
        Method::PATCH,
        &path,
        json!({"title":"Our flat","date":"2022-08-02","kind":"yearly","person_ids":[]}),
    )
    .await;
    assert_eq!(edited["person_ids"], json!([]));
    assert_eq!(edited["kind"], "yearly");
    assert_eq!(edited["show_when_idle"], true);
    for invalid in [
        json!({"title":null}),
        json!({"person_ids":null}),
        json!({"date":"2022-13-01"}),
        json!({"owner_id":alex}),
    ] {
        let status = request(&app, Method::PATCH, &path, invalid.clone()).await.0;
        assert!(status.is_client_error(), "{invalid}");
    }
    let (_, unchanged) = request(&app, Method::GET, &path, Value::Null).await;
    assert_eq!(unchanged["title"], "Our flat");
    assert_eq!(
        request(
            &app,
            Method::PATCH,
            "/api/important-dates/missing",
            json!({"title":"x"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send_request_with_method(app.clone(), "/api/important-dates", Method::GET, None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
