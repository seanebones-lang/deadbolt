//! TR-8 acceptance tests (recorded 2026-11-21): the external
//! watchdog's own liveness heartbeat. Exercised through the public API
//! only, with a manual clock for the 2-second / 30-second contract and
//! the real heartbeat thread on a shortened cadence for the live case.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use deadbolt::{
    spawn_watchdog_heartbeat_with_interval, WatchdogEmitter, WatchdogHeartbeatLog, WatchdogMonitor,
    WATCHDOG_HEARTBEAT_INTERVAL_SECS, WATCHDOG_SILENCE_LIMIT_SECS,
};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// TR-8 (a): with the watchdog alive and pinging on the standard
/// 2-second cadence, the independent monitor never alarms — checked at
/// every ping across two full silence windows.
#[test]
fn normal_heartbeat_cadence_never_alarms() {
    assert_eq!(WATCHDOG_HEARTBEAT_INTERVAL_SECS, 2);
    assert_eq!(WATCHDOG_SILENCE_LIMIT_SECS, 30);

    let start = 1_700_000_000;
    let mut emitter = WatchdogEmitter::new("watchdog-a").unwrap();
    let mut monitor = WatchdogMonitor::new("watchdog-a", start);

    // 31 pings, one every 2 seconds, covering 60 seconds of live time.
    for step in 0..=30_i64 {
        let at = start + step * WATCHDOG_HEARTBEAT_INTERVAL_SECS as i64;
        let ping = emitter.ping(at);
        assert_eq!(ping.sequence, (step + 1) as u64);
        monitor.observe(&ping, at);
        assert!(
            monitor.check(at).is_none(),
            "live watchdog alarmed at step {step}"
        );
        assert!(monitor.require_alive(at).is_ok());
        // One second before the next ping is still inside the limit.
        assert!(monitor.check(at + 1).is_none());
    }
    assert_eq!(emitter.sequence(), 31);
    assert_eq!(monitor.last_seen_at(), Some(start + 60));
}

/// TR-8 (b): when the watchdog stalls, 30 seconds of silence on the
/// monitor's own clock raises the fail-closed alarm — visibly, with the
/// watchdog's identity and the silence length — and a watchdog that
/// never emits at all alarms from the monitor's start.
#[test]
fn thirty_seconds_of_silence_raises_the_alarm() {
    let start = 1_700_000_000;
    let mut emitter = WatchdogEmitter::new("watchdog-a").unwrap();
    let mut monitor = WatchdogMonitor::new("watchdog-a", start);

    let ping = emitter.ping(start);
    monitor.observe(&ping, start);

    // 29 seconds of silence: not yet an alarm.
    assert!(monitor.check(start + 29).is_none());
    assert!(monitor.require_alive(start + 29).is_ok());

    // 30 seconds of silence: the stall/hang alarm, in the fail-closed
    // class, carrying the watchdog's identity.
    let alarm = monitor
        .check(start + 30)
        .expect("30 seconds of watchdog silence must alarm");
    assert_eq!(alarm.as_str(), "watchdog_silence");
    assert_eq!(alarm.watchdog_id, "watchdog-a");
    assert_eq!(alarm.last_seen_at, Some(start));
    assert_eq!(alarm.silent_for_secs, 30);
    assert_eq!(alarm.detected_at, start + 30);
    assert_eq!(monitor.require_alive(start + 30), Err(alarm));

    // A watchdog that never emits alarms too — silence from the
    // monitor's start, never a silent pass.
    let never = WatchdogMonitor::new("watchdog-b", start);
    assert!(never.check(start + 29).is_none());
    let alarm = never
        .check(start + 30)
        .expect("a watchdog that never pings must alarm");
    assert_eq!(alarm.last_seen_at, None);
    assert_eq!(alarm.silent_for_secs, 30);
}

/// TR-8 live thread: the spawned heartbeat thread pings into the shared
/// log on its cadence; once it is stopped, the monitor — judging on its
/// own clock, polled independently — raises the silence alarm. Runs on
/// a shortened 10 ms cadence with a bounded startup wait. The monitor's
/// silence limit is checked on its independent clock below.
#[test]
fn spawned_watchdog_pings_then_silence_alarms() {
    let log = Arc::new(WatchdogHeartbeatLog::new());
    let started = now_secs();
    let mut handle = spawn_watchdog_heartbeat_with_interval(
        "watchdog-live",
        Arc::clone(&log),
        Duration::from_millis(10),
    )
    .expect("valid watchdog id");

    // Observe progress instead of assuming three thread schedules fit inside
    // 65 ms on a shared CI runner. A missing/stalled emitter still fails.
    let startup_deadline = Instant::now() + Duration::from_secs(2);
    let pings = loop {
        let pings = log.pings_for("watchdog-live");
        if pings.len() >= 3 {
            break pings;
        }
        assert!(
            Instant::now() < startup_deadline,
            "expected at least 3 heartbeats before startup deadline, got {}",
            pings.len()
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(pings[0].sequence, 1);

    // A monitor with a 1-second limit, judging on its own clock while
    // the watchdog is alive, stays quiet.
    let mut monitor = WatchdogMonitor::with_silence_limit("watchdog-live", started, 1);
    monitor.poll_log(&log, now_secs());
    assert!(monitor.check(now_secs()).is_none());

    // Kill the watchdog. Silence past the monitor's (shortened) limit
    // is the alarm — nothing about the dead thread reports itself.
    handle.stop();
    let count_at_stop = log.pings_for("watchdog-live").len();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        log.pings_for("watchdog-live").len(),
        count_at_stop,
        "a stopped watchdog must not keep pinging"
    );

    let mut stalled = WatchdogMonitor::with_silence_limit("watchdog-live", started, 1);
    stalled.poll_log(&log, now_secs());
    let deadline = now_secs() + 2;
    assert!(
        stalled.check(deadline).is_some(),
        "silence past the limit must alarm"
    );
    let alarm = stalled.check(deadline).unwrap();
    assert_eq!(alarm.as_str(), "watchdog_silence");
    assert_eq!(alarm.watchdog_id, "watchdog-live");
}
