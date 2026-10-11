//! Operator-reviewed exact actions and atomic single-use admission.

use super::*;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use std::fmt;

const MAX_ACTION_BYTES: usize = 32768;
const MAX_SAFE_INTEGER: f64 = 9007199254740991.0;

/// A versioned request for one exact action, prepared by the trusted executor.
/// This is public review data, not an authorization token. Approval belongs to
/// the operator. Protect its arguments from logs and bind them to actual execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    /// Protocol version. Only 1 is accepted.
    pub version: u8,
    /// Executor-assigned run identity.
    pub agent_id: String,
    /// Trusted routed tool identity, including any application-controlled version.
    pub tool: String,
    /// Actual destination host token; null for local effects.
    pub dest: Option<String>,
    /// Validated effect-affecting arguments. Must be a JSON object.
    pub arguments: Value,
    /// Executor-assigned unique token. Never reuse within this agent/store.
    pub nonce: String,
    /// Fixed Unix deadline, also included in the reviewed fingerprint.
    pub expires_at: i64,
}

impl ActionRequest {
    /// Prepare a request with OS-random nonce and a 1..=86400 second lifetime.
    /// Does not create a lease, grant permission, renew state or write evidence.
    pub fn new(
        agent_id: &str,
        tool: &str,
        dest: Option<&str>,
        arguments: Value,
        ttl_secs: u64,
    ) -> Result<Self, DeadboltError> {
        if !(1..=86400).contains(&ttl_secs) {
            return Err(DeadboltError::BadRequest);
        }
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| DeadboltError::StoreUnavailable)?;
        let action = Self {
            version: 1,
            agent_id: agent_id.into(),
            tool: tool.into(),
            dest: dest.map(str::to_string),
            arguments,
            nonce: hex_encode(&bytes),
            expires_at: now_secs() + ttl_secs as i64,
        };
        action.fingerprint()?;
        Ok(action)
    }

    /// Parse bounded JSON, rejecting duplicate keys anywhere and unknown fields.
    /// Use this for operator review files and HTTP action requests.
    pub fn from_json(raw: &str) -> Result<Self, DeadboltError> {
        if raw.len() > MAX_ACTION_BYTES {
            return Err(DeadboltError::BadRequest);
        }
        let value = strict_json(raw)?;
        let action: Self = serde_json::from_value(value).map_err(|_| DeadboltError::BadRequest)?;
        action.fingerprint()?;
        Ok(action)
    }

    /// Domain-separated SHA-256 over the RFC 8785 canonical envelope. Object
    /// order/JSON whitespace do not affect it. Unsafe integers, non-finite numbers,
    /// excessive size/depth and unknown versions are refused. Money/high-precision
    /// values should use explicit decimal strings or safe integer minor units.
    pub fn fingerprint(&self) -> Result<String, DeadboltError> {
        if self.version != 1
            || !is_token(&self.agent_id)
            || !is_token(&self.tool)
            || !is_token(&self.nonce)
            || self.dest.as_ref().is_some_and(|s| !is_token(s))
            || !self.arguments.is_object()
            || self.expires_at <= 0
            || self.expires_at as f64 > MAX_SAFE_INTEGER
        {
            return Err(DeadboltError::BadRequest);
        }
        let mut budget = 4096;
        validate_value(&self.arguments, 0, &mut budget)?;
        let bytes = serde_jcs::to_vec(self).map_err(|_| DeadboltError::BadRequest)?;
        if bytes.len() > MAX_ACTION_BYTES {
            return Err(DeadboltError::BadRequest);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"deadbolt-action-v1\0");
        hasher.update(bytes);
        Ok(format!("sha256:{}", hex_encode(&hasher.finalize())))
    }
}

impl Deadbolt {
    /// Require exact approval for this agent/tool, closing the legacy admit and
    /// broad-approval paths. Does not renew a lease. The operator can explicitly
    /// restore legacy behavior with `required=false`; this does not erase grants.
    pub fn require_exact_action(
        &self,
        agent_id: &str,
        tool: &str,
        required: bool,
    ) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        require_token(tool)?;
        self.operator_grant("action_requirement",json!({"agent_id":agent_id,"tool":tool,"required":required}),|tx|{
            let lease=load_lease(tx,agent_id).map_err(|_|DeadboltError::StoreUnavailable)?.ok_or(DeadboltError::NotFound)?;
            if lease.state=="killed" { return Err(DeadboltError::Killed); }
            if required {
                tx.execute("INSERT OR IGNORE INTO exact_action_requirements (agent_id,tool) VALUES (?1,?2)",params![agent_id,tool])
            } else {
                tx.execute("DELETE FROM exact_action_requirements WHERE agent_id=?1 AND tool=?2",params![agent_id,tool])
            }.map_err(|_|DeadboltError::StoreUnavailable)?;
            Ok(())
        })
    }

    /// Approve the reviewed envelope once and atomically require exact admission
    /// for its tool. Existing policy/lease checks still apply at execution time.
    /// Nonces cannot be reused, even after consumption/revocation/expiry. No raw
    /// arguments are stored in grant rows or evidence. Primary evidence failure
    /// rolls back the grant with the default SQLite sink.
    pub fn approve_action(&self, action: &ActionRequest) -> Result<(), DeadboltError> {
        let fingerprint = action.fingerprint()?;
        self.operator_grant("action_approve",json!({"agent_id":action.agent_id,"tool":action.tool,"nonce":action.nonce,"fingerprint":fingerprint,"expires_at":action.expires_at}),|tx|{
            // Validate time after acquiring the writer lock, not before waiting
            // for another connection. Never report a newly expired review approved.
            let now = now_secs();
            if action.expires_at <= now || action.expires_at > now + 86400 {
                return Err(DeadboltError::BadRequest);
            }
            let lease=load_lease(tx,&action.agent_id).map_err(|_|DeadboltError::StoreUnavailable)?.ok_or(DeadboltError::NotFound)?;
            if lease.state=="killed" { return Err(DeadboltError::Killed); }
            if now>=lease.expires_at { return Err(DeadboltError::BadRequest); }
            tx.execute("INSERT INTO action_grants (agent_id,nonce,tool,fingerprint,expires_at,created_at) VALUES (?1,?2,?3,?4,?5,?6)",params![action.agent_id,action.nonce,action.tool,fingerprint,action.expires_at,now])
                .map_err(|e|match e {rusqlite::Error::SqliteFailure(ref detail,_) if detail.code==rusqlite::ErrorCode::ConstraintViolation=>DeadboltError::BadRequest,_=>DeadboltError::StoreUnavailable})?;
            tx.execute("INSERT OR IGNORE INTO exact_action_requirements (agent_id,tool) VALUES (?1,?2)",params![action.agent_id,action.tool]).map_err(|_|DeadboltError::StoreUnavailable)?;
            Ok(())
        })
    }

    /// Revoke one exact-action grant. Does not remove exact-only tool policy.
    pub fn revoke_action(&self, agent_id: &str, nonce: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        require_token(nonce)?;
        self.operator_grant("action_revoke",json!({"agent_id":agent_id,"nonce":nonce}),|tx|{
            let changed=tx.execute("UPDATE action_grants SET revoked_at=COALESCE(revoked_at,?1) WHERE agent_id=?2 AND nonce=?3",params![now_secs(),agent_id,nonce]).map_err(|_|DeadboltError::StoreUnavailable)?;
            if changed==0 { return Err(DeadboltError::NotFound); } Ok(())
        })
    }

    /// Admit the exact envelope then pass its copied arguments to a trusted body.
    /// The body must use these arguments and executor-controlled tool routing.
    /// Denial/errors never invoke it; tool returns/errors/panics are preserved.
    pub fn dispatch_action<T>(
        &self,
        action: &ActionRequest,
        body: impl FnOnce(Value) -> T,
    ) -> Result<T, DenyCode> {
        match self.admit_action(action) {
            Ok(AdmitDecision::Allow) => Ok(body(action.arguments.clone())),
            Ok(AdmitDecision::Deny { code }) => Err(code),
            Err(_) => Err(DenyCode::StoreUnavailable),
        }
    }

    /// Check and consume one matching reviewed action. Always fails closed.
    /// Caller must execute these exact arguments immediately after allow; no
    /// cached allow, implicit retries, cancellation or OS isolation is provided.
    pub fn admit_action(&self, action: &ActionRequest) -> Result<AdmitDecision, DeadboltError> {
        self.admit_action_inner(action, None)
    }

    /// Exact-action admission using a single-agent workload credential. Identity,
    /// credential validity, policy and grant consumption share one transaction.
    pub fn admit_action_credential(
        &self,
        secret: &str,
        action: &ActionRequest,
    ) -> Result<AdmitDecision, DeadboltError> {
        self.admit_action_inner(action, Some(secret))
    }

    fn admit_action_inner(
        &self,
        action: &ActionRequest,
        secret: Option<&str>,
    ) -> Result<AdmitDecision, DeadboltError> {
        let fingerprint = action.fingerprint()?;
        if !self.enabled {
            return Err(DeadboltError::Disabled);
        }
        let store = self.store()?;
        let default_sink = self.uses_default_sink(&store);
        let payload = json!({"agent_id":action.agent_id,"tool":action.tool,"nonce":action.nonce,"fingerprint":fingerprint});
        let (decision, record, consumed) = {
            let mut g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let StoreInner { conn, events } = &mut *g;
            let tx = rusqlite::Transaction::new_unchecked(
                conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            if let Some(secret) = secret {
                credentials::authenticate(&tx, secret, &action.agent_id)?;
            }
            let mut decision = self
                .evaluate_in_transaction(
                    &tx,
                    &action.agent_id,
                    &action.tool,
                    action.dest.as_deref(),
                    true,
                )
                .map_err(|_| DeadboltError::StoreUnavailable)?;
            let mut record = None;
            let mut consumed = false;
            if decision == AdmitDecision::Allow {
                let changed=tx.execute("UPDATE action_grants SET consumed_at=?1 WHERE agent_id=?2 AND nonce=?3 AND fingerprint=?4 AND expires_at>?1 AND revoked_at IS NULL AND consumed_at IS NULL",params![now_secs(),action.agent_id,action.nonce,fingerprint]).map_err(|_|DeadboltError::StoreUnavailable)?;
                if changed == 1 {
                    consumed = true;
                    if default_sink {
                        record = Some(
                            emit_in_transaction(
                                &tx,
                                events,
                                EpistemicClass::Observed,
                                "action_consume",
                                payload.clone(),
                                &[],
                            )
                            .map_err(|_| DeadboltError::StoreUnavailable)?,
                        );
                    }
                } else {
                    decision = AdmitDecision::Deny {
                        code: DenyCode::NeedsHuman,
                    };
                }
            }
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
            (decision, record, consumed)
        };
        if let Some(record) = record {
            self.append_witness(&record)
                .map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        if consumed && !default_sink {
            self.emit(EpistemicClass::Observed, "action_consume", payload, &[])
                .map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        self.record_decision(&action.agent_id, &action.tool, &decision)
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(decision)
    }
}

fn validate_value(value: &Value, depth: usize, budget: &mut usize) -> Result<(), DeadboltError> {
    if depth > 32 || *budget == 0 {
        return Err(DeadboltError::BadRequest);
    }
    *budget -= 1;
    match value {
        Value::Number(n)
            if n.as_f64().is_none_or(|x| {
                !x.is_finite() || x.abs() > MAX_SAFE_INTEGER || (x == 0.0 && x.is_sign_negative())
            }) =>
        {
            Err(DeadboltError::BadRequest)
        }
        Value::String(s) if s.len() > MAX_ACTION_BYTES => Err(DeadboltError::BadRequest),
        Value::Array(a) => a
            .iter()
            .try_for_each(|v| validate_value(v, depth + 1, budget)),
        Value::Object(o) => o.iter().try_for_each(|(k, v)| {
            if k.len() > MAX_ACTION_BYTES {
                return Err(DeadboltError::BadRequest);
            }
            validate_value(v, depth + 1, budget)
        }),
        _ => Ok(()),
    }
}

// serde_json::Value normally accepts duplicate keys, keeping the last. Exact
// review input must instead have only one interpretation at every nesting level.
struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StrictValue(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StrictValue(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(StrictValue(
                    serde_json::Number::from_f64(v)
                        .ok_or_else(|| E::custom("invalid number"))?
                        .into(),
                ))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StrictValue(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(StrictValue(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                self.visit_unit()
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(StrictValue(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(StrictValue(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut out = Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    let StrictValue(v) = a.next_value()?;
                    out.insert(k, v);
                }
                Ok(StrictValue(Value::Object(out)))
            }
        }
        d.deserialize_any(StrictVisitor)
    }
}

pub(crate) fn strict_json(raw: &str) -> Result<Value, DeadboltError> {
    serde_json::from_str::<StrictValue>(raw)
        .map(|s| s.0)
        .map_err(|_| DeadboltError::BadRequest)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Deadbolt, ActionRequest) {
        let dir = tempfile::tempdir().unwrap();
        let db = Deadbolt::open_at(dir.path(), true, 60);
        db.ensure_agent("run").unwrap();
        let request = ActionRequest::new(
            "run",
            "send",
            Some("example.com"),
            json!({"to":"reviewed@example.com","body":"private review body","amount_minor":100}),
            300,
        )
        .unwrap();
        (dir, db, request)
    }
    fn human() -> AdmitDecision {
        AdmitDecision::Deny {
            code: DenyCode::NeedsHuman,
        }
    }
    #[test]
    fn exact_arguments_identity_and_deadline_then_single_use() {
        let (_dir, db, request) = setup();
        db.ensure_agent("other").unwrap();
        db.approve_action(&request).unwrap();
        for field in [
            "to",
            "body",
            "amount_minor",
            "dest",
            "agent",
            "tool",
            "nonce",
            "expiry",
        ] {
            let mut changed = request.clone();
            match field {
                "to" => changed.arguments["to"] = json!("other@example.com"),
                "body" => changed.arguments["body"] = json!("unreviewed"),
                "amount_minor" => changed.arguments["amount_minor"] = json!(1000),
                "dest" => changed.dest = Some("other.com".into()),
                "agent" => changed.agent_id = "other".into(),
                "tool" => changed.tool = "other".into(),
                "nonce" => changed.nonce = "other".into(),
                _ => changed.expires_at += 1,
            }
            assert_eq!(db.admit_action(&changed).unwrap(), human(), "{field}");
        }
        assert_eq!(db.admit("run", "send"), human());
        assert_eq!(db.probe("run", "send"), human());
        assert!(matches!(
            db.approve("run", "send"),
            Err(DeadboltError::BadRequest)
        ));
        assert_eq!(db.admit_action(&request).unwrap(), AdmitDecision::Allow);
        assert_eq!(db.admit_action(&request).unwrap(), human());
        assert!(db.approve_action(&request).is_err());
    }
    #[test]
    fn legacy_approval_cannot_bypass_requirement() {
        let (_dir, db, request) = setup();
        db.set_policy(
            "run",
            PolicyPatch {
                irreversible: Some(vec!["send".into()]),
                ..Default::default()
            },
        )
        .unwrap();
        db.approve("run", "send").unwrap();
        db.require_exact_action("run", "send", true).unwrap();
        assert_eq!(db.admit("run", "send"), human());
        db.approve_action(&request).unwrap();
        assert_eq!(db.admit_action(&request).unwrap(), AdmitDecision::Allow);
        assert_eq!(db.admit("run", "send"), human());
        db.require_exact_action("run", "send", false).unwrap();
        assert_eq!(db.admit("run", "send"), AdmitDecision::Allow);
    }
    #[test]
    fn policies_pause_kill_expiry_and_revocation_are_rechecked() {
        for mode in ["policy", "pause", "kill", "expiry", "revoke"] {
            let (_dir, db, request) = setup();
            db.approve_action(&request).unwrap();
            match mode {
                "policy" => db
                    .set_policy(
                        "run",
                        PolicyPatch {
                            tools_allow: Some(vec!["read".into()]),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                "pause" => db.pause("run").unwrap(),
                "kill" => {
                    db.kill("run").unwrap();
                }
                "expiry" => {
                    db.store
                        .as_ref()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .conn
                        .execute("UPDATE action_grants SET expires_at=0", [])
                        .unwrap();
                }
                _ => db.revoke_action("run", &request.nonce).unwrap(),
            }
            assert_ne!(
                db.admit_action(&request).unwrap(),
                AdmitDecision::Allow,
                "{mode}"
            );
            let g = db.store.as_ref().unwrap().lock().unwrap();
            let consumed: Option<i64> = g
                .conn
                .query_row("SELECT consumed_at FROM action_grants", [], |r| r.get(0))
                .unwrap();
            assert_eq!(consumed, None, "{mode}");
        }
    }
    #[test]
    fn invalid_workload_identity_does_not_consume_grant() {
        let (dir, db, request) = setup();
        db.ensure_agent("other").unwrap();
        db.issue_credential("other", "other-key", 300, &dir.path().join("key"))
            .unwrap();
        let secret = std::fs::read_to_string(dir.path().join("key")).unwrap();
        db.approve_action(&request).unwrap();
        assert!(matches!(
            db.admit_action_credential(&secret, &request),
            Err(DeadboltError::TokenRequired)
        ));
        db.issue_credential("run", "run-key", 300, &dir.path().join("run-key"))
            .unwrap();
        let valid = std::fs::read_to_string(dir.path().join("run-key")).unwrap();
        assert_eq!(
            db.admit_action_credential(&valid, &request).unwrap(),
            AdmitDecision::Allow
        );
        assert_eq!(
            db.admit_action_credential(&valid, &request).unwrap(),
            human()
        );
    }
    #[test]
    fn independent_connections_consume_once_and_restart_preserves_mode() {
        let (dir, db, request) = setup();
        db.approve_action(&request).unwrap();
        let other = Deadbolt::open_at(dir.path(), true, 60);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let b = barrier.clone();
        let r = request.clone();
        let effect = dir.path().join("concurrent-effect");
        let other_effect = effect.clone();
        let thread = std::thread::spawn(move || {
            b.wait();
            other.dispatch_action(&r, |arguments| {
                std::fs::write(other_effect, arguments["body"].as_str().unwrap()).unwrap();
                42
            })
        });
        barrier.wait();
        let first = db.dispatch_action(&request, |arguments| {
            std::fs::write(&effect, arguments["body"].as_str().unwrap()).unwrap();
            42
        });
        let second = thread.join().unwrap();
        assert_eq!([first, second].iter().filter(|d| **d == Ok(42)).count(), 1);
        assert_eq!(
            std::fs::read_to_string(effect).unwrap(),
            "private review body"
        );
        drop(db);
        let reopened = Deadbolt::open_at(dir.path(), true, 60);
        assert_eq!(reopened.admit("run", "send"), human());
        assert_eq!(reopened.admit_action(&request).unwrap(), human());
    }
    #[test]
    fn failed_primary_approval_and_consume_evidence_roll_back() {
        for kind in ["approve", "consume"] {
            let (dir, db, request) = setup();
            if kind == "consume" {
                db.approve_action(&request).unwrap();
            }
            let store = db.store.as_ref().unwrap();
            {
                store.lock().unwrap().events =
                    std::fs::File::open(dir.path().join("deadbolt-events.jsonl")).unwrap();
            }
            let result = if kind == "approve" {
                db.approve_action(&request)
            } else {
                db.admit_action(&request).map(|_| ())
            };
            assert!(matches!(result, Err(DeadboltError::StoreUnavailable)));
            {
                let mut g = store.lock().unwrap();
                if kind == "approve" {
                    let count: i64 = g
                        .conn
                        .query_row("SELECT COUNT(*) FROM action_grants", [], |r| r.get(0))
                        .unwrap();
                    assert_eq!(count, 0);
                    let count: i64 = g
                        .conn
                        .query_row("SELECT COUNT(*) FROM exact_action_requirements", [], |r| {
                            r.get(0)
                        })
                        .unwrap();
                    assert_eq!(count, 0);
                } else {
                    let consumed: Option<i64> = g
                        .conn
                        .query_row("SELECT consumed_at FROM action_grants", [], |r| r.get(0))
                        .unwrap();
                    assert_eq!(consumed, None);
                }
                g.events = OpenOptions::new()
                    .append(true)
                    .open(dir.path().join("deadbolt-events.jsonl"))
                    .unwrap();
            }
            if kind == "approve" {
                db.approve_action(&request).unwrap();
            }
            assert_eq!(db.admit_action(&request).unwrap(), AdmitDecision::Allow);
        }
    }
    #[test]
    fn approval_deadline_is_checked_after_writer_lock_wait() {
        let (dir, db, mut request) = setup();
        request.expires_at = now_secs() + 2;
        let mut blocker = rusqlite::Connection::open(dir.path().join("deadbolt.db")).unwrap();
        let tx = blocker
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .unwrap();
        let (started, ready) = std::sync::mpsc::channel();
        let deadline = request.expires_at;
        let thread = std::thread::spawn(move || {
            started.send(()).unwrap();
            db.approve_action(&request)
        });
        ready.recv().unwrap();
        while now_secs() <= deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        tx.commit().unwrap();
        assert!(matches!(
            thread.join().unwrap(),
            Err(DeadboltError::BadRequest)
        ));
        let count: i64 = blocker
            .query_row("SELECT COUNT(*) FROM action_grants", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn exact_admission_never_uses_legacy_fail_open_override() {
        let (dir, db, request) = setup();
        db.approve_action(&request).unwrap();
        let disabled = Deadbolt::open_paths(
            &dir.path().join("deadbolt.db"),
            &dir.path().join("disabled-events"),
            false,
            false,
            60,
        );
        assert!(disabled.admit_action(&request).is_err());
        std::fs::write(dir.path().join("blocked"), "not a directory").unwrap();
        let unavailable = Deadbolt::open_paths(
            &dir.path().join("blocked").join("db"),
            &dir.path().join("blocked").join("events"),
            true,
            false,
            60,
        );
        assert!(matches!(
            unavailable.admit_action(&request),
            Err(DeadboltError::StoreUnavailable)
        ));
        assert_eq!(db.admit_action(&request).unwrap(), AdmitDecision::Allow);
    }

    #[test]
    fn evidence_excludes_private_arguments() {
        let (dir, db, request) = setup();
        db.approve_action(&request).unwrap();
        db.admit_action(&request).unwrap();
        let evidence = std::fs::read_to_string(dir.path().join("deadbolt-events.jsonl")).unwrap();
        assert!(evidence.contains("action_approve") && evidence.contains("action_consume"));
        assert!(
            !evidence.contains("private review body") && !evidence.contains("reviewed@example.com")
        );
    }
    #[test]
    fn canonicalization_and_strict_input_contract() {
        let (_dir, _db, request) = setup();
        let mut equivalent = request.clone();
        equivalent.arguments = serde_json::from_str(
            r#"{"amount_minor":1e2,"body":"private review body","to":"reviewed@example.com"}"#,
        )
        .unwrap();
        assert_eq!(
            request.fingerprint().unwrap(),
            equivalent.fingerprint().unwrap()
        );
        let raw = serde_json::to_string(&request).unwrap();
        assert!(ActionRequest::from_json(
            &raw.replace("\"version\":1", "\"version\":1,\"version\":1")
        )
        .is_err());
        assert!(ActionRequest::from_json(&raw.replace(
            "\"amount_minor\":100",
            "\"amount_minor\":100,\"amount_minor\":100"
        ))
        .is_err());
        assert!(ActionRequest::from_json(
            &raw.replace("\"version\":1", "\"version\":1,\"extra\":1")
        )
        .is_err());
        for invalid in [
            json!(9007199254740992u64),
            json!(-0.0),
            json!("x".repeat(32769)),
        ] {
            let mut r = request.clone();
            r.arguments = json!({"value":invalid});
            assert!(r.fingerprint().is_err());
        }
        let mut nested = json!(0);
        for _ in 0..34 {
            nested = json!([nested]);
        }
        equivalent.arguments = json!({"value":nested});
        assert!(equivalent.fingerprint().is_err());
        // RFC 8785 example numbers and UTF-16 ordering (supplementary before U+E000).
        assert_eq!(
            String::from_utf8(
                serde_jcs::to_vec(&json!({"numbers":[333333333.3333333,1e30,4.50,0.002,1e-27]}))
                    .unwrap()
            )
            .unwrap(),
            r#"{"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27]}"#
        );
        assert_eq!(
            String::from_utf8(serde_jcs::to_vec(&json!({"\u{e000}":1,"\u{1f600}":2})).unwrap())
                .unwrap(),
            "{\"😀\":2,\"\u{e000}\":1}"
        );
        assert!(ActionRequest::from_json(
            &raw.replace("\"amount_minor\":100", "\"amount_minor\":-0")
        )
        .is_err());
        assert!(ActionRequest::from_json(
            &raw.replace("\"amount_minor\":100", "\"amount_minor\":1e400")
        )
        .is_err());
    }
}
