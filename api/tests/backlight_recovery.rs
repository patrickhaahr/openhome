mod backlight_support;
mod common;

use axum::http::Method;
use backlight_support::{
    ControlledClock, ManualDevice, RecoveringDevice, authenticated, select, wait_for_mode,
};
use common::send_request;
use common::send_request_with_method;
use openhome_api::services::backlight::BacklightService;

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
        ("2026-10-24T10:00:00Z", "tv-sync", "automatic"),
    ] {
        clock.set(now);
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
