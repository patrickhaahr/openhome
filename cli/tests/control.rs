mod common;

use common::{StubServer, openhome, stderr, stdout, test_api_key};

#[test]
fn lights_on_posts_an_empty_json_object_to_the_on_endpoint() {
    let api_key = test_api_key("lights-on");
    let server = StubServer::start("200 OK", r#"{"message":"on"}"#);

    let output = openhome()
        .args(["--url", &server.url, "lights", "on"])
        .env("OPENHOME_API_KEY", &api_key)
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"message\":\"on\"}\n");
    assert!(request.starts_with("POST /api/lights/on HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {api_key}").to_ascii_lowercase())
    );
    assert!(
        request
            .to_ascii_lowercase()
            .contains("content-type: application/json")
    );
    assert!(request.ends_with("{}"));
}

#[test]
fn lights_off_posts_an_empty_json_object_to_the_off_endpoint() {
    let server = StubServer::start("200 OK", r#"{"message":"off"}"#);

    let output = openhome()
        .args(["--url", &server.url, "lights", "off"])
        .env("OPENHOME_API_KEY", test_api_key("lights-off"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"message\":\"off\"}\n");
    assert!(request.starts_with("POST /api/lights/off HTTP/1.1\r\n"));
    assert!(request.ends_with("{}"));
}

#[test]
fn ir_status_gets_the_ir_status_endpoint() {
    let api_key = test_api_key("ir-status");
    let server = StubServer::start("200 OK", r#"{"remotes":[]}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "status"])
        .env("OPENHOME_API_KEY", &api_key)
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"remotes\":[]}\n");
    assert!(request.starts_with("GET /api/ir HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {api_key}").to_ascii_lowercase())
    );
}

#[test]
fn ir_edifier_sends_the_named_command() {
    let server = StubServer::start("200 OK", r#"{"message":"sent"}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "edifier", "volume-up"])
        .env("OPENHOME_API_KEY", test_api_key("ir-edifier"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"message\":\"sent\"}\n");
    assert!(request.starts_with("POST /api/ir/edifier HTTP/1.1\r\n"));
    assert!(request.ends_with("{\"command\":\"volume-up\"}"));
}

#[test]
fn ir_lgtv_sends_the_named_command() {
    let server = StubServer::start("200 OK", r#"{"message":"sent"}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "lgtv", "power"])
        .env("OPENHOME_API_KEY", test_api_key("ir-lgtv"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"message\":\"sent\"}\n");
    assert!(request.starts_with("POST /api/ir/lgtv HTTP/1.1\r\n"));
    assert!(request.ends_with("{\"command\":\"power\"}"));
}

#[test]
fn ir_command_names_are_passed_through_to_the_api() {
    let server = StubServer::start("200 OK", r#"{"message":"sent"}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "edifier", "Some-New-Command"])
        .env("OPENHOME_API_KEY", test_api_key("ir-passthrough"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(request.ends_with("{\"command\":\"Some-New-Command\"}"));
}

#[test]
fn empty_ir_command_is_left_for_the_api_to_reject() {
    let server = StubServer::start("400 Bad Request", r#"{"error":"command is required"}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "edifier", ""])
        .env("OPENHOME_API_KEY", test_api_key("ir-empty"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(request.ends_with("{\"command\":\"\"}"));
    assert!(stderr(&output).contains("400 Bad Request"));
}

#[test]
fn missing_ir_command_is_rejected_before_any_request() {
    let server = StubServer::start("200 OK", r#"{"message":"sent"}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "edifier"])
        .env("OPENHOME_API_KEY", test_api_key("ir-missing"))
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn missing_lights_state_is_rejected_before_any_request() {
    let server = StubServer::start("200 OK", r#"{"message":"on"}"#);

    let output = openhome()
        .args(["--url", &server.url, "lights"])
        .env("OPENHOME_API_KEY", test_api_key("lights-missing"))
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn ir_api_error_writes_diagnostics_and_fails() {
    let server = StubServer::start("502 Bad Gateway", r#"{"error":"IR device unreachable"}"#);

    let output = openhome()
        .args(["--url", &server.url, "ir", "lgtv", "power"])
        .env("OPENHOME_API_KEY", test_api_key("ir-error"))
        .output()
        .expect("run openhome");

    server.request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("502 Bad Gateway"));
}
