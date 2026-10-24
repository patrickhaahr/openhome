mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::{send_request, send_request_with_method};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::backlight::BacklightService,
};

async fn app() -> Router {
    let (_, state) = common::test_app_with_db().await;
    let service = BacklightService::new(state.db.clone(), None, None)
        .await
        .unwrap();
    routes::backlight::router(service)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".into()))
        }))
}

#[tokio::test]
async fn backlight_requires_authorization_and_reports_incomplete_setup() {
    let app = app().await;
    assert_eq!(
        send_request(app.clone(), "/api/backlight", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["setup"], "incomplete");
    assert_eq!(body["reachability"], "unconfigured");
    assert!(body["observed"].is_null());
    let (status, _) = send_request_with_method(
        app,
        "/api/backlight/selection",
        Method::PUT,
        Some(serde_json::json!({"mode":"tv-sync"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn off_remains_available_without_bluetooth_and_does_not_claim_acceptance() {
    let app = app().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/backlight/selection",
        Method::PUT,
        Some(serde_json::json!({"mode":"off"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send_request(app, "/api/backlight", Some("test-api-key")).await;
    assert_eq!(body["intended"]["mode"], "off");
    assert_eq!(body["command"], "failed");
    assert!(body["acknowledged"].is_null());
}

#[tokio::test]
async fn captured_profile_is_private_durable_and_cannot_be_replaced_from_static_mode() {
    use openhome_api::services::backlight::{Exchange, GattTransport, Packet, TransportFuture};
    use std::sync::Arc;
    struct Device {
        mode: std::sync::atomic::AtomicU8,
        writes: std::sync::Mutex<Vec<Packet>>,
    }
    impl GattTransport for Device {
        fn matches_device(&self, _identity: &str) -> bool {
            false
        }
        fn exchange<'a>(
            &'a self,
            writes: &'a [Packet],
            queries: &'a [Vec<u8>],
            current: &'a openhome_api::services::backlight::CurrentOperation,
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                if !current() {
                    return Err(openhome_api::services::backlight::TransportError::Superseded);
                }
                self.writes.lock().unwrap().extend_from_slice(writes);
                // Synthetic protocol responses, unrelated to any household's settings.
                let replies = queries
                    .iter()
                    .map(|query| {
                        let mut packet = [0_u8; 20];
                        packet[..query.len()].copy_from_slice(query);
                        packet[2] = match query[1] {
                            1 => 1,
                            4 => 45,
                            _ => self.mode.load(std::sync::atomic::Ordering::SeqCst),
                        };
                        if query[1] == 5 {
                            packet[3..6].copy_from_slice(&[7, 42, 19]);
                        }
                        packet[19] = packet[..19].iter().fold(0, |a, b| a ^ b);
                        packet
                    })
                    .collect();
                Ok(Exchange {
                    reconnected: false,
                    writes_accepted: true,
                    replies,
                })
            })
        }
    }
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(Device {
        mode: std::sync::atomic::AtomicU8::new(0),
        writes: std::sync::Mutex::new(Vec::new()),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = routes::backlight::router(service)
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".into()))
        }));
    let (status, captured) = send_request_with_method(
        app.clone(),
        "/api/backlight/profile",
        Method::POST,
        Some(serde_json::json!({"source":"capture"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(captured["setup"], "ready");
    assert!(captured.get("tv_sync_profile").is_none());
    assert!(captured.get("device").is_none());
    device.mode.store(0x15, std::sync::atomic::Ordering::SeqCst);
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/backlight/profile",
        Method::POST,
        Some(serde_json::json!({"source":"capture"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    device.mode.store(0, std::sync::atomic::Ordering::SeqCst);
    let (status, selected) = send_request_with_method(
        app,
        "/api/backlight/selection",
        Method::PUT,
        Some(serde_json::json!({"mode":"tv-sync"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(selected["acknowledged"]["mode"], "tv-sync");
    assert_eq!(selected["observed"]["brightness"], 45);
    assert_eq!(
        device.writes.lock().unwrap()[1],
        [
            0x33, 5, 0, 7, 42, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8
        ]
    );
    let restarted = BacklightService::new(state.db.clone(), None, None)
        .await
        .unwrap()
        .status()
        .await;
    let restarted = serde_json::to_value(restarted).unwrap();
    assert_eq!(restarted["setup"], "ready");
    assert_eq!(restarted["intended"]["mode"], "tv-sync");
    assert!(restarted["acknowledged"].is_null());
    assert!(restarted["observed"].is_null());
}

#[tokio::test]
async fn accepted_write_without_readback_does_not_invent_an_observation() {
    use openhome_api::services::backlight::{Exchange, GattTransport, Packet, TransportFuture};
    struct UnreadableDevice;
    impl GattTransport for UnreadableDevice {
        fn matches_device(&self, _identity: &str) -> bool {
            false
        }
        fn exchange<'a>(
            &'a self,
            writes: &'a [Packet],
            _queries: &'a [Vec<u8>],
            current: &'a openhome_api::services::backlight::CurrentOperation,
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                if !current() {
                    return Err(openhome_api::services::backlight::TransportError::Superseded);
                }
                Ok(Exchange {
                    reconnected: false,
                    writes_accepted: !writes.is_empty(),
                    replies: Vec::new(),
                })
            })
        }
    }
    let (_, state) = common::test_app_with_db().await;
    let service = BacklightService::new(
        state.db.clone(),
        Some(std::sync::Arc::new(UnreadableDevice)),
        None,
    )
    .await
    .unwrap();
    let app =
        routes::backlight::router(service)
            .with_state(state)
            .layer(axum::middleware::from_fn(|req, next| {
                auth_middleware(req, next, ApiKey::new("test-api-key".into()))
            }));
    let (status, body) = send_request_with_method(
        app,
        "/api/backlight/selection",
        Method::PUT,
        Some(serde_json::json!({"mode":"off"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["intended"]["mode"], "off");
    assert_eq!(body["acknowledged"]["mode"], "off");
    assert!(body["observed"].is_null());
    assert_eq!(body["stale"], true);
    assert_eq!(body["setup"], "incomplete");
}

/// Synthetic controller at the complete GATT-operation boundary.
struct ManualDevice {
    packets: std::sync::Mutex<Vec<Vec<openhome_api::services::backlight::Packet>>>,
}
impl openhome_api::services::backlight::GattTransport for ManualDevice {
    fn matches_device(&self, _identity: &str) -> bool {
        false
    }
    fn exchange<'a>(
        &'a self,
        writes: &'a [openhome_api::services::backlight::Packet],
        _queries: &'a [Vec<u8>],
        current: &'a openhome_api::services::backlight::CurrentOperation,
    ) -> openhome_api::services::backlight::TransportFuture<'a> {
        Box::pin(async move {
            if !current() {
                return Err(openhome_api::services::backlight::TransportError::Superseded);
            }
            self.packets.lock().unwrap().push(writes.to_vec());
            // Known synthetic TV Sync profile, brightness and power replies.
            Ok(openhome_api::services::backlight::Exchange {
                reconnected: false,
                writes_accepted: !writes.is_empty(),
                replies: vec![
                    [
                        0xaa, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xaa,
                    ],
                    [
                        0xaa, 4, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x83,
                    ],
                    [
                        0xaa, 5, 0, 7, 42, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x91,
                    ],
                ],
            })
        })
    }
}
fn authenticated(service: BacklightService, state: openhome_api::AppState) -> Router {
    routes::backlight::router(service)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".into()))
        }))
}
async fn select(app: Router, selection: serde_json::Value) -> (StatusCode, serde_json::Value) {
    send_request_with_method(
        app,
        "/api/backlight/selection",
        Method::PUT,
        Some(selection),
        Some("test-api-key"),
    )
    .await
}

#[tokio::test]
async fn manual_rgb_is_durable_and_tv_sync_restores_its_separate_brightness() {
    let (_, state) = common::test_app_with_db().await;
    let device = std::sync::Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state.clone());
    let target =
        serde_json::json!({"mode":"color", "rgb":{"red":255,"green":128,"blue":0},"brightness":20});
    let (code, result) = select(app.clone(), target.clone()).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(result["intended"], target);
    assert_eq!(result["acknowledged"], target);
    assert!(result["observed"].get("rgb").is_none());
    // H605C's validated whole-strip RGBIC encoding; generic 33 05 02 is ineffective.
    assert_eq!(
        device.packets.lock().unwrap()[1][1],
        [
            0x33, 5, 0x15, 1, 255, 128, 0, 0, 0, 0, 0, 0, 255, 127, 0, 0, 0, 0, 0, 0xdd
        ]
    );
    let restarted = BacklightService::new(state.db.clone(), None, None)
        .await
        .unwrap();
    let (_, read) = send_request(
        authenticated(restarted, state),
        "/api/backlight",
        Some("test-api-key"),
    )
    .await;
    assert_eq!(read["intended"], target);
    let (code, _) = select(
        app.clone(),
        serde_json::json!({"mode":"tv-sync","brightness":15}),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    let (code, _) = select(app, serde_json::json!({"mode":"tv-sync"})).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(
        device.packets.lock().unwrap().last().unwrap()[2][..3],
        [0x33, 4, 45]
    );
}

#[tokio::test]
async fn invalid_manual_selections_leave_the_durable_target_untouched() {
    let app = app().await;
    for invalid in [
        serde_json::json!({"mode":"color","rgb":{"red":256,"green":0,"blue":0},"brightness":20}),
        serde_json::json!({"mode":"color","rgb":{"red":0,"green":0,"blue":0},"brightness":101}),
        serde_json::json!({"mode":"color","rgb":{"red":-1,"green":0,"blue":0},"brightness":20}),
        serde_json::json!({"mode":"color","rgb":{"red":0,"green":0,"blue":0},"brightness":1.5}),
        serde_json::json!({"mode":"color","rgb":{"red":0,"green":0,"blue":0}}),
        serde_json::json!({"mode":"tv-sync","brightness":101}),
        serde_json::json!({"mode":"off","brightness":10}),
    ] {
        assert_eq!(
            select(app.clone(), invalid).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let (_, status) = send_request(app, "/api/backlight", Some("test-api-key")).await;
    assert_eq!(status["intended"], serde_json::json!({"mode":"off"}));
}

#[tokio::test]
async fn rapid_effect_clients_cancel_partial_uploads_and_keep_only_the_latest_target() {
    use openhome_api::services::backlight::{GattTransport, Packet, TransportFuture};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    struct BlockedDevice {
        device: ManualDevice,
        first: AtomicBool,
        frames: std::sync::Mutex<Vec<Packet>>,
        active: AtomicUsize,
        entered: tokio::sync::Notify,
        release: tokio::sync::Semaphore,
    }
    impl GattTransport for BlockedDevice {
        fn matches_device(&self, _identity: &str) -> bool {
            false
        }
        fn exchange<'a>(
            &'a self,
            writes: &'a [Packet],
            queries: &'a [Vec<u8>],
            current: &'a openhome_api::services::backlight::CurrentOperation,
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                assert_eq!(
                    self.active.fetch_add(1, Ordering::SeqCst),
                    0,
                    "GATT operations must not overlap"
                );
                for packet in writes {
                    if !current() {
                        self.active.fetch_sub(1, Ordering::SeqCst);
                        return Err(openhome_api::services::backlight::TransportError::Superseded);
                    }
                    self.frames.lock().unwrap().push(*packet);
                    if self.first.swap(false, Ordering::SeqCst) {
                        self.entered.notify_one();
                        self.release.acquire().await.unwrap().forget();
                    }
                }
                let result = self.device.exchange(writes, queries, current).await;
                self.active.fetch_sub(1, Ordering::SeqCst);
                result
            })
        }
    }
    async fn wait_for_target(app: Router, target: &serde_json::Value) {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let (_, status) =
                    send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
                if &status["intended"] == target {
                    assert_eq!(status["command"], "pending");
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("latest target should be durable while an earlier GATT operation is blocked");
    }
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(BlockedDevice {
        device: ManualDevice {
            packets: std::sync::Mutex::default(),
        },
        first: AtomicBool::new(true),
        frames: std::sync::Mutex::default(),
        active: AtomicUsize::new(0),
        entered: tokio::sync::Notify::default(),
        release: tokio::sync::Semaphore::new(0),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state.clone());
    let target = |effect| serde_json::json!({"mode":"effect","effect":effect,"brightness":20});
    let first = tokio::spawn(select(app.clone(), target("party")));
    tokio::time::timeout(std::time::Duration::from_secs(2), device.entered.notified())
        .await
        .unwrap();
    let obsolete_target = target("forest");
    let obsolete = tokio::spawn(select(app.clone(), obsolete_target.clone()));
    wait_for_target(app.clone(), &obsolete_target).await;
    let latest_target = target("aurora");
    let latest = tokio::spawn(select(app.clone(), latest_target.clone()));
    wait_for_target(app.clone(), &latest_target).await;
    let restarted = BacklightService::new(state.db.clone(), None, None)
        .await
        .unwrap();
    let (_, persisted) = send_request(
        authenticated(restarted, state),
        "/api/backlight",
        Some("test-api-key"),
    )
    .await;
    assert_eq!(persisted["intended"], latest_target);
    // Refreshes must use the same whole-operation serialization as selections.
    let refresh = tokio::spawn(send_request_with_method(
        app.clone(),
        "/api/backlight/refresh",
        Method::POST,
        None,
        Some("test-api-key"),
    ));
    device.release.add_permits(1);
    for result in [
        first.await.unwrap(),
        obsolete.await.unwrap(),
        latest.await.unwrap(),
        refresh.await.unwrap(),
    ] {
        assert_eq!(result.0, StatusCode::OK);
    }
    let (_, final_status) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(final_status["acknowledged"], latest_target);
    let operations = device.device.packets.lock().unwrap().len();
    // Reading or waiting must not fight a physical button / other controller.
    for _ in 0..3 {
        send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    }
    assert_eq!(device.device.packets.lock().unwrap().len(), operations);
    let writes = device.device.packets.lock().unwrap();
    let changes = writes.iter().filter(|packets| !packets.is_empty()).count();
    assert_eq!(changes, 1, "obsolete uploads must never finish");
    let frames = device.frames.lock().unwrap();
    assert_eq!(
        &frames[0][..5],
        &[0xa3, 0, 1, 9, 2],
        "Party began uploading"
    );
    assert_eq!(
        &frames[1][..5],
        &[0xa3, 0, 1, 5, 2],
        "Aurora resets the partial upload"
    );
    let activations: Vec<_> = frames
        .iter()
        .filter(|frame| frame[..3] == [0x33, 5, 4])
        .collect();
    assert_eq!(
        activations.len(),
        1,
        "no obsolete scene activation or interleaving"
    );
    assert_eq!(&activations[0][..5], &[0x33, 5, 4, 215, 0]);
}

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

struct ControlledClock(std::sync::Mutex<chrono::DateTime<chrono::Utc>>);
impl ControlledClock {
    fn at(instant: &str) -> Self {
        Self(std::sync::Mutex::new(instant.parse().unwrap()))
    }
    fn set(&self, instant: &str) {
        *self.0.lock().unwrap() = instant.parse().unwrap();
    }
}
impl openhome_api::services::backlight::routine::Clock for ControlledClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        *self.0.lock().unwrap()
    }
}

async fn wait_for_mode(app: Router, mode: &str) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            let (_, body) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
            if body["intended"]["mode"] == mode
                && body["command"] == "acknowledged"
                && body["recovery"] == "recovered"
            {
                return body;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn routine_runs_without_dashboard_expires_off_at_start_and_restores_saved_tv_sync_at_end() {
    use std::sync::Arc;
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let clock = Arc::new(ControlledClock::at("2026-10-23T20:59:59Z")); // Copenhagen 22:59:59
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
    // Wait for the end transition, rather than the preceding manual TV Sync write.
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

#[test]
fn bedtime_default_calendar_includes_start_excludes_end_and_tracks_copenhagen_dates() {
    use openhome_api::services::backlight::routine::{BedtimeSettings, evaluate};
    let settings: BedtimeSettings = serde_json::from_value(
        serde_json::json!({"enabled":true,"start":"23:00","end":"00:00","brightness":20}),
    )
    .unwrap();
    for (instant, mode, next) in [
        ("2026-10-23T20:59:59Z", "tv-sync", "2026-10-23T21:00:00Z"),
        ("2026-10-23T21:00:00Z", "color", "2026-10-23T22:00:00Z"),
        ("2026-10-23T21:59:59Z", "color", "2026-10-23T22:00:00Z"),
        ("2026-10-23T22:00:00Z", "tv-sync", "2026-10-24T21:00:00Z"),
        ("2026-10-25T22:00:00Z", "color", "2026-10-25T23:00:00Z"),
    ] {
        let result = evaluate(&settings, instant.parse().unwrap());
        assert_eq!(
            serde_json::to_value(result.selection).unwrap()["mode"],
            mode
        );
        assert_eq!(
            result.next_transition.unwrap(),
            next.parse::<chrono::DateTime<chrono::Utc>>().unwrap()
        );
    }
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
    let clock = Arc::new(ControlledClock::at("2026-10-23T21:30:00Z")); // 23:30 Copenhagen
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

#[test]
fn edited_windows_follow_actual_copenhagen_minutes_through_midnight_skip_and_repeat() {
    use openhome_api::services::backlight::routine::{BedtimeSettings, evaluate};
    // Expected UTC instants are calendar examples, independent of the evaluator.
    for (start, end, instant, mode, next) in [
        (
            "22:00",
            "02:00",
            "2026-01-15T23:00:00Z",
            "color",
            "2026-01-16T01:00:00Z",
        ),
        (
            "22:00",
            "02:00",
            "2026-01-16T01:00:00Z",
            "tv-sync",
            "2026-01-16T21:00:00Z",
        ),
        // 29 March jumps from 01:59 CET to 03:00 CEST.
        (
            "02:30",
            "03:30",
            "2026-03-29T00:59:59Z",
            "tv-sync",
            "2026-03-29T01:00:00Z",
        ),
        (
            "02:30",
            "03:30",
            "2026-03-29T01:00:00Z",
            "color",
            "2026-03-29T01:30:00Z",
        ),
        (
            "01:30",
            "02:30",
            "2026-03-29T00:45:00Z",
            "color",
            "2026-03-29T01:00:00Z",
        ),
        // A wholly skipped red interval does not invent an action that day.
        (
            "02:10",
            "02:20",
            "2026-03-29T00:59:00Z",
            "tv-sync",
            "2026-03-30T00:10:00Z",
        ),
        // 25 October repeats 02:00–02:59, so a contained window runs twice.
        (
            "02:15",
            "02:45",
            "2026-10-25T00:14:59Z",
            "tv-sync",
            "2026-10-25T00:15:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T00:15:00Z",
            "color",
            "2026-10-25T00:45:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T00:45:00Z",
            "tv-sync",
            "2026-10-25T01:15:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T01:15:00Z",
            "color",
            "2026-10-25T01:45:00Z",
        ),
        // Crossing the rollback can leave/reenter even without a named boundary.
        (
            "02:30",
            "03:30",
            "2026-10-25T00:45:00Z",
            "color",
            "2026-10-25T01:00:00Z",
        ),
        (
            "02:30",
            "03:30",
            "2026-10-25T01:00:00Z",
            "tv-sync",
            "2026-10-25T01:30:00Z",
        ),
        // A window containing the whole repeated hour stays active through it.
        (
            "01:30",
            "03:30",
            "2026-10-25T00:30:00Z",
            "color",
            "2026-10-25T02:30:00Z",
        ),
    ] {
        let settings: BedtimeSettings = serde_json::from_value(
            serde_json::json!({"enabled":true,"start":start,"end":end,"brightness":20}),
        )
        .unwrap();
        let result = evaluate(&settings, instant.parse().unwrap());
        assert_eq!(
            serde_json::to_value(result.selection).unwrap()["mode"],
            mode,
            "{start}–{end} at {instant}"
        );
        assert_eq!(
            result.next_transition.unwrap(),
            next.parse::<chrono::DateTime<chrono::Utc>>().unwrap(),
            "{start}–{end} at {instant}"
        );
    }
}

#[tokio::test]
async fn restart_recovers_current_custom_window_and_only_unexpired_overrides() {
    use std::sync::Arc;
    let (_, mut state) = common::test_app_with_db().await;
    let file = std::env::temp_dir().join(format!("openhome-backlight-{}.db", uuid::Uuid::new_v4()));
    let database_url = format!("sqlite:{}?mode=rwc", file.display());
    state.db = sqlx::SqlitePool::connect(&database_url).await.unwrap();
    sqlx::migrate!("./migrations").run(&state.db).await.unwrap();
    let device = Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let clock = Arc::new(ControlledClock::at("2026-10-23T20:00:00Z"));
    let service =
        BacklightService::with_clock(state.db.clone(), Some(device.clone()), None, clock.clone())
            .await
            .unwrap();
    let app = authenticated(service, state.clone());
    send_request_with_method(
        app.clone(),
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({"enabled":true,"start":"22:30","end":"02:00","brightness":31})),
        Some("test-api-key"),
    )
    .await;
    select(app, serde_json::json!({"mode":"off"})).await;
    for (now, expected, source) in [
        ("2026-10-23T20:10:00Z", "off", "manual"),
        ("2026-10-23T20:45:00Z", "color", "automatic"),
        // Both boundaries were missed; only the current TV Sync target is applied.
        ("2026-10-24T10:00:00Z", "tv-sync", "automatic"),
    ] {
        clock.set(now);
        // Reopen the file-backed main database, not the original connection pool.
        state.db.close().await;
        state.db = sqlx::SqlitePool::connect(&database_url).await.unwrap();
        device.packets.lock().unwrap().clear();
        let restarted = BacklightService::with_clock(
            state.db.clone(),
            Some(device.clone()),
            None,
            clock.clone(),
        )
        .await
        .unwrap();
        let app = authenticated(restarted.clone(), state.clone());
        let runner = tokio::spawn(restarted.run_routine());
        let recovered = wait_for_mode(app, expected).await;
        assert_eq!(recovered["control_source"], source);
        assert_eq!(recovered["recovery"], "recovered");
        {
            let packets = device.packets.lock().unwrap();
            let writes: Vec<_> = packets.iter().filter(|p| !p.is_empty()).collect();
            assert_eq!(writes.len(), 1, "restart must not replay elapsed actions");
            if expected == "color" {
                assert_eq!(writes[0][2][..3], [0x33, 4, 31]);
            }
            if expected == "tv-sync" {
                assert_eq!(writes[0][1][..6], [0x33, 5, 0, 7, 42, 19]);
                assert_eq!(writes[0][2][..3], [0x33, 4, 45]);
            }
        }
        runner.abort();
        let _ = runner.await;
    }
    state.db.close().await;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
    }
}

struct RecoveringDevice {
    device: ManualDevice,
    online: std::sync::atomic::AtomicBool,
    readable: std::sync::atomic::AtomicBool,
    reconnected: std::sync::atomic::AtomicBool,
    active: std::sync::atomic::AtomicUsize,
    block: std::sync::atomic::AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Semaphore,
}
impl RecoveringDevice {
    fn new() -> Self {
        Self {
            device: ManualDevice {
                packets: std::sync::Mutex::default(),
            },
            online: true.into(),
            readable: true.into(),
            reconnected: false.into(),
            active: 0.into(),
            block: false.into(),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Semaphore::new(0),
        }
    }
}
impl openhome_api::services::backlight::GattTransport for RecoveringDevice {
    fn matches_device(&self, _identity: &str) -> bool {
        false
    }
    fn exchange<'a>(
        &'a self,
        writes: &'a [openhome_api::services::backlight::Packet],
        queries: &'a [Vec<u8>],
        current: &'a openhome_api::services::backlight::CurrentOperation,
    ) -> openhome_api::services::backlight::TransportFuture<'a> {
        Box::pin(async move {
            use std::sync::atomic::Ordering::SeqCst;
            assert_eq!(
                self.active.fetch_add(1, SeqCst),
                0,
                "whole GATT operations must be serialized"
            );
            if self.block.swap(false, SeqCst) {
                self.entered.notify_one();
                self.release.acquire().await.unwrap().forget();
            }
            let result = if self.online.load(SeqCst) {
                openhome_api::services::backlight::GattTransport::exchange(
                    &self.device,
                    writes,
                    queries,
                    current,
                )
                .await
                .map(|mut result| {
                    result.reconnected = self.reconnected.swap(false, SeqCst);
                    if !self.readable.load(SeqCst) {
                        result.replies.clear();
                    }
                    result
                })
            } else {
                Err(openhome_api::services::backlight::TransportError::Unavailable)
            };
            self.active.fetch_sub(1, SeqCst);
            result
        })
    }
}

#[tokio::test]
async fn outage_retains_stale_observation_and_recovers_only_latest_target_during_concurrent_commands()
 {
    use std::sync::{Arc, atomic::Ordering::SeqCst};
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(RecoveringDevice::new());
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service.clone(), state);
    select(app.clone(), serde_json::json!({"mode":"tv-sync"})).await;
    let runner = tokio::spawn(service.run_routine());
    device.online.store(false, SeqCst);
    let (_, disconnected) = send_request_with_method(
        app.clone(),
        "/api/backlight/refresh",
        Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(disconnected["reachability"], "unreachable");
    assert_eq!(disconnected["stale"], true);
    assert!(
        disconnected["observed"].is_object(),
        "last observation is retained but marked stale"
    );
    assert_eq!(disconnected["recovery"], "retrying");
    let target = |red, green, blue| serde_json::json!({"mode":"color","rgb":{"red":red,"green":green,"blue":blue},"brightness":29});
    select(app.clone(), target(255, 0, 0)).await;
    select(app.clone(), target(0, 255, 0)).await;
    device.device.packets.lock().unwrap().clear();
    device.block.store(true, SeqCst);
    // The retry owns the full operation; newer commands still durably supersede its target.
    tokio::time::timeout(std::time::Duration::from_secs(4), device.entered.notified())
        .await
        .unwrap();
    let latest = target(0, 0, 255);
    let command = tokio::spawn(select(app.clone(), latest.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let (_, current) =
                send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
            if current["intended"] == latest {
                assert_eq!(current["command"], "pending");
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // The blocked attempt fails; only the newest blue target reaches the restored device.
    device.release.add_permits(1);
    command.await.unwrap();
    device.online.store(true, SeqCst);
    let recovered = wait_for_mode(app.clone(), "color").await;
    assert_eq!(recovered["acknowledged"], latest);
    assert_eq!(recovered["recovery"], "recovered");
    let packets = device.device.packets.lock().unwrap().clone();
    assert!(
        packets
            .iter()
            .filter(|p| !p.is_empty())
            .all(|p| p[1][4..7] == [0, 0, 255])
    );
    let writes = packets.iter().filter(|p| !p.is_empty()).count();
    // Read-only healthy refreshes must preserve external changes, not repeat target writes.
    for _ in 0..3 {
        send_request_with_method(
            app.clone(),
            "/api/backlight/refresh",
            Method::POST,
            None,
            Some("test-api-key"),
        )
        .await;
    }
    assert_eq!(
        device
            .device
            .packets
            .lock()
            .unwrap()
            .iter()
            .filter(|p| !p.is_empty())
            .count(),
        writes
    );
    runner.abort();
}

#[tokio::test]
async fn silent_reconnect_reapplies_current_target_but_healthy_refresh_does_not() {
    use std::sync::Arc;
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(RecoveringDevice::new());
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service.clone(), state);
    select(app.clone(), serde_json::json!({"mode":"tv-sync"})).await;
    device.device.packets.lock().unwrap().clear();
    // Adapter establishes a new link while the API was not polling. An external
    // controller may have changed the mode; this successful read is still recovery.
    device
        .reconnected
        .store(true, std::sync::atomic::Ordering::SeqCst);
    send_request_with_method(
        app.clone(),
        "/api/backlight/refresh",
        Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;
    let runner = tokio::spawn(service.run_routine());
    wait_for_mode(app.clone(), "tv-sync").await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        device
            .device
            .packets
            .lock()
            .unwrap()
            .iter()
            .filter(|p| !p.is_empty())
            .count(),
        1
    );
    runner.abort();
}

#[tokio::test]
async fn failed_transition_persistence_is_retried_with_current_clock_before_any_device_write() {
    use std::sync::Arc;
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let clock = Arc::new(ControlledClock::at("2026-10-23T20:00:00Z"));
    let service =
        BacklightService::with_clock(state.db.clone(), Some(device.clone()), None, clock.clone())
            .await
            .unwrap();
    let app = authenticated(service.clone(), state.clone());
    send_request_with_method(
        app.clone(),
        "/api/backlight/bedtime",
        Method::PATCH,
        Some(serde_json::json!({"enabled":true})),
        Some("test-api-key"),
    )
    .await;
    sqlx::query("CREATE TRIGGER unavailable_backlight_storage BEFORE UPDATE ON backlight BEGIN SELECT RAISE(FAIL, 'storage unavailable'); END").execute(&state.db).await.unwrap();
    device.packets.lock().unwrap().clear();
    clock.set("2026-10-23T21:30:00Z");
    let runner = tokio::spawn(service.run_routine());
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let (_, failed) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(failed["recovery"], "retrying");
    assert_eq!(failed["command"], "failed");
    assert!(
        device.packets.lock().unwrap().is_empty(),
        "unsaved target must never reach hardware"
    );
    // Storage returns after the entire red window has passed.
    clock.set("2026-10-24T10:00:00Z");
    sqlx::query("DROP TRIGGER unavailable_backlight_storage")
        .execute(&state.db)
        .await
        .unwrap();
    wait_for_mode(app, "tv-sync").await;
    let packets = device.packets.lock().unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0][1][..6], [0x33, 5, 0, 7, 42, 19]);
    runner.abort();
}

#[tokio::test]
async fn missing_or_invalid_profile_blocks_recovery_without_capturing_or_guessing_camera_settings()
{
    use std::sync::Arc;
    for invalid in [None, Some("{\"schema_version\":99}")] {
        let (_, state) = common::test_app_with_db().await;
        sqlx::query("UPDATE backlight SET tv_sync_profile = ?, manual_override = NULL, intended = '{\"mode\":\"tv-sync\"}' WHERE id = 1").bind(invalid).execute(&state.db).await.unwrap();
        let device = Arc::new(ManualDevice {
            packets: std::sync::Mutex::default(),
        });
        let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
            .await
            .unwrap();
        let app = authenticated(service.clone(), state);
        let runner = tokio::spawn(service.run_routine());
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let (_, status) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
        assert_eq!(status["setup"], "incomplete");
        assert_eq!(status["recovery"], "profile-required");
        assert!(
            device.packets.lock().unwrap().is_empty(),
            "startup must not implicitly capture a new camera profile"
        );
        assert_eq!(
            select(app, serde_json::json!({"mode":"off"})).await.0,
            StatusCode::OK
        );
        runner.abort();
    }
}

#[tokio::test]
async fn incomplete_profile_expires_off_override_and_persists_current_automatic_target_without_writes()
 {
    use std::sync::Arc;
    for invalid in [None, Some("{\"schema_version\":99}")] {
        let (_, state) = common::test_app_with_db().await;
        sqlx::query("UPDATE backlight SET tv_sync_profile = ?, bedtime = ?, manual_override = ? WHERE id = 1")
            .bind(invalid)
            .bind(r#"{"enabled":true,"start":"23:00","end":"00:00","brightness":20}"#)
            .bind(r#"{"selection":{"mode":"off"},"expires_at":"2026-10-23T21:00:00Z"}"#)
            .execute(&state.db).await.unwrap();
        let device = Arc::new(ManualDevice {
            packets: std::sync::Mutex::default(),
        });
        let clock = Arc::new(ControlledClock::at("2026-10-23T20:59:59Z"));
        let service = BacklightService::with_clock(
            state.db.clone(),
            Some(device.clone()),
            None,
            clock.clone(),
        )
        .await
        .unwrap();
        let app = authenticated(service.clone(), state.clone());
        let (_, before) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
        assert_eq!(before["intended"]["mode"], "off");
        assert_eq!(before["control_source"], "manual");
        clock.set("2026-10-23T21:00:00Z");
        let runner = tokio::spawn(service.run_routine());
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let (_, current) = send_request(app, "/api/backlight", Some("test-api-key")).await;
        runner.abort();
        assert_eq!(current["control_source"], "automatic");
        assert_eq!(
            current["intended"],
            serde_json::json!({
                "mode":"color", "rgb":{"red":255,"green":0,"blue":0}, "brightness":20
            })
        );
        assert!(current["override_expires_at"].is_null());
        assert_eq!(current["next_transition"], "2026-10-23T22:00:00Z");
        assert_eq!(current["setup"], "incomplete");
        assert_eq!(current["recovery"], "profile-required");
        assert_eq!(current["command"], "failed");
        let (intended, manual_override): (String, Option<String>) =
            sqlx::query_as("SELECT intended, manual_override FROM backlight WHERE id = 1")
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&intended).unwrap(),
            current["intended"]
        );
        assert!(manual_override.is_none());
        assert!(
            device.packets.lock().unwrap().is_empty(),
            "incomplete setup must neither capture a profile nor dispatch the automatic target"
        );
    }
}

#[tokio::test]
async fn incomplete_readback_stays_stale_until_current_target_has_been_recovered() {
    use std::sync::{Arc, atomic::Ordering::SeqCst};
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(RecoveringDevice::new());
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service.clone(), state);
    select(app.clone(), serde_json::json!({"mode":"tv-sync"})).await;
    device.readable.store(false, SeqCst);
    let (_, unreadable) = send_request_with_method(
        app.clone(),
        "/api/backlight/refresh",
        Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(unreadable["stale"], true);
    assert_eq!(unreadable["recovery"], "retrying");
    let (_, selected) = select(app.clone(), serde_json::json!({"mode":"off"})).await;
    assert_eq!(selected["command"], "acknowledged");
    assert_eq!(selected["stale"], true);
    assert_eq!(
        selected["recovery"], "retrying",
        "accepted writes are not successful recovery without readable state"
    );
    device.readable.store(true, SeqCst);
    let runner = tokio::spawn(service.run_routine());
    let recovered = wait_for_mode(app, "off").await;
    assert_eq!(recovered["recovery"], "recovered");
    assert_eq!(recovered["stale"], false);
    runner.abort();
}

#[tokio::test]
async fn manual_command_expiring_while_waiting_for_the_worker_never_reaches_hardware() {
    use std::sync::{Arc, atomic::Ordering::SeqCst};
    for waiting_for_worker in [true, false] {
        let (_, state) = common::test_app_with_db().await;
        let device = Arc::new(RecoveringDevice::new());
        let clock = Arc::new(ControlledClock::at("2026-10-23T20:59:59Z"));
        let service = BacklightService::with_clock(
            state.db.clone(),
            Some(device.clone()),
            None,
            clock.clone(),
        )
        .await
        .unwrap();
        let app = authenticated(service.clone(), state);
        send_request_with_method(
            app.clone(),
            "/api/backlight/bedtime",
            Method::PATCH,
            Some(serde_json::json!({"enabled":true})),
            Some("test-api-key"),
        )
        .await;
        device.device.packets.lock().unwrap().clear();
        device.block.store(true, SeqCst);
        let refresh = if waiting_for_worker {
            let refresh = tokio::spawn(send_request_with_method(
                app.clone(),
                "/api/backlight/refresh",
                Method::POST,
                None,
                Some("test-api-key"),
            ));
            device.entered.notified().await;
            Some(refresh)
        } else {
            None
        };
        let command = tokio::spawn(select(app.clone(), serde_json::json!({"mode":"off"})));
        if !waiting_for_worker {
            device.entered.notified().await;
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let (_, current) =
                    send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
                if current["intended"]["mode"] == "off" {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        clock.set("2026-10-23T21:05:00Z");
        device.release.add_permits(1);
        if let Some(refresh) = refresh {
            refresh.await.unwrap();
        }
        command.await.unwrap();
        assert!(
            device
                .device
                .packets
                .lock()
                .unwrap()
                .iter()
                .all(Vec::is_empty),
            "expired queued or reconnecting Off must not flash before current red"
        );
        let runner = tokio::spawn(service.run_routine());
        wait_for_mode(app, "color").await;
        runner.abort();
    }
}

#[tokio::test]
async fn native_effect_is_validated_durable_and_tv_sync_keeps_its_profile() {
    let (_, state) = common::test_app_with_db().await;
    let device = std::sync::Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state.clone());
    let target = serde_json::json!({"mode":"effect","effect":"aurora","brightness":29});
    let (code, result) = select(app.clone(), target.clone()).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(result["intended"], target);
    assert_eq!(result["acknowledged"], target);
    assert_eq!(result["control_source"], "manual");
    assert!(result["observed"].get("effect").is_none());
    {
        let batches = device.packets.lock().unwrap();
        let upload = batches.last().unwrap();
        // Independent H605C catalog/ptReal reference: Aurora 215, five data frames.
        assert_eq!(&upload[0][..5], &[0xa3, 0, 1, 5, 2]);
        assert_eq!(&upload[4][..2], &[0xa3, 0xff]);
        assert_eq!(&upload[5][..5], &[0x33, 5, 4, 215, 0]);
    }
    let restarted = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(restarted.status().await).unwrap()["intended"],
        target
    );
    let app = authenticated(restarted, state);
    let (code, _) = select(
        app.clone(),
        serde_json::json!({"mode":"effect","effect":"other-model","brightness":29}),
    )
    .await;
    assert!(code.is_client_error());
    let (_, unchanged) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(unchanged["intended"], target);
    let (_, result) = select(app, serde_json::json!({"mode":"tv-sync"})).await;
    assert_eq!(result["acknowledged"]["mode"], "tv-sync");
    let batches = device.packets.lock().unwrap();
    let restore = batches.last().unwrap();
    assert_eq!(&restore[1][..6], &[0x33, 5, 0, 7, 42, 19]);
    assert_eq!(&restore[2][..3], &[0x33, 4, 45]);
}

#[tokio::test]
async fn inactivity_hold_is_verified_on_device_without_changing_lighting_intent() {
    use openhome_api::services::backlight::{
        CurrentOperation, Exchange, GattTransport, Packet, TransportFuture,
    };
    use std::sync::{Arc, Mutex};
    fn frame(payload: &[u8]) -> Packet {
        let mut packet = [0; 20];
        packet[..payload.len()].copy_from_slice(payload);
        packet[19] = packet[..19].iter().fold(0, |a, b| a ^ b);
        packet
    }
    struct Device {
        settings: Mutex<Packet>,
        writes: Mutex<Vec<Packet>>,
        accepts_settings: std::sync::atomic::AtomicBool,
    }
    impl GattTransport for Device {
        fn matches_device(&self, _: &str) -> bool {
            false
        }
        fn exchange<'a>(
            &'a self,
            writes: &'a [Packet],
            queries: &'a [Vec<u8>],
            _: &'a CurrentOperation,
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                for write in writes {
                    self.writes.lock().unwrap().push(*write);
                    if !self
                        .accepts_settings
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        continue;
                    }
                    let mut reply = *write;
                    reply[0] = 0xaa;
                    reply[19] = reply[..19].iter().fold(0, |a, b| a ^ b);
                    *self.settings.lock().unwrap() = reply;
                }
                Ok(Exchange {
                    reconnected: false,
                    writes_accepted: true,
                    replies: queries
                        .iter()
                        .map(|query| {
                            if query == &[0xaa, 0xa9, 0x0a] {
                                *self.settings.lock().unwrap()
                            } else {
                                frame(&[0xaa, query[1], u8::from(query[1] != 5)])
                            }
                        })
                        .collect(),
                })
            })
        }
    }
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(Device {
        settings: Mutex::new(frame(&[0xaa, 0xa9, 0x0a, 6, 1, 2, 10, 0, 120, 0])),
        writes: Mutex::default(),
        accepts_settings: std::sync::atomic::AtomicBool::new(true),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state);
    let (_, before) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(
        send_request(app.clone(), "/api/backlight/inactivity", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, initial) = send_request(
        app.clone(),
        "/api/backlight/inactivity",
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["same_tone_seconds"], 120);
    let (status, changed) = send_request_with_method(
        app.clone(),
        "/api/backlight/inactivity",
        Method::POST,
        Some(serde_json::json!({"seconds":300})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        changed,
        serde_json::json!({"enabled":true,"mode":"same-tone","low_brightness_seconds":10,"same_tone_seconds":300})
    );
    assert_eq!(
        *device.writes.lock().unwrap(),
        vec![frame(&[0x33, 0xa9, 0x0a, 6, 1, 2, 10, 0, 0x2c, 1])]
    );
    let (_, after) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    for field in [
        "intended",
        "control_source",
        "bedtime",
        "override_expires_at",
    ] {
        assert_eq!(after[field], before[field]);
    }
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/backlight/inactivity",
        Method::POST,
        Some(serde_json::json!({"seconds":1801})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(device.writes.lock().unwrap().len(), 1);
    device
        .accepts_settings
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/backlight/inactivity",
        Method::POST,
        Some(serde_json::json!({"seconds":420})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "a write receipt alone cannot confirm the hold"
    );
    // A malformed native response must prevent even attempting a settings write.
    device.settings.lock().unwrap()[19] ^= 1;
    let count = device.writes.lock().unwrap().len();
    let (status, _) = send_request_with_method(
        app,
        "/api/backlight/inactivity",
        Method::POST,
        Some(serde_json::json!({"seconds":300})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(device.writes.lock().unwrap().len(), count);
}
