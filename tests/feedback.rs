//! Status and evidence contract regressions from real desktop testing.
use deadbolt::{AdmitDecision, Deadbolt, DenyCode, PolicyPatch};
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn status_projects_expiry_and_deadline_without_mutating_state_or_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    for id in [
        "idle",
        "paused-idle",
        "fixed",
        "paused-fixed",
        "killed",
        "live",
    ] {
        gate.issue_agent(id, now() + 600).unwrap();
    }
    gate.pause("paused-idle").unwrap();
    gate.pause("paused-fixed").unwrap();
    gate.kill("killed").unwrap();
    let sql = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    sql.execute("UPDATE leases SET expires_at=?1 WHERE agent_id IN ('idle','paused-idle','fixed','paused-fixed')", [now()]).unwrap();
    sql.execute(
        "UPDATE leases SET hard_deadline_at=?1 WHERE agent_id IN ('fixed','paused-fixed','killed')",
        [now()],
    )
    .unwrap();
    let snapshot = || {
        let mut q = sql
            .prepare("SELECT agent_id,state,expires_at,updated_at FROM leases ORDER BY agent_id")
            .unwrap();
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let count: i64 = sql
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        (rows, count)
    };
    let before = snapshot();
    for _ in 0..2 {
        for (id, expected) in [
            ("idle", "expired"),
            ("paused-idle", "expired"),
            ("fixed", "deadline_exceeded"),
            ("paused-fixed", "deadline_exceeded"),
            ("killed", "killed"),
            ("live", "active"),
        ] {
            assert_eq!(gate.status(Some(id)).unwrap()[0].state, expected);
        }
        assert!(gate.status(Some("missing")).unwrap().is_empty());
        assert_eq!(snapshot(), before);
    }
}

#[test]
fn actual_deny_paths_export_the_same_code_as_the_admission() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    let sql = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    for id in [
        "killed",
        "paused",
        "clip",
        "allowlist",
        "destination",
        "expiry",
        "deadline",
        "cap",
        "approval",
        "broken",
    ] {
        gate.issue_agent(id, now() + 600).unwrap();
    }
    gate.kill("killed").unwrap();
    gate.pause("paused").unwrap();
    gate.clip("clip", "write").unwrap();
    gate.set_policy(
        "allowlist",
        PolicyPatch {
            tools_allow: Some(vec!["read".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    gate.set_policy(
        "destination",
        PolicyPatch {
            dest_allow: Some(vec!["approved.example".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    sql.execute(
        "UPDATE leases SET expires_at=?1 WHERE agent_id='expiry'",
        [now()],
    )
    .unwrap();
    sql.execute(
        "UPDATE leases SET hard_deadline_at=?1 WHERE agent_id='deadline'",
        [now()],
    )
    .unwrap();
    gate.set_policy(
        "cap",
        PolicyPatch {
            spend_cap_usd: Some(0.0),
            ..Default::default()
        },
    )
    .unwrap();
    gate.set_policy(
        "approval",
        PolicyPatch {
            irreversible: Some(vec!["write".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    for (id, dest, expected) in [
        ("missing", None, DenyCode::NoLease),
        ("killed", None, DenyCode::Killed),
        ("paused", None, DenyCode::Paused),
        ("clip", None, DenyCode::PurposeExceeded),
        ("allowlist", None, DenyCode::PurposeExceeded),
        ("destination", None, DenyCode::PurposeExceeded),
        (
            "destination",
            Some("foreign.example"),
            DenyCode::PurposeExceeded,
        ),
        ("expiry", None, DenyCode::LeaseExpired),
        ("deadline", None, DenyCode::DeadlineExceeded),
        ("cap", None, DenyCode::SpendCap),
        ("approval", None, DenyCode::NeedsHuman),
    ] {
        let mut effects = 0;
        assert_eq!(
            gate.dispatch(id, "write", dest, || effects += 1),
            Err(expected)
        );
        assert_eq!(effects, 0);
        let rows = gate.export(id, false).unwrap();
        let decision = rows.last().unwrap();
        assert_eq!(decision.kind, "decision");
        assert_eq!(decision.decision.as_deref(), Some("deny"));
        assert_eq!(decision.code.as_deref(), Some(expected.as_str()));
        if expected == DenyCode::PurposeExceeded {
            assert_eq!(rows[rows.len() - 2].kind, "purpose_exceeded");
        }
    }
    // Failure to read leases can still record into the intact evidence store.
    sql.execute("DROP TABLE leases", []).unwrap();
    assert_eq!(
        gate.admit("broken", "write"),
        AdmitDecision::Deny {
            code: DenyCode::StoreUnavailable
        }
    );
    assert_eq!(
        gate.export("broken", false)
            .unwrap()
            .last()
            .unwrap()
            .code
            .as_deref(),
        Some("store_unavailable")
    );
}

#[test]
fn named_missing_status_is_failure_but_empty_inventory_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_deadbolt"))
            .args(args)
            .env("DEADBOLT_DB", dir.path().join("deadbolt.db"))
            .env("DEADBOLT_EVENTS", dir.path().join("events.jsonl"))
            .output()
            .unwrap()
    };
    assert!(run(&["status"]).status.success());
    let missing = run(&["status", "--agent", "missing"]);
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(missing.stdout).unwrap().trim(),
        "deadbolt status none"
    );
    assert_eq!(
        String::from_utf8(missing.stderr).unwrap().trim(),
        "deadbolt:not_found"
    );
    let deadline = (now() + 600).to_string();
    assert!(
        run(&["issue", "--agent", "live", "--hard-deadline-at", &deadline])
            .status
            .success()
    );
    let live = run(&["status", "--agent", "live"]);
    assert!(live.status.success());
    assert!(String::from_utf8(live.stdout)
        .unwrap()
        .contains("state=active"));
    Connection::open(dir.path().join("deadbolt.db"))
        .unwrap()
        .execute("UPDATE leases SET expires_at=?1", params![now()])
        .unwrap();
    let expired = run(&["status", "--agent", "live"]);
    assert!(expired.status.success()); // Query succeeded; not an admission/readiness grant.
    assert!(String::from_utf8(expired.stdout)
        .unwrap()
        .contains("state=expired"));
}
