//! Operator-issued, single-agent HTTP admission credentials.

use super::*;

/// Public credential metadata. Contains neither the bearer secret nor its hash.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CredentialStatus {
    /// Operator-selected identifier. Identifiers cannot be reused, even after revocation.
    pub credential_id: String,
    /// The only agent this credential may admit for.
    pub agent_id: String,
    /// Fixed Unix expiration time; admission never extends it.
    pub expires_at: i64,
    /// Revocation time, if the operator revoked this credential.
    pub revoked_at: Option<i64>,
}

impl Deadbolt {
    pub(crate) fn has_credentials(&self) -> Result<bool, DeadboltError> {
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        g.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM admission_credentials)",
                [],
                |r| r.get(0),
            )
            .map_err(|_| DeadboltError::StoreUnavailable)
    }

    /// Issue an admission-only bearer credential to a new file, without printing it.
    /// Requires an existing live, non-killed lease. TTL is 1..=86400 seconds.
    /// Existing files (including symlinks) are refused. Unix files are mode 0600;
    /// on Windows the containing directory must have an operator-protected ACL.
    /// Only the SHA-256 hash is stored in SQLite. Protect the file and database
    /// from the workload: possession of the database remains operator authority.
    pub fn issue_credential(
        &self,
        agent_id: &str,
        credential_id: &str,
        ttl_secs: u64,
        out: &Path,
    ) -> Result<CredentialStatus, DeadboltError> {
        require_token(agent_id)?;
        require_token(credential_id)?;
        if !self.enabled || !(1..=86400).contains(&ttl_secs) {
            return Err(DeadboltError::BadRequest);
        }
        let mut entropy = [0u8; 32];
        getrandom::fill(&mut entropy).map_err(|_| DeadboltError::StoreUnavailable)?;
        let secret = format!("dbw1-{}", hex_encode(&entropy));
        let hash = credential_hash(&secret)?;
        let parent = out
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| DeadboltError::ExportRefused("credential_file"))?;
        file.write_all(secret.as_bytes())
            .and_then(|()| file.as_file().sync_all())
            .map_err(|_| DeadboltError::ExportRefused("credential_file"))?;
        // Publish before activating: a concurrent read cannot authorize until
        // the database transaction commits. Never replace an existing path.
        file.persist_noclobber(out)
            .map_err(|_| DeadboltError::ExportRefused("credential_file"))?;
        let expires_at = now_secs() + ttl_secs as i64;
        let issued = self.operator_grant(
            "credential_issue",
            json!({"agent_id":agent_id,"credential_id":credential_id,"expires_at":expires_at}),
            |tx| {
                let lease = load_lease(tx, agent_id)
                    .map_err(|_| DeadboltError::StoreUnavailable)?
                    .ok_or(DeadboltError::NotFound)?;
                if lease.state == "killed" {
                    return Err(DeadboltError::Killed);
                }
                if now_secs() >= lease.expires_at {
                    return Err(DeadboltError::BadRequest);
                }
                tx.execute(
                    "INSERT INTO admission_credentials (credential_id,agent_id,token_hash,expires_at,created_at) VALUES (?1,?2,?3,?4,?5)",
                    params![credential_id, agent_id, hash, expires_at, now_secs()],
                ).map_err(|e| match e {
                    rusqlite::Error::SqliteFailure(ref detail, _) if detail.code == rusqlite::ErrorCode::ConstraintViolation => DeadboltError::BadRequest,
                    _ => DeadboltError::StoreUnavailable,
                })?;
                Ok(())
            },
        );
        if let Err(e) = issued {
            // Primary failure rolls back. A custom/Witness sink may fail after
            // commit: retain the operator's file if activation may have occurred.
            let active = self.store().and_then(|store| {
                let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
                g.conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM admission_credentials WHERE token_hash=?1)",
                        params![hash],
                        |r| r.get::<_, bool>(0),
                    )
                    .map_err(|_| DeadboltError::StoreUnavailable)
            });
            if active == Ok(false) {
                let _ = fs::remove_file(out);
            }
            return Err(e);
        }
        Ok(CredentialStatus {
            credential_id: credential_id.into(),
            agent_id: agent_id.into(),
            expires_at,
            revoked_at: None,
        })
    }

    /// Revoke one credential without killing its lease or other credentials.
    /// Serialized with credential validation and admission by the SQLite writer transaction.
    pub fn revoke_credential(&self, credential_id: &str) -> Result<(), DeadboltError> {
        require_token(credential_id)?;
        // Credential IDs are immutable and never reused. Read the binding for
        // agent-specific incident/export linkage; mutation still owns its transaction.
        let agent_id: String = {
            let store = self.store()?;
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            g.conn
                .query_row(
                    "SELECT agent_id FROM admission_credentials WHERE credential_id=?1",
                    params![credential_id],
                    |r| r.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => DeadboltError::NotFound,
                    _ => DeadboltError::StoreUnavailable,
                })?
        };
        self.operator_grant("credential_revoke", json!({"credential_id":credential_id,"agent_id":agent_id}), |tx| {
            let count = tx.execute(
                "UPDATE admission_credentials SET revoked_at=COALESCE(revoked_at,?1) WHERE credential_id=?2",
                params![now_secs(), credential_id],
            ).map_err(|_| DeadboltError::StoreUnavailable)?;
            if count == 0 { return Err(DeadboltError::NotFound); }
            Ok(())
        })
    }

    /// Read credential metadata without renewing leases or disclosing secrets.
    pub fn credentials(&self) -> Result<Vec<CredentialStatus>, DeadboltError> {
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let mut stmt = g.conn.prepare("SELECT credential_id,agent_id,expires_at,revoked_at FROM admission_credentials ORDER BY credential_id")
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CredentialStatus {
                    credential_id: row.get(0)?,
                    agent_id: row.get(1)?,
                    expires_at: row.get(2)?,
                    revoked_at: row.get(3)?,
                })
            })
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| DeadboltError::StoreUnavailable)
    }

    /// Admit using a bearer credential bound to this agent. Authentication and
    /// lease/policy/one-shot evaluation share one writer transaction. Always fails
    /// closed, including disabled enforcement or unavailable storage. Does not
    /// grant any operator control or cancel already admitted work. Invalid bearer
    /// credentials return [`DeadboltError::TokenRequired`] (HTTP maps this to 401).
    pub fn admit_credential(
        &self,
        secret: &str,
        agent_id: &str,
        tool: &str,
        dest: Option<&str>,
    ) -> Result<AdmitDecision, DeadboltError> {
        let hash = credential_hash(secret)?;
        if !self.enabled {
            return Err(DeadboltError::Disabled);
        }
        if !is_token(agent_id) || !is_token(tool) {
            return Err(DeadboltError::BadRequest);
        }
        let store = self.store()?;
        let decision = {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            let valid: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM admission_credentials WHERE token_hash=?1 AND agent_id=?2 AND expires_at>?3 AND revoked_at IS NULL)",
                params![hash, agent_id, now_secs()], |row| row.get(0),
            ).map_err(|_| DeadboltError::StoreUnavailable)?;
            if !valid {
                return Err(DeadboltError::TokenRequired);
            }
            let decision = self
                .evaluate_in_transaction(&tx, agent_id, tool, dest, false)
                .map_err(|_| DeadboltError::StoreUnavailable)?;
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
            decision
        };
        self.record_decision(agent_id, tool, &decision)
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(decision)
    }
}

fn credential_hash(secret: &str) -> Result<String, DeadboltError> {
    if secret.len() != 69
        || !secret.starts_with("dbw1-")
        || !secret.as_bytes()[5..]
            .iter()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(DeadboltError::TokenRequired);
    }
    Ok(hex_encode(&Sha256::digest(secret.as_bytes())))
}

pub(crate) fn authenticate(
    tx: &rusqlite::Transaction<'_>,
    secret: &str,
    agent_id: &str,
) -> Result<(), DeadboltError> {
    let hash = credential_hash(secret)?;
    let valid:bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM admission_credentials WHERE token_hash=?1 AND agent_id=?2 AND expires_at>?3 AND revoked_at IS NULL)", params![hash,agent_id,now_secs()], |r| r.get(0)).map_err(|_|DeadboltError::StoreUnavailable)?;
    if valid {
        Ok(())
    } else {
        Err(DeadboltError::TokenRequired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Deadbolt) {
        let dir = tempfile::tempdir().unwrap();
        let db = Deadbolt::open_paths(
            &dir.path().join("db"),
            &dir.path().join("events"),
            true,
            true,
            60,
        );
        db.ensure_agent("A").unwrap();
        (dir, db)
    }

    fn issue(dir: &tempfile::TempDir, db: &Deadbolt, id: &str) -> String {
        let path = dir.path().join(id);
        db.issue_credential("A", id, 3600, &path).unwrap();
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn credentials_bind_agent_persist_and_revoke_independently() {
        let (dir, db) = fixture();
        db.ensure_agent("B").unwrap();
        let secret = issue(&dir, &db, "key-one");
        let second = issue(&dir, &db, "key-two");
        assert_eq!(
            db.admit_credential(&secret, "B", "read", None),
            Err(DeadboltError::TokenRequired)
        );
        assert_eq!(
            db.admit_credential(&secret, "A", "read", None),
            Ok(AdmitDecision::Allow)
        );
        let reopened = Deadbolt::open_paths(
            &dir.path().join("db"),
            &dir.path().join("events"),
            true,
            true,
            60,
        );
        reopened.revoke_credential("key-one").unwrap();
        assert_eq!(
            db.admit_credential(&secret, "A", "read", None),
            Err(DeadboltError::TokenRequired)
        );
        assert_eq!(
            db.admit_credential(&second, "A", "read", None),
            Ok(AdmitDecision::Allow)
        );
        db.kill("A").unwrap();
        assert_eq!(
            db.admit_credential(&second, "A", "read", None),
            Ok(AdmitDecision::Deny {
                code: DenyCode::Killed
            })
        );
        assert!(db.credentials().unwrap()[0].revoked_at.is_some());
        let exported = db.export("A", false).unwrap();
        assert!(exported.iter().any(|row| row.kind == "credential_issue"));
        assert!(exported.iter().any(|row| row.kind == "credential_revoke"));
    }

    #[test]
    fn credential_expiration_does_not_extend_or_consume_approval() {
        let (dir, db) = fixture();
        let secret = issue(&dir, &db, "key");
        db.set_policy(
            "A",
            PolicyPatch {
                irreversible: Some(vec!["send".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        db.approve("A", "send").unwrap();
        let before = db.status(Some("A")).unwrap()[0].expires_at;
        let conn = Connection::open(dir.path().join("db")).unwrap();
        conn.execute("UPDATE admission_credentials SET expires_at=0", [])
            .unwrap();
        assert_eq!(
            db.admit_credential(&secret, "A", "send", None),
            Err(DeadboltError::TokenRequired)
        );
        assert_eq!(db.status(Some("A")).unwrap()[0].expires_at, before);
        assert_eq!(db.admit("A", "send"), AdmitDecision::Allow);
        assert_eq!(
            db.admit("A", "send"),
            AdmitDecision::Deny {
                code: DenyCode::NeedsHuman
            }
        );
    }

    #[test]
    fn secret_output_is_exclusive_and_evidence_contains_no_bearer() {
        let (dir, db) = fixture();
        let secret = issue(&dir, &db, "key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path().join("key"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            std::os::unix::fs::symlink(dir.path().join("key"), dir.path().join("link")).unwrap();
            assert!(db
                .issue_credential("A", "link", 60, &dir.path().join("link"))
                .is_err());
        }
        assert!(db
            .issue_credential("A", "other", 60, &dir.path().join("key"))
            .is_err());
        assert_eq!(fs::read_to_string(dir.path().join("key")).unwrap(), secret);
        db.revoke_credential("key").unwrap();
        assert!(db
            .issue_credential("A", "key", 60, &dir.path().join("fresh"))
            .is_err());
        assert!(!dir.path().join("fresh").exists());
        let public = serde_json::to_string(&db.credentials().unwrap()).unwrap();
        assert!(!public.contains(&secret));
        assert!(!public.contains("token_hash"));
        assert!(!fs::read_to_string(dir.path().join("events"))
            .unwrap()
            .contains(&secret));
        let conn = Connection::open(dir.path().join("db")).unwrap();
        let stored: String = conn
            .query_row("SELECT token_hash FROM admission_credentials", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(stored, credential_hash(&secret).unwrap());
        assert_ne!(stored, secret);
    }

    #[test]
    fn failed_primary_evidence_rolls_back_issuance_and_removes_output() {
        let (dir, db) = fixture();
        let conn = Connection::open(dir.path().join("db")).unwrap();
        conn.execute_batch("CREATE TRIGGER fail_issue BEFORE INSERT ON events WHEN NEW.kind='credential_issue' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert_eq!(
            db.issue_credential("A", "key", 60, &dir.path().join("key")),
            Err(DeadboltError::StoreUnavailable)
        );
        assert!(!dir.path().join("key").exists());
        assert!(db.credentials().unwrap().is_empty());
    }

    #[test]
    fn post_commit_witness_failure_preserves_file_for_reconciliation() {
        struct Failed;
        impl EvidenceSink for Failed {
            fn append(&self, _: &EvidenceRecord) -> Result<(), SinkError> {
                Err(SinkError::Unavailable)
            }
        }
        let (dir, mut db) = fixture();
        db.witness = Some(Arc::new(Failed));
        assert_eq!(
            db.issue_credential("A", "key", 60, &dir.path().join("key")),
            Err(DeadboltError::StoreUnavailable)
        );
        assert!(dir.path().join("key").exists());
        assert_eq!(db.credentials().unwrap().len(), 1);
        let reopened = Deadbolt::open_paths(
            &dir.path().join("db"),
            &dir.path().join("events"),
            true,
            true,
            60,
        );
        reopened.revoke_credential("key").unwrap();
        assert_eq!(
            reopened.admit_credential(
                &fs::read_to_string(dir.path().join("key")).unwrap(),
                "A",
                "read",
                None
            ),
            Err(DeadboltError::TokenRequired)
        );
    }

    #[test]
    fn scoped_admission_never_uses_disabled_or_fail_open_configuration() {
        let (dir, db) = fixture();
        let secret = issue(&dir, &db, "key");
        let disabled = Deadbolt::open_paths(
            &dir.path().join("db"),
            &dir.path().join("events"),
            false,
            false,
            60,
        );
        assert_eq!(
            disabled.admit_credential(&secret, "A", "read", None),
            Err(DeadboltError::Disabled)
        );
        fs::write(dir.path().join("blocked"), b"x").unwrap();
        let unavailable = Deadbolt::open_paths(
            &dir.path().join("blocked/db"),
            &dir.path().join("blocked/events"),
            true,
            false,
            60,
        );
        assert_eq!(
            unavailable.admit_credential(&secret, "A", "read", None),
            Err(DeadboltError::StoreUnavailable)
        );
        assert_eq!(unavailable.admit("A", "read"), AdmitDecision::Allow);
    }

    #[test]
    fn scoped_independent_connections_consume_only_one_approval() {
        let (dir, db) = fixture();
        db.set_policy(
            "A",
            PolicyPatch {
                irreversible: Some(vec!["send".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        db.approve("A", "send").unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let mut inputs = Vec::new();
        for n in 0..8 {
            let secret = issue(&dir, &db, &format!("key-{n}"));
            let other = Deadbolt::open_paths(
                &dir.path().join("db"),
                &dir.path().join("events"),
                true,
                true,
                60,
            );
            inputs.push((secret, other));
        }
        let threads: Vec<_> = inputs
            .into_iter()
            .map(|(secret, other)| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    other.admit_credential(&secret, "A", "send", None).unwrap()
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
        assert_eq!(
            decisions
                .iter()
                .filter(|d| **d
                    == AdmitDecision::Deny {
                        code: DenyCode::NeedsHuman
                    })
                .count(),
            7
        );
    }

    #[test]
    fn expired_lease_cannot_be_rearmed_by_issuance() {
        let (dir, db) = fixture();
        let secret = issue(&dir, &db, "key");
        let conn = Connection::open(dir.path().join("db")).unwrap();
        conn.execute("UPDATE leases SET expires_at=0", []).unwrap();
        assert_eq!(
            db.admit_credential(&secret, "A", "read", None),
            Ok(AdmitDecision::Deny {
                code: DenyCode::LeaseExpired
            })
        );
        assert!(db
            .issue_credential("A", "new", 60, &dir.path().join("new"))
            .is_err());
        assert!(!dir.path().join("new").exists());
        assert_eq!(db.status(Some("A")).unwrap()[0].expires_at, 0);
        assert!(db
            .issue_credential("A", "zero", 0, &dir.path().join("zero"))
            .is_err());
        assert!(db
            .issue_credential("A", "huge", u64::MAX, &dir.path().join("huge"))
            .is_err());
    }

    #[test]
    fn independent_writer_revocation_is_checked_after_lock_acquisition() {
        let (dir, db) = fixture();
        let secret = issue(&dir, &db, "key");
        let other = Deadbolt::open_paths(
            &dir.path().join("db"),
            &dir.path().join("events"),
            true,
            true,
            60,
        );
        let mut conn = Connection::open(dir.path().join("db")).unwrap();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        tx.execute("UPDATE admission_credentials SET revoked_at=1", [])
            .unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let candidate = secret.clone();
        let blocked = std::thread::spawn(move || {
            sent.send(()).unwrap();
            other.admit_credential(&candidate, "A", "read", None)
        });
        received.recv().unwrap();
        tx.commit().unwrap();
        assert_eq!(blocked.join().unwrap(), Err(DeadboltError::TokenRequired));
        let mut threads = Vec::new();
        for _ in 0..8 {
            let db = db.clone();
            let secret = secret.clone();
            threads.push(std::thread::spawn(move || {
                db.admit_credential(&secret, "A", "read", None)
            }));
        }
        for thread in threads {
            assert_eq!(thread.join().unwrap(), Err(DeadboltError::TokenRequired));
        }
    }
}
