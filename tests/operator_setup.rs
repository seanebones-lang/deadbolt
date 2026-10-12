//! Real CLI provisioning contracts, including child inheritance and idle windows.
use deadbolt::{AdmitDecision, Deadbolt, DenyCode, PolicyPatch};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn operator(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_deadbolt"))
        .args(args)
        .env("DEADBOLT_DB", dir.join("deadbolt.db"))
        .env("DEADBOLT_EVENTS", dir.join("deadbolt-events.jsonl"))
        .output()
        .unwrap()
}

#[test]
fn cli_idle_window_is_bounded_and_does_not_refresh_existing_runs() {
    let dir = tempfile::tempdir().unwrap();
    for value in ["0", "86401", "18446744073709551615", "-1"] {
        assert!(!operator(
            dir.path(),
            &["--lease-ttl-secs", value, "ensure", "--agent", "bad"]
        )
        .status
        .success());
        assert!(!dir.path().join("deadbolt.db").exists());
    }
    let before = now();
    assert!(operator(
        dir.path(),
        &["ensure", "--agent", "setup", "--lease-ttl-secs", "120"]
    )
    .status
    .success());
    let gate = Deadbolt::open_at(dir.path(), true, 120);
    let original = gate.status(Some("setup")).unwrap()[0].expires_at;
    assert!(original >= before + 120 && original <= now() + 120);
    assert!(operator(
        dir.path(),
        &["--lease-ttl-secs", "600", "ensure", "--agent", "setup"]
    )
    .status
    .success());
    assert_eq!(gate.status(Some("setup")).unwrap()[0].expires_at, original);
}

#[test]
fn cli_registered_child_inherits_supervision_deadline_and_parent_policy() {
    let dir = tempfile::tempdir().unwrap();
    let deadline = (now() + 600).to_string();
    assert!(operator(
        dir.path(),
        &[
            "--lease-ttl-secs",
            "180",
            "supervise",
            "--agent",
            "root",
            "--hard-deadline-at",
            &deadline
        ]
    )
    .status
    .success());
    let gate = Deadbolt::open_at(dir.path(), true, 180);
    gate.set_policy(
        "root",
        PolicyPatch {
            tools_allow: Some(vec!["read".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let child = operator(
        dir.path(),
        &[
            "register-child",
            "--parent",
            "root",
            "--agent",
            "child",
            "--swarm-task-id",
            "task-1",
            "--lease-ttl-secs",
            "180",
        ],
    );
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert_eq!(
        String::from_utf8(child.stdout).unwrap().trim(),
        "agent=child parent=root live=true"
    );
    let row = &gate.status(Some("child")).unwrap()[0];
    assert_eq!(row.parent_id.as_deref(), Some("root"));
    assert_eq!(row.supervision_root.as_deref(), Some("root"));
    assert_eq!(row.hard_deadline_at, Some(deadline.parse().unwrap()));
    assert_eq!(row.swarm_task_id.as_deref(), Some("task-1"));
    assert_eq!(
        gate.admit("child", "write"),
        AdmitDecision::Deny {
            code: DenyCode::PurposeExceeded
        }
    );
    assert_eq!(gate.admit("child", "read"), AdmitDecision::Allow);
    gate.pause("root").unwrap();
    assert_eq!(
        gate.admit("child", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Paused
        }
    );
}

#[test]
fn inactive_child_registration_exits_unsuccessfully_and_cannot_be_reissued() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("root").unwrap();
    gate.kill("root").unwrap();
    let result = operator(
        dir.path(),
        &["register-child", "--parent", "root", "--agent", "child"],
    );
    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8(result.stderr).unwrap().trim(),
        "deadbolt:child_inactive"
    );
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("live=false"));
    assert_eq!(gate.status(Some("child")).unwrap()[0].state, "killed");
    assert!(operator(dir.path(), &["ensure", "--agent", "child"])
        .status
        .success());
    assert_eq!(
        gate.admit("child", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
}
