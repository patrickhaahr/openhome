//! Test app, devices and clock shared by the backlight test files.

#![allow(dead_code)]

use crate::common;
use crate::common::send_request;
use crate::common::send_request_with_method;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use openhome_api::auth::ApiKey;
use openhome_api::auth::auth_middleware;
use openhome_api::routes;
use openhome_api::services::backlight::BacklightService;

pub(super) async fn app() -> Router {
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

/// Synthetic controller at the complete GATT-operation boundary.
pub(super) struct ManualDevice {
    pub(super) packets: std::sync::Mutex<Vec<Vec<openhome_api::services::backlight::Packet>>>,
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

pub(super) fn authenticated(service: BacklightService, state: openhome_api::AppState) -> Router {
    routes::backlight::router(service)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".into()))
        }))
}

pub(super) async fn select(
    app: Router,
    selection: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    send_request_with_method(
        app,
        "/api/backlight/selection",
        Method::PUT,
        Some(selection),
        Some("test-api-key"),
    )
    .await
}

pub(super) struct ControlledClock(std::sync::Mutex<chrono::DateTime<chrono::Utc>>);

impl ControlledClock {
    pub(super) fn at(instant: &str) -> Self {
        Self(std::sync::Mutex::new(instant.parse().unwrap()))
    }
    pub(super) fn set(&self, instant: &str) {
        *self.0.lock().unwrap() = instant.parse().unwrap();
    }
}

impl openhome_api::services::backlight::routine::Clock for ControlledClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        *self.0.lock().unwrap()
    }
}

pub(super) async fn wait_for_mode(app: Router, mode: &str) -> serde_json::Value {
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

pub(super) struct RecoveringDevice {
    pub(super) device: ManualDevice,
    pub(super) online: std::sync::atomic::AtomicBool,
    pub(super) readable: std::sync::atomic::AtomicBool,
    pub(super) reconnected: std::sync::atomic::AtomicBool,
    pub(super) active: std::sync::atomic::AtomicUsize,
    pub(super) block: std::sync::atomic::AtomicBool,
    pub(super) entered: tokio::sync::Notify,
    pub(super) release: tokio::sync::Semaphore,
}

impl RecoveringDevice {
    pub(super) fn new() -> Self {
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
