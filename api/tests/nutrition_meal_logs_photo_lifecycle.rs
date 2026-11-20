mod common;
#[path = "nutrition_support/meal_logs.rs"]
mod support;

use axum::{http::Method, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Nutrition, at, breakfast, lock_app_db, release, synthetic, with_photo};

#[tokio::test]
async fn a_retried_log_with_a_photo_replays_and_another_photo_conflicts() {
    let api = Nutrition::start().await;
    let body = with_photo(breakfast("photo-retry"), &synthetic::png(640, 480));
    let meal_log = api.logged(body.clone()).await;

    let (status, replayed) = api.log(body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"meal_log": meal_log, "replayed": true}));
    assert_eq!(api.photo_files().len(), 2, "the retry stored nothing");

    for changed in [
        with_photo(breakfast("photo-retry"), &synthetic::png(641, 480)),
        breakfast("photo-retry"),
    ] {
        let (status, response) = api.log(changed).await;
        assert_eq!(status, StatusCode::CONFLICT, "{response}");
    }
    assert_eq!(api.photo_files().len(), 2);
}

#[tokio::test]
async fn a_database_refusal_with_a_photo_writes_nothing_and_a_retry_commits_its_files() {
    let api = Nutrition::start_file().await;
    let body = with_photo(
        at("2026-10-29T12:00:00+01:00", "refused-write"),
        &synthetic::png(600, 400),
    );

    let (lock, locker) = lock_app_db(&api.dir).await;
    let (status, response) = api.log(body.clone()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{response}");
    assert_eq!(response["status"], 500, "{response}");
    release(lock, &locker).await;

    assert_eq!(api.photo_files(), Vec::<String>::new());
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());

    let meal_log = api.logged(body).await;
    let id = meal_log["photo"]["id"].as_str().unwrap();
    let (status, content_type, _) = api.media(meal_log["photo"]["url"].as_str().unwrap()).await;
    assert_eq!(
        (status, content_type.as_deref()),
        (StatusCode::OK, Some("image/avif"))
    );
    assert_eq!(
        api.photo_files(),
        [format!("{id}.avif"), format!("{id}.thumbnail.avif")]
    );
}

#[tokio::test]
async fn a_database_refusal_during_replacement_removes_its_new_photo_and_keeps_the_committed_one() {
    let api = Nutrition::start_file().await;
    let meal_log = api
        .logged(with_photo(
            at("2026-10-29T12:00:00+01:00", "replaced-write"),
            &synthetic::png(600, 400),
        ))
        .await;
    let committed = meal_log["photo"].clone();
    let committed_id = committed["id"].as_str().unwrap();
    let uri = format!(
        "/api/nutrition/meal-logs/{}",
        meal_log["id"].as_str().unwrap()
    );
    let replacement = json!({"photo": synthetic::base64(&synthetic::png(500, 300))});

    let (lock, locker) = lock_app_db(&api.dir).await;
    let (status, response) = api
        .send(Method::PATCH, &uri, Some(replacement.clone()))
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{response}");
    release(lock, &locker).await;

    assert_eq!(
        api.photo_files(),
        [
            format!("{committed_id}.avif"),
            format!("{committed_id}.thumbnail.avif")
        ]
    );
    assert_eq!(api.day("2026-10-29").await, [meal_log]);
    let (status, content_type, _) = api.media(committed["url"].as_str().unwrap()).await;
    assert_eq!(
        (status, content_type.as_deref()),
        (StatusCode::OK, Some("image/avif"))
    );

    let (status, replaced) = api.send(Method::PATCH, &uri, Some(replacement)).await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    let replaced_id = replaced["photo"]["id"].as_str().unwrap();
    assert_ne!(replaced_id, committed_id);
    assert_eq!(
        api.photo_files(),
        [
            format!("{replaced_id}.avif"),
            format!("{replaced_id}.thumbnail.avif")
        ]
    );
    assert_eq!(
        api.media(committed["url"].as_str().unwrap()).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api.media(replaced["photo"]["url"].as_str().unwrap())
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_photo_is_attached_replaced_or_removed_and_leaves_with_its_meal_log() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("no-photo")).await;
    assert_eq!(meal_log["photo"], Value::Null);
    let id = meal_log["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/nutrition/meal-logs/{id}");

    let (status, attached) = api.upload(&id, "photo", &synthetic::png(1000, 800)).await;
    assert_eq!(status, StatusCode::OK, "{attached}");
    assert_eq!(
        (&attached["portions"], &attached["note"]),
        (&meal_log["portions"], &meal_log["note"])
    );
    let first = attached["photo"].clone();
    assert_eq!(
        (&first["width"], &first["height"]),
        (&json!(1000), &json!(800))
    );

    let (status, replaced) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"photo": synthetic::base64(&synthetic::jpeg_with_gps(600, 600, 1))})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    let second = replaced["photo"].clone();
    assert_ne!(second["id"], first["id"]);
    let second_id = second["id"].as_str().unwrap();
    assert_eq!(
        api.photo_files(),
        [
            format!("{second_id}.avif"),
            format!("{second_id}.thumbnail.avif")
        ]
    );
    for url in [&first["url"], &first["thumbnail"]["url"]] {
        assert_eq!(
            api.media(url.as_str().unwrap()).await.0,
            StatusCode::NOT_FOUND
        );
    }

    let (_, noted) = api
        .send(Method::PATCH, &uri, Some(json!({"note": "late"})))
        .await;
    assert_eq!(noted["photo"], second);
    let (status, removed) = api
        .send(Method::PATCH, &uri, Some(json!({"photo": null})))
        .await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(removed["photo"], Value::Null);
    assert_eq!(api.photo_files(), Vec::<String>::new());
    assert_eq!(
        api.media(second["url"].as_str().unwrap()).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, attached) = api.upload(&id, "photo", &synthetic::png(50, 50)).await;
    assert_eq!(api.photo_files().len(), 2);
    let (status, _) = api.send(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(api.photo_files(), Vec::<String>::new());
    assert_eq!(
        api.media(attached["photo"]["url"].as_str().unwrap())
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (status, missing) = api.upload(&id, "photo", &synthetic::png(50, 50)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    assert_eq!(api.photo_files(), Vec::<String>::new());
    for uri in [
        "/api/nutrition/meal-photos/not-a-photo",
        "/api/nutrition/meal-photos/..%2Fapp.db",
        "/api/nutrition/meal-photos/3f2b8c1e-0000-4000-8000-000000000000/thumbnail",
    ] {
        assert_eq!(api.media(uri).await.0, StatusCode::NOT_FOUND, "{uri}");
    }
}
