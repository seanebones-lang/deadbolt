use deadbolt::{ActionRequest, AdmitDecision, Deadbolt, DeadboltError, DenyCode, PolicyPatch};
use rusqlite::{params, Connection};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn wait_for(deadline: i64) {
    while now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[test]
fn active_and_denied_loops_cannot_slide_deadline_or_execute_after_it() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    let file = dir.path().join("effect");
    std::fs::write(&file, "before").unwrap();
    let deadline = now() + 2;
    gate.issue_agent("active", deadline).unwrap();
    gate.issue_agent("denied", deadline).unwrap();
    gate.ensure_agent("unrelated").unwrap();
    gate.set_policy(
        "denied",
        PolicyPatch {
            tools_allow: Some(vec!["read".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    while now() < deadline {
        // Time can cross between the loop condition and admission.
        let result = gate.dispatch("active", "write", None, || std::fs::write(&file, "before"));
        assert!(matches!(
            result,
            Ok(Ok(())) | Err(DenyCode::DeadlineExceeded)
        ));
        assert!(matches!(
            gate.admit("denied", "write"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded | DenyCode::DeadlineExceeded
            }
        ));
        for id in ["active", "denied"] {
            let status = &gate.status(Some(id)).unwrap()[0];
            assert_eq!(status.hard_deadline_at, Some(deadline));
            assert!(status.expires_at <= deadline);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let before = std::fs::read(&file).unwrap();
    for id in ["active", "denied"] {
        assert_eq!(
            gate.dispatch(id, "write", None, || std::fs::write(&file, "wrong")
                .unwrap()),
            Err(DenyCode::DeadlineExceeded)
        );
        gate.ensure_agent(id).unwrap();
        assert!(matches!(gate.resume(id), Err(DeadboltError::BadRequest)));
        assert!(matches!(
            gate.issue_agent(id, now() + 600),
            Err(DeadboltError::BadRequest)
        ));
        assert_eq!(
            gate.status(Some(id)).unwrap()[0].hard_deadline_at,
            Some(deadline)
        );
        let rows = gate.export(id, false).unwrap();
        assert!(rows.iter().any(|r| r.kind == "lease"
            && r.class == "observed"
            && r.hard_deadline_at == Some(deadline)));
        assert!(rows
            .iter()
            .any(|r| r.kind == "decision" && r.code.as_deref() == Some("deadline_exceeded")));
        let incident: serde_json::Value =
            serde_json::from_str(&gate.incident(id).unwrap()).unwrap();
        assert_eq!(incident["hard_deadline_at"], deadline);
    }
    assert_eq!(std::fs::read(file).unwrap(), before);
    assert_eq!(gate.admit("unrelated", "write"), AdmitDecision::Allow);
    gate.issue_agent("fresh-run", now() + 60).unwrap();
    assert_eq!(gate.admit("fresh-run", "write"), AdmitDecision::Allow);
}

#[test]
fn children_inherit_fixed_deadline_across_restart_and_cannot_be_reissued() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    let deadline = now() + 120;
    gate.issue_agent("parent", deadline).unwrap();
    assert!(gate.register_child("parent", "child", None).unwrap());
    assert!(gate.register_child("child", "grandchild", None).unwrap());
    gate.pause("child").unwrap();
    gate.resume("child").unwrap();
    drop(gate);
    let gate = Deadbolt::open_at(dir.path(), true, 600);
    for id in ["parent", "child", "grandchild"] {
        gate.ensure_agent(id).unwrap();
        assert_eq!(gate.admit(id, "read"), AdmitDecision::Allow);
        let status = &gate.status(Some(id)).unwrap()[0];
        assert_eq!(status.hard_deadline_at, Some(deadline));
        assert!(status.expires_at <= deadline);
        assert!(gate.issue_agent(id, deadline + 600).is_err());
    }
    // Move the fixture to the exact boundary without waiting two minutes.
    Connection::open(dir.path().join("deadbolt.db"))
        .unwrap()
        .execute(
            "UPDATE leases SET hard_deadline_at=?1, expires_at=?2",
            params![now(), now() + 600],
        )
        .unwrap();
    for id in ["parent", "child", "grandchild"] {
        assert_eq!(
            gate.admit(id, "read"),
            AdmitDecision::Deny {
                code: DenyCode::DeadlineExceeded
            }
        );
    }
    assert!(!gate.register_child("parent", "late-child", None).unwrap());
    assert!(!gate.register_child("parent", "child", None).unwrap());
    assert_eq!(
        gate.admit("late-child", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
}

#[test]
fn exact_and_scoped_deadline_denials_do_not_consume_approval() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_agent("run", now() + 120).unwrap();
    let action =
        ActionRequest::new("run", "send", None, serde_json::json!({"sample":true}), 600).unwrap();
    gate.approve_action(&action).unwrap();
    let key_path = dir.path().join("credential");
    gate.issue_credential("run", "worker", 600, &key_path)
        .unwrap();
    let secret = std::fs::read_to_string(key_path).unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute(
        "UPDATE leases SET hard_deadline_at=?1, expires_at=?2",
        params![now(), now() + 600],
    )
    .unwrap();
    let denied = AdmitDecision::Deny {
        code: DenyCode::DeadlineExceeded,
    };
    assert_eq!(gate.admit_action(&action).unwrap(), denied);
    assert_eq!(
        gate.admit_action_credential(&secret, &action).unwrap(),
        denied
    );
    assert_eq!(
        gate.admit_credential(&secret, "run", "send", None).unwrap(),
        denied
    );
    let consumed: Option<i64> = conn
        .query_row("SELECT consumed_at FROM action_grants", [], |r| r.get(0))
        .unwrap();
    assert_eq!(consumed, None);
    assert!(gate
        .approve_action(
            &ActionRequest::new("run", "send", None, serde_json::json!({}), 60).unwrap()
        )
        .is_err());
    let refused_file = dir.path().join("refused-key");
    assert!(gate
        .issue_credential("run", "refused", 60, &refused_file)
        .is_err());
    assert!(!refused_file.exists());
}

#[test]
fn writer_lock_wait_rechecks_deadline_for_admission_and_issuance() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Arc::new(Deadbolt::open_at(dir.path(), true, 60));
    let deadline = now() + 2;
    gate.issue_agent("run", deadline).unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let other = gate.clone();
    let worker = std::thread::spawn(move || {
        other.dispatch("run", "write", None, || {
            panic!("expired callback must not run")
        })
    });
    wait_for(deadline);
    conn.execute_batch("COMMIT").unwrap();
    assert_eq!(worker.join().unwrap(), Err(DenyCode::DeadlineExceeded));
    let deadline = now() + 2;
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let other = gate.clone();
    let worker = std::thread::spawn(move || other.issue_agent("new", deadline));
    wait_for(deadline);
    conn.execute_batch("COMMIT").unwrap();
    assert!(matches!(
        worker.join().unwrap(),
        Err(DeadboltError::BadRequest)
    ));
    assert!(gate.status(Some("new")).unwrap().is_empty());
}

#[test]
fn old_store_migration_keeps_idle_only_leases_and_rejects_in_place_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute_batch("CREATE TABLE leases (agent_id TEXT PRIMARY KEY, parent_id TEXT, state TEXT NOT NULL, expires_at INTEGER NOT NULL, clips TEXT NOT NULL, clip_cids TEXT NOT NULL, swarm_task_id TEXT, updated_at INTEGER NOT NULL)").unwrap();
    // A complete historical store includes its evidence and sequence tables.
    // Lease-only databases are partial state, not a supported upgrade input.
    conn.execute_batch("CREATE TABLE events (seq INTEGER PRIMARY KEY,cid TEXT NOT NULL UNIQUE,payload_sha256 TEXT NOT NULL,class TEXT NOT NULL,kind TEXT NOT NULL,payload TEXT NOT NULL,premises TEXT NOT NULL,ts INTEGER NOT NULL);
        CREATE TABLE event_sequence (id INTEGER PRIMARY KEY CHECK(id=1),seq INTEGER NOT NULL);").unwrap();
    conn.execute(
        "INSERT INTO leases VALUES ('old',NULL,'active',?1,'[]','{}',NULL,?2)",
        params![now() + 60, now()],
    )
    .unwrap();
    drop(conn);
    // Operator migration of the quiesced old schema's ordinary SQLite file.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join("deadbolt.db"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    assert_eq!(gate.status(Some("old")).unwrap()[0].hard_deadline_at, None);
    assert_eq!(gate.admit("old", "read"), AdmitDecision::Allow);
    assert!(gate.issue_agent("old", now() + 600).is_err());
    for deadline in [0, -1, now(), 9_007_199_254_740_992, i64::MAX] {
        assert!(gate.issue_agent("invalid", deadline).is_err());
    }
    assert!(gate.status(Some("invalid")).unwrap().is_empty());
}

#[test]
fn operator_cli_issues_once_and_rejects_invalid_deadlines() {
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_deadbolt"))
            .args(args)
            .env("DEADBOLT_DB", dir.path().join("deadbolt.db"))
            .env("DEADBOLT_EVENTS", dir.path().join("events.jsonl"))
            .output()
            .unwrap()
    };
    for invalid in [
        "0",
        "-1",
        "1.5",
        "no",
        "9223372036854775808",
        "9007199254740992",
    ] {
        assert!(
            !run(&["issue", "--agent", "run", "--hard-deadline-at", invalid])
                .status
                .success()
        );
    }
    let deadline = (now() + 600).to_string();
    let args = ["issue", "--agent", "run", "--hard-deadline-at", &deadline];
    assert!(run(&args).status.success());
    assert!(!run(&args).status.success());
    let output = run(&["status", "--agent", "run"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains(&format!("hard_deadline_at={deadline}")));
}

#[test]
fn async_future_rechecks_deadline_when_polled_before_constructing_body() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_agent("run", now() + 600).unwrap();
    let file = dir.path().join("effect");
    let mut future = Box::pin(gate.dispatch_async("run", "write", None, || {
        std::fs::write(&file, "wrong").unwrap();
        async {}
    }));
    Connection::open(dir.path().join("deadbolt.db"))
        .unwrap()
        .execute("UPDATE leases SET hard_deadline_at=0", [])
        .unwrap();
    assert_eq!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(DenyCode::DeadlineExceeded))
    );
    assert!(!file.exists());
}
