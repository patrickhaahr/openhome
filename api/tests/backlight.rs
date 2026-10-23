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
        ) -> TransportFuture<'a> {
            Box::pin(async move {
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
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                Ok(Exchange {
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
    ) -> openhome_api::services::backlight::TransportFuture<'a> {
        Box::pin(async move {
            self.packets.lock().unwrap().push(writes.to_vec());
            // Known synthetic TV Sync profile, brightness and power replies.
            Ok(openhome_api::services::backlight::Exchange {
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
async fn rapid_clients_persist_the_latest_target_before_dispatch_and_skip_obsolete_work() {
    use openhome_api::services::backlight::{GattTransport, Packet, TransportFuture};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    struct BlockedDevice {
        device: ManualDevice,
        first: AtomicBool,
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
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                assert_eq!(
                    self.active.fetch_add(1, Ordering::SeqCst),
                    0,
                    "GATT operations must not overlap"
                );
                if !writes.is_empty() && self.first.swap(false, Ordering::SeqCst) {
                    self.entered.notify_one();
                    self.release.acquire().await.unwrap().forget();
                }
                let result = self.device.exchange(writes, queries).await;
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
        active: AtomicUsize::new(0),
        entered: tokio::sync::Notify::default(),
        release: tokio::sync::Semaphore::new(0),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state.clone());
    let target = |red, green, blue| serde_json::json!({"mode":"color","rgb":{"red":red,"green":green,"blue":blue},"brightness":20});
    let first = tokio::spawn(select(app.clone(), target(255, 0, 0)));
    tokio::time::timeout(std::time::Duration::from_secs(2), device.entered.notified())
        .await
        .unwrap();
    let obsolete_target = target(0, 255, 0);
    let obsolete = tokio::spawn(select(app.clone(), obsolete_target.clone()));
    wait_for_target(app.clone(), &obsolete_target).await;
    let latest_target = target(0, 0, 255);
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
    let changes: Vec<_> = writes
        .iter()
        .filter(|packets| !packets.is_empty())
        .collect();
    assert_eq!(
        changes.len(),
        2,
        "superseded green must never reach the controller"
    );
    assert_eq!(changes[0][1][4..7], [255, 0, 0]);
    assert_eq!(changes[1][1][4..7], [0, 0, 255]);
}
