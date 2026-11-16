//! Backlight native and uploaded effects and the inactivity hold.

mod backlight_support;
mod common;

use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use backlight_support::{ManualDevice, authenticated, select};
use common::send_request;
use common::send_request_with_method;
use openhome_api::services::backlight::BacklightService;

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
