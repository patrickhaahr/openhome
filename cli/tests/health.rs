use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static TEMP_DIRECTORY_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn health_sends_authenticated_request_and_prints_json() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);
    let api_key = test_api_key("health");

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY", &api_key)
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"status\":\"ok\"}\n");
    assert!(request.starts_with("GET /api/health HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {api_key}").to_ascii_lowercase())
    );
}

#[test]
fn explicit_url_overrides_environment_url() {
    let explicit_server = StubServer::start("200 OK", r#"{"source":"flag"}"#);
    let environment_server = StubServer::start("200 OK", r#"{"source":"environment"}"#);

    let output = openhome()
        .args(["--url", &explicit_server.url, "health"])
        .env("OPENHOME_API_URL", &environment_server.url)
        .env("OPENHOME_API_KEY", test_api_key("url-precedence"))
        .output()
        .expect("run openhome");

    explicit_server.request();
    environment_server.assert_no_request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"source\":\"flag\"}\n");
}

#[test]
fn environment_url_overrides_production_default() {
    let server = StubServer::start("200 OK", r#"{"source":"environment"}"#);

    let output = openhome()
        .arg("health")
        .env("OPENHOME_API_URL", &server.url)
        .env("OPENHOME_API_KEY", test_api_key("environment-url"))
        .output()
        .expect("run openhome");

    server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "{\"source\":\"environment\"}\n");
}

#[test]
fn help_reports_the_production_default_url() {
    let output = openhome().arg("--help").output().expect("run openhome");

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("https://openhome.haahr.me"));
}

#[test]
fn production_default_targets_the_public_host() {
    let proxy = StubServer::start("502 Bad Gateway", "");

    let output = openhome()
        .arg("health")
        .env("HTTPS_PROXY", &proxy.url)
        .env("OPENHOME_API_KEY", test_api_key("default-url"))
        .output()
        .expect("run openhome");

    let request = proxy.request();
    assert!(!output.status.success());
    assert!(request.starts_with("CONNECT openhome.haahr.me:443 HTTP/1.1\r\n"));
}

#[test]
fn api_key_file_overrides_environment_key() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);
    let directory = TempDirectory::new();
    let key_file = directory.path().join("api-key");
    let file_key = test_api_key("file");
    fs::write(&key_file, format!("{file_key}\n")).expect("write API Key file");

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY_FILE", &key_file)
        .env("OPENHOME_API_KEY", test_api_key("environment"))
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {file_key}").to_ascii_lowercase())
    );
}

#[test]
fn environment_key_overrides_conventional_key_file() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);
    let config_home = TempDirectory::new();
    write_conventional_key(config_home.path(), &test_api_key("conventional"));
    let environment_key = test_api_key("environment");

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("OPENHOME_API_KEY", &environment_key)
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {environment_key}").to_ascii_lowercase())
    );
}

#[test]
fn conventional_key_file_is_used_as_fallback() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);
    let config_home = TempDirectory::new();
    let conventional_key = test_api_key("conventional");
    write_conventional_key(config_home.path(), &conventional_key);

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("XDG_CONFIG_HOME", config_home.path())
        .output()
        .expect("run openhome");

    let request = server.request();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {conventional_key}").to_ascii_lowercase())
    );
}

#[test]
fn missing_key_fails_before_request() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);
    let config_home = TempDirectory::new();

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("XDG_CONFIG_HOME", config_home.path())
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("API Key"));
}

#[test]
fn unreadable_key_file_fails_before_request_without_exposing_other_keys() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);
    let directory = TempDirectory::new();
    let missing_file = directory.path().join("missing-key");
    let unused_key = test_api_key("must-not-appear");

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY_FILE", missing_file)
        .env("OPENHOME_API_KEY", &unused_key)
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("read API Key file"));
    assert!(!stderr(&output).contains(&unused_key));
}

#[test]
fn empty_key_fails_before_request() {
    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY", "  \n")
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("API Key is empty"));
}

#[test]
fn api_failure_writes_diagnostics_and_fails() {
    let api_key = test_api_key("rejected");
    let server = StubServer::start("401 Unauthorized", format!(r#"{{"error":"{api_key}"}}"#));

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY", &api_key)
        .output()
        .expect("run openhome");

    server.request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("401 Unauthorized"));
    assert!(!stderr(&output).contains(&api_key));
}

#[cfg(unix)]
#[test]
fn non_utf8_environment_key_fails_before_request() {
    use std::os::unix::ffi::OsStringExt;

    let server = StubServer::start("200 OK", r#"{"status":"ok"}"#);

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY", std::ffi::OsString::from_vec(vec![0xff]))
        .output()
        .expect("run openhome");

    server.assert_no_request();
    assert!(!output.status.success());
    assert!(stderr(&output).contains("not valid UTF-8"));
}

#[test]
fn malformed_success_response_writes_diagnostics_and_fails() {
    let server = StubServer::start("200 OK", "not-json");

    let output = openhome()
        .args(["--url", &server.url, "health"])
        .env("OPENHOME_API_KEY", test_api_key("malformed"))
        .output()
        .expect("run openhome");

    server.request();
    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("malformed JSON"));
}

#[test]
fn transport_failure_writes_diagnostics_and_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve unused port");
    let url = format!("http://{}", listener.local_addr().expect("read address"));
    drop(listener);

    let output = openhome()
        .args(["--url", &url, "health"])
        .env("OPENHOME_API_KEY", test_api_key("transport"))
        .output()
        .expect("run openhome");

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("request failed"));
}

fn openhome() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openhome"));
    command.env_clear();
    command
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn test_api_key(label: &str) -> String {
    format!(
        "test-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    )
}

fn write_conventional_key(config_home: &Path, key: &str) {
    let directory = config_home.join("openhome");
    fs::create_dir(&directory).expect("create OpenHome config directory");
    fs::write(directory.join("api-key"), format!("{key}\n")).expect("write conventional key");
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let id = TEMP_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("openhome-cli-test-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create temporary directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

struct StubServer {
    url: String,
    request_rx: mpsc::Receiver<String>,
    thread: thread::JoinHandle<()>,
}

impl StubServer {
    fn start(status: impl Into<String>, body: impl Into<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub server");
        let address = listener.local_addr().expect("read stub server address");
        let (request_tx, request_rx) = mpsc::channel();
        let status = status.into();
        let body = body.into();
        let thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set read timeout");

            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).expect("read request");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
            }
            request_tx
                .send(String::from_utf8_lossy(&request).into_owned())
                .expect("record request");

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("send response");
        });

        Self {
            url: format!("http://{address}"),
            request_rx,
            thread,
        }
    }

    fn request(self) -> String {
        let request = self
            .request_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("receive request");
        self.thread.join().expect("join stub server");
        request
    }

    fn assert_no_request(&self) {
        assert!(
            self.request_rx
                .recv_timeout(Duration::from_millis(200))
                .is_err(),
            "stub server received an unexpected request"
        );
    }
}
