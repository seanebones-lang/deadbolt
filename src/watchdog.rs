//! Watchdog liveness heartbeat (TR-8, recorded 2026-11-21 from the
//! fourth-round review follow-up: the watchdog's own liveness).
//!
//! TR-7 moved the overdue judgment to an independently clocked external
//! watchdog: the component that decides a trace record is past its
//! deadline shares no clock and no scheduler with the system it watches.
//! TR-8 closes the hole that leaves behind — the watchdog itself. A
//! watchdog that stalls or hangs must not take the alarm down with it
//! silently.
//!
//! - The watchdog emits a heartbeat ([`WatchdogHeartbeat`]) every
//!   [`WATCHDOG_HEARTBEAT_INTERVAL_SECS`] seconds while it is alive.
//! - An independent monitor ([`WatchdogMonitor`]) reads those heartbeats
//!   against its own clock — never the watchdog's. Silence of
//!   [`WATCHDOG_SILENCE_LIMIT_SECS`] seconds or more is a stall/hang and
//!   raises [`WatchdogSilenceAlarm`], a fail-closed alarm in the same
//!   class as TR-2's unmatched-effect alarm. Silence is never read as
//!   health, and a watchdog that never emits at all alarms from the
//!   monitor's start, so there is no window in which a dead watchdog
//!   looks alive.
//! - The regress terminates at the monitor, exactly as TR-7 prescribes:
//!   the monitor's output is a surface an operator (or a further layer)
//!   reads, and its judgment needs nothing from the watchdog but the
//!   heartbeats the watchdog already emitted.
//!
//! This module is the watchdog's liveness component only. It does not
//! change TR-7's missing-trace alarm or any other DeadBolt behavior.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{is_token, now_secs};

/// Seconds between watchdog heartbeats. The watchdog emits one ping
/// every this many seconds while it is alive.
pub const WATCHDOG_HEARTBEAT_INTERVAL_SECS: u64 = 2;

/// Seconds of watchdog silence that mean stall/hang. Once the monitor's
/// own clock shows at least this long since the last heartbeat, the
/// silence itself is the alarm.
pub const WATCHDOG_SILENCE_LIMIT_SECS: u64 = 30;

/// One heartbeat ping emitted by the external watchdog. Emitted by the
/// watchdog itself; the monitor only reads these, it never creates one,
/// so a dead watchdog cannot forge its own liveness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchdogHeartbeat {
    /// Identity of the watchdog emitting the ping.
    pub watchdog_id: String,
    /// Monotonic ping sequence for this watchdog, starting at 1. Gaps or
    /// a frozen sequence are visible evidence of a stall.
    pub sequence: u64,
    /// Unix second the watchdog emitted the ping, on the watchdog's
    /// clock. The monitor records when it *saw* the ping on its own
    /// clock; that receipt time, not this field, drives the alarm.
    pub emitted_at: i64,
}

/// Fail-closed alarm (TR-8): the external watchdog has been silent for
/// at least the silence limit. This is an alarm condition, not a
/// warning: every judgment the watchdog owed is now unmade, and the
/// system must treat the monitoring layer as down until heartbeats
/// resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchdogSilenceAlarm {
    /// Identity of the silent watchdog.
    pub watchdog_id: String,
    /// Unix second the monitor last saw a heartbeat from this watchdog
    /// on its own clock, or `None` when the watchdog has never emitted
    /// since the monitor started.
    pub last_seen_at: Option<i64>,
    /// Whole seconds of silence at detection, on the monitor's clock.
    pub silent_for_secs: i64,
    /// Unix second the monitor raised the alarm, on its own clock.
    pub detected_at: i64,
}

impl WatchdogSilenceAlarm {
    /// Stable machine token for this alarm condition.
    pub fn as_str(&self) -> &'static str {
        "watchdog_silence"
    }
}

/// The watchdog-side heartbeat emitter. Owns the ping sequence; each
/// [`WatchdogEmitter::ping`] produces the next heartbeat. The emitter
/// holds no alarm state — judging liveness is the monitor's job, on the
/// monitor's clock.
#[derive(Debug)]
pub struct WatchdogEmitter {
    watchdog_id: String,
    sequence: u64,
}

impl WatchdogEmitter {
    /// A new emitter for `watchdog_id`, with no pings emitted yet.
    /// Returns `None` for an id that is not a valid token.
    pub fn new(watchdog_id: &str) -> Option<Self> {
        if is_token(watchdog_id) {
            Some(Self {
                watchdog_id: watchdog_id.to_string(),
                sequence: 0,
            })
        } else {
            None
        }
    }

    /// Emit the next heartbeat, stamped at Unix second `at`.
    pub fn ping(&mut self, at: i64) -> WatchdogHeartbeat {
        self.sequence += 1;
        WatchdogHeartbeat {
            watchdog_id: self.watchdog_id.clone(),
            sequence: self.sequence,
            emitted_at: at,
        }
    }

    /// How many heartbeats this emitter has produced.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// Shared heartbeat log: the surface the watchdog writes and the
/// monitor reads. Kept separate from both, so either side can be
/// killed and replaced without erasing the pings already emitted.
#[derive(Debug, Default)]
pub struct WatchdogHeartbeatLog {
    pings: Mutex<Vec<WatchdogHeartbeat>>,
}

impl WatchdogHeartbeatLog {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one heartbeat emitted by the watchdog.
    pub fn record(&self, ping: WatchdogHeartbeat) {
        if let Ok(mut guard) = self.pings.lock() {
            guard.push(ping);
        }
    }

    /// The most recent heartbeat from `watchdog_id`, if any.
    pub fn latest(&self, watchdog_id: &str) -> Option<WatchdogHeartbeat> {
        self.pings
            .lock()
            .ok()?
            .iter()
            .rev()
            .find(|ping| ping.watchdog_id == watchdog_id)
            .cloned()
    }

    /// All heartbeats from `watchdog_id`, in emission order.
    pub fn pings_for(&self, watchdog_id: &str) -> Vec<WatchdogHeartbeat> {
        self.pings
            .lock()
            .map(|guard| {
                guard
                    .iter()
                    .filter(|ping| ping.watchdog_id == watchdog_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The independent silence monitor. It judges the watchdog's liveness
/// against timestamps supplied by its caller — the monitor's own clock
/// in production wiring — and never consults the watchdog's clock or
/// scheduler. Feed it heartbeats with [`WatchdogMonitor::observe`] as
/// they arrive; ask it for a judgment with [`WatchdogMonitor::check`].
#[derive(Debug)]
pub struct WatchdogMonitor {
    watchdog_id: String,
    silence_limit_secs: i64,
    started_at: i64,
    last_seen_at: Option<i64>,
    last_sequence: u64,
}

impl WatchdogMonitor {
    /// A monitor for `watchdog_id` with the standard
    /// [`WATCHDOG_SILENCE_LIMIT_SECS`] silence limit, started at Unix
    /// second `started_at` on the monitor's clock.
    pub fn new(watchdog_id: &str, started_at: i64) -> Self {
        Self::with_silence_limit(watchdog_id, started_at, WATCHDOG_SILENCE_LIMIT_SECS as i64)
    }

    /// A monitor with an explicit silence limit in seconds. A limit
    /// below 1 second is raised to 1: a zero limit would alarm on a
    /// live watchdog.
    pub fn with_silence_limit(watchdog_id: &str, started_at: i64, silence_limit_secs: i64) -> Self {
        Self {
            watchdog_id: watchdog_id.to_string(),
            silence_limit_secs: silence_limit_secs.max(1),
            started_at,
            last_seen_at: None,
            last_sequence: 0,
        }
    }

    /// Record a heartbeat seen at Unix second `seen_at` on the
    /// monitor's clock. Heartbeats from another watchdog, and replays
    /// at or below the last sequence seen, are ignored: stale evidence
    /// cannot revive a silent watchdog.
    pub fn observe(&mut self, ping: &WatchdogHeartbeat, seen_at: i64) {
        if ping.watchdog_id != self.watchdog_id || ping.sequence <= self.last_sequence {
            return;
        }
        self.last_sequence = ping.sequence;
        self.last_seen_at = Some(seen_at);
    }

    /// Read the latest heartbeat from a shared log, stamped with the
    /// monitor's own current time `seen_at`.
    pub fn poll_log(&mut self, log: &WatchdogHeartbeatLog, seen_at: i64) {
        if let Some(ping) = log.latest(&self.watchdog_id) {
            self.observe(&ping, seen_at);
        }
    }

    /// The last Unix second (monitor's clock) a heartbeat was seen, or
    /// `None` when none has been seen since this monitor started.
    pub fn last_seen_at(&self) -> Option<i64> {
        self.last_seen_at
    }

    /// Judge liveness at Unix second `now` on the monitor's clock.
    /// Returns the fail-closed alarm when silence has reached the
    /// limit — measured from the last seen heartbeat, or from the
    /// monitor's start when the watchdog has never emitted — and
    /// `None` while heartbeats are arriving inside the limit.
    pub fn check(&self, now: i64) -> Option<WatchdogSilenceAlarm> {
        let since = self.last_seen_at.unwrap_or(self.started_at);
        let silent_for = now.saturating_sub(since);
        if silent_for >= self.silence_limit_secs {
            Some(WatchdogSilenceAlarm {
                watchdog_id: self.watchdog_id.clone(),
                last_seen_at: self.last_seen_at,
                silent_for_secs: silent_for,
                detected_at: now,
            })
        } else {
            None
        }
    }

    /// Fail-closed check: `Err` carrying the alarm when the watchdog
    /// has been silent too long at `now`.
    pub fn require_alive(&self, now: i64) -> Result<(), WatchdogSilenceAlarm> {
        match self.check(now) {
            Some(alarm) => Err(alarm),
            None => Ok(()),
        }
    }
}

/// A running watchdog heartbeat thread: emits a ping into `log` every
/// [`WATCHDOG_HEARTBEAT_INTERVAL_SECS`] seconds (or the interval given
/// to [`spawn_watchdog_heartbeat_with_interval`]) until stopped or
/// dropped. Stopping is explicit so tests and operators can simulate —
/// or perform — a real watchdog halt and watch the monitor alarm.
#[derive(Debug)]
pub struct WatchdogHeartbeatHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WatchdogHeartbeatHandle {
    /// Stop the heartbeat thread and wait for it to exit. After this
    /// returns, no further pings are emitted: the silence the monitor
    /// then measures is real.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for WatchdogHeartbeatHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawn the watchdog heartbeat thread with the standard 2-second
/// interval, writing pings into `log`.
pub fn spawn_watchdog_heartbeat(
    watchdog_id: &str,
    log: Arc<WatchdogHeartbeatLog>,
) -> Option<WatchdogHeartbeatHandle> {
    spawn_watchdog_heartbeat_with_interval(
        watchdog_id,
        log,
        Duration::from_secs(WATCHDOG_HEARTBEAT_INTERVAL_SECS),
    )
}

/// Spawn the watchdog heartbeat thread with an explicit interval.
/// Production wiring uses the standard interval; the explicit form
/// exists so tests can run the same machinery on a shortened cadence.
/// Returns `None` for an invalid watchdog id.
pub fn spawn_watchdog_heartbeat_with_interval(
    watchdog_id: &str,
    log: Arc<WatchdogHeartbeatLog>,
    interval: Duration,
) -> Option<WatchdogHeartbeatHandle> {
    let mut emitter = WatchdogEmitter::new(watchdog_id)?;
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let join = std::thread::spawn(move || loop {
        if thread_stop.load(Ordering::SeqCst) {
            break;
        }
        let ping = emitter.ping(now_secs());
        log.record(ping);
        // Sleep in short slices so a stop lands promptly instead of
        // waiting out the full interval.
        let mut waited = Duration::ZERO;
        while waited < interval && !thread_stop.load(Ordering::SeqCst) {
            let slice = (interval - waited).min(Duration::from_millis(5));
            std::thread::sleep(slice);
            waited += slice;
        }
    });
    Some(WatchdogHeartbeatHandle {
        stop,
        join: Some(join),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitter_sequences_pings_from_one() {
        let mut emitter = WatchdogEmitter::new("watchdog-a").unwrap();
        let first = emitter.ping(1_700_000_000);
        let second = emitter.ping(1_700_000_002);
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(emitter.sequence(), 2);
    }

    #[test]
    fn invalid_watchdog_id_is_rejected() {
        assert!(WatchdogEmitter::new("not a token!").is_none());
        assert!(WatchdogEmitter::new("").is_none());
    }

    #[test]
    fn replayed_ping_does_not_revive_a_silent_watchdog() {
        let mut emitter = WatchdogEmitter::new("watchdog-a").unwrap();
        let ping = emitter.ping(100);
        let mut monitor = WatchdogMonitor::new("watchdog-a", 100);
        monitor.observe(&ping, 100);
        // The same ping replayed later must not move last_seen forward.
        monitor.observe(&ping, 125);
        let alarm = monitor.check(130).expect("silence must alarm");
        assert_eq!(alarm.last_seen_at, Some(100));
        assert_eq!(alarm.silent_for_secs, 30);
    }
}
