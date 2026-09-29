use std::process::Command;

use deadbolt::{Deadbolt, DeadboltConfig};

#[test]
fn incident_json_flag_without_value_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("deadbolt.db");
    let db = Deadbolt::open(&DeadboltConfig {
        db_path: Some(db_path.display().to_string()),
        ..DeadboltConfig::default()
    });
    db.ensure_agent("A").unwrap();
    let bin = env!("CARGO_BIN_EXE_deadbolt");
    let bare = Command::new(bin)
        .args(["incident", "--agent", "A", "--json", "--children"])
        .env("DEADBOLT_DB", &db_path)
        .output()
        .unwrap();
    assert_eq!(
        bare.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&bare.stderr)
    );
    let body = String::from_utf8_lossy(&bare.stdout);
    assert!(body.contains("\"policy\""), "{body}");
    assert!(!body.contains("killed because"));
    let valued = Command::new(bin)
        .args([
            "incident",
            "--agent",
            "A",
            "--json",
            "true",
            "--children",
            "true",
        ])
        .env("DEADBOLT_DB", &db_path)
        .output()
        .unwrap();
    assert_eq!(valued.status.code(), Some(0));
}
