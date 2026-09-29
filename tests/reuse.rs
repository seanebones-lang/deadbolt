use deadbolt::{AdmitDecision, Deadbolt, DenyCode, PolicyPatch};
use std::sync::{Arc, Barrier};

#[test]
fn registering_existing_child_preserves_kill_and_parent() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("parent").unwrap();
    gate.ensure_agent("other").unwrap();
    assert!(gate.register_child("parent", "child", None).unwrap());
    gate.kill("child").unwrap();
    assert!(!gate.register_child("parent", "child", None).unwrap());
    assert_eq!(
        gate.admit("child", "shell"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
    assert!(gate.register_child("other", "child", None).is_err());
    assert!(gate.register_child("parent", "parent", None).is_err());
}

#[test]
fn inactive_parent_cannot_create_live_child() {
    for expired in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let gate = Deadbolt::open_at(dir.path(), true, 60);
        gate.ensure_agent("parent").unwrap();
        if expired {
            gate.force_expire("parent").unwrap();
        } else {
            gate.pause("parent").unwrap();
        }
        assert!(!gate.register_child("parent", "child", None).unwrap());
    }
}

#[test]
fn separate_connections_consume_approval_once() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("agent").unwrap();
    gate.set_policy(
        "agent",
        PolicyPatch {
            irreversible: Some(vec!["shell".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    gate.approve("agent", "shell").unwrap();
    let barrier = Arc::new(Barrier::new(16));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let other = Deadbolt::open_at(dir.path(), true, 60);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                other.admit("agent", "shell")
            })
        })
        .collect();
    let decisions: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        decisions
            .iter()
            .filter(|d| **d == AdmitDecision::Allow)
            .count(),
        1
    );
}

#[test]
fn concurrent_connections_preserve_evidence_and_spend() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("agent").unwrap();
    let barrier = Arc::new(Barrier::new(16));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let other = Deadbolt::open_at(dir.path(), true, 60);
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                other.spend_add("agent", 1.0).unwrap();
                assert_eq!(other.admit("agent", "shell"), AdmitDecision::Allow);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let total = gate.spend_add("agent", 0.0).unwrap();
    assert_eq!(total.spend_usd, 16.0);
    let jsonl = std::fs::read_to_string(dir.path().join("deadbolt-events.jsonl")).unwrap();
    for line in jsonl.lines() {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
    let rows = gate.export("agent", false).unwrap();
    assert_eq!(
        rows.iter()
            .filter(|r| r.kind == "decision" && r.decision.as_deref() == Some("allow"))
            .count(),
        16
    );
}
