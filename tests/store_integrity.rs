use deadbolt::{AdmitDecision, Deadbolt, DeadboltError, DenyCode};
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

fn deadline() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 600
}

#[test]
fn partial_schema_is_refused_without_recreating_tables_or_reissuing_killed_id() {
    for table in [
        "leases",
        "events",
        "event_sequence",
        "action_grants",
        "exact_action_requirements",
        "admission_credentials",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let gate = Deadbolt::open_at(dir.path(), true, 60);
        gate.issue_agent("killed", deadline()).unwrap();
        gate.kill("killed").unwrap();
        drop(gate);
        let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
        conn.execute_batch(&format!("DROP TABLE {table}")).unwrap();
        drop(conn);
        let reopened = Deadbolt::open_at(dir.path(), true, 60);
        assert_eq!(
            reopened.admit("killed", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            },
            "{table}"
        );
        assert_eq!(
            reopened.issue_agent("killed", deadline()),
            Err(DeadboltError::StoreUnavailable)
        );
        let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
        let present: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!present, "startup repaired damaged {table}");
    }
}

#[test]
fn historical_identity_cannot_be_recreated_through_any_issuance_path() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_agent("parent", deadline()).unwrap();
    gate.issue_agent("killed", deadline()).unwrap();
    gate.kill("killed").unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute("DELETE FROM leases WHERE agent_id='killed'", [])
        .unwrap();
    drop(conn);
    for result in [
        gate.issue_agent("killed", deadline()),
        gate.issue_activity_agent("killed", deadline()),
        gate.ensure_agent("killed"),
        gate.ensure_activity_agent("killed"),
    ] {
        assert_eq!(result, Err(DeadboltError::BadRequest));
    }
    assert_eq!(
        gate.register_child("parent", "killed", None),
        Err(DeadboltError::BadRequest)
    );
    assert!(gate.status(Some("killed")).unwrap().is_empty());
    assert_eq!(
        gate.admit("killed", "read"),
        AdmitDecision::Deny {
            code: DenyCode::NoLease
        }
    );
    gate.issue_agent("fresh", deadline()).unwrap();
    assert_eq!(gate.admit("fresh", "read"), AdmitDecision::Allow);
}

#[test]
fn sidecar_refuses_damaged_store_before_accepting_connections() {
    use std::net::TcpListener;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_agent("killed", deadline()).unwrap();
    gate.kill("killed").unwrap();
    drop(gate);
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute_batch("DROP TABLE leases").unwrap();
    drop(conn);
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_deadbolt"))
        .args(["serve", "--bind", &address.to_string()])
        .env("DEADBOLT_DB", dir.path().join("deadbolt.db"))
        .env("DEADBOLT_EVENTS", dir.path().join("events.jsonl"))
        .env("DEADBOLT_TOKEN", "disposable-store-test")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while child.try_wait().unwrap().is_none() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    let exited = child.try_wait().unwrap().is_some();
    if !exited {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(exited, "sidecar stayed alive with a damaged store");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("store_unavailable"));
}

#[test]
fn denied_unissued_attempt_does_not_reserve_an_operator_run_identity() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    for id in ["ensure", "issue", "child"] {
        assert_eq!(
            gate.admit(id, "read"),
            AdmitDecision::Deny {
                code: DenyCode::NoLease
            }
        );
    }
    gate.ensure_agent("ensure").unwrap();
    gate.issue_agent("issue", deadline()).unwrap();
    gate.issue_agent("parent", deadline()).unwrap();
    assert!(gate.register_child("parent", "child", None).unwrap());
    for id in ["ensure", "issue", "child"] {
        assert_eq!(gate.admit(id, "read"), AdmitDecision::Allow);
    }
}

#[test]
fn unversioned_modern_schema_without_sequence_is_not_a_pre_v1_store() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_agent("run", deadline()).unwrap();
    drop(gate);
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute_batch("PRAGMA user_version=0; DROP TABLE event_sequence;")
        .unwrap();
    drop(conn);
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    assert_eq!(
        gate.admit("run", "read"),
        AdmitDecision::Deny {
            code: DenyCode::StoreUnavailable
        }
    );
}

#[test]
fn versioned_policy_and_credential_columns_are_not_repaired_with_defaults() {
    for (table, column) in [
        ("leases", "tools_allow"),
        ("leases", "offline_tools"),
        ("leases", "hard_deadline_at"),
        ("leases", "supervision_root"),
        ("leases", "irreversible"),
        ("leases", "approvals"),
        ("admission_credentials", "scope"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let gate = Deadbolt::open_at(dir.path(), true, 60);
        gate.issue_agent("damaged", deadline()).unwrap();
        drop(gate);
        let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
        conn.execute_batch(&format!("ALTER TABLE {table} DROP COLUMN {column}"))
            .unwrap();
        drop(conn);
        let reopened = Deadbolt::open_at(dir.path(), true, 60);
        assert_eq!(
            reopened.admit("damaged", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            },
            "{table}.{column}"
        );
        let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            !columns.iter().any(|name| name == column),
            "repaired {table}.{column}"
        );
    }
}
