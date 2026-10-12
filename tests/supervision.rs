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
fn key(gate: &Deadbolt, dir: &std::path::Path, agent: &str, id: &str) -> String {
    let path = dir.join(id);
    gate.issue_supervisor_credential(agent, id, 60, &path)
        .unwrap();
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn workload_cannot_keep_supervised_run_alive_while_legacy_mode_still_slides() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 2);
    gate.issue_supervised_agent("allowed", None).unwrap();
    gate.issue_supervised_agent("denied", None).unwrap();
    gate.ensure_agent("legacy").unwrap();
    gate.set_policy(
        "denied",
        PolicyPatch {
            tools_allow: Some(vec!["read".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let original = gate.status(Some("allowed")).unwrap()[0].expires_at;
    let effect = dir.path().join("effect");
    while now() < original {
        assert!(matches!(
            gate.dispatch("allowed", "write", None, || std::fs::write(
                &effect, "before"
            )),
            Ok(Ok(())) | Err(DenyCode::LeaseExpired)
        ));
        assert!(matches!(
            gate.admit("denied", "write"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded | DenyCode::LeaseExpired
            }
        ));
        assert_eq!(
            gate.status(Some("allowed")).unwrap()[0].expires_at,
            original
        );
        gate.admit("legacy", "read");
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = std::fs::read(&effect).unwrap();
    for id in ["allowed", "denied"] {
        assert_eq!(
            gate.dispatch(id, "write", None, || std::fs::write(&effect, "wrong")
                .unwrap()),
            Err(DenyCode::LeaseExpired)
        );
    }
    assert_eq!(std::fs::read(effect).unwrap(), before);
    assert_eq!(gate.admit("legacy", "read"), AdmitDecision::Allow);
    assert!(gate.status(Some("legacy")).unwrap()[0].expires_at > original);
    gate.ensure_agent("allowed").unwrap();
    assert!(gate.resume("allowed").is_err());
    assert!(gate.issue_supervised_agent("allowed", None).is_err());
}

#[test]
fn renewal_maintains_live_children_without_reviving_expired_or_killed_children() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_supervised_agent("root", Some(now() + 120))
        .unwrap();
    for child in ["child", "expired", "killed"] {
        assert!(gate.register_child("root", child, None).unwrap());
    }
    assert!(gate.register_child("child", "grandchild", None).unwrap());
    gate.ensure_agent("other").unwrap();
    let unrelated = gate.status(Some("other")).unwrap()[0].expires_at;
    let secret = key(&gate, dir.path(), "root", "supervisor");
    assert!(gate
        .issue_supervisor_credential("child", "wrong", 60, &dir.path().join("wrong"))
        .is_err());
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute(
        "UPDATE leases SET expires_at=?1 WHERE supervision_root='root'",
        params![now() + 2],
    )
    .unwrap();
    conn.execute(
        "UPDATE leases SET expires_at=0 WHERE agent_id='expired'",
        [],
    )
    .unwrap();
    gate.kill("killed").unwrap();
    gate.renew_supervised("root", &secret).unwrap();
    for id in ["root", "child", "grandchild"] {
        let status = &gate.status(Some(id)).unwrap()[0];
        assert_eq!(status.supervision_root.as_deref(), Some("root"));
        assert!(status.expires_at > now() + 2);
        assert_eq!(gate.admit(id, "read"), AdmitDecision::Allow);
    }
    assert_eq!(gate.status(Some("expired")).unwrap()[0].expires_at, 0);
    assert_eq!(
        gate.admit("killed", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
    assert_eq!(gate.status(Some("other")).unwrap()[0].expires_at, unrelated);
    assert!(gate
        .export("root", false)
        .unwrap()
        .iter()
        .any(|e| e.kind == "supervisor_renew" && e.class == "observed"));
    let events = std::fs::read_to_string(dir.path().join("deadbolt-events.jsonl")).unwrap();
    assert!(!events.contains(&secret));
}

#[test]
fn separate_credentials_cannot_cross_roles_or_agents_and_revoke_is_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_supervised_agent("root", None).unwrap();
    gate.issue_supervised_agent("other", None).unwrap();
    gate.ensure_agent("legacy").unwrap();
    let secret = key(&gate, dir.path(), "root", "sup");
    gate.issue_credential("root", "worker", 60, &dir.path().join("worker"))
        .unwrap();
    let worker = std::fs::read_to_string(dir.path().join("worker")).unwrap();
    assert!(matches!(
        gate.admit_credential(&secret, "root", "read", None),
        Err(DeadboltError::TokenRequired)
    ));
    assert!(matches!(
        gate.renew_supervised("root", &worker),
        Err(DeadboltError::TokenRequired)
    ));
    assert!(matches!(
        gate.renew_supervised("other", &secret),
        Err(DeadboltError::TokenRequired)
    ));
    assert!(gate
        .issue_supervisor_credential("legacy", "legacy-sup", 60, &dir.path().join("legacy-sup"))
        .is_err());
    assert_eq!(gate.credentials().unwrap().len(), 1);
    assert_eq!(gate.supervisor_credentials().unwrap().len(), 1);
    assert!(gate.revoke_credential("sup").is_err());
    assert!(gate.revoke_supervisor_credential("worker").is_err());
    gate.renew_supervised("root", &secret).unwrap();
    gate.revoke_supervisor_credential("sup").unwrap();
    assert!(matches!(
        gate.renew_supervised("root", &secret),
        Err(DeadboltError::TokenRequired)
    ));
    assert_eq!(
        gate.admit_credential(&worker, "root", "read", None)
            .unwrap(),
        AdmitDecision::Allow
    );
    assert!(gate
        .issue_supervisor_credential("root", "sup", 60, &dir.path().join("replacement"))
        .is_err());
}

#[test]
fn hard_deadline_always_wins_and_restart_preserves_mode_and_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    let deadline = now() + 2;
    gate.issue_supervised_agent("root", Some(deadline)).unwrap();
    gate.register_child("root", "child", None).unwrap();
    let secret = key(&gate, dir.path(), "root", "sup");
    drop(gate);
    let gate = Deadbolt::open_at(dir.path(), true, 600);
    while now() < deadline {
        let _ = gate.renew_supervised("root", &secret);
        assert!(gate.status(Some("root")).unwrap()[0].expires_at <= deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    for id in ["root", "child"] {
        assert_eq!(
            gate.admit(id, "read"),
            AdmitDecision::Deny {
                code: DenyCode::DeadlineExceeded
            }
        );
        assert_eq!(
            gate.status(Some(id)).unwrap()[0].hard_deadline_at,
            Some(deadline)
        );
    }
    assert!(gate.renew_supervised("root", &secret).is_err());
    assert!(gate.resume("root").is_err());
    assert!(!gate.register_child("root", "late", None).unwrap());
}

#[test]
fn pause_and_resume_do_not_renew_and_expiry_does_not_consume_exact_approval() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_supervised_agent("root", None).unwrap();
    let secret = key(&gate, dir.path(), "root", "sup");
    let action =
        ActionRequest::new("root", "send", None, serde_json::json!({"dummy":true}), 600).unwrap();
    gate.approve_action(&action).unwrap();
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute(
        "UPDATE leases SET expires_at=?1 WHERE agent_id='root'",
        params![now() + 3],
    )
    .unwrap();
    let expiry = gate.status(Some("root")).unwrap()[0].expires_at;
    gate.pause("root").unwrap();
    gate.resume("root").unwrap();
    assert_eq!(gate.status(Some("root")).unwrap()[0].expires_at, expiry);
    conn.execute("UPDATE leases SET expires_at=0 WHERE agent_id='root'", [])
        .unwrap();
    assert!(gate.renew_supervised("root", &secret).is_err());
    assert!(gate.resume("root").is_err());
    assert_eq!(
        gate.admit_action(&action).unwrap(),
        AdmitDecision::Deny {
            code: DenyCode::LeaseExpired
        }
    );
    let consumed: Option<i64> = conn
        .query_row("SELECT consumed_at FROM action_grants", [], |r| r.get(0))
        .unwrap();
    assert_eq!(consumed, None);
}

#[test]
fn renewal_rechecks_credential_and_lease_after_waiting_for_writer_lock() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Arc::new(Deadbolt::open_at(dir.path(), true, 60));
    gate.issue_supervised_agent("root", None).unwrap();
    let secret = key(&gate, dir.path(), "root", "sup");
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute(
        "UPDATE admission_credentials SET expires_at=?1 WHERE credential_id='sup'",
        params![now() + 1],
    )
    .unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let clone = gate.clone();
    let worker = std::thread::spawn(move || clone.renew_supervised("root", &secret));
    std::thread::sleep(Duration::from_millis(1200));
    conn.execute_batch("COMMIT").unwrap();
    assert!(matches!(
        worker.join().unwrap(),
        Err(DeadboltError::TokenRequired)
    ));
}

#[test]
fn primary_evidence_failure_rolls_back_renewal() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_supervised_agent("root", None).unwrap();
    let secret = key(&gate, dir.path(), "root", "sup");
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    conn.execute(
        "UPDATE leases SET expires_at=?1 WHERE agent_id='root'",
        params![now() + 3],
    )
    .unwrap();
    let before = gate.status(Some("root")).unwrap()[0].expires_at;
    conn.execute_batch("CREATE TRIGGER reject_renew BEFORE INSERT ON events WHEN NEW.kind='supervisor_renew' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(matches!(
        gate.renew_supervised("root", &secret),
        Err(DeadboltError::StoreUnavailable)
    ));
    assert_eq!(gate.status(Some("root")).unwrap()[0].expires_at, before);
}

#[test]
fn prior_schema_upgrade_preserves_existing_admission_authority_and_terminal_state() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("legacy").unwrap();
    gate.ensure_agent("killed").unwrap();
    gate.kill("killed").unwrap();
    gate.issue_credential("legacy", "old-key", 60, &dir.path().join("old-key"))
        .unwrap();
    let secret = std::fs::read_to_string(dir.path().join("old-key")).unwrap();
    let events_before = gate.export("killed", false).unwrap().len();
    drop(gate);
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    // Reconstruct the prior source schema: credentials had admission authority only.
    conn.execute_batch("ALTER TABLE admission_credentials DROP COLUMN scope; ALTER TABLE leases DROP COLUMN supervision_root;").unwrap();
    drop(conn);
    let upgraded = Deadbolt::open_at(dir.path(), true, 60);
    assert_eq!(
        upgraded.status(Some("legacy")).unwrap()[0].supervision_root,
        None
    );
    assert_eq!(
        upgraded
            .admit_credential(&secret, "legacy", "read", None)
            .unwrap(),
        AdmitDecision::Allow
    );
    assert_eq!(upgraded.credentials().unwrap().len(), 1);
    assert!(upgraded.supervisor_credentials().unwrap().is_empty());
    assert_eq!(
        upgraded.admit("killed", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
    assert!(upgraded.export("killed", false).unwrap().len() >= events_before);
    upgraded.issue_supervised_agent("new", None).unwrap();
    assert_eq!(
        upgraded.status(Some("new")).unwrap()[0]
            .supervision_root
            .as_deref(),
        Some("new")
    );
}

#[test]
fn heartbeat_preserves_pause_clips_policy_and_approvals_and_kill_stays_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.issue_supervised_agent("root", None).unwrap();
    gate.register_child("root", "child", None).unwrap();
    let secret = key(&gate, dir.path(), "root", "sup");
    gate.clip("child", "write").unwrap();
    gate.pause("root").unwrap();
    gate.set_policy(
        "root",
        PolicyPatch {
            tools_allow: Some(vec!["read".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let action =
        ActionRequest::new("root", "send", None, serde_json::json!({"dummy":true}), 600).unwrap();
    gate.approve_action(&action).unwrap();
    gate.renew_supervised("root", &secret).unwrap();
    let root = &gate.status(Some("root")).unwrap()[0];
    assert_eq!(root.state, "paused");
    assert_eq!(gate.status(Some("child")).unwrap()[0].clips, vec!["write"]);
    assert_eq!(
        gate.admit("child", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Paused
        }
    );
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    let (policy, consumed): (String,Option<i64>) = conn.query_row("SELECT tools_allow, (SELECT consumed_at FROM action_grants LIMIT 1) FROM leases WHERE agent_id='root'", [], |r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(policy, r#"["read"]"#);
    assert_eq!(consumed, None);
    gate.kill("root").unwrap();
    assert!(matches!(
        gate.renew_supervised("root", &secret),
        Err(DeadboltError::Killed)
    ));
    for agent in ["root", "child"] {
        assert_eq!(
            gate.admit(agent, "read"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        );
    }
}
