//! Workload traffic must not keep a newly default-issued run alive.
use deadbolt::{ActionRequest, AdmitDecision, Deadbolt, DenyCode, PolicyPatch};
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn explicit_activity_is_preserved_on_restart_without_converting_default_runs() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_activity_agent("legacy").unwrap();
    gate.issue_activity_agent("fixed-legacy", now() + 600)
        .unwrap();
    gate.ensure_agent("default").unwrap();
    let before = gate.status(None).unwrap();
    drop(gate);
    let gate = Deadbolt::open_at(dir.path(), true, 120);
    for id in ["legacy", "fixed-legacy", "default"] {
        gate.ensure_agent(id).unwrap();
        gate.ensure_activity_agent(id).unwrap();
        let original = before.iter().find(|r| r.agent_id == id).unwrap();
        let actual = &gate.status(Some(id)).unwrap()[0];
        assert_eq!(actual.supervision_root, original.supervision_root);
        assert_eq!(actual.expires_at, original.expires_at);
        assert_eq!(actual.hard_deadline_at, original.hard_deadline_at);
    }
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    let expiry = now() + 10;
    conn.execute(
        "UPDATE leases SET expires_at=?1 WHERE agent_id='legacy'",
        [expiry],
    )
    .unwrap();
    assert_eq!(gate.admit("legacy", "read"), AdmitDecision::Allow);
    assert!(gate.status(Some("legacy")).unwrap()[0].expires_at > expiry);
    gate.kill("legacy").unwrap();
    gate.ensure_activity_agent("legacy").unwrap();
    assert_eq!(
        gate.admit("legacy", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
}

#[test]
fn scoped_and_exact_admissions_cannot_slide_default_supervised_children() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("root").unwrap();
    gate.register_child("root", "child", None).unwrap();
    gate.set_policy(
        "child",
        PolicyPatch {
            tools_allow: Some(vec!["read".into(), "write".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let keyfile = dir.path().join("worker-key");
    gate.issue_credential("child", "worker", 60, &keyfile)
        .unwrap();
    let key = std::fs::read_to_string(keyfile).unwrap();
    let action = ActionRequest::new(
        "child",
        "write",
        None,
        serde_json::json!({"fixture":true}),
        60,
    )
    .unwrap();
    gate.approve_action(&action).unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    let expiry = now() + 10;
    conn.execute("UPDATE leases SET expires_at=?1", [expiry])
        .unwrap();
    assert_eq!(
        gate.admit_credential(&key, "child", "read", None).unwrap(),
        AdmitDecision::Allow
    );
    assert_eq!(
        gate.admit_action_credential(&key, &action).unwrap(),
        AdmitDecision::Allow
    );
    assert_eq!(
        gate.admit_action(&action).unwrap(),
        AdmitDecision::Deny {
            code: DenyCode::NeedsHuman
        }
    );
    for id in ["root", "child"] {
        assert_eq!(gate.status(Some(id)).unwrap()[0].expires_at, expiry);
    }
}

#[test]
fn default_issuance_is_supervised_and_allow_deny_traffic_never_slides_it() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("allowed").unwrap();
    gate.issue_agent("denied", now() + 600).unwrap();
    gate.set_policy(
        "denied",
        PolicyPatch {
            tools_allow: Some(vec!["read".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    let expiry = now() + 10;
    conn.execute("UPDATE leases SET expires_at=?1", [expiry])
        .unwrap();
    for id in ["allowed", "denied"] {
        assert_eq!(
            gate.status(Some(id)).unwrap()[0]
                .supervision_root
                .as_deref(),
            Some(id)
        );
    }
    for _ in 0..5 {
        assert_eq!(gate.admit("allowed", "write"), AdmitDecision::Allow);
        assert_eq!(
            gate.admit("denied", "write"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        );
        for id in ["allowed", "denied"] {
            assert_eq!(gate.status(Some(id)).unwrap()[0].expires_at, expiry);
        }
    }
    conn.execute("UPDATE leases SET expires_at=?1", [now()])
        .unwrap();
    for id in ["allowed", "denied"] {
        assert_eq!(
            gate.dispatch(id, "write", None, || panic!("expired effect ran")),
            Err(DenyCode::LeaseExpired)
        );
    }
}
