//! Descendant admission checks every ancestor, across renewal modes.
use deadbolt::{ActionRequest, AdmitDecision, Deadbolt, DeadboltError, DenyCode, PolicyPatch};
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn tree(dir: &std::path::Path, supervised: bool) -> Deadbolt {
    let gate = Deadbolt::open_at(dir, true, 60);
    if supervised {
        gate.issue_supervised_agent("root", Some(now() + 600))
            .unwrap();
    } else {
        gate.ensure_agent("root").unwrap();
    }
    assert!(gate.register_child("root", "child", None).unwrap());
    assert!(gate.register_child("child", "grandchild", None).unwrap());
    gate.ensure_agent("unrelated").unwrap();
    gate
}

#[test]
fn paused_intermediate_denies_descendant_without_sliding_ttl() {
    for supervised in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let gate = tree(dir.path(), supervised);
        assert_eq!(gate.admit("grandchild", "read"), AdmitDecision::Allow);
        gate.pause("child").unwrap();
        Connection::open(dir.path().join("deadbolt.db"))
            .unwrap()
            .execute(
                "UPDATE leases SET expires_at=?1 WHERE agent_id='grandchild'",
                params![now() + 10],
            )
            .unwrap();
        let before = gate.status(Some("grandchild")).unwrap()[0].expires_at;
        let marker = dir.path().join("denied-effect");
        assert_eq!(
            gate.dispatch("grandchild", "read", None, || std::fs::write(
                &marker, "wrong"
            )
            .unwrap()),
            Err(DenyCode::Paused)
        );
        assert!(!marker.exists());
        assert_eq!(
            gate.probe("grandchild", "read"),
            AdmitDecision::Deny {
                code: DenyCode::Paused
            }
        );
        assert_eq!(
            gate.status(Some("grandchild")).unwrap()[0].expires_at,
            before
        );
        assert_eq!(gate.admit("root", "read"), AdmitDecision::Allow);
        assert_eq!(gate.admit("unrelated", "read"), AdmitDecision::Allow);
        gate.resume("child").unwrap();
        assert!(gate
            .dispatch("grandchild", "read", None, || std::fs::write(
                &marker, "allowed"
            ))
            .unwrap()
            .is_ok());
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "allowed");
    }
}

#[test]
fn ancestor_pause_preserves_broad_and_exact_grants_across_credential_paths() {
    for supervised in [false, true] {
        for paused in ["root", "child"] {
            let dir = tempfile::tempdir().unwrap();
            let gate = tree(dir.path(), supervised);
            let key_path = dir.path().join("worker.key");
            gate.issue_credential("grandchild", "worker", 600, &key_path)
                .unwrap();
            let secret = std::fs::read_to_string(key_path).unwrap();
            gate.set_policy(
                "grandchild",
                PolicyPatch {
                    irreversible: Some(vec!["broad_write".into()]),
                    ..Default::default()
                },
            )
            .unwrap();
            gate.approve("grandchild", "broad_write").unwrap();
            let action = ActionRequest::new(
                "grandchild",
                "exact_write",
                None,
                serde_json::json!({"value": 1}),
                600,
            )
            .unwrap();
            gate.approve_action(&action).unwrap();
            gate.pause(paused).unwrap();
            let denial = AdmitDecision::Deny {
                code: DenyCode::Paused,
            };
            assert_eq!(
                gate.admit_credential(&secret, "grandchild", "broad_write", None)
                    .unwrap(),
                denial
            );
            assert_eq!(gate.admit_action(&action).unwrap(), denial);
            assert_eq!(
                gate.admit_action_credential(&secret, &action).unwrap(),
                denial
            );
            assert_eq!(
                gate.dispatch_action(&action, |_| panic!("blocked exact body ran")),
                Err(DenyCode::Paused)
            );
            gate.resume(paused).unwrap();
            assert_eq!(
                gate.admit_credential(&secret, "grandchild", "broad_write", None)
                    .unwrap(),
                AdmitDecision::Allow
            );
            assert_eq!(
                gate.admit("grandchild", "broad_write"),
                AdmitDecision::Deny {
                    code: DenyCode::NeedsHuman
                }
            );
            assert_eq!(
                gate.admit_action_credential(&secret, &action).unwrap(),
                AdmitDecision::Allow
            );
            assert_eq!(
                gate.admit_action(&action).unwrap(),
                AdmitDecision::Deny {
                    code: DenyCode::NeedsHuman
                }
            );
            assert_eq!(gate.admit("unrelated", "read"), AdmitDecision::Allow);
        }
    }
}

#[test]
fn ancestor_deadline_and_kill_apply_without_rewriting_descendant_state() {
    for supervised in [false, true] {
        for terminal in ["deadline", "killed"] {
            let dir = tempfile::tempdir().unwrap();
            let gate = tree(dir.path(), supervised);
            let sql = Connection::open(dir.path().join("deadbolt.db")).unwrap();
            // Synthetic state isolates the ancestor check from child deadline
            // inheritance and kill's separate cascading implementation.
            let code = if terminal == "deadline" {
                sql.execute(
                    "UPDATE leases SET hard_deadline_at=?1 WHERE agent_id='child'",
                    params![now()],
                )
                .unwrap();
                DenyCode::DeadlineExceeded
            } else {
                sql.execute(
                    "UPDATE leases SET state='killed' WHERE agent_id='child'",
                    [],
                )
                .unwrap();
                DenyCode::Killed
            };
            assert_eq!(gate.status(Some("grandchild")).unwrap()[0].state, "active");
            assert_eq!(
                gate.dispatch("grandchild", "read", None, || panic!(
                    "terminal ancestor body ran"
                )),
                Err(code)
            );
            assert_eq!(gate.status(Some("grandchild")).unwrap()[0].state, "active");
            assert_eq!(gate.admit("unrelated", "read"), AdmitDecision::Allow);
            // The requested run's own killed state retains priority.
            sql.execute(
                "UPDATE leases SET state='killed' WHERE agent_id='grandchild'",
                [],
            )
            .unwrap();
            assert_eq!(
                gate.admit("grandchild", "read"),
                AdmitDecision::Deny {
                    code: DenyCode::Killed
                }
            );
        }
    }
}

#[test]
fn malformed_lineage_fails_closed_for_regular_scoped_and_exact_admission() {
    let fixtures = [
        "UPDATE leases SET parent_id='missing' WHERE agent_id='child'",
        "UPDATE leases SET parent_id='child' WHERE agent_id='child'",
        "UPDATE leases SET parent_id='grandchild' WHERE agent_id='root'",
        "UPDATE leases SET parent_id='invalid parent' WHERE agent_id='child'",
        "UPDATE leases SET state='unknown' WHERE agent_id='child'",
        "UPDATE leases SET supervision_root='unrelated' WHERE agent_id='child'",
        "UPDATE leases SET parent_id=NULL WHERE agent_id='child'",
    ];
    for fixture in fixtures {
        let dir = tempfile::tempdir().unwrap();
        let gate = tree(dir.path(), true);
        let key_path = dir.path().join("worker.key");
        gate.issue_credential("grandchild", "worker", 600, &key_path)
            .unwrap();
        let secret = std::fs::read_to_string(key_path).unwrap();
        let action =
            ActionRequest::new("grandchild", "write", None, serde_json::json!({}), 600).unwrap();
        gate.approve_action(&action).unwrap();
        Connection::open(dir.path().join("deadbolt.db"))
            .unwrap()
            .execute(fixture, [])
            .unwrap();
        let before = gate.status(Some("grandchild")).unwrap()[0].expires_at;
        assert_eq!(
            gate.dispatch("grandchild", "read", None, || panic!(
                "invalid lineage body ran"
            )),
            Err(DenyCode::StoreUnavailable),
            "{fixture}"
        );
        assert_eq!(
            gate.probe("grandchild", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            }
        );
        assert!(matches!(
            gate.admit_credential(&secret, "grandchild", "read", None),
            Err(DeadboltError::StoreUnavailable)
        ));
        assert!(matches!(
            gate.admit_action(&action),
            Err(DeadboltError::StoreUnavailable)
        ));
        assert!(matches!(
            gate.admit_action_credential(&secret, &action),
            Err(DeadboltError::StoreUnavailable)
        ));
        assert_eq!(
            gate.status(Some("grandchild")).unwrap()[0].expires_at,
            before
        );
        assert_eq!(gate.admit("unrelated", "read"), AdmitDecision::Allow);
    }
}

#[test]
fn deep_healthy_tree_still_executes_and_activity_does_not_renew_ancestors() {
    for supervised in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let gate = tree(dir.path(), supervised);
        let mut parent = "grandchild".to_string();
        for n in 0..32 {
            let child = format!("depth-{n}");
            assert!(gate.register_child(&parent, &child, None).unwrap());
            parent = child;
        }
        let sql = Connection::open(dir.path().join("deadbolt.db")).unwrap();
        sql.execute(
            "UPDATE leases SET expires_at=?1 WHERE agent_id='root'",
            params![now() + 10],
        )
        .unwrap();
        let root_expiry = gate.status(Some("root")).unwrap()[0].expires_at;
        assert_eq!(
            gate.dispatch(&parent, "read", None, || "effect"),
            Ok("effect")
        );
        assert_eq!(
            gate.status(Some("root")).unwrap()[0].expires_at,
            root_expiry
        );
        gate.pause("depth-15").unwrap();
        assert_eq!(
            gate.dispatch(&parent, "read", None, || panic!("deep paused body ran")),
            Err(DenyCode::Paused)
        );
        gate.resume("depth-15").unwrap();
        assert_eq!(
            gate.dispatch(&parent, "read", None, || "effect"),
            Ok("effect")
        );
    }
}

#[test]
fn supervisor_renewal_cannot_bypass_expired_intermediate() {
    let dir = tempfile::tempdir().unwrap();
    let gate = tree(dir.path(), true);
    let key_path = dir.path().join("supervisor.key");
    gate.issue_supervisor_credential("root", "supervisor", 600, &key_path)
        .unwrap();
    let secret = std::fs::read_to_string(key_path).unwrap();
    let sql = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    sql.execute(
        "UPDATE leases SET expires_at=?1 WHERE agent_id IN ('root','grandchild')",
        params![now() + 10],
    )
    .unwrap();
    sql.execute("UPDATE leases SET expires_at=0 WHERE agent_id='child'", [])
        .unwrap();
    gate.renew_supervised("root", &secret).unwrap();
    assert!(gate.status(Some("grandchild")).unwrap()[0].expires_at > now() + 10);
    assert_eq!(gate.status(Some("child")).unwrap()[0].expires_at, 0);
    assert_eq!(
        gate.dispatch("grandchild", "read", None, || panic!(
            "renewed descendant bypassed expired ancestor"
        )),
        Err(DenyCode::LeaseExpired)
    );
}

#[test]
fn root_and_intermediate_expiry_stop_live_descendants_in_both_modes() {
    for supervised in [false, true] {
        for expired in ["root", "child"] {
            let dir = tempfile::tempdir().unwrap();
            let gate = tree(dir.path(), supervised);
            let sql = Connection::open(dir.path().join("deadbolt.db")).unwrap();
            sql.execute(
                "UPDATE leases SET expires_at=?1 WHERE agent_id=?2",
                params![now(), expired],
            )
            .unwrap();
            let before = gate.status(Some("grandchild")).unwrap()[0].expires_at;
            assert_eq!(
                gate.dispatch("grandchild", "read", None, || panic!(
                    "expired ancestor body ran"
                )),
                Err(DenyCode::LeaseExpired)
            );
            assert_eq!(
                gate.status(Some("grandchild")).unwrap()[0].expires_at,
                before
            );
            assert_eq!(gate.admit("unrelated", "read"), AdmitDecision::Allow);
        }
    }
}
