//! Outbound heartbeat to an external monitor.
//!
//! Rather than serving a liveness endpoint, the daemon pushes a heartbeat to a
//! configured URL after each successful refresh (a dead-man's-switch, as used by
//! Healthchecks.io and similar). The external monitor alerts when the pings
//! stop, which catches a daemon stuck retrying a failing fetch — the failure
//! mode worth watching — without exposing any inbound port on the device.

use anyhow::Result;

use crate::weather::http_agent;

/// Send a heartbeat to `url`. Best-effort by contract: callers log the error and
/// keep going, since a failed ping must never interrupt the refresh loop.
pub fn ping(url: &str) -> Result<()> {
    http_agent().get(url).call()?;
    Ok(())
}
