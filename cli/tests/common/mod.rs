#![allow(dead_code)]

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

pub fn openhome() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openhome"));
    command.env_clear();
    command
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn test_api_key(label: &str) -> String {
    format!(
        "test-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    )
}

pub fn write_conventional_key(config_home: &Path, key: &str) {
    let directory = config_home.join("openhome");
    fs::create_dir(&directory).expect("create OpenHome config directory");
    fs::write(directory.join("api-key"), format!("{key}\n")).expect("write conventional key");
}

pub struct TempDirectory(PathBuf);

impl TempDirectory {
    pub fn new() -> Self {
        let id = TEMP_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("openhome-cli-test-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create temporary directory");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

pub struct StubServer {
    pub url: String,
    request_rx: mpsc::Receiver<String>,
    thread: thread::JoinHandle<()>,
}

impl StubServer {
    pub fn start(status: impl Into<String>, body: impl Into<String>) -> Self {
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
            let header_end = loop {
                if let Some(index) = find(&request, b"\r\n\r\n") {
                    break index + 4;
                }
                let count = stream.read(&mut buffer).expect("read request");
                if count == 0 {
                    break request.len();
                }
                request.extend_from_slice(&buffer[..count]);
            };
            let content_length = String::from_utf8_lossy(&request[..header_end])
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while request.len() < header_end + content_length {
                let count = stream.read(&mut buffer).expect("read request body");
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

    pub fn request(self) -> String {
        let request = self
            .request_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("receive request");
        self.thread.join().expect("join stub server");
        request
    }

    pub fn assert_no_request(&self) {
        assert!(
            self.request_rx
                .recv_timeout(Duration::from_millis(200))
                .is_err(),
            "stub server received an unexpected request"
        );
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
