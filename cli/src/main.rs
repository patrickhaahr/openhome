use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use reqwest::{Method, blocking::Client};
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
    /// Verify access to the `OpenHome` API.
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
    /// Inspect and control `AdGuard` Protection.
    Adguard {
        #[command(subcommand)]
        action: Adguard,
    },
    /// Inspect or configure the TV Backlight.
    Backlight {
        #[command(subcommand)]
        action: Backlight,
    },
}

#[derive(Debug, Subcommand)]
enum Backlight {
    #[command(
        about = "Show the intended selection, the device observation and the available effects"
    )]
    Status,
    #[command(about = "Switch the Backlight off")]
    Off,
    #[command(
        about = "Clear manual overrides and apply the current routine target (TV Sync outside Bedtime)"
    )]
    Automatic,
    #[command(about = "Follow what is on screen with TV Sync, the default mode")]
    Tv {
        #[arg(long, value_parser = brightness(), help = "Brightness from 0 to 100; omit to restore the saved profile brightness")]
        brightness: Option<u8>,
    },
    #[command(about = "Show solid red, at brightness 100 unless --brightness is given")]
    Red {
        #[arg(long, value_parser = brightness(), default_value_t = 100, help = "Brightness from 0 to 100")]
        brightness: u8,
    },
    #[command(about = "Show a solid color, at brightness 100 unless --brightness is given")]
    Color {
        #[arg(value_parser = parse_rgb, help = "Hex like 0080ff or '#0080ff' (quote the #), or red,green,blue like 0,128,255")]
        rgb: Rgb,
        #[arg(long, value_parser = brightness(), default_value_t = 100, help = "Brightness from 0 to 100")]
        brightness: u8,
    },
    #[command(
        about = "Read the native inactivity hold, or keep the last tone for this many seconds"
    )]
    Inactivity {
        #[arg(long, value_parser = clap::value_parser!(u16).range(120..=1800))]
        seconds: Option<u16>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

const RED: Rgb = Rgb {
    red: 255,
    green: 0,
    blue: 0,
};

fn brightness() -> clap::builder::RangedI64ValueParser<u8> {
    clap::value_parser!(u8).range(0..=100)
}

fn parse_rgb(input: &str) -> Result<Rgb, String> {
    let invalid = || {
        format!(
            "expected hex like 0080ff or #0080ff, or red,green,blue like 0,128,255; got {input:?}"
        )
    };
    let channels: Vec<u8> = if input.contains(',') {
        input
            .split(',')
            .map(|channel| channel.trim().parse::<u8>().map_err(|_| invalid()))
            .collect::<Result<_, _>>()?
    } else {
        let hex = input.strip_prefix('#').unwrap_or(input);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        (0..6)
            .step_by(2)
            .map(|start| u8::from_str_radix(&hex[start..start + 2], 16).map_err(|_| invalid()))
            .collect::<Result<_, _>>()?
    };
    match channels[..] {
        [red, green, blue] => Ok(Rgb { red, green, blue }),
        _ => Err(invalid()),
    }
}

fn color_selection(rgb: Rgb, brightness: u8) -> Value {
    json!({
        "mode": "color",
        "rgb": {"red": rgb.red, "green": rgb.green, "blue": rgb.blue},
        "brightness": brightness,
    })
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
    /// Show whether `AdGuard` Protection is filtering.
    Status,
    /// Enable `AdGuard` Protection.
    Enable,
    /// Disable `AdGuard` Protection.
    Disable,
    /// Pause `AdGuard` Protection for a number of minutes.
    Pause {
        /// Minutes before protection resumes automatically.
        minutes: u64,
    },
}

struct Request {
    method: Method,
    path: &'static str,
    body: Option<Value>,
}

fn selection(body: Value) -> Request {
    Request {
        method: Method::PUT,
        path: "/api/backlight/selection",
        body: Some(body),
    }
}

fn request(command: &Command) -> Request {
    match command {
        Command::Backlight { action } => match action {
            Backlight::Status => Request {
                method: Method::GET,
                path: "/api/backlight",
                body: None,
            },
            Backlight::Off => selection(json!({"mode": "off"})),
            Backlight::Automatic => Request {
                method: Method::POST,
                path: "/api/backlight/automatic",
                body: Some(json!({})),
            },
            Backlight::Tv { brightness } => selection(match brightness {
                Some(brightness) => json!({"mode": "tv-sync", "brightness": brightness}),
                None => json!({"mode": "tv-sync"}),
            }),
            Backlight::Red { brightness } => selection(color_selection(RED, *brightness)),
            Backlight::Color { rgb, brightness } => selection(color_selection(*rgb, *brightness)),
            Backlight::Inactivity { seconds } => Request {
                method: if seconds.is_some() {
                    Method::POST
                } else {
                    Method::GET
                },
                path: "/api/backlight/inactivity",
                body: seconds.map(|seconds| json!({"seconds":seconds})),
            },
        },
        Command::Health => Request {
            method: Method::GET,
            path: "/api/health",
            body: None,
        },
        Command::Lights { state } => Request {
            method: Method::POST,
            path: match state {
                LightState::On => "/api/lights/on",
                LightState::Off => "/api/lights/off",
            },
            body: Some(json!({})),
        },
        Command::Ir { action } => match action {
            Ir::Status => Request {
                method: Method::GET,
                path: "/api/ir",
                body: None,
            },
            Ir::Edifier { command } => Request {
                method: Method::POST,
                path: "/api/ir/edifier",
                body: Some(json!({ "command": command })),
            },
            Ir::LgTv { command } => Request {
                method: Method::POST,
                path: "/api/ir/lgtv",
                body: Some(json!({ "command": command })),
            },
        },
        Command::Adguard { action } => match action {
            Adguard::Status => Request {
                method: Method::GET,
                path: "/api/adguard/status",
                body: None,
            },
            Adguard::Enable => Request {
                method: Method::POST,
                path: "/api/adguard/enable",
                body: Some(json!({})),
            },
            Adguard::Disable => Request {
                method: Method::POST,
                path: "/api/adguard/disable",
                body: Some(json!({})),
            },
            Adguard::Pause { minutes } => Request {
                method: Method::POST,
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
        .timeout(Duration::from_secs(
            if matches!(cli.command, Command::Backlight { .. }) {
                90
            } else {
                10
            },
        ))
        .build()
        .context("failed to create HTTP client")?;
    let url = format!("{}{}", cli.url.trim_end_matches('/'), request.path);
    let mut builder = client.request(request.method, url);
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
            Ok(key) => return validate_key(&key),
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

    validate_key(&key)
}

fn validate_key(key: &str) -> Result<String> {
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
