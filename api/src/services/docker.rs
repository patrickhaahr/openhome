use crate::models::docker::{ContainerDetailResponse, ContainerStatus};
use bollard::container::LogOutput;
use bollard::errors::Error;
use bollard::models::{HostConfig, PortSummary};
use bollard::query_parameters::{
    InspectContainerOptions, ListContainersOptionsBuilder, LogsOptionsBuilder,
    RestartContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use chrono::{DateTime, TimeZone, Utc};
use futures_util::stream::TryStreamExt;

#[derive(Clone)]
pub struct DockerService {
    client: bollard::Docker,
}

impl DockerService {
    pub fn new() -> Result<Self, Error> {
        let client = bollard::Docker::connect_with_local_defaults()?;
        Ok(Self { client })
    }

    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerStatus>, Error> {
        let options = Some(ListContainersOptionsBuilder::new().all(all).build());
        let containers = self.client.list_containers(options).await?;
        let mut statuses = Vec::with_capacity(containers.len());
        for container in containers {
            let display_status = map_display_status(container.state.as_ref().map(|s| s.as_ref()));
            let state = container
                .state
                .as_ref()
                .map(|s| s.as_ref().to_string())
                .unwrap_or_default();
            let health_status = container
                .health
                .as_ref()
                .and_then(|h| h.status.as_ref())
                .map(|s| s.as_ref().to_string());
            let uptime_seconds = parse_uptime_seconds(container.status.as_deref());
            statuses.push(ContainerStatus {
                name: container
                    .names
                    .as_ref()
                    .and_then(|n| n.first())
                    .map(|n| n.trim_start_matches('/').to_string())
                    .unwrap_or_default(),
                display_status,
                state,
                health_status,
                uptime_seconds,
                image: container.image.as_ref().cloned().unwrap_or_default(),
                ports: parse_ports(container.ports.as_ref()),
                labels: container.labels.as_ref().cloned().unwrap_or_default(),
                created_at: container
                    .created
                    .and_then(|c| Utc.timestamp_opt(c, 0).single())
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                restart_count: 0,
            });
        }
        Ok(statuses)
    }

    pub async fn inspect_container(&self, name: &str) -> Result<ContainerDetailResponse, Error> {
        let options = Some(InspectContainerOptions { size: false });
        let container = self.client.inspect_container(name, options).await?;
        let state = container.state.as_ref();
        let status = state
            .and_then(|s| s.status.as_ref())
            .map(|s| s.as_ref().to_string());
        let display_status =
            map_display_status(state.and_then(|s| s.status.as_ref()).map(|s| s.as_ref()));
        let health_status = state
            .and_then(|s| s.health.as_ref())
            .and_then(|h| h.status.as_ref())
            .map(|s| s.as_ref().to_string());
        let uptime = state
            .and_then(|s| s.started_at.as_ref())
            .and_then(|started| DateTime::parse_from_rfc3339(started).ok())
            .map(|parsed| (Utc::now() - parsed.with_timezone(&Utc)).num_seconds());
        Ok(ContainerDetailResponse {
            name: name.to_string(),
            display_status,
            state: status.unwrap_or_default(),
            health_status,
            uptime_seconds: uptime,
            image: container
                .config
                .as_ref()
                .and_then(|c| c.image.as_ref())
                .cloned()
                .unwrap_or_default(),
            image_id: container.image.clone().unwrap_or_default(),
            ports: parse_container_ports(container.host_config.as_ref()),
            volumes: parse_binds(container.host_config.as_ref()),
            networks: container
                .network_settings
                .as_ref()
                .and_then(|n| n.networks.as_ref())
                .map(|networks| networks.keys().cloned().collect())
                .unwrap_or_default(),
            labels: container
                .config
                .as_ref()
                .and_then(|c| c.labels.as_ref())
                .cloned()
                .unwrap_or_default(),
            created_at: container
                .created
                .map(|c| c.to_rfc3339())
                .unwrap_or_default(),
            started_at: state
                .and_then(|s| s.started_at.as_ref())
                .cloned()
                .unwrap_or_default(),
            restart_count: container
                .restart_count
                .map(|n| n as i32)
                .unwrap_or_default(),
            memory_usage_mb: None,
            cpu_percent: None,
        })
    }

    pub async fn restart_container(&self, name: &str, timeout: u64) -> Result<(), Error> {
        let timeout_i32 = timeout
            .try_into()
            .map_err(|_| Error::DockerResponseServerError {
                status_code: 400,
                message: format!("Timeout exceeds maximum allowed value: {}", timeout),
            })?;
        let options = Some(
            RestartContainerOptionsBuilder::new()
                .t(timeout_i32)
                .build(),
        );
        self.client.restart_container(name, options).await?;
        Ok(())
    }

    pub async fn start_container(&self, name: &str) -> Result<(), Error> {
        self.client.start_container(name, None).await?;
        Ok(())
    }

    pub async fn stop_container(&self, name: &str, timeout: u64) -> Result<bool, Error> {
        let inspect = self.inspect_container(name).await?;
        if inspect.state != "running" {
            return Ok(false);
        }

        let timeout_i32 = timeout
            .try_into()
            .map_err(|_| Error::DockerResponseServerError {
                status_code: 400,
                message: format!("Timeout exceeds maximum allowed value: {}", timeout),
            })?;
        let options = Some(
            StopContainerOptionsBuilder::new()
                .t(timeout_i32)
                .build(),
        );
        self.client.stop_container(name, options).await?;
        Ok(true)
    }

    pub async fn get_container_logs(
        &self,
        name: &str,
        tail: Option<usize>,
        since: Option<DateTime<Utc>>,
        timestamps: bool,
    ) -> Result<String, Error> {
        let tail_str = tail
            .map(|t| t.to_string())
            .unwrap_or_else(|| "100".to_string());
        let since_timestamp = since.map(|s| s.timestamp() as i32).unwrap_or(0);
        let options = Some(
            LogsOptionsBuilder::new()
                .follow(false)
                .stdout(true)
                .stderr(true)
                .since(since_timestamp)
                .tail(&tail_str)
                .timestamps(timestamps)
                .build(),
        );
        let log_outputs: Vec<LogOutput> = self.client.logs(name, options).try_collect().await?;
        let logs = log_outputs
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("");
        Ok(logs)
    }
}

fn parse_uptime_seconds(status: Option<&str>) -> Option<i64> {
    let status = status?;
    if !status.starts_with("Up ") {
        return None;
    }

    let rest = &status[3..];

    let (value, unit) = if let Some(space_idx) = rest.find(' ') {
        let value: i64 = rest[..space_idx].parse().ok()?;
        let unit = &rest[space_idx + 1..];
        (value, unit)
    } else {
        return None;
    };

    let seconds = match unit {
        s if s.starts_with("second") => value,
        s if s.starts_with("minute") => value * 60,
        s if s.starts_with("hour") => value * 3600,
        s if s.starts_with("day") => value * 86400,
        _ => return None,
    };

    Some(seconds)
}

fn map_display_status(state: Option<&str>) -> String {
    match state {
        Some("running") => "running".to_string(),
        Some("exited") | Some("dead") => "stopped".to_string(),
        Some("restarting") => "restarting".to_string(),
        Some(s) => s.to_string(),
        None => "unknown".to_string(),
    }
}

fn parse_ports(ports: Option<&Vec<PortSummary>>) -> Vec<String> {
    ports
        .map(|p| {
            p.iter()
                .map(|port| {
                    let ip = port.ip.as_deref().unwrap_or("");
                    let public_port = port.public_port.map(|p| p.to_string()).unwrap_or_default();
                    let private_port = port.private_port.to_string();
                    let type_ = port
                        .typ
                        .as_ref()
                        .map(|t| t.as_ref().to_string())
                        .unwrap_or_default();
                    format!("{}:{}->{}/{}", ip, public_port, private_port, type_)
                })
                .filter(|s| !s.starts_with(":0->"))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_container_ports(host_config: Option<&HostConfig>) -> Vec<String> {
    if let Some(hc) = host_config {
        let mut ports = Vec::new();
        if let Some(port_bindings) = &hc.port_bindings {
            for (container_port, bindings) in port_bindings {
                if let Some(binding_vec) = bindings {
                    for binding in binding_vec {
                        let host_ip = binding.host_ip.as_deref().unwrap_or("0.0.0.0");
                        let host_port = binding.host_port.as_deref().unwrap_or("");
                        ports.push(format!("{}:{}->{}/tcp", host_ip, host_port, container_port));
                    }
                }
            }
        }
        ports
    } else {
        Vec::new()
    }
}

fn parse_binds(host_config: Option<&HostConfig>) -> Vec<String> {
    if let Some(hc) = host_config {
        hc.binds.as_ref().map(|b| b.to_vec()).unwrap_or_default()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uptime_seconds_with_seconds() {
        let result = parse_uptime_seconds(Some("Up 30 seconds"));
        assert_eq!(result, Some(30));
    }

    #[test]
    fn test_parse_uptime_seconds_with_minutes() {
        let result = parse_uptime_seconds(Some("Up 5 minutes"));
        assert_eq!(result, Some(300)); // 5 * 60
    }

    #[test]
    fn test_parse_uptime_seconds_with_hours() {
        let result = parse_uptime_seconds(Some("Up 2 hours"));
        assert_eq!(result, Some(7200)); // 2 * 3600
    }

    #[test]
    fn test_parse_uptime_seconds_with_days() {
        let result = parse_uptime_seconds(Some("Up 1 day"));
        assert_eq!(result, Some(86400)); // 1 * 86400
    }

#[test]
    fn test_parse_uptime_seconds_with_complex_time() {
        let result = parse_uptime_seconds(Some("Up 30 invalidunit"));
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_uptime_seconds_returns_none_for_exited_container() {
        let result = parse_uptime_seconds(Some("Exited (0) 2 weeks ago"));
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_uptime_seconds_returns_none_for_none_input() {
        let result = parse_uptime_seconds(None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_uptime_seconds_returns_none_for_empty_string() {
        let result = parse_uptime_seconds(Some(""));
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_uptime_seconds_returns_none_for_invalid_format() {
        let result = parse_uptime_seconds(Some("Running for 5 minutes"));
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_uptime_seconds_handles_plural_seconds() {
        let result = parse_uptime_seconds(Some("Up 45 seconds"));
        assert_eq!(result, Some(45));
    }

    #[test]
    fn test_parse_uptime_seconds_handles_plural_minutes() {
        let result = parse_uptime_seconds(Some("Up 10 minutes"));
        assert_eq!(result, Some(600)); // 10 * 60
    }

    #[test]
    fn test_parse_uptime_seconds_handles_plural_hours() {
        let result = parse_uptime_seconds(Some("Up 3 hours"));
        assert_eq!(result, Some(10800)); // 3 * 3600
    }

    #[test]
    fn test_parse_uptime_seconds_handles_plural_days() {
        let result = parse_uptime_seconds(Some("Up 7 days"));
        assert_eq!(result, Some(604800)); // 7 * 86400
    }

    #[test]
    fn test_parse_uptime_seconds_handles_large_values() {
        let result = parse_uptime_seconds(Some("Up 999 days"));
        assert_eq!(result, Some(999 * 86400));
    }

    #[test]
    fn test_map_display_status_running() {
        let result = map_display_status(Some("running"));
        assert_eq!(result, "running");
    }

    #[test]
    fn test_map_display_status_exited() {
        let result = map_display_status(Some("exited"));
        assert_eq!(result, "stopped");
    }

    #[test]
    fn test_map_display_status_dead() {
        let result = map_display_status(Some("dead"));
        assert_eq!(result, "stopped");
    }

    #[test]
    fn test_map_display_status_restarting() {
        let result = map_display_status(Some("restarting"));
        assert_eq!(result, "restarting");
    }

    #[test]
    fn test_map_display_status_unknown_state() {
        let result = map_display_status(Some("paused"));
        assert_eq!(result, "paused");
    }

    #[test]
    fn test_map_display_status_none() {
        let result = map_display_status(None);
        assert_eq!(result, "unknown");
    }
}
