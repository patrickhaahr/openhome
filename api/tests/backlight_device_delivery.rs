//! Backlight device delivery: accepted, failed and superseded exchanges.

mod backlight_support;
mod common;

use axum::http::StatusCode;
use backlight_support::{RecoveringDevice, authenticated, select, wait_for_mode};
use common::send_request;
use openhome_api::services::backlight::BacklightService;

#[tokio::test]
async fn disconnecting_after_durable_acceptance_completes_the_accepted_device_work() {
    use std::sync::{Arc, atomic::Ordering::SeqCst};
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(RecoveringDevice::new());
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state);
    select(app.clone(), serde_json::json!({"mode":"tv-sync"})).await;
    device.device.packets.lock().unwrap().clear();
    let latest =
        serde_json::json!({"mode":"color","rgb":{"red":0,"green":0,"blue":255},"brightness":10});
    // Hold the accepted whole operation so its request cannot finish while it is abandoned.
    device.block.store(true, SeqCst);
    let request = tokio::spawn(select(app.clone(), latest.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(2), device.entered.notified())
        .await
        .unwrap();
    // The target persisted before the exchange started: the requesting client no longer
    // matters, and dropping it must never cancel the detached device work.
    let (_, durable) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(durable["intended"], latest);
    assert_eq!(durable["command"], "pending");
    request.abort();
    device.release.add_permits(1);
    let finished = wait_for_mode(app, "color").await;
    assert_eq!(finished["acknowledged"], latest);
    assert_eq!(finished["recovery"], "recovered");
    let packets = device.device.packets.lock().unwrap();
    let writes = packets.iter().filter(|p| !p.is_empty()).count();
    assert_eq!(
        writes, 1,
        "exactly the accepted exchange must reach the device"
    );
    assert_eq!(&packets[0][1][4..7], [0, 0, 255].as_slice());
}

#[tokio::test]
async fn off_write_not_applied_reports_failure_while_readback_stays_honest() {
    use openhome_api::services::backlight::{
        CurrentOperation, Exchange, GattTransport, Packet, TransportFuture,
    };
    use std::sync::Arc;
    // The controller answers every query but applies no write; a receipt or readback
    // alone never becomes acceptance.
    struct UnappliedDevice;
    impl GattTransport for UnappliedDevice {
        fn matches_device(&self, _: &str) -> bool {
            false
        }
        fn exchange<'a>(
            &'a self,
            _writes: &'a [Packet],
            _queries: &'a [Vec<u8>],
            _: &'a CurrentOperation,
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                Ok(Exchange {
                    reconnected: false,
                    writes_accepted: false,
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
    let (_, state) = common::test_app_with_db().await;
    let service = BacklightService::new(state.db.clone(), Some(Arc::new(UnappliedDevice)), None)
        .await
        .unwrap();
    let app = authenticated(service, state);
    let (code, body) = select(app, serde_json::json!({"mode":"off"})).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["command"], "failed");
    assert_eq!(body["recovery"], "retrying");
    assert_eq!(body["reachability"], "reachable");
    assert!(
        body["acknowledged"].is_null(),
        "an unapplied write is never acknowledged"
    );
    assert_eq!(body["observed"]["brightness"], 45);
    assert_eq!(body["stale"], false);
}

#[tokio::test]
async fn a_superseded_exchange_that_finished_writing_stays_acknowledged() {
    use openhome_api::services::backlight::{
        CurrentOperation, Exchange, GattTransport, Packet, TransportError, TransportFuture,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering::SeqCst},
    };
    // Characteristic writes happen the moment they are sent: after an exchange's last
    // frame nothing undoes it, so bookkeeping records it even after supersession.
    struct SentDevice {
        hold: AtomicBool,
        entered: tokio::sync::Notify,
        release: tokio::sync::Semaphore,
    }
    impl SentDevice {
        fn new() -> Self {
            Self {
                hold: AtomicBool::new(false),
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Semaphore::new(0),
            }
        }
        fn replies() -> Exchange {
            Exchange {
                reconnected: false,
                writes_accepted: true,
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
            }
        }
    }
    impl GattTransport for SentDevice {
        fn matches_device(&self, _: &str) -> bool {
            false
        }
        fn exchange<'a>(
            &'a self,
            _writes: &'a [Packet],
            _queries: &'a [Vec<u8>],
            current: &'a CurrentOperation,
        ) -> TransportFuture<'a> {
            Box::pin(async move {
                if !current() {
                    return Err(TransportError::Superseded);
                }
                if self.hold.load(SeqCst) {
                    self.entered.notify_one();
                    self.release.acquire().await.unwrap().forget();
                }
                // No check after the hold sent its frames: supersession never rewinds
                // an exchange that already wrote.
                Ok(Self::replies())
            })
        }
    }
    let (_, state) = common::test_app_with_db().await;
    let device = Arc::new(SentDevice::new());
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service, state);
    select(app.clone(), serde_json::json!({"mode":"tv-sync"})).await;
    let first = serde_json::json!(
        {"mode":"color","rgb":{"red":0,"green":0,"blue":255},"brightness":10}
    );
    device.hold.store(true, SeqCst);
    let earlier = tokio::spawn(select(app.clone(), first.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(2), device.entered.notified())
        .await
        .unwrap();
    let second = serde_json::json!({"mode":"tv-sync","brightness":10});
    let newer = tokio::spawn(select(app.clone(), second.clone()));
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let (_, current) =
                send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
            if current["intended"] == second {
                assert_eq!(current["command"], "pending");
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    device.release.add_permits(1);
    let (_, superseded_status) = earlier.await.unwrap();
    // The finished write is a physical fact independent of the superseding intent, but
    // only the still-current revision may claim acknowledgment.
    assert_eq!(superseded_status["acknowledged"], first);
    assert_eq!(superseded_status["command"], "pending");
    assert_eq!(superseded_status["recovery"], "pending");
    device.release.add_permits(1);
    let (code, current) = newer.await.unwrap();
    assert_eq!(code, StatusCode::OK);
    assert_eq!(current["command"], "acknowledged");
    assert_eq!(current["acknowledged"], second);
    assert_eq!(current["recovery"], "recovered");
}
