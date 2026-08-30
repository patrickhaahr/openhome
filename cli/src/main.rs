use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::blocking::Client;
use serde_json::{Value, json};

const DEFAULT_API_URL: &str = "https://openhome.haahr.me";

#[derive(Debug, Parser)]
#[command(name = "openhome", about = "Control OpenHome through its API")]
struct Cli {
    #[arg(long, env = "OPENHOME_API_URL", default_value = DEFAULT_API_URL)]
    url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Verify access to the OpenHome API.
    Health,
    /// Switch the lights on or off.
    Lights {
        /// The light state to switch to.
        state: LightState,
    },
    /// Inspect and control IR remotes.
    Ir {
        #[command(subcommand)]
        action: Ir,
    },
    /// Inspect and control AdGuard Protection.
    Adguard {
        #[command(subcommand)]
        action: Adguard,
    },
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum LightState {
    On,
    Off,
}

#[derive(Debug, Subcommand)]
enum Ir {
    /// Show the IR remotes and their available commands.
    Status,
    /// Send a command to the Edifier speaker.
    Edifier {
        /// Command name, passed through to the API.
        command: String,
    },
    /// Send a command to the LG TV.
    #[command(name = "lgtv")]
    LgTv {
        /// Command name, passed through to the API.
        command: String,
    },
}

#[derive(Debug, Subcommand)]
enum Adguard {
    /// Show whether AdGuard Protection is filtering.
    Status,
    /// Enable AdGuard Protection.
    Enable,
    /// Disable AdGuard Protection.
    Disable,
    /// Pause AdGuard Protection for a number of minutes.
    Pause {
        /// Minutes before protection resumes automatically.
        minutes: u64,
    },
}

/// The API call selected by the parsed command.
struct Request {
    post: bool,
    path: &'static str,
    body: Option<Value>,
}

fn request(command: &Command) -> Request {
    match command {
        Command::Health => Request {
            post: false,
            path: "/api/health",
            body: None,
        },
        Command::Lights { state } => Request {
            post: true,
            path: match state {
                LightState::On => "/api/lights/on",
                LightState::Off => "/api/lights/off",
            },
            body: Some(json!({})),
        },
        Command::Ir { action } => match action {
            Ir::Status => Request {
                post: false,
                path: "/api/ir",
                body: None,
            },
            Ir::Edifier { command } => Request {
                post: true,
                path: "/api/ir/edifier",
                body: Some(json!({ "command": command })),
            },
            Ir::LgTv { command } => Request {
                post: true,
                path: "/api/ir/lgtv",
                body: Some(json!({ "command": command })),
            },
        },
        Command::Adguard { action } => match action {
            Adguard::Status => Request {
                post: false,
                path: "/api/adguard/status",
                body: None,
            },
            Adguard::Enable => Request {
                post: true,
                path: "/api/adguard/enable",
                body: Some(json!({})),
            },
            Adguard::Disable => Request {
                post: true,
                path: "/api/adguard/disable",
                body: Some(json!({})),
            },
            Adguard::Pause { minutes } => Request {
                post: true,
                path: "/api/adguard/pause",
                body: Some(json!({ "minutes": minutes })),
            },
        },
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let api_key = api_key()?;
    let request = request(&cli.command);

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to create HTTP client")?;
    let url = format!("{}{}", cli.url.trim_end_matches('/'), request.path);
    let mut builder = if request.post {
        client.post(url)
    } else {
        client.get(url)
    };
    if let Some(body) = request.body {
        builder = builder
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string());
    }
    let response = builder
        .bearer_auth(&api_key)
        .send()
        .context("request failed")?;
    let status = response.status();
    let body = response.text().context("failed to read API response")?;
    if !status.is_success() {
        bail!("API returned {status}");
    }

    let json: Value = serde_json::from_str(&body).context("API returned malformed JSON")?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &json).context("failed to write response")?;
    writeln!(stdout).context("failed to write response")?;
    Ok(())
}

fn api_key() -> Result<String> {
    let key = if let Some(path) = std::env::var_os("OPENHOME_API_KEY_FILE") {
        read_key(Path::new(&path))?
    } else {
        match std::env::var("OPENHOME_API_KEY") {
            Ok(key) => return validate_key(key),
            Err(std::env::VarError::NotUnicode(_)) => {
                bail!("OPENHOME_API_KEY is not valid UTF-8")
            }
            Err(std::env::VarError::NotPresent) => {}
        }

        let path = conventional_key_path().context("API Key is not configured")?;
        match std::fs::read_to_string(&path) {
            Ok(key) => key,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                bail!("API Key is not configured")
            }
            Err(error) => {
                return Err(error)
                    .context(format!("failed to read API Key file {}", path.display()));
            }
        }
    };

    validate_key(key)
}

fn validate_key(key: String) -> Result<String> {
    let key = key.trim();
    if key.is_empty() {
        bail!("API Key is empty");
    }
    Ok(key.to_owned())
}

fn read_key(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("failed to read API Key file {}", path.display()))
}

fn conventional_key_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|path| !path.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .map(|config_home| config_home.join("openhome/api-key"))
}
