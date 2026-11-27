mod common;

use common::{StubServer, openhome, stderr, stdout, test_api_key};

fn run(server: &StubServer, args: &[&str]) -> std::process::Output {
    openhome()
        .args(["--url", &server.url, "backlight"])
        .args(args)
        .env("OPENHOME_API_KEY", test_api_key("backlight"))
        .output()
        .expect("run openhome")
}

#[test]
fn backlight_commands_map_to_the_backlight_api() {
    let color = |rgb: &str, brightness: u8| {
        format!(r#"{{"brightness":{brightness},"mode":"color","rgb":{rgb}}}"#)
    };
    let azure = r#"{"blue":255,"green":128,"red":10}"#;
    let red = r#"{"blue":0,"green":0,"red":255}"#;
    for (args, method, path, body) in [
        (vec!["status"], "GET", "/api/backlight", None),
        (
            vec!["off"],
            "PUT",
            "/api/backlight/selection",
            Some(r#"{"mode":"off"}"#.to_owned()),
        ),
        (
            vec!["automatic"],
            "POST",
            "/api/backlight/automatic",
            Some("{}".to_owned()),
        ),
        (
            vec!["tv"],
            "PUT",
            "/api/backlight/selection",
            Some(r#"{"mode":"tv-sync"}"#.to_owned()),
        ),
        (
            vec!["tv", "--brightness", "30"],
            "PUT",
            "/api/backlight/selection",
            Some(r#"{"brightness":30,"mode":"tv-sync"}"#.to_owned()),
        ),
        (
            vec!["red"],
            "PUT",
            "/api/backlight/selection",
            Some(color(red, 100)),
        ),
        (
            vec!["red", "--brightness", "20"],
            "PUT",
            "/api/backlight/selection",
            Some(color(red, 20)),
        ),
        (
            vec!["color", "0a80ff"],
            "PUT",
            "/api/backlight/selection",
            Some(color(azure, 100)),
        ),
        (
            vec!["color", "#0A80FF"],
            "PUT",
            "/api/backlight/selection",
            Some(color(azure, 100)),
        ),
        (
            vec!["color", "10, 128,255"],
            "PUT",
            "/api/backlight/selection",
            Some(color(azure, 100)),
        ),
        (
            vec!["color", "10,128,255", "--brightness", "0"],
            "PUT",
            "/api/backlight/selection",
            Some(color(azure, 0)),
        ),
    ] {
        let server = StubServer::start("200 OK", r#"{"intended":{"mode":"off"}}"#);
        let output = run(&server, &args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        assert_eq!(stdout(&output), "{\"intended\":{\"mode\":\"off\"}}\n");
        let request = server.request();
        assert!(
            request.starts_with(&format!("{method} {path} HTTP/1.1\r\n")),
            "{args:?}: {request}"
        );
        match body {
            Some(body) => assert!(request.ends_with(&body), "{args:?}: {request}"),
            None => assert!(request.ends_with("\r\n\r\n"), "{args:?}: {request}"),
        }
    }
}

#[test]
fn invalid_colors_and_brightness_fail_before_any_request() {
    for args in [
        vec!["color", "ff88"],
        vec!["color", "gg8800"],
        vec!["color", "#ff88001"],
        vec!["color", "256,0,0"],
        vec!["color", "255,0"],
        vec!["color", "255,0,0,0"],
        vec!["color", "-1,0,0"],
        vec!["color", "fé000"],
        vec!["color", ""],
        vec!["color", "ff8800", "--brightness", "101"],
        vec!["red", "--brightness", "101"],
        vec!["tv", "--brightness", "101"],
    ] {
        let server = StubServer::start("200 OK", "{}");
        let output = run(&server, &args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        server.assert_no_request();
    }
}
