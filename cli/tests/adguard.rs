mod common;

use common::{StubServer, openhome, stderr, stdout, test_api_key};

#[test]
fn adguard_status_gets_the_status_endpoint() {
    let api_key = test_api_key("adguard-status");
    let server = StubServer::start("200 OK", r#"{"protection_enabled":true}"#);

    let output = openhome()
        .args(["--url", &server.url, "adguard", "status"])
        .env("OPENHOME_API_KEY", &api_key)
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"protection_enabled\":true}\n");
    assert!(request.starts_with("GET /api/adguard/status HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {api_key}").to_ascii_lowercase())
    );
}

#[test]
fn adguard_enable_posts_to_the_enable_endpoint() {
    let server = StubServer::start("200 OK", r#"{"protection_enabled":true}"#);

    let output = openhome()
        .args(["--url", &server.url, "adguard", "enable"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-enable"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"protection_enabled\":true}\n");
    assert!(request.starts_with("POST /api/adguard/enable HTTP/1.1\r\n"));
    assert!(request.ends_with("{}"));
}

#[test]
fn adguard_disable_posts_to_the_disable_endpoint() {
    let server = StubServer::start("200 OK", r#"{"protection_enabled":false}"#);

    let output = openhome()
        .args(["--url", &server.url, "adguard", "disable"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-disable"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"protection_enabled\":false}\n");
    assert!(request.starts_with("POST /api/adguard/disable HTTP/1.1\r\n"));
    assert!(request.ends_with("{}"));
}

#[test]
fn adguard_pause_sends_the_requested_minutes() {
    let server = StubServer::start("200 OK", r#"{"protection_enabled":false}"#);

    let output = openhome()
        .args(["--url", &server.url, "adguard", "pause", "30"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-pause"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"protection_enabled\":false}\n");
    assert!(request.starts_with("POST /api/adguard/pause HTTP/1.1\r\n"));
    assert!(request.ends_with("{\"minutes\":30}"));
}

#[test]
fn adguard_pause_zero_minutes_is_left_for_the_api_to_reject() {
    let server = StubServer::start(
        "400 Bad Request",
        r#"{"error":"minutes must be between 1 and 1440"}"#,
    );

    let output = openhome()
        .args(["--url", &server.url, "adguard", "pause", "0"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-pause-zero"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(request.ends_with("{\"minutes\":0}"));
    assert!(stderr(&output).contains("400 Bad Request"));
}

#[test]
fn missing_pause_minutes_is_rejected_before_any_request() {
    let server = StubServer::start("200 OK", r#"{"protection_enabled":true}"#);

    let output = openhome()
        .args(["--url", &server.url, "adguard", "pause"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-pause-missing"))
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn invalid_pause_minutes_is_rejected_before_any_request() {
    let server = StubServer::start("200 OK", r#"{"protection_enabled":true}"#);

    let output = openhome()
        .args(["--url", &server.url, "adguard", "pause", "abc"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-pause-invalid"))
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn adguard_api_error_writes_diagnostics_and_fails() {
    let server = StubServer::start(
        "503 Service Unavailable",
        r#"{"error":"AdGuard is not configured"}"#,
    );

    let output = openhome()
        .args(["--url", &server.url, "adguard", "status"])
        .env("OPENHOME_API_KEY", test_api_key("adguard-error"))
        .output()
        .expect("run openhome");

    server.request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("503 Service Unavailable"));
}
