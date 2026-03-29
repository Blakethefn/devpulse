use anyhow::Result;
use std::net::TcpStream;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RemoteStatus {
    pub name: String,
    pub kind: RemoteKind,
    pub status: CheckResult,
    pub latency_ms: Option<u64>,
    pub detail: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RemoteKind {
    Http,
    Ssh,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckResult {
    Up,
    Down,
    Degraded,
    Unknown,
}

impl std::fmt::Display for RemoteKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteKind::Http => write!(f, "HTTP"),
            RemoteKind::Ssh => write!(f, "SSH"),
        }
    }
}

impl std::fmt::Display for CheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckResult::Up => write!(f, "UP"),
            CheckResult::Down => write!(f, "DOWN"),
            CheckResult::Degraded => write!(f, "SLOW"),
            CheckResult::Unknown => write!(f, "???"),
        }
    }
}

pub async fn check_http(name: &str, url: &str) -> RemoteStatus {
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .danger_accept_invalid_certs(false)
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return RemoteStatus {
                name: name.to_string(),
                kind: RemoteKind::Http,
                status: CheckResult::Down,
                latency_ms: None,
                detail: String::new(),
                error: Some(e.to_string()),
            };
        }
    };

    match client.get(url).send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status_code = resp.status();
            let check_result = if status_code.is_success() {
                if latency > 2000 {
                    CheckResult::Degraded
                } else {
                    CheckResult::Up
                }
            } else if status_code.is_server_error() {
                CheckResult::Down
            } else {
                CheckResult::Degraded
            };

            RemoteStatus {
                name: name.to_string(),
                kind: RemoteKind::Http,
                status: check_result,
                latency_ms: Some(latency),
                detail: format!("{}", status_code.as_u16()),
                error: None,
            }
        }
        Err(e) => RemoteStatus {
            name: name.to_string(),
            kind: RemoteKind::Http,
            status: CheckResult::Down,
            latency_ms: None,
            detail: String::new(),
            error: Some(e.to_string()),
        },
    }
}

pub fn check_ssh(name: &str, host: &str, port: u16, user: &Option<String>) -> RemoteStatus {
    let start = Instant::now();
    let addr = format!("{}:{}", host, port);

    // First, check TCP connectivity
    let stream = match TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5)) {
        Ok(s) => s,
        Err(e) => {
            return RemoteStatus {
                name: name.to_string(),
                kind: RemoteKind::Ssh,
                status: CheckResult::Down,
                latency_ms: None,
                detail: String::new(),
                error: Some(format!("TCP connect failed: {}", e)),
            };
        }
    };

    let latency = start.elapsed().as_millis() as u64;

    // Try SSH handshake
    let mut sess = match ssh2::Session::new() {
        Ok(s) => s,
        Err(e) => {
            return RemoteStatus {
                name: name.to_string(),
                kind: RemoteKind::Ssh,
                status: CheckResult::Degraded,
                latency_ms: Some(latency),
                detail: "TCP ok, SSH init failed".to_string(),
                error: Some(e.to_string()),
            };
        }
    };

    sess.set_tcp_stream(stream);
    if let Err(e) = sess.handshake() {
        return RemoteStatus {
            name: name.to_string(),
            kind: RemoteKind::Ssh,
            status: CheckResult::Degraded,
            latency_ms: Some(latency),
            detail: "TCP ok, handshake failed".to_string(),
            error: Some(e.to_string()),
        };
    }

    // Try auth with SSH agent if user is provided
    let auth_detail = if let Some(user) = user {
        match sess.userauth_agent(user) {
            Ok(()) => format!("authed as {}", user),
            Err(_) => format!("handshake ok (auth as {} failed)", user),
        }
    } else {
        "handshake ok".to_string()
    };

    let total_latency = start.elapsed().as_millis() as u64;

    RemoteStatus {
        name: name.to_string(),
        kind: RemoteKind::Ssh,
        status: if total_latency > 2000 {
            CheckResult::Degraded
        } else {
            CheckResult::Up
        },
        latency_ms: Some(total_latency),
        detail: auth_detail,
        error: None,
    }
}
