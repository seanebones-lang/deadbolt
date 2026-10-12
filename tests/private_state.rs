#![cfg(unix)]

use deadbolt::{AdmitDecision, Deadbolt, DeadboltConfig, DenyCode, WitnessSink};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};

fn cfg(db: &std::path::Path, events: &std::path::Path) -> DeadboltConfig {
    DeadboltConfig {
        db_path: Some(db.to_string_lossy().into()),
        events_path: Some(events.to_string_lossy().into()),
        ..Default::default()
    }
}

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

#[test]
fn new_core_and_direct_witness_files_are_private_before_wal_writes() {
    let dir = private_dir();
    let db = dir.path().join("state/db");
    let events = dir.path().join("logs/events");
    let gate = Deadbolt::open(&cfg(&db, &events));
    gate.ensure_agent("run").unwrap();
    for file in [&db, &events, &db.with_extension("jsonl")] {
        if file.exists() {
            assert_eq!(mode(file), 0o600, "{}", file.display());
        }
    }
    for suffix in ["-wal", "-shm"] {
        assert_eq!(
            mode(&std::path::PathBuf::from(format!(
                "{}{suffix}",
                db.display()
            ))),
            0o600
        );
    }
    assert_eq!(mode(db.parent().unwrap()), 0o700);
    assert_eq!(mode(events.parent().unwrap()), 0o700);
    let witness_db = dir.path().join("witness/nodes.db");
    let witness = WitnessSink::open(&witness_db).unwrap();
    assert_eq!(mode(&witness_db), 0o600);
    assert_eq!(mode(witness.jsonl_path()), 0o600);
    for suffix in ["-wal", "-shm"] {
        assert_eq!(
            mode(&std::path::PathBuf::from(format!(
                "{}{suffix}",
                witness_db.display()
            ))),
            0o600
        );
    }
}

#[test]
fn unsafe_existing_files_are_refused_without_repair_or_state_mutation() {
    for target in ["db", "events", "db-wal", "db-shm", "db-journal"] {
        let dir = private_dir();
        let db = dir.path().join("db");
        let events = dir.path().join("events");
        let candidate = dir.path().join(target);
        fs::write(&candidate, b"sentinel").unwrap();
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o644)).unwrap();
        let gate = Deadbolt::open(&cfg(&db, &events));
        assert_eq!(
            gate.admit("run", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            },
            "{target}"
        );
        assert_eq!(fs::read(&candidate).unwrap(), b"sentinel");
        assert_eq!(mode(&candidate), 0o644);
    }
}

#[test]
fn leaf_symlinks_and_nonregular_targets_never_redirect_state_writes() {
    for target in ["db", "events", "db-wal", "db-shm", "db-journal"] {
        for is_link in [true, false] {
            let dir = private_dir();
            let db = dir.path().join("db");
            let events = dir.path().join("events");
            let sentinel = dir.path().join("sentinel");
            fs::write(&sentinel, b"unchanged").unwrap();
            let candidate = dir.path().join(target);
            if is_link {
                symlink(&sentinel, &candidate).unwrap();
            } else {
                fs::create_dir(&candidate).unwrap();
            }
            let gate = Deadbolt::open(&cfg(&db, &events));
            assert_eq!(
                gate.admit("run", "read"),
                AdmitDecision::Deny {
                    code: DenyCode::StoreUnavailable
                },
                "{target}"
            );
            assert_eq!(fs::read(&sentinel).unwrap(), b"unchanged");
        }
    }
}

#[test]
fn owned_private_files_reopen_under_nonwritable_shared_parent_and_alias() {
    let dir = private_dir();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let state = dir.path().join("state");
    let gate = Deadbolt::open_at(&state, true, 60);
    gate.ensure_agent("run").unwrap();
    gate.kill("run").unwrap();
    let before = fs::read(state.join("deadbolt-events.jsonl")).unwrap();
    let alias = dir.path().join("operator-alias");
    symlink(&state, &alias).unwrap();
    let reopened = Deadbolt::open_at(&alias, true, 60);
    assert_eq!(
        reopened.admit("run", "read"),
        AdmitDecision::Deny {
            code: DenyCode::Killed
        }
    );
    assert!(fs::read(state.join("deadbolt-events.jsonl"))
        .unwrap()
        .starts_with(&before));
}

#[test]
fn writable_parents_special_sqlite_targets_and_colliding_outputs_are_refused() {
    let dir = private_dir();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o777)).unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    assert_eq!(
        gate.admit("run", "read"),
        AdmitDecision::Deny {
            code: DenyCode::StoreUnavailable
        }
    );
    assert!(!dir.path().join("deadbolt.db").exists());
    assert!(WitnessSink::open(&dir.path().join("same.jsonl")).is_err());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(WitnessSink::open(&dir.path().join("same.jsonl")).is_err());
    for file in [":memory:", "file:unsafe.db?mode=memory"] {
        let db = dir.path().join(file);
        let gate = Deadbolt::open(&cfg(&db, &dir.path().join("events")));
        assert_eq!(
            gate.admit("run", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            }
        );
        assert!(!db.exists());
    }
}

#[test]
fn fifos_and_hardlinked_state_are_refused_without_opening_or_changing_targets() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    for target in ["db", "events", "db-wal", "db-shm", "db-journal"] {
        let dir = private_dir();
        let candidate = dir.path().join(target);
        let name = CString::new(candidate.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a live, NUL-terminated path to a disposable fixture.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let started = std::time::Instant::now();
        let gate = Deadbolt::open(&cfg(&dir.path().join("db"), &dir.path().join("events")));
        assert_eq!(
            gate.admit("run", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            }
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        fs::remove_file(&candidate).unwrap();
        let other = dir.path().join("alias");
        fs::write(&other, b"sentinel").unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&other, &candidate).unwrap();
        let gate = Deadbolt::open(&cfg(&dir.path().join("db"), &dir.path().join("events")));
        assert_eq!(
            gate.admit("run", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            }
        );
        assert_eq!(fs::read(&other).unwrap(), b"sentinel");
    }
}

#[test]
fn filesystem_case_aliases_cannot_mix_jsonl_and_sqlite_or_sidecars() {
    let probe = private_dir();
    fs::write(probe.path().join("CaseProbe"), b"x").unwrap();
    if !probe.path().join("caseprobe").exists() {
        return;
    }
    assert!(WitnessSink::open(&probe.path().join("nodes.JSONL")).is_err());
    for events_leaf in [
        "state.db",
        "state.db-WAL",
        "state.db-SHM",
        "state.db-JOURNAL",
    ] {
        let dir = private_dir();
        let db = dir.path().join("State.db");
        let events = dir.path().join(events_leaf);
        let gate = Deadbolt::open(&cfg(&db, &events));
        assert_eq!(
            gate.admit("run", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            },
            "{events_leaf}"
        );
        assert_eq!(fs::read(db).unwrap(), b"");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn extended_acl_grants_are_refused_for_parents_and_existing_state() {
    let grant =
        "everyone allow read,readattr,readextattr,readsecurity,file_inherit,directory_inherit";
    for leaf in ["parent", "db", "events", "db-wal", "db-shm", "db-journal"] {
        let dir = private_dir();
        let path = if leaf == "parent" {
            dir.path().to_path_buf()
        } else {
            let path = dir.path().join(leaf);
            fs::write(&path, b"").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            path
        };
        let entry = if leaf == "parent" {
            grant
        } else {
            "everyone allow read"
        };
        assert!(std::process::Command::new("/bin/chmod")
            .args(["+a", entry])
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let gate = Deadbolt::open(&cfg(&dir.path().join("db"), &dir.path().join("events")));
        assert_eq!(
            gate.admit("run", "read"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            },
            "{leaf}"
        );
        assert!(std::process::Command::new("/bin/chmod")
            .arg("-N")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let control = Deadbolt::open(&cfg(&dir.path().join("db"), &dir.path().join("events")));
        control.ensure_agent("run").unwrap();
    }
}
