use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde_json::Value;

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

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to create HTTP client")?;
    let path = match cli.command {
        Command::Health => "/api/health",
    };
    let response = client
        .get(format!("{}{path}", cli.url.trim_end_matches('/')))
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
