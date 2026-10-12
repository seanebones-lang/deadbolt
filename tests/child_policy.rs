//! Registered descendants retain the restrictions of their ancestry.
use deadbolt::{ActionRequest, AdmitDecision, Deadbolt, DenyCode, PolicyPatch};

fn denied(code: DenyCode) -> AdmitDecision {
    AdmitDecision::Deny { code }
}

fn tree(dir: &std::path::Path) -> Deadbolt {
    let db = Deadbolt::open_at(dir, true, 60);
    db.ensure_agent("root").unwrap();
    db.set_policy(
        "root",
        PolicyPatch {
            tools_allow: Some(vec!["read".into(), "write".into()]),
            dest_allow: Some(vec!["allowed.example".into()]),
            offline_tools: Some(vec!["read".into(), "write".into()]),
            irreversible: Some(vec!["write".into()]),
            spend_cap_usd: Some(10.0),
        },
    )
    .unwrap();
    assert!(db.register_child("root", "child", None).unwrap());
    assert!(db.register_child("child", "grandchild", None).unwrap());
    assert!(db.register_child("root", "sibling", None).unwrap());
    db.ensure_agent("unrelated").unwrap();
    db
}

#[test]
fn child_cannot_widen_tools_destinations_or_offline_exceptions() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    assert_eq!(db.admit("grandchild", "read"), AdmitDecision::Allow);
    let marker = dir.path().join("shell-ran");
    assert_eq!(
        db.dispatch("grandchild", "shell", None, || std::fs::write(
            &marker, "wrong"
        )
        .unwrap()),
        Err(DenyCode::PurposeExceeded)
    );
    assert!(!marker.exists());
    db.set_policy(
        "grandchild",
        PolicyPatch {
            tools_allow: Some(vec!["read".into(), "write".into(), "shell".into()]),
            dest_allow: Some(vec!["allowed.example".into(), "foreign.example".into()]),
            offline_tools: Some(vec!["read".into(), "write".into(), "shell".into()]),
            irreversible: Some(vec![]),
            spend_cap_usd: Some(100.0),
        },
    )
    .unwrap();
    assert_eq!(
        db.admit("grandchild", "shell"),
        denied(DenyCode::PurposeExceeded)
    );
    assert_eq!(
        db.admit_dest("grandchild", "read", Some("foreign.example")),
        denied(DenyCode::PurposeExceeded)
    );
    assert_eq!(
        db.admit_dest("grandchild", "read", Some("ALLOWED.EXAMPLE")),
        AdmitDecision::Allow
    );
    db.set_policy(
        "root",
        PolicyPatch {
            offline_tools: Some(vec![]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        db.admit("grandchild", "read"),
        denied(DenyCode::PurposeExceeded)
    );
    assert_eq!(
        db.admit_dest("grandchild", "read", Some("allowed.example")),
        AdmitDecision::Allow
    );
    assert_eq!(db.admit("unrelated", "shell"), AdmitDecision::Allow);
}

#[test]
fn parent_changes_and_clips_restrict_existing_children_without_transfer_of_grants() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    db.approve("root", "write").unwrap();
    assert_eq!(
        db.admit("grandchild", "write"),
        denied(DenyCode::NeedsHuman)
    );
    db.approve("grandchild", "write").unwrap();
    db.clip("child", "write").unwrap();
    assert_eq!(
        db.admit("grandchild", "write"),
        denied(DenyCode::PurposeExceeded)
    );
    db.resume("child").unwrap();
    assert_eq!(db.admit("grandchild", "write"), AdmitDecision::Allow);
    assert_eq!(
        db.admit("grandchild", "write"),
        denied(DenyCode::NeedsHuman)
    );
    assert_eq!(db.admit("root", "write"), AdmitDecision::Allow);
    db.set_policy(
        "root",
        PolicyPatch {
            tools_allow: Some(vec!["write".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        db.admit("grandchild", "read"),
        denied(DenyCode::PurposeExceeded)
    );
}

#[test]
fn ancestor_exact_requirement_cannot_be_relaxed_by_child() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    db.require_exact_action("root", "read", true).unwrap();
    db.require_exact_action("grandchild", "read", false)
        .unwrap();
    assert!(db.approve("grandchild", "read").is_err());
    assert_eq!(db.admit("grandchild", "read"), denied(DenyCode::NeedsHuman));
    let action =
        ActionRequest::new("grandchild", "read", None, serde_json::json!({}), 300).unwrap();
    db.approve_action(&action).unwrap();
    assert_eq!(db.admit_action(&action).unwrap(), AdmitDecision::Allow);
    assert_eq!(
        db.admit_action(&action).unwrap(),
        denied(DenyCode::NeedsHuman)
    );
}

#[test]
fn tree_spend_is_aggregated_once_and_prevents_sibling_cap_splitting() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    assert_eq!(db.spend_add("root", 1.0).unwrap().spend_usd, 1.0);
    assert_eq!(db.spend_add("child", 2.0).unwrap().spend_usd, 2.0);
    assert_eq!(db.spend_add("grandchild", 3.0).unwrap().spend_usd, 3.0);
    assert_eq!(db.spend_add("sibling", 3.0).unwrap().spend_usd, 3.0);
    assert_eq!(db.admit("grandchild", "read"), AdmitDecision::Allow);
    assert!(db.spend_add("sibling", 1.0).unwrap().paused);
    for id in ["root", "child", "grandchild", "sibling"] {
        assert_eq!(db.admit(id, "read"), denied(DenyCode::SpendCap));
    }
    db.resume("root").unwrap();
    assert_eq!(db.admit("grandchild", "read"), denied(DenyCode::SpendCap));
    assert_eq!(db.admit("unrelated", "shell"), AdmitDecision::Allow);
    // Ledgers remain per identity; aggregation is computed, never charged twice.
    assert_eq!(db.spend_add("root", 0.0).unwrap().spend_usd, 1.0);
    let conn = rusqlite::Connection::open(dir.path().join("deadbolt.db")).unwrap();
    assert_eq!(
        conn.query_row("SELECT SUM(spend_usd) FROM leases", [], |r| r
            .get::<_, f64>(0))
            .unwrap(),
        10.0
    );
    drop(conn);
    drop(db);
    let restarted = Deadbolt::open_at(dir.path(), true, 60);
    assert_eq!(
        restarted.admit("grandchild", "read"),
        denied(DenyCode::SpendCap)
    );
}

#[test]
fn tightened_intermediate_cap_uses_existing_spend_and_policy_persists_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    db.spend_add("grandchild", 2.0).unwrap();
    db.set_policy(
        "child",
        PolicyPatch {
            spend_cap_usd: Some(2.0),
            ..Default::default()
        },
    )
    .unwrap();
    let reopened = Deadbolt::open_at(dir.path(), true, 60);
    assert_eq!(
        reopened.admit("grandchild", "read"),
        denied(DenyCode::SpendCap)
    );
    assert_eq!(reopened.admit("sibling", "read"), AdmitDecision::Allow);
    let report: serde_json::Value =
        serde_json::from_str(&reopened.incident("child").unwrap()).unwrap();
    assert_eq!(report["policy"]["spend_usd"], 0.0);
    assert_eq!(report["policy"]["subtree_spend_usd"], 2.0);
}

#[test]
fn concurrent_sibling_reports_preserve_tree_accounting_without_replaying_charges() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|index| {
            let other = Deadbolt::open_at(dir.path(), true, 60);
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                match other.spend_add(
                    if index % 2 == 0 {
                        "grandchild"
                    } else {
                        "sibling"
                    },
                    2.0,
                ) {
                    Ok(_) => true,
                    Err(deadbolt::DeadboltError::StoreUnavailable) => false,
                    Err(error) => panic!("unexpected report: {error:?}"),
                }
            })
        })
        .collect();
    let committed = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .filter(|ok| *ok)
        .count();
    let report: serde_json::Value = serde_json::from_str(&db.incident("root").unwrap()).unwrap();
    assert!(committed > 0, "at least one report must commit");
    assert_eq!(
        report["policy"]["subtree_spend_usd"],
        committed as f64 * 2.0
    );
    assert_eq!(report["policy"]["spend_usd"], 0.0);
    if committed >= 5 {
        assert_eq!(db.admit("child", "read"), denied(DenyCode::SpendCap));
    }
}

#[test]
fn inherited_policy_denial_preserves_activity_and_supervised_liveness_contracts() {
    for supervised in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let db = Deadbolt::open_at(dir.path(), true, 60);
        if supervised {
            db.issue_supervised_agent("root", None).unwrap();
            db.issue_supervised_agent("local-policy", None).unwrap();
        } else {
            db.ensure_activity_agent("root").unwrap();
            db.ensure_activity_agent("local-policy").unwrap();
        }
        db.register_child("root", "child", None).unwrap();
        for id in ["root", "local-policy"] {
            db.set_policy(
                id,
                PolicyPatch {
                    tools_allow: Some(vec!["read".into()]),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        db.approve("child", "write").unwrap();
        let parent_expiry = db.status(Some("root")).unwrap()[0].expires_at;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let conn = rusqlite::Connection::open(dir.path().join("deadbolt.db")).unwrap();
        conn.execute(
            "UPDATE leases SET expires_at=?1 WHERE agent_id IN ('child','local-policy')",
            [now + 10],
        )
        .unwrap();
        for id in ["child", "local-policy"] {
            assert_eq!(db.admit(id, "write"), denied(DenyCode::PurposeExceeded));
            let expiry = db.status(Some(id)).unwrap()[0].expires_at;
            if supervised {
                assert_eq!(expiry, now + 10);
            } else {
                assert!(expiry >= now + 60);
            }
        }
        assert_eq!(
            db.status(Some("root")).unwrap()[0].expires_at,
            parent_expiry
        );
        assert_eq!(
            conn.query_row(
                "SELECT approvals FROM leases WHERE agent_id='child'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "[\"write\"]"
        );
    }
}

#[test]
fn scoped_admission_and_exact_actions_cannot_bypass_ancestor_policy() {
    let dir = tempfile::tempdir().unwrap();
    let db = tree(dir.path());
    let key = dir.path().join("worker.key");
    db.issue_credential("grandchild", "worker", 600, &key)
        .unwrap();
    let secret = std::fs::read_to_string(key).unwrap();
    assert_eq!(
        db.admit_credential(&secret, "grandchild", "shell", None)
            .unwrap(),
        denied(DenyCode::PurposeExceeded)
    );
    let action = ActionRequest::new(
        "grandchild",
        "write",
        None,
        serde_json::json!({"marker": "harmless"}),
        300,
    )
    .unwrap();
    db.approve_action(&action).unwrap();
    db.clip("root", "write").unwrap();
    assert_eq!(
        db.admit_action_credential(&secret, &action).unwrap(),
        denied(DenyCode::PurposeExceeded)
    );
    db.resume("root").unwrap();
    assert_eq!(
        db.admit_action_credential(&secret, &action).unwrap(),
        AdmitDecision::Allow
    );
    assert_eq!(
        db.admit_action_credential(&secret, &action).unwrap(),
        denied(DenyCode::NeedsHuman)
    );
}
