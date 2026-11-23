use crate::common::send_request;
use crate::common::send_request_with_method;
use crate::support::{
    ControlledClock, ManualDevice, RecoveringDevice, authenticated, select, wait_for_mode,
};
use axum::http::Method;
use axum::http::StatusCode;
use openhome_api::services::backlight::BacklightService;

#[tokio::test]
async fn missing_or_invalid_profile_blocks_recovery_without_capturing_or_guessing_camera_settings()
{
    use std::sync::Arc;
    for invalid in [None, Some("{\"schema_version\":99}")] {
        let (_, state) = crate::common::test_app_with_db().await;
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
        let (_, state) = crate::common::test_app_with_db().await;
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
async fn manual_command_expiring_while_waiting_for_the_worker_never_reaches_hardware() {
    use std::sync::{Arc, atomic::Ordering::SeqCst};
    for waiting_for_worker in [true, false] {
        let (_, state) = crate::common::test_app_with_db().await;
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
async fn saving_a_valid_profile_resumes_profile_required_delivery() {
    use std::sync::Arc;
    let (_, state) = crate::common::test_app_with_db().await;
    sqlx::query(
        "UPDATE backlight SET tv_sync_profile = NULL, manual_override = NULL, \
         intended = '{\"mode\":\"tv-sync\"}' WHERE id = 1",
    )
    .execute(&state.db)
    .await
    .unwrap();
    let device = Arc::new(ManualDevice {
        packets: std::sync::Mutex::default(),
    });
    let service = BacklightService::new(state.db.clone(), Some(device.clone()), None)
        .await
        .unwrap();
    let app = authenticated(service.clone(), state);
    let runner = tokio::spawn(service.run_routine());
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let (_, blocked) = send_request(app.clone(), "/api/backlight", Some("test-api-key")).await;
    assert_eq!(blocked["recovery"], "profile-required");
    assert!(
        device.packets.lock().unwrap().is_empty(),
        "delivery must still wait for setup"
    );
    let (code, saved) = send_request_with_method(
        app.clone(),
        "/api/backlight/profile",
        Method::POST,
        Some(serde_json::json!({"source":"capture"})),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(saved["setup"], "ready");
    assert_eq!(
        saved["recovery"], "pending",
        "a stored profile releases recovery"
    );
    let resumed = wait_for_mode(app, "tv-sync").await;
    assert_eq!(resumed["recovery"], "recovered");
    runner.abort();
}
