//! Malformed persisted fields are an unavailable store, never an open policy.
//! This is corruption handling; valid edits by the operator remain trusted.
use deadbolt::{AdmitDecision, Deadbolt, DenyCode, PolicyPatch};
use rusqlite::{params, Connection};

#[test]
fn malformed_local_or_ancestor_fields_withhold_effects_without_repair() {
    for (column, bad) in [
        ("tools_allow", "not-json"),
        ("tools_allow", "null"),
        ("dest_allow", "{}"),
        ("offline_tools", "[1]"),
        ("irreversible", "\"write\""),
        ("clips", "{}"),
        ("clip_cids", "[]"),
        ("approvals", "null"),
        ("state", "corrupt"),
        ("spend_usd", "-1"),
        ("spend_cap_usd", "-1"),
    ] {
        for ancestor in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let gate = Deadbolt::open_at(dir.path(), true, 60);
            gate.ensure_agent("root").unwrap();
            gate.register_child("root", "child", None).unwrap();
            for id in ["root", "child"] {
                gate.set_policy(
                    id,
                    PolicyPatch {
                        tools_allow: Some(vec!["read".into()]),
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            assert_eq!(gate.admit("child", "read"), AdmitDecision::Allow);
            let id = if ancestor { "root" } else { "child" };
            let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
            conn.execute(
                &format!("UPDATE leases SET {column}=?1 WHERE agent_id=?2"),
                params![bad, id],
            )
            .unwrap();
            let stored: String = conn
                .query_row(
                    &format!("SELECT CAST({column} AS TEXT) FROM leases WHERE agent_id=?1"),
                    [id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                gate.dispatch("child", "read", None, || panic!("corrupt policy executed")),
                Err(DenyCode::StoreUnavailable),
                "{column} ancestor={ancestor}"
            );
            assert!(
                gate.status(Some(id)).is_err(),
                "status must not present malformed fields as an empty policy"
            );
            assert!(
                gate.resume(id).is_err(),
                "operator mutations must not silently repair corrupted policy"
            );
            let actual: String = conn
                .query_row(
                    &format!("SELECT CAST({column} AS TEXT) FROM leases WHERE agent_id=?1"),
                    [id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(actual, stored);
        }
    }
}

#[test]
fn sql_null_and_valid_empty_lists_keep_distinct_policy_meanings() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("unrestricted").unwrap();
    gate.ensure_agent("deny-all").unwrap();
    gate.set_policy(
        "deny-all",
        PolicyPatch {
            tools_allow: Some(vec![]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(gate.admit("unrestricted", "read"), AdmitDecision::Allow);
    assert_eq!(
        gate.admit("deny-all", "read"),
        AdmitDecision::Deny {
            code: DenyCode::PurposeExceeded
        }
    );
    assert_eq!(gate.status(None).unwrap().len(), 2);
    let conn = Connection::open(dir.path().join("deadbolt.db")).unwrap();
    let unset: Option<String> = conn
        .query_row(
            "SELECT tools_allow FROM leases WHERE agent_id='unrestricted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let empty: Option<String> = conn
        .query_row(
            "SELECT tools_allow FROM leases WHERE agent_id='deny-all'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unset, None);
    assert_eq!(empty, Some("[]".into()));
}
