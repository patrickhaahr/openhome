mod backlight_support;
mod common;

use axum::http::Method;
use axum::http::StatusCode;
use backlight_support::{ControlledClock, ManualDevice, app, authenticated, select, wait_for_mode};
use common::send_request;
use common::send_request_with_method;
use openhome_api::services::backlight::BacklightService;

#[tokio::test]
async fn bedtime_enable_applies_red_and_automatic_clears_a_durable_override() {
    let (_, state) = common::test_app_with_db().await;
    let device = std::sync::Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let service = BacklightService::with_clock(
        state.db.clone(),
        Some(device.clone()),
        None,
        std::sync::Arc::new(ControlledClock::at("2026-10-23T21:30:00Z")),
    )
    .await
    .unwrap();
    let app = authenticated(service, state.clone());
    let (_, initial) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(initial["bedtime"]["start"], "23:00");
    assert_eq!(initial["bedtime"]["end"], "00:00");
    assert_eq!(initial["bedtime"]["brightness"], 20);
    let (code, enabled) = send_request_with_method(
        app.clone(),
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({"enabled":true})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(enabled["control_source"], "automatic");
    assert_eq!(
        enabled["intended"],
        serde_json::json!({"mode":"color","rgb":{"red":255,"green":0,"blue":0},"brightness":20})
    );
    let (_, manual) = select(app.clone(), serde_json::json!({"mode":"off"})).await;
    assert_eq!(manual["control_source"], "manual");
    assert!(manual["override_expires_at"].is_string());
    let restarted = BacklightService::with_clock(
        state.db.clone(),
        None,
        None,
        std::sync::Arc::new(ControlledClock::at("2026-10-23T21:30:00Z")),
    )
    .await
    .unwrap();
    let (_, saved) = send_request(
        authenticated(restarted, state),
        "/api/backlight",
        Some("test-api-key"),
    )
    .await;
    assert_eq!(saved["override_expires_at"], manual["override_expires_at"]);
    let (code, automatic) = send_request_with_method(
        app,
        "/api/backlight/automatic",
        Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(automatic["control_source"], "automatic");
    assert!(automatic["override_expires_at"].is_null());
    assert_eq!(automatic["intended"], enabled["intended"]);
}

#[tokio::test]
async fn routine_runs_without_dashboard_expires_off_at_start_and_restores_saved_tv_sync_at_end() {
    use std::sync::Arc;
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let clock = Arc::new(ControlledClock::at("2026-10-23T20:59:59Z"));
    let service =
        BacklightService::with_clock(state.db.clone(), Some(device.clone()), None, clock.clone())
            .await
            .unwrap();
    let app = authenticated(service.clone(), state.clone());
    let (code, enabled) = send_request_with_method(
        app.clone(),
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({"enabled":true})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(enabled["intended"], serde_json::json!({"mode":"tv-sync"}));
    let (_, manual) = select(app.clone(), serde_json::json!({"mode":"off"})).await;
    assert_eq!(manual["override_expires_at"], "2026-10-23T21:00:00Z");
    let runner = tokio::spawn(service.run_routine());
    clock.set("2026-10-23T21:00:00Z");
    let red = wait_for_mode(app.clone(), "color").await;
    assert_eq!(
        red["intended"],
        serde_json::json!({"mode":"color","rgb":{"red":255,"green":0,"blue":0},"brightness":20})
    );
    assert_eq!(red["control_source"], "automatic");
    assert!(red["override_expires_at"].is_null());
    let writes = device.packets.lock().unwrap().len();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        device.packets.lock().unwrap().len(),
        writes,
        "steady operation must leave external changes alone"
    );
    let (_, overridden) = select(
        app.clone(),
        serde_json::json!({"mode":"tv-sync","brightness":80}),
    )
    .await;
    assert_eq!(overridden["override_expires_at"], "2026-10-23T22:00:00Z");
    clock.set("2026-10-23T22:00:00Z");
    let restored = wait_for_mode(app.clone(), "tv-sync").await;
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            let (_, body) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
            if body["control_source"] == "automatic" && body["command"] == "acknowledged" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(restored["bedtime"]["brightness"], 20);
    assert_eq!(
        device.packets.lock().unwrap().last().unwrap()[2][..3],
        [0x33, 4, 45]
    );
    select(app.clone(), serde_json::json!({"mode":"off"})).await;
    let (_, disabled) = send_request_with_method(
        app.clone(),
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({"enabled":false})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(disabled["intended"], serde_json::json!({"mode":"tv-sync"}));
    assert!(disabled["next_transition"].is_null());
    assert_eq!(disabled["control_source"], "automatic");
    assert_eq!(
        disabled["bedtime"],
        serde_json::json!({"enabled":false,"start":"23:00","end":"00:00","brightness":20})
    );
    let (_, off) = select(app.clone(), serde_json::json!({"mode":"off"})).await;
    assert!(off["override_expires_at"].is_null());
    clock.set("2026-10-24T22:30:00Z");
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let (_, still_off) = send_request(app, "/api/backlight", Some("test-api-key")).await;
    assert_eq!(still_off["intended"], serde_json::json!({"mode":"off"}));
    runner.abort();
}

#[tokio::test]
async fn omitted_bedtime_patch_preserves_settings_and_override_without_device_writes() {
    let app = app().await;
    let (_, initial) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    let (code, unchanged) = send_request_with_method(
        app.clone(),
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(unchanged, initial);
    let (code, _) = send_request_with_method(
        app,
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({"enabled":null})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(code, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bedtime_edits_apply_now_preserve_manual_expiry_and_persist_while_disabled() {
    use std::sync::Arc;
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let clock = Arc::new(ControlledClock::at("2026-10-23T21:30:00Z"));
    let service =
        BacklightService::with_clock(state.db.clone(), Some(device.clone()), None, clock.clone())
            .await
            .unwrap();
    let app = authenticated(service, state.clone());
    let patch = |body| {
        send_request_with_method(
            app.clone(),
            "/api/backlight/bedtime",
            Method::PATCH,
            Some(body),
            Some("test-api-key"),
        )
    };
    let (code, edited) =
        patch(serde_json::json!({"enabled":true,"start":"22:00","end":"02:00","brightness":31}))
            .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(edited["intended"]["brightness"], 31);
    assert_eq!(edited["next_transition"], "2026-10-24T00:00:00Z");
    let (_, outside) = patch(serde_json::json!({"start":"00:30","end":"02:00"})).await;
    assert_eq!(outside["intended"]["mode"], "tv-sync");
    let (_, manual) = select(app.clone(), serde_json::json!({"mode":"off"})).await;
    assert_eq!(manual["override_expires_at"], "2026-10-23T22:30:00Z");
    let (_, preserved) = patch(serde_json::json!({"start":"01:00","brightness":0})).await;
    assert_eq!(preserved["intended"], serde_json::json!({"mode":"off"}));
    assert_eq!(preserved["control_source"], "manual");
    assert_eq!(preserved["override_expires_at"], "2026-10-23T23:00:00Z");
    patch(serde_json::json!({"enabled":false})).await;
    select(app.clone(), serde_json::json!({"mode":"off"})).await;
    let (_, disabled) =
        patch(serde_json::json!({"start":"22:30","end":"03:15","brightness":100})).await;
    assert_eq!(disabled["intended"]["mode"], "off");
    assert!(disabled["override_expires_at"].is_null());
    let reopened = BacklightService::with_clock(state.db.clone(), Some(device), None, clock)
        .await
        .unwrap();
    let (_, saved) = send_request(
        authenticated(reopened, state),
        "/api/backlight",
        Some("test-api-key"),
    )
    .await;
    assert_eq!(
        saved["bedtime"],
        serde_json::json!({"enabled":false,"start":"22:30","end":"03:15","brightness":100})
    );
    assert_eq!(saved["intended"]["mode"], "off");
}

#[tokio::test]
async fn bedtime_patch_rejects_ambiguous_or_malformed_values_atomically() {
    let (_, state) = common::test_app_with_db().await;
    let device = std::sync::Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state);
    select(app.clone(), serde_json::json!({"mode":"off"})).await;
    let (_, before) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    let writes = device.packets.lock().unwrap().len();
    for body in [
        serde_json::json!({"start":"00:00","brightness":31}),
        serde_json::json!({"start":"24:00"}),
        serde_json::json!({"end":"12:60"}),
        serde_json::json!({"start":"9:00"}),
        serde_json::json!({"start":"09:00:00"}),
        serde_json::json!({"end":"tomorrow"}),
        serde_json::json!({"start":null}),
        serde_json::json!({"end":null}),
        serde_json::json!({"brightness":null}),
        serde_json::json!({"brightness":101}),
        serde_json::json!({"brightness":-1}),
        serde_json::json!({"brightness":1.5}),
    ] {
        let (code, _) = send_request_with_method(
            app.clone(),
            "/api/backlight/bedtime",
            Method::PATCH,
            Some(body.clone()),
            Some("test-api-key"),
        )
        .await;
        assert!(
            matches!(
                code,
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
            ),
            "{body}: {code}"
        );
        let (_, after) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
        assert_eq!(after["bedtime"], before["bedtime"]);
        assert_eq!(after["intended"], before["intended"]);
    }
    assert_eq!(device.packets.lock().unwrap().len(), writes);
}
