//! Minimal liveness endpoint, std-only (no HTTP framework).
//!
//! A single background thread accepts connections one at a time and answers
//! `GET /health`. "Healthy" means a refresh succeeded recently — a daemon stuck
//! retrying a failing fetch goes stale and reports 503, which is the failure
//! mode worth catching with an external probe.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Shared liveness state. `last_success_unix == 0` means "never succeeded yet".
#[derive(Default)]
pub struct Health {
    last_success_unix: AtomicU64,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Health {
    /// Record that a refresh just completed successfully.
    pub fn mark_success(&self) {
        self.last_success_unix.store(now_unix(), Ordering::Relaxed);
    }

    /// `(healthy, age_seconds)`. Healthy if a refresh succeeded within
    /// `stale_after` seconds.
    fn snapshot(&self, stale_after: u64) -> (bool, u64) {
        let last = self.last_success_unix.load(Ordering::Relaxed);
        let age = now_unix().saturating_sub(last);
        (last != 0 && age <= stale_after, age)
    }
}

/// Bind `listen` and spawn a background accept loop. Returns once bound so the
/// caller can log a bind failure without taking down the refresh loop.
pub fn serve(listen: &str, health: Arc<Health>, stale_after: u64) -> std::io::Result<()> {
    let listener = TcpListener::bind(listen)?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = handle(stream, &health, stale_after);
        }
    });
    Ok(())
}

fn handle(mut stream: TcpStream, health: &Health, stale_after: u64) -> std::io::Result<()> {
    let mut buf = [0u8; 256];
    let _ = stream.read(&mut buf); // we only need the request line's path
    let is_health = String::from_utf8_lossy(&buf).starts_with("GET /health");
    let (healthy, age) = health.snapshot(stale_after);

    let (status, body) = match (is_health, healthy) {
        (true, true) => (
            "200 OK",
            format!("{{\"status\":\"ok\",\"last_success_age_s\":{age}}}"),
        ),
        (true, false) => (
            "503 Service Unavailable",
            format!("{{\"status\":\"stale\",\"last_success_age_s\":{age}}}"),
        ),
        (false, _) => ("404 Not Found", "{\"status\":\"not found\"}".to_string()),
    };

    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unhealthy_before_first_success() {
        let h = Health::default();
        let (healthy, _) = h.snapshot(3600);
        assert!(!healthy);
    }

    #[test]
    fn healthy_right_after_success() {
        let h = Health::default();
        h.mark_success();
        let (healthy, age) = h.snapshot(3600);
        assert!(healthy);
        assert!(age <= 1);
    }

    #[test]
    fn stale_when_success_is_too_old() {
        let h = Health::default();
        h.last_success_unix
            .store(now_unix().saturating_sub(100), Ordering::Relaxed);
        let (healthy, age) = h.snapshot(50);
        assert!(!healthy);
        assert!(age >= 100);
    }
}
