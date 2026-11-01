//! Isolated, loopback-only health, Wishlist, Watchlist, fitness, map, walking-loop and nutrition seam for
//! tests and synthetic previews. Uses production routes, migrations and authentication without
//! household configuration; the map explicitly serves the unrelated Copenhagen demo origin.
//!
//! Nutrition serves a few synthetic Foods and one synthetic Open Food Facts product
//! (`providers::SYNTHETIC_BARCODE`); `--food-database=importing` keeps the Food database
//! importing instead, for unavailable states.
//!
//! Loops come from a loopback provider serving synthetic squares that are not real paths. Set
//! `OPENROUTESERVICE_URL` to a household instance to plan real loops from the demo origin instead.

use std::sync::Arc;

use openhome_api::{
    AppState, DockerCache, auth, routes,
    services::{
        map::MapOrigin,
        noop_push,
        route_loop::{LoopPlanner, ProviderEndpoint},
    },
};
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::net::TcpListener;

#[path = "support/providers.rs"]
mod providers;

// The preview also seeds Meal Logs; these tests log their own.
#[path = "support/foods.rs"]
#[allow(dead_code)]
mod foods;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    let noop_db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    noop_push::initialize(&noop_db).await?;
    seed_training(&noop_db).await?;
    let state = AppState {
        db,
        noop_db,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let key = auth::ApiKey::new("synthetic-dashboard-api-key".to_string());
    let demo_origin = MapOrigin::parse(None, None, Some("true"))?;
    // A configured household instance plans real loops; otherwise serve synthetic ones.
    let endpoint =
        match ProviderEndpoint::parse(std::env::var("OPENROUTESERVICE_URL").ok().as_deref())? {
            Some(endpoint) => endpoint,
            None => providers::synthetic_provider().await?,
        };
    let planner = LoopPlanner::new(demo_origin, Some(endpoint))?;
    let tmdb = providers::synthetic_tmdb().await?;
    let backlight =
        openhome_api::services::backlight::BacklightService::new(state.db.clone(), None, None)
            .await?;
    let (meal_photos, _meal_photo_dir) = foods::photo_store()?;
    let app = routes::health::router()
        .merge(routes::wishlist::router(None))
        .merge(routes::watchlist::router(Some(tmdb)))
        .merge(routes::important_dates::router())
        .merge(routes::backlight::router(backlight))
        .merge(routes::map::router(demo_origin))
        .merge(routes::route_loop::router(planner))
        .merge(routes::photos::router(None))
        .merge(routes::fitness::router())
        .merge(routes::training::router())
        .merge(routes::nutrition::router(
            foods::synthetic_foods(foods::FoodState::from_args()).await?,
            meal_photos,
            providers::synthetic_open_food_facts().await?,
        ))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Synthetic NOOP observations, ingested through the existing push protocol. Empty windows are
/// real applied replacements; no fixture claims coverage merely because a measurement exists.
async fn seed_training(db: &sqlx::SqlitePool) -> anyhow::Result<()> {
    use chrono::DateTime;
    use noop_push::ingest::{self, ContentCoding};
    use uuid::Uuid;

    let at = |text: &str| DateTime::parse_from_rfc3339(text).map(|at| at.timestamp());
    let from = at("2026-09-27T00:00:00+02:00")?;
    let to = at("2026-10-02T00:00:00+02:00")?;
    let start = at("2026-09-30T12:00:00+02:00")?;
    let sleep_start = at("2026-09-29T23:00:00+02:00")?;
    let source = "00000000-0000-4000-8000-000000000123";
    for device in ["my-whoop", "my-whoop-noop"] {
        let computed = device == "my-whoop-noop";
        let daily = json!({
            "type": "record", "key": { "day": "2026-09-30" }, "data": {
                "totalSleepMin": if computed { 450.0 } else { 420.0 },
                "efficiency": if computed { Some(0.9) } else { None },
                "deepMin": if computed { Some(90.0) } else { None },
                "remMin": if computed { Some(100.0) } else { None },
                "lightMin": if computed { Some(260.0) } else { None },
                "disturbances": null, "restingHr": if computed { None } else { Some(50.0) },
                "avgHrv": if computed { Some(62.0) } else { None },
                "recovery": 78.0, "strain": 35.0, "exerciseCount": null, "spo2Pct": null,
                "skinTempDevC": null, "respRateBpm": 14.0, "steps": null,
                "activeKcalEst": null, "spo2Red": null, "spo2Ir": null,
            }
        });
        // Ordered segments totalling light 260, deep 90 and rem 100 minutes.
        let stages = serde_json::to_string(
            &[
                (0, 130, "light"),
                (130, 175, "deep"),
                (175, 225, "rem"),
                (225, 355, "light"),
                (355, 400, "deep"),
                (400, 450, "rem"),
            ]
            .map(|(from, to, stage)| {
                json!({"start": sleep_start + from * 60, "end": sleep_start + to * 60, "stage": stage})
            }),
        )?;
        let sleep = json!({
            "type": "record", "key": { "startTs": sleep_start }, "data": {
                "endTs": sleep_start + 27000, "efficiency": 0.9, "restingHr": 50.0,
                "avgHrv": 62.0, "stagesJSON": if computed { Some(stages.clone()) } else { None }, "userEdited": computed,
                "startTsAdjusted": if computed { Some(sleep_start + 300) } else { None },
                "motionJSON": null, "sleepStateJSON": null, "stagingSparse": null,
            }
        });
        let workouts: Vec<Value> = if computed {
            vec![]
        } else {
            [("Run / walk", 0), ("Cycling", 0), ("Mobility", 3600)]
                .into_iter()
                .map(|(sport, offset)| {
                    json!({
                        "type": "record", "key": { "startTs": start + offset, "sport": sport }, "data": {
                            "endTs": start + offset + 600, "source": "manual", "durationS": 600.0,
                            "energyKcal": 120.0, "avgHr": 140.0, "maxHr": 170.0, "strain": 25.0,
                            "distanceM": 2000.0, "zonesJSON": null, "notes": null,
                            "routePolyline": null, "steps": null,
                        }
                    })
                })
                .collect()
        };
        for (stream, selector, lower, upper, records) in [
            (
                "dailyMetric",
                "day",
                json!("2026-09-27"),
                json!("2026-10-02"),
                vec![daily],
            ),
            (
                "sleepSession",
                "startTs",
                json!(from),
                json!(to),
                vec![sleep],
            ),
            ("workout", "startTs", json!(from), json!(to), workouts),
            (
                "journal",
                "day",
                json!("2026-09-27"),
                json!("2026-10-02"),
                if computed {
                    vec![]
                } else {
                    vec![json!({
                        "type": "record", "key": { "day": "2026-09-30", "question": "Felt recovered" },
                        "data": { "answeredYes": false, "numericValue": null, "notes": "Synthetic journal note" },
                    })]
                },
            ),
        ] {
            let header = json!({
                "type": "batch", "protocolVersion": "1.0", "batchId": Uuid::new_v4(),
                "sourceId": source, "deviceId": device, "stream": stream,
                "delivery": "replace_window", "recordCount": records.len(),
                "startCursor": null, "endCursor": null,
                "window": { "replacementId": Uuid::new_v4(), "selector": selector,
                    "startInclusive": lower, "endExclusive": upper, "part": 1, "parts": 1 },
            });
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
        }
    }
    // Leave a gap in the series and no samples at the five-minute recovery mark.
    let records: Vec<Value> = (0..=735).step_by(5).filter(|offset| !(90..150).contains(offset))
        .map(|offset| json!({ "type": "record", "key": { "ts": start + offset },
            "data": { "bpm": if offset < 570 { 150 } else if offset < 600 { 165 } else if offset < 690 { 120 } else { 110 } } }))
        .collect();
    let header = json!({
        "type": "batch", "protocolVersion": "1.0", "batchId": Uuid::new_v4(),
        "sourceId": source, "deviceId": "my-whoop", "stream": "hrSample",
        "delivery": "append", "recordCount": records.len(), "startCursor": null,
        "endCursor": { "rowId": records.len(), "keySha256": "0".repeat(64) },
    });
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
