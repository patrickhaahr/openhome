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
