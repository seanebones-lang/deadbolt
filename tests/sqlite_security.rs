//! Enforce the linked-library floor for RUSTSEC-2026-0459, not only Cargo metadata.
#[test]
fn linked_sqlite_contains_wal_page_initialization_fix() {
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "RUSTSEC-2026-0459 requires SQLite >= 3.54.0; actually linked {}",
        rusqlite::version()
    );
}
