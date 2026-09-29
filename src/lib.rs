//! Deadbolt v0 — out-of-band lease gate.
//!
//! Harness calls [`Deadbolt`]. Deadbolt writes evidence through a thin
//! Witness-shaped [`EvidenceSink`] (Observed / Inferred / Generated, premises
//! on inferences, sha256 content id). This crate does not depend on Witness,
//! EvidenceLens, the TUI, or provider types, and it exposes no model tool.

#![deny(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod mcp;
mod serve;
mod witness;
pub use mcp::mcp_proxy;
pub use serve::{bind_refused, default_bind_path, serve};
pub use witness::WitnessSink;

const TOKEN_MAX: usize = 128;

/// Operator configuration (`[deadbolt]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadboltConfig {
    /// When false, Harness does not attach the gate.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Deny tool actions when the store cannot be opened or written.
    #[serde(default = "default_true")]
    pub fail_closed: bool,
    /// Lease lifetime. A live admit, allow or deny, renews it. Silence does not. An expired lease is not slid.
    #[serde(default = "default_ttl")]
    pub lease_ttl_secs: u64,
    /// SQLite path. Default `~/.deadbolt/deadbolt.db`.
    /// Harness consumers keep `~/.harness/deadbolt.db` in their own config.
    #[serde(default)]
    pub db_path: Option<String>,
    /// Append-only JSONL path. Default `~/.deadbolt/deadbolt-events.jsonl`.
    #[serde(default)]
    pub events_path: Option<String>,
    /// Env var that holds the sidecar token. Unset means no HTTP token.
    #[serde(default)]
    pub token_env: Option<String>,
    /// Token file. Used only when mode is `0600` and `token_env` is empty.
    #[serde(default)]
    pub token_file: Option<String>,
    /// Write a second Witness-shaped store. Default off. Drill stays JSONL-only.
    #[serde(default)]
    pub witness: bool,
    /// Witness sqlite path. Default `~/.deadbolt/deadbolt-witness.db`.
    #[serde(default)]
    pub witness_db: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_ttl() -> u64 {
    60
}

impl Default for DeadboltConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fail_closed: true,
            lease_ttl_secs: 60,
            db_path: None,
            events_path: None,
            token_env: None,
            token_file: None,
            witness: false,
            witness_db: None,
        }
    }
}

/// Witness-shaped epistemic class. Three buckets, not one prose log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicClass {
    /// Measured fact. Payload is structured tokens, never prose.
    Observed,
    /// Judgment that cites premise content ids.
    Inferred,
    /// Model or operator text. Deadbolt v0 does not write this class.
    Generated,
}

/// Content-addressed evidence record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// sha256 of the canonical record body (includes a monotonic seq).
    pub cid: String,
    /// sha256 of the canonical payload alone.
    pub payload_sha256: String,
    /// Monotonic sequence included in the content id.
    pub seq: u64,
    /// Epistemic class.
    pub class: EpistemicClass,
    /// Machine kind token (`tool_attempt`, `decision`, `purpose_exceeded`, …).
    pub kind: String,
    /// Structured payload. Strings must be tokens.
    pub payload: Value,
    /// Premise cids. Required and non-empty for [`EpistemicClass::Inferred`].
    pub premises: Vec<String>,
}

/// Append-only evidence sink. Deadbolt consumes this; it is not Witness.
pub trait EvidenceSink: Send + Sync {
    /// Append one record. Implementations must not rewrite or delete prior rows.
    fn append(&self, record: &EvidenceRecord) -> Result<(), SinkError>;
}

/// Sink failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SinkError {
    /// Store or JSONL could not be written.
    #[error("deadbolt:store_unavailable")]
    Unavailable,
    /// Payload contained prose or an illegal token.
    #[error("deadbolt:prose_rejected")]
    Prose,
    /// Inferred record without premise cids.
    #[error("deadbolt:premises_required")]
    PremisesRequired,
}

/// Deadbolt failure returned to the operator CLI.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DeadboltError {
    /// Store missing or unwritable.
    #[error("deadbolt:store_unavailable")]
    StoreUnavailable,
    /// Gate is disabled in config.
    #[error("deadbolt:disabled")]
    Disabled,
    /// No lease row for that agent_id.
    #[error("deadbolt:not_found")]
    NotFound,
    /// Kill is terminal. Resume does not resurrect.
    #[error("deadbolt:killed")]
    Killed,
    /// Drill scenario failed. The string is a code token, not prose.
    #[error("deadbolt:drill_failed:{0}")]
    DrillFailed(&'static str),
    /// Bind was not loopback. `0.0.0.0` is refused.
    #[error("deadbolt:bind_refused")]
    BindRefused,
    /// TCP serve started without a token.
    #[error("deadbolt:token_required")]
    TokenRequired,
    /// Export refused. The string is a code token, not prose.
    #[error("deadbolt:export_refused:{0}")]
    ExportRefused(&'static str),
    /// Child MCP server did not start.
    #[error("deadbolt:mcp_spawn")]
    McpSpawn,
    /// Operator input was not a token.
    #[error("deadbolt:bad_request")]
    BadRequest,
}

/// Why admit refused the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyCode {
    /// Agent lease was killed.
    Killed,
    /// Agent is paused.
    Paused,
    /// Tool is outside the clipped purpose. Recorded as an inferred breaker.
    PurposeExceeded,
    /// Lease timestamp is in the past. Not slid forward.
    LeaseExpired,
    /// Store missing or unwritable and fail_closed is set.
    StoreUnavailable,
    /// No lease row. Fail closed.
    NoLease,
    /// Spend crossed `spend_cap_usd`. The lease is paused.
    SpendCap,
    /// Irreversible tool. One operator approve has not been granted.
    NeedsHuman,
}

impl DenyCode {
    /// Stable token. Not a sentence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Killed => "killed",
            Self::Paused => "paused",
            Self::PurposeExceeded => "purpose_exceeded",
            Self::LeaseExpired => "lease_expired",
            Self::StoreUnavailable => "store_unavailable",
            Self::NoLease => "no_lease",
            Self::SpendCap => "spend_cap",
            Self::NeedsHuman => "needs_human",
        }
    }
}

/// Result of [`Deadbolt::admit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitDecision {
    /// Lease is live and the tool is inside purpose.
    Allow,
    /// Do not run the tool body.
    Deny {
        /// Machine code.
        code: DenyCode,
    },
}

/// Agents whose leases were revoked, plus swarm task ids to cancel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillReport {
    /// Agent the operator named.
    pub agent_id: String,
    /// That agent plus every descendant.
    pub revoked: Vec<String>,
    /// Swarm task ids bound to the revoked set. Not a fleet list.
    pub swarm_task_ids: Vec<String>,
}

/// One lease row for `status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    /// Lease key. Not a vendor API key.
    pub agent_id: String,
    /// `active`, `paused`, or `killed`.
    pub state: String,
    /// Unix seconds. Zero when killed.
    pub expires_at: i64,
    /// Parent agent, if this lease was registered as a child.
    pub parent_id: Option<String>,
    /// Clipped tool names.
    pub clips: Vec<String>,
    /// Swarm task id, if this child is a swarm worker.
    pub swarm_task_id: Option<String>,
}

/// Object-safe admit used by the tool executor.
pub trait ToolAdmit: Send + Sync {
    /// Recheck the lease and record the attempt. Called before any tool body.
    fn admit_tool(&self, agent_id: &str, tool: &str) -> AdmitDecision;
    /// Same decision as admit, but records only when the recheck denies.
    fn probe_tool(&self, agent_id: &str, tool: &str) -> AdmitDecision;
}

/// Operator policy fields. `None` leaves the stored value. Unset lists stay open.
#[derive(Debug, Clone, Default)]
pub struct PolicyPatch {
    /// If set, a tool not on the list is `purpose_exceeded`.
    pub tools_allow: Option<Vec<String>>,
    /// If set, a present foreign dest is `purpose_exceeded`. A missing dest is
    /// `purpose_exceeded` only for a network-class tool (`http`, `fetch`,
    /// `browser`, `web_search`). A local tool with no host is not denied for
    /// this list alone.
    pub dest_allow: Option<Vec<String>>,
    /// Crossing this pauses the lease. Admit then returns `spend_cap`.
    pub spend_cap_usd: Option<f64>,
    /// Tools that deny `needs_human` until one `approve`.
    pub irreversible: Option<Vec<String>>,
}

/// Result of [`Deadbolt::spend_add`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpendAdded {
    /// Total USD after the add.
    pub spend_usd: f64,
    /// True when the cap was crossed and the lease is paused.
    pub paused: bool,
}

struct Lease {
    agent_id: String,
    parent_id: Option<String>,
    state: String,
    expires_at: i64,
    clips: Vec<String>,
    clip_cids: BTreeMap<String, String>,
    swarm_task_id: Option<String>,
    tools_allow: Option<Vec<String>>,
    dest_allow: Option<Vec<String>>,
    spend_cap_usd: Option<f64>,
    spend_usd: f64,
    irreversible: Option<Vec<String>>,
    approvals: Vec<String>,
}

struct StoreInner {
    conn: Connection,
    events: std::fs::File,
}

/// SQLite leases + append-only JSONL/SQLite evidence. Implements [`EvidenceSink`].
pub struct JsonlSqliteSink {
    inner: Mutex<StoreInner>,
}

impl JsonlSqliteSink {
    fn open(db: &Path, events_path: &Path) -> Result<Self, SinkError> {
        if let Some(parent) = db.parent() {
            fs::create_dir_all(parent).map_err(|_| SinkError::Unavailable)?;
        }
        if let Some(parent) = events_path.parent() {
            fs::create_dir_all(parent).map_err(|_| SinkError::Unavailable)?;
        }
        let conn = Connection::open(db).map_err(|_| SinkError::Unavailable)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS leases (
               agent_id TEXT PRIMARY KEY,
               parent_id TEXT,
               state TEXT NOT NULL,
               expires_at INTEGER NOT NULL,
               clips TEXT NOT NULL,
               clip_cids TEXT NOT NULL,
               swarm_task_id TEXT,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS events (
               seq INTEGER PRIMARY KEY,
               cid TEXT NOT NULL UNIQUE,
               payload_sha256 TEXT NOT NULL,
               class TEXT NOT NULL,
               kind TEXT NOT NULL,
               payload TEXT NOT NULL,
               premises TEXT NOT NULL,
               ts INTEGER NOT NULL
             );",
        )
        .map_err(|_| SinkError::Unavailable)?;
        migrate_leases(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS event_sequence (id INTEGER PRIMARY KEY CHECK(id=1), seq INTEGER NOT NULL);
             INSERT INTO event_sequence (id, seq) SELECT 1, COALESCE(MAX(seq), 0) FROM events WHERE true
             ON CONFLICT(id) DO UPDATE SET seq=MAX(event_sequence.seq, excluded.seq);"
        ).map_err(|_| SinkError::Unavailable)?;
        let events = OpenOptions::new()
            .create(true)
            .append(true)
            .open(events_path)
            .map_err(|_| SinkError::Unavailable)?;
        Ok(Self {
            inner: Mutex::new(StoreInner { conn, events }),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, StoreInner>, SinkError> {
        self.inner.lock().map_err(|_| SinkError::Unavailable)
    }
}

impl EvidenceSink for JsonlSqliteSink {
    fn append(&self, record: &EvidenceRecord) -> Result<(), SinkError> {
        let mut g = self.lock()?;
        write_record(&mut g, record)
    }
}

fn write_record(g: &mut StoreInner, record: &EvidenceRecord) -> Result<(), SinkError> {
    let payload = serde_json::to_string(&record.payload).map_err(|_| SinkError::Unavailable)?;
    let premises = serde_json::to_string(&record.premises).map_err(|_| SinkError::Unavailable)?;
    let class = match record.class {
        EpistemicClass::Observed => "observed",
        EpistemicClass::Inferred => "inferred",
        EpistemicClass::Generated => "generated",
    };
    let tx =
        rusqlite::Transaction::new_unchecked(&g.conn, rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| SinkError::Unavailable)?;
    let start = std::time::Instant::now();
    loop {
        match tx.execute(
            "INSERT INTO events (seq, cid, payload_sha256, class, kind, payload, premises, ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.seq as i64,
                record.cid,
                record.payload_sha256,
                class,
                record.kind,
                payload,
                premises,
                now_secs()
            ],
        ) {
            Ok(_) => break,
            Err(e) if is_sqlite_busy(&e) && start.elapsed() < BUSY_BUDGET => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(_) => return Err(SinkError::Unavailable),
        }
    }
    let mut line = serde_json::to_string(record).map_err(|_| SinkError::Unavailable)?;
    line.push('\n');
    g.events
        .write_all(line.as_bytes())
        .map_err(|_| SinkError::Unavailable)?;
    g.events.flush().map_err(|_| SinkError::Unavailable)?;
    tx.commit().map_err(|_| SinkError::Unavailable)?;
    Ok(())
}

const BUSY_BUDGET: std::time::Duration = std::time::Duration::from_millis(5000);

fn is_sqlite_busy(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(e, _) => {
            matches!(
                e.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
        }
        _ => false,
    }
}

fn network_class(tool: &str) -> bool {
    matches!(tool, "http" | "fetch" | "browser" | "web_search")
}

fn keeps_deny_code(decision: AdmitDecision) -> bool {
    matches!(
        decision,
        AdmitDecision::Deny {
            code: DenyCode::Killed
                | DenyCode::Paused
                | DenyCode::PurposeExceeded
                | DenyCode::LeaseExpired
                | DenyCode::NoLease
                | DenyCode::SpendCap
                | DenyCode::NeedsHuman
        }
    )
}

/// Out-of-band lease gate. Clone shares the store.
#[derive(Clone)]
pub struct Deadbolt {
    store: Option<Arc<JsonlSqliteSink>>,
    sink: Option<Arc<dyn EvidenceSink>>,
    witness: Option<Arc<dyn EvidenceSink>>,
    fail_closed: bool,
    ttl: u64,
    enabled: bool,
}

impl Deadbolt {
    /// Open from config. A missing store with `fail_closed` yields a gate that denies.
    pub fn open(cfg: &DeadboltConfig) -> Self {
        let db = cfg
            .db_path
            .as_deref()
            .map(expand_path)
            .unwrap_or_else(default_db_path);
        let events = cfg
            .events_path
            .as_deref()
            .map(expand_path)
            .unwrap_or_else(default_events_path);
        let mut gate = Self::open_paths(
            &db,
            &events,
            cfg.enabled,
            cfg.fail_closed,
            cfg.lease_ttl_secs,
        );
        if cfg.witness && gate.enabled && gate.store.is_some() {
            let path = cfg
                .witness_db
                .as_deref()
                .map(expand_path)
                .unwrap_or_else(default_witness_path);
            gate.witness = Some(match WitnessSink::open(&path) {
                Ok(sink) => Arc::new(sink),
                Err(_) => Arc::new(ClosedWitness),
            });
        }
        gate
    }

    /// Open a store under `dir` (tests and drill). Does not touch `~/.deadbolt`.
    pub fn open_at(dir: &Path, fail_closed: bool, ttl: u64) -> Self {
        Self::open_paths(
            &dir.join("deadbolt.db"),
            &dir.join("deadbolt-events.jsonl"),
            true,
            fail_closed,
            ttl,
        )
    }

    fn open_paths(db: &Path, events: &Path, enabled: bool, fail_closed: bool, ttl: u64) -> Self {
        if !enabled {
            return Self {
                store: None,
                sink: None,
                witness: None,
                fail_closed,
                ttl,
                enabled: false,
            };
        }
        match JsonlSqliteSink::open(db, events) {
            Ok(store) => {
                let store = Arc::new(store);
                let sink: Arc<dyn EvidenceSink> = store.clone();
                Self {
                    store: Some(store),
                    sink: Some(sink),
                    witness: None,
                    fail_closed,
                    ttl,
                    enabled: true,
                }
            }
            Err(_) => Self {
                store: None,
                sink: None,
                witness: None,
                fail_closed,
                ttl,
                enabled: true,
            },
        }
    }

    /// True when the executor should attach this gate.
    pub fn should_attach(&self) -> bool {
        self.enabled && (self.store.is_some() || self.fail_closed)
    }

    /// Issue a lease if this agent has none. Does not resurrect a killed lease.
    pub fn ensure_agent(&self, agent_id: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let tx =
            rusqlite::Transaction::new_unchecked(&g.conn, rusqlite::TransactionBehavior::Immediate)
                .map_err(|_| DeadboltError::StoreUnavailable)?;
        if load_lease(&tx, agent_id)
            .map_err(|_| DeadboltError::StoreUnavailable)?
            .is_some()
        {
            return Ok(());
        }
        let exp = now_secs().saturating_add(self.ttl as i64);
        save_lease(
            &tx,
            &Lease {
                agent_id: agent_id.to_string(),
                parent_id: None,
                state: "active".into(),
                expires_at: exp,
                clips: Vec::new(),
                clip_cids: BTreeMap::new(),
                swarm_task_id: None,
                tools_allow: None,
                dest_allow: None,
                spend_cap_usd: None,
                spend_usd: 0.0,
                irreversible: None,
                approvals: Vec::new(),
            },
        )
        .map_err(|_| DeadboltError::StoreUnavailable)?;
        tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        drop(g);
        self.emit(
            EpistemicClass::Observed,
            "lease",
            json_tokens(&[("agent_id", agent_id), ("state", "active")]),
            &[],
        )
        .map(|_| ())
        .map_err(|_| DeadboltError::StoreUnavailable)
    }

    /// Register `child` under `parent`. An inactive or missing parent births a dead child.
    /// Repeated registration preserves state. Reparenting and self-parenting are refused.
    ///
    /// Returns `Ok(true)` when the child lease is live, `Ok(false)` when the child
    /// was recorded killed. `swarm_task_id` is cancelled when the parent is killed.
    pub fn register_child(
        &self,
        parent: &str,
        child: &str,
        swarm_task_id: Option<&str>,
    ) -> Result<bool, DeadboltError> {
        require_token(parent)?;
        require_token(child)?;
        if parent == child {
            return Err(DeadboltError::BadRequest);
        }
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let tx =
            rusqlite::Transaction::new_unchecked(&g.conn, rusqlite::TransactionBehavior::Immediate)
                .map_err(|_| DeadboltError::StoreUnavailable)?;
        if let Some(existing) =
            load_lease(&tx, child).map_err(|_| DeadboltError::StoreUnavailable)?
        {
            if existing.parent_id.as_deref() != Some(parent) {
                return Err(DeadboltError::BadRequest);
            }
            return Ok(existing.state == "active" && now_secs() < existing.expires_at);
        }
        let parent_lease =
            { load_lease(&tx, parent).map_err(|_| DeadboltError::StoreUnavailable)? };
        let parent_dead = match &parent_lease {
            Some(p) => p.state != "active" || now_secs() >= p.expires_at,
            None => true,
        };
        let state = if parent_dead { "killed" } else { "active" };
        let exp = if parent_dead {
            0
        } else {
            now_secs().saturating_add(self.ttl as i64)
        };
        {
            save_lease(
                &tx,
                &Lease {
                    agent_id: child.to_string(),
                    parent_id: Some(parent.to_string()),
                    state: state.into(),
                    expires_at: exp,
                    clips: Vec::new(),
                    clip_cids: BTreeMap::new(),
                    swarm_task_id: swarm_task_id.map(str::to_string),
                    tools_allow: None,
                    dest_allow: None,
                    spend_cap_usd: None,
                    spend_usd: 0.0,
                    irreversible: None,
                    approvals: Vec::new(),
                },
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        drop(g);
        self.emit(
            EpistemicClass::Observed,
            "lease",
            json_tokens(&[
                ("agent_id", child),
                ("parent_id", parent),
                ("state", state),
                ("swarm_task_id", swarm_task_id.unwrap_or("-")),
            ]),
            &[],
        )
        .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(!parent_dead)
    }

    /// Recheck the lease and record the attempt plus the decision.
    pub fn admit(&self, agent_id: &str, tool: &str) -> AdmitDecision {
        self.admit_dest(agent_id, tool, None)
    }

    /// Same as [`Self::admit`]. `dest` is a host token. Unset `dest_allow` ignores it.
    /// A present dest is checked against the list. A missing dest is checked only
    /// when `tool` is network-class.
    pub fn admit_dest(&self, agent_id: &str, tool: &str, dest: Option<&str>) -> AdmitDecision {
        self.decide(agent_id, tool, dest, true)
    }

    /// Recheck before the tool body. Records only a new denial.
    pub fn probe(&self, agent_id: &str, tool: &str) -> AdmitDecision {
        self.decide(agent_id, tool, None, false)
    }

    fn decide(
        &self,
        agent_id: &str,
        tool: &str,
        dest: Option<&str>,
        record_allow: bool,
    ) -> AdmitDecision {
        if !self.enabled {
            return AdmitDecision::Allow;
        }
        if self.store.is_none() {
            return if self.fail_closed {
                AdmitDecision::Deny {
                    code: DenyCode::StoreUnavailable,
                }
            } else {
                AdmitDecision::Allow
            };
        }
        if !is_token(agent_id) || !is_token(tool) {
            return AdmitDecision::Deny {
                code: DenyCode::NoLease,
            };
        }
        let decision = self.evaluate(agent_id, tool, dest);
        let deny = matches!(decision, AdmitDecision::Deny { .. });
        if (record_allow || deny) && self.record_decision(agent_id, tool, &decision).is_err() {
            if keeps_deny_code(decision) {
                return decision;
            }
            if self.fail_closed {
                return AdmitDecision::Deny {
                    code: DenyCode::StoreUnavailable,
                };
            }
        }
        decision
    }

    fn evaluate(&self, agent_id: &str, tool: &str, dest: Option<&str>) -> AdmitDecision {
        self.evaluate_atomic(agent_id, tool, dest)
            .unwrap_or(AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable,
            })
    }

    fn evaluate_atomic(
        &self,
        agent_id: &str,
        tool: &str,
        dest: Option<&str>,
    ) -> Result<AdmitDecision, rusqlite::Error> {
        let Some(store) = &self.store else {
            return Ok(AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable,
            });
        };
        let g = match store.lock() {
            Ok(g) => g,
            Err(_) => {
                return Ok(AdmitDecision::Deny {
                    code: DenyCode::StoreUnavailable,
                })
            }
        };
        let tx = rusqlite::Transaction::new_unchecked(
            &g.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let lease = match load_lease(&tx, agent_id) {
            Ok(v) => v,
            Err(_) => {
                return Ok(AdmitDecision::Deny {
                    code: DenyCode::StoreUnavailable,
                })
            }
        };
        let Some(lease) = lease else {
            return Ok(AdmitDecision::Deny {
                code: DenyCode::NoLease,
            });
        };
        if lease.state == "killed" {
            return Ok(AdmitDecision::Deny {
                code: DenyCode::Killed,
            });
        }
        if now_secs() >= lease.expires_at {
            return Ok(AdmitDecision::Deny {
                code: DenyCode::LeaseExpired,
            });
        }
        let exp = now_secs().saturating_add(self.ttl as i64);
        tx.execute(
            "UPDATE leases SET expires_at=?1, updated_at=?1 WHERE agent_id=?2 AND state!='killed'",
            params![exp, agent_id],
        )?;
        if let Some(cap) = lease.spend_cap_usd {
            if lease.spend_usd >= cap {
                tx.execute(
                    "UPDATE leases SET state='paused', updated_at=?1 WHERE agent_id=?2 AND state!='killed'",
                    params![now_secs(), agent_id],
                )?;
                tx.commit()?;
                return Ok(AdmitDecision::Deny {
                    code: DenyCode::SpendCap,
                });
            }
        }
        if lease.state == "paused" {
            tx.commit()?;
            return Ok(AdmitDecision::Deny {
                code: DenyCode::Paused,
            });
        }
        if lease.clips.iter().any(|c| c == tool) {
            tx.commit()?;
            return Ok(AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded,
            });
        }
        if let Some(allow) = &lease.tools_allow {
            if !allow.iter().any(|t| t == tool) {
                tx.commit()?;
                return Ok(AdmitDecision::Deny {
                    code: DenyCode::PurposeExceeded,
                });
            }
        }
        if let Some(allow) = &lease.dest_allow {
            let foreign = dest.is_some_and(|d| !allow.iter().any(|h| h.eq_ignore_ascii_case(d)));
            let missing_network = dest.is_none() && network_class(tool);
            if foreign || missing_network {
                tx.commit()?;
                return Ok(AdmitDecision::Deny {
                    code: DenyCode::PurposeExceeded,
                });
            }
        }
        let shot = lease
            .irreversible
            .as_ref()
            .is_some_and(|list| list.iter().any(|t| t == tool));
        if shot && !lease.approvals.iter().any(|t| t == tool) {
            tx.commit()?;
            return Ok(AdmitDecision::Deny {
                code: DenyCode::NeedsHuman,
            });
        }
        if shot {
            let approvals: Vec<_> = lease
                .approvals
                .iter()
                .filter(|t| t.as_str() != tool)
                .collect();
            let raw =
                serde_json::to_string(&approvals).map_err(|_| rusqlite::Error::InvalidQuery)?;
            tx.execute(
                "UPDATE leases SET approvals=?1 WHERE agent_id=?2",
                params![raw, agent_id],
            )?;
        }
        tx.commit()?;
        Ok(AdmitDecision::Allow)
    }

    fn record_decision(
        &self,
        agent_id: &str,
        tool: &str,
        decision: &AdmitDecision,
    ) -> Result<(), SinkError> {
        let attempt_cid = self.emit(
            EpistemicClass::Observed,
            "tool_attempt",
            json_tokens(&[("agent_id", agent_id), ("tool", tool)]),
            &[],
        )?;
        match decision {
            AdmitDecision::Allow => {
                self.emit(
                    EpistemicClass::Observed,
                    "decision",
                    json_tokens(&[
                        ("agent_id", agent_id),
                        ("tool", tool),
                        ("outcome", "allow"),
                        ("code", "ok"),
                    ]),
                    &[],
                )?;
            }
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded,
            } => {
                let clip_cid = self.clip_cid(agent_id, tool).unwrap_or_default();
                let mut premises = Vec::new();
                if !clip_cid.is_empty() {
                    premises.push(clip_cid);
                }
                premises.push(attempt_cid);
                self.emit(
                    EpistemicClass::Inferred,
                    "purpose_exceeded",
                    json_tokens(&[
                        ("agent_id", agent_id),
                        ("tool", tool),
                        ("breaker", "purpose_exceeded"),
                    ]),
                    &premises,
                )?;
                self.emit(
                    EpistemicClass::Observed,
                    "decision",
                    json_tokens(&[
                        ("agent_id", agent_id),
                        ("tool", tool),
                        ("outcome", "deny"),
                        ("code", "clipped"),
                    ]),
                    &[],
                )?;
            }
            AdmitDecision::Deny { code } => {
                self.emit(
                    EpistemicClass::Observed,
                    "decision",
                    json_tokens(&[
                        ("agent_id", agent_id),
                        ("tool", tool),
                        ("outcome", "deny"),
                        ("code", code.as_str()),
                    ]),
                    &[],
                )?;
            }
        }
        Ok(())
    }

    fn clip_cid(&self, agent_id: &str, tool: &str) -> Option<String> {
        let store = self.store.as_ref()?;
        let g = store.lock().ok()?;
        let lease = load_lease(&g.conn, agent_id).ok()??;
        lease.clip_cids.get(tool).cloned()
    }

    /// Revoke this agent and every child. Does not touch other agents.
    pub fn kill(&self, agent_id: &str) -> Result<KillReport, DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        let (revoked, swarm_task_ids) = {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            if load_lease(&tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .is_none()
            {
                return Err(DeadboltError::NotFound);
            }
            let mut ids = vec![agent_id.to_string()];
            ids.extend(descendant_ids(&tx, agent_id).map_err(|_| DeadboltError::StoreUnavailable)?);
            let mut tasks = Vec::new();
            let ts = now_secs();
            for id in &ids {
                if let Some(lease) =
                    load_lease(&tx, id).map_err(|_| DeadboltError::StoreUnavailable)?
                {
                    if let Some(task) = lease.swarm_task_id {
                        tasks.push(task);
                    }
                }
                tx
                    .execute(
                        "UPDATE leases SET state='killed', expires_at=0, updated_at=?1 WHERE agent_id=?2",
                        params![ts, id],
                    )
                    .map_err(|_| DeadboltError::StoreUnavailable)?;
            }
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
            (ids, tasks)
        };
        let mut payload = Map::new();
        payload.insert("agent_id".into(), Value::String(agent_id.to_string()));
        payload.insert("revoked_n".into(), Value::Number(revoked.len().into()));
        payload.insert("swarm_n".into(), Value::Number(swarm_task_ids.len().into()));
        self.emit(
            EpistemicClass::Observed,
            "kill",
            Value::Object(payload),
            &[],
        )
        .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(KillReport {
            agent_id: agent_id.to_string(),
            revoked,
            swarm_task_ids,
        })
    }

    /// Pause one agent. Not a fleet halt.
    pub fn pause(&self, agent_id: &str) -> Result<(), DeadboltError> {
        self.set_state(agent_id, "paused")
    }

    /// Clear pause and clips. Does not resurrect a killed lease.
    pub fn resume(&self, agent_id: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            let lease = load_lease(&tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .ok_or(DeadboltError::NotFound)?;
            if lease.state == "killed" {
                return Err(DeadboltError::Killed);
            }
            let exp = now_secs().saturating_add(self.ttl as i64);
            tx
                .execute(
                    "UPDATE leases SET state='active', expires_at=?1, clips='[]', clip_cids='{}', updated_at=?1 WHERE agent_id=?2",
                    params![exp, agent_id],
                )
                .map_err(|_| DeadboltError::StoreUnavailable)?;
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        self.emit(
            EpistemicClass::Observed,
            "resume",
            json_tokens(&[("agent_id", agent_id), ("state", "active")]),
            &[],
        )
        .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(())
    }

    /// Clip one tool on one agent. Other tools stay admitted.
    pub fn clip(&self, agent_id: &str, tool: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        require_token(tool)?;
        let store = self.store()?;
        let cid = self
            .emit(
                EpistemicClass::Observed,
                "clip",
                json_tokens(&[("agent_id", agent_id), ("tool", tool)]),
                &[],
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let tx =
            rusqlite::Transaction::new_unchecked(&g.conn, rusqlite::TransactionBehavior::Immediate)
                .map_err(|_| DeadboltError::StoreUnavailable)?;
        let mut lease = load_lease(&tx, agent_id)
            .map_err(|_| DeadboltError::StoreUnavailable)?
            .ok_or(DeadboltError::NotFound)?;
        if lease.state == "killed" {
            return Err(DeadboltError::Killed);
        }
        if !lease.clips.iter().any(|c| c == tool) {
            lease.clips.push(tool.to_string());
        }
        lease.clip_cids.insert(tool.to_string(), cid);
        save_lease(&tx, &lease).map_err(|_| DeadboltError::StoreUnavailable)?;
        tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(())
    }

    /// Set blast-radius fields. Omitted fields stay as stored. Unset lists stay open.
    pub fn set_policy(&self, agent_id: &str, patch: PolicyPatch) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        for list in [&patch.tools_allow, &patch.dest_allow, &patch.irreversible]
            .into_iter()
            .flatten()
        {
            for item in list {
                require_token(item)?;
            }
        }
        if patch
            .spend_cap_usd
            .is_some_and(|cap| !cap.is_finite() || cap < 0.0)
        {
            return Err(DeadboltError::BadRequest);
        }
        let store = self.store()?;
        {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            let lease = load_lease(&tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .ok_or(DeadboltError::NotFound)?;
            if lease.state == "killed" {
                return Err(DeadboltError::Killed);
            }
            let tools = json_list(patch.tools_allow.as_deref());
            let dest = json_list(patch.dest_allow.as_deref());
            let irrev = json_list(patch.irreversible.as_deref());
            tx
                .execute(
                    "UPDATE leases SET tools_allow=COALESCE(?1, tools_allow), dest_allow=COALESCE(?2, dest_allow), spend_cap_usd=COALESCE(?3, spend_cap_usd), irreversible=COALESCE(?4, irreversible), updated_at=?5 WHERE agent_id=?6",
                    params![tools, dest, patch.spend_cap_usd, irrev, now_secs(), agent_id],
                )
                .map_err(|_| DeadboltError::StoreUnavailable)?;
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        self.emit(
            EpistemicClass::Observed,
            "policy",
            json_tokens(&[("agent_id", agent_id), ("state", "set")]),
            &[],
        )
        .map(|_| ())
        .map_err(|_| DeadboltError::StoreUnavailable)
    }

    /// One shot for an irreversible tool. Not a model tool.
    pub fn approve(&self, agent_id: &str, tool: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        require_token(tool)?;
        let store = self.store()?;
        {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            let mut lease = load_lease(&tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .ok_or(DeadboltError::NotFound)?;
            if lease.state == "killed" {
                return Err(DeadboltError::Killed);
            }
            if !lease.approvals.iter().any(|t| t == tool) {
                lease.approvals.push(tool.to_string());
            }
            let raw = serde_json::to_string(&lease.approvals).unwrap_or_else(|_| "[]".into());
            tx.execute(
                "UPDATE leases SET approvals=?1, updated_at=?2 WHERE agent_id=?3",
                params![raw, now_secs(), agent_id],
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        self.emit(
            EpistemicClass::Observed,
            "approve",
            json_tokens(&[("agent_id", agent_id), ("tool", tool)]),
            &[],
        )
        .map(|_| ())
        .map_err(|_| DeadboltError::StoreUnavailable)
    }

    /// Add USD to the lease. Crossing `spend_cap_usd` pauses it.
    pub fn spend_add(&self, agent_id: &str, usd: f64) -> Result<SpendAdded, DeadboltError> {
        require_token(agent_id)?;
        if !usd.is_finite() || usd < 0.0 {
            return Err(DeadboltError::BadRequest);
        }
        let store = self.store()?;
        let (total, paused) = {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            let lease = load_lease(&tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .ok_or(DeadboltError::NotFound)?;
            if lease.state == "killed" {
                return Err(DeadboltError::Killed);
            }
            let total = lease.spend_usd + usd;
            let paused = lease.spend_cap_usd.is_some_and(|cap| total >= cap);
            let state = if paused {
                "paused"
            } else {
                lease.state.as_str()
            };
            tx.execute(
                "UPDATE leases SET spend_usd=?1, state=?2, updated_at=?3 WHERE agent_id=?4",
                params![total, state, now_secs(), agent_id],
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
            (total, paused)
        };
        let usd_tok = usd_token(usd);
        self.emit(
            EpistemicClass::Observed,
            "spend",
            json_tokens(&[("agent_id", agent_id), ("usd", &usd_tok)]),
            &[],
        )
        .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(SpendAdded {
            spend_usd: total,
            paused,
        })
    }

    /// One JSON document. Tokens only. No generated prose.
    pub fn incident(&self, agent_id: &str) -> Result<String, DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let lease = load_lease(&g.conn, agent_id)
            .map_err(|_| DeadboltError::StoreUnavailable)?
            .ok_or(DeadboltError::NotFound)?;
        let updated_at: i64 = g
            .conn
            .query_row(
                "SELECT updated_at FROM leases WHERE agent_id=?1",
                params![agent_id],
                |r| r.get(0),
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let children =
            descendant_ids(&g.conn, agent_id).map_err(|_| DeadboltError::StoreUnavailable)?;
        let mut ids = BTreeSet::from([agent_id.to_string()]);
        for id in &children {
            ids.insert(id.clone());
        }
        let mut stmt = g
            .conn
            .prepare("SELECT cid, class, kind, payload, premises, ts FROM events ORDER BY seq")
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let scanned = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let mut first_seen: Option<i64> = None;
        let mut decisions = Vec::new();
        let mut inferred = Vec::new();
        for row in scanned {
            let (cid, class, kind, payload, premises, ts) =
                row.map_err(|_| DeadboltError::StoreUnavailable)?;
            let payload: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
            let Some(row_agent) = payload.get("agent_id").and_then(|v| v.as_str()) else {
                continue;
            };
            if !ids.contains(row_agent) {
                continue;
            }
            if class == "generated" {
                return Err(DeadboltError::ExportRefused("generated"));
            }
            first_seen = Some(first_seen.map_or(ts, |n| n.min(ts)));
            let premises: Vec<String> = serde_json::from_str(&premises).unwrap_or_default();
            if class == "inferred" {
                if premises.is_empty() {
                    return Err(DeadboltError::ExportRefused("premises"));
                }
                inferred.push(json!({
                    "class": "inferred",
                    "kind": kind,
                    "cid": cid,
                    "tool": payload.get("tool").and_then(|v| v.as_str()).unwrap_or("-"),
                    "premises": premises,
                    "ts": ts,
                }));
            } else if kind == "decision" {
                decisions.push(json!({
                    "class": "observed",
                    "kind": "decision",
                    "cid": cid,
                    "tool": payload.get("tool").and_then(|v| v.as_str()).unwrap_or("-"),
                    "code": payload.get("code").and_then(|v| v.as_str()).unwrap_or("-"),
                    "ts": ts,
                }));
            }
        }
        let killed_at = if lease.state == "killed" {
            Value::from(updated_at)
        } else {
            Value::Null
        };
        let body = json!({
            "agent": agent_id,
            "children": children,
            "first_seen": first_seen,
            "killed_at": killed_at,
            "decisions": decisions,
            "inferred": inferred,
            "policy": {
                "tools_allow": lease.tools_allow,
                "dest_allow": lease.dest_allow,
                "spend_cap_usd": lease.spend_cap_usd,
                "spend_usd": lease.spend_usd,
                "irreversible": lease.irreversible,
            }
        });
        Ok(body.to_string())
    }

    /// List one agent, or every lease when `agent_id` is `None`.
    pub fn status(&self, agent_id: Option<&str>) -> Result<Vec<AgentStatus>, DeadboltError> {
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let leases = if let Some(id) = agent_id {
            require_token(id)?;
            match load_lease(&g.conn, id).map_err(|_| DeadboltError::StoreUnavailable)? {
                Some(l) => vec![l],
                None => Vec::new(),
            }
        } else {
            load_all(&g.conn).map_err(|_| DeadboltError::StoreUnavailable)?
        };
        Ok(leases.into_iter().map(status_of).collect())
    }

    /// Evidence rows for one agent. JSONL-shaped. Children only when asked.
    pub fn export(&self, agent_id: &str, children: bool) -> Result<Vec<ExportRow>, DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let mut ids = BTreeSet::from([agent_id.to_string()]);
        if children {
            for id in
                descendant_ids(&g.conn, agent_id).map_err(|_| DeadboltError::StoreUnavailable)?
            {
                ids.insert(id);
            }
        }
        let mut stmt = g
            .conn
            .prepare(
                "SELECT cid, payload_sha256, class, kind, payload, premises, ts FROM events ORDER BY seq",
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let scanned = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                ))
            })
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        let mut out = Vec::new();
        for row in scanned {
            let (cid, payload_sha256, class, kind, payload, premises, ts) =
                row.map_err(|_| DeadboltError::StoreUnavailable)?;
            let payload: Value = serde_json::from_str(&payload).unwrap_or(Value::Null);
            let Some(row_agent) = payload.get("agent_id").and_then(|v| v.as_str()) else {
                continue;
            };
            if !ids.contains(row_agent) {
                continue;
            }
            if class != "observed" && class != "inferred" {
                return Err(DeadboltError::ExportRefused("generated"));
            }
            let premises: Vec<String> = serde_json::from_str(&premises).unwrap_or_default();
            if class == "inferred" && premises.is_empty() {
                return Err(DeadboltError::ExportRefused("premises"));
            }
            out.push(ExportRow {
                class,
                kind,
                cid,
                payload_sha256,
                premises,
                agent_id: row_agent.to_string(),
                tool: payload
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                decision: payload
                    .get("outcome")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                code: payload
                    .get("code")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                ts,
            });
        }
        Ok(out)
    }

    /// True when the lease is killed. Missing is not revoked (not yet registered).
    pub fn is_revoked(&self, agent_id: &str) -> bool {
        let Some(store) = &self.store else {
            return self.fail_closed && self.enabled;
        };
        let Ok(g) = store.lock() else {
            return self.fail_closed;
        };
        match load_lease(&g.conn, agent_id) {
            Ok(Some(l)) => l.state == "killed",
            Ok(None) => false,
            Err(_) => self.fail_closed,
        }
    }

    /// Resolves when the lease is killed. Pends forever when the gate is off.
    pub async fn wait_until_revoked(&self, agent_id: &str) {
        if !self.should_attach() {
            std::future::pending::<()>().await;
            return;
        }
        loop {
            if self.is_revoked(agent_id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Force the lease into the past. Used by drill and tests. Not a CLI command.
    pub fn force_expire(&self, agent_id: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
        let n = g
            .conn
            .execute(
                "UPDATE leases SET expires_at=0, updated_at=?1 WHERE agent_id=?2",
                params![now_secs(), agent_id],
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
        if n == 0 {
            return Err(DeadboltError::NotFound);
        }
        Ok(())
    }

    /// In-process scenario. Uses a temp store. Does not touch `~/.deadbolt`.
    pub fn drill() -> Result<(), DeadboltError> {
        let dir = DrillDir::new()?;
        let db = Self::open_at(&dir.0, true, 60);
        db.ensure_agent("drill-a")?;
        db.ensure_agent("drill-b")?;
        match db.admit("drill-a", "read_file") {
            AdmitDecision::Allow => {}
            AdmitDecision::Deny { .. } => return Err(DeadboltError::DrillFailed("admit_a")),
        }
        db.kill("drill-a")?;
        match db.admit("drill-a", "shell") {
            AdmitDecision::Deny {
                code: DenyCode::Killed,
            } => {}
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable,
            } => return Err(DeadboltError::DrillFailed("post_kill_store_unavailable")),
            _ => return Err(DeadboltError::DrillFailed("post_kill_not_killed")),
        }
        match db.admit("drill-b", "read_file") {
            AdmitDecision::Allow => {}
            _ => return Err(DeadboltError::DrillFailed("isolate_b")),
        }
        let live = db.register_child("drill-b", "drill-c", Some("swarm-c"))?;
        if !live {
            return Err(DeadboltError::DrillFailed("child_live"));
        }
        let report = db.kill("drill-b")?;
        if !report.swarm_task_ids.iter().any(|t| t == "swarm-c") {
            return Err(DeadboltError::DrillFailed("swarm_task"));
        }
        match db.admit("drill-c", "read_file") {
            AdmitDecision::Deny {
                code: DenyCode::Killed,
            } => {}
            _ => return Err(DeadboltError::DrillFailed("child_deny")),
        }
        db.ensure_agent("line-p")?;
        db.ensure_agent("line-b")?;
        let line_live = db.register_child("line-p", "line-c", Some("swarm-line"))?;
        if !line_live {
            return Err(DeadboltError::DrillFailed("lineage_child_live"));
        }
        match db.admit("line-c", "read_file") {
            AdmitDecision::Allow => {}
            _ => return Err(DeadboltError::DrillFailed("lineage_child_allow")),
        }
        db.kill("line-p")?;
        match db.admit("line-c", "shell") {
            AdmitDecision::Deny {
                code: DenyCode::Killed,
            } => {}
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable,
            } => return Err(DeadboltError::DrillFailed("lineage_store_unavailable")),
            _ => return Err(DeadboltError::DrillFailed("lineage_not_killed")),
        }
        match db.admit("line-b", "shell") {
            AdmitDecision::Allow => {}
            _ => return Err(DeadboltError::DrillFailed("lineage_unrelated")),
        }
        db.ensure_agent("drill-d")?;
        db.clip("drill-d", "shell")?;
        match db.admit("drill-d", "shell") {
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded,
            } => {}
            _ => return Err(DeadboltError::DrillFailed("clip_shell")),
        }
        match db.admit("drill-d", "read_file") {
            AdmitDecision::Allow => {}
            _ => return Err(DeadboltError::DrillFailed("clip_read")),
        }
        db.resume("drill-d")?;
        match db.admit("drill-d", "shell") {
            AdmitDecision::Allow => {}
            _ => return Err(DeadboltError::DrillFailed("resume")),
        }
        db.force_expire("drill-d")?;
        match db.admit("drill-d", "read_file") {
            AdmitDecision::Deny {
                code: DenyCode::LeaseExpired,
            } => {}
            _ => return Err(DeadboltError::DrillFailed("expire")),
        }
        let blocked = dir.0.join("not-a-directory");
        fs::write(&blocked, b"x").map_err(|_| DeadboltError::DrillFailed("fixture"))?;
        let closed = Self::open_paths(
            &blocked.join("deadbolt.db"),
            &blocked.join("events.jsonl"),
            true,
            true,
            60,
        );
        match closed.admit("drill-a", "read_file") {
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable,
            } => {}
            _ => return Err(DeadboltError::DrillFailed("fail_closed")),
        }
        if is_shutdown_tool("read_file") || !is_shutdown_tool("shutdown") {
            return Err(DeadboltError::DrillFailed("shutdown_name"));
        }
        Ok(())
    }

    fn set_state(&self, agent_id: &str, state: &str) -> Result<(), DeadboltError> {
        require_token(agent_id)?;
        let store = self.store()?;
        {
            let g = store.lock().map_err(|_| DeadboltError::StoreUnavailable)?;
            let tx = rusqlite::Transaction::new_unchecked(
                &g.conn,
                rusqlite::TransactionBehavior::Immediate,
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            let lease = load_lease(&tx, agent_id)
                .map_err(|_| DeadboltError::StoreUnavailable)?
                .ok_or(DeadboltError::NotFound)?;
            if lease.state == "killed" {
                return Err(DeadboltError::Killed);
            }
            tx.execute(
                "UPDATE leases SET state=?1, updated_at=?2 WHERE agent_id=?3",
                params![state, now_secs(), agent_id],
            )
            .map_err(|_| DeadboltError::StoreUnavailable)?;
            tx.commit().map_err(|_| DeadboltError::StoreUnavailable)?;
        }
        self.emit(
            EpistemicClass::Observed,
            "pause",
            json_tokens(&[("agent_id", agent_id), ("state", state)]),
            &[],
        )
        .map_err(|_| DeadboltError::StoreUnavailable)?;
        Ok(())
    }

    fn emit(
        &self,
        class: EpistemicClass,
        kind: &str,
        payload: Value,
        premises: &[String],
    ) -> Result<String, SinkError> {
        let sink = self.sink.as_ref().ok_or(SinkError::Unavailable)?;
        let store = self.store.as_ref().ok_or(SinkError::Unavailable)?;
        let seq = {
            let g = store.lock()?;
            g.conn
                .query_row(
                    "UPDATE event_sequence SET seq=seq+1 WHERE id=1 RETURNING seq",
                    [],
                    |row| row.get::<_, u64>(0),
                )
                .map_err(|_| SinkError::Unavailable)?
        };
        let record = EvidenceRecord::seal(class, kind, payload, premises, seq)?;
        sink.append(&record)?;
        if let Some(witness) = &self.witness {
            witness.append(&record)?;
        }
        Ok(record.cid)
    }

    fn store(&self) -> Result<Arc<JsonlSqliteSink>, DeadboltError> {
        if !self.enabled {
            return Err(DeadboltError::Disabled);
        }
        self.store.clone().ok_or(DeadboltError::StoreUnavailable)
    }

    #[cfg(test)]
    fn events(&self) -> Vec<EvidenceRecord> {
        let Some(store) = &self.store else {
            return Vec::new();
        };
        let Ok(g) = store.lock() else {
            return Vec::new();
        };
        let mut stmt = match g.conn.prepare(
            "SELECT cid, payload_sha256, class, kind, payload, premises FROM events ORDER BY seq",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |r| {
            let class = match r.get::<_, String>(2)?.as_str() {
                "inferred" => EpistemicClass::Inferred,
                "generated" => EpistemicClass::Generated,
                _ => EpistemicClass::Observed,
            };
            let payload: Value =
                serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or(Value::Null);
            let premises: Vec<String> =
                serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default();
            Ok(EvidenceRecord {
                cid: r.get(0)?,
                payload_sha256: r.get(1)?,
                seq: 0,
                class,
                kind: r.get(3)?,
                payload,
                premises,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }
}

impl ToolAdmit for Deadbolt {
    fn admit_tool(&self, agent_id: &str, tool: &str) -> AdmitDecision {
        self.admit(agent_id, tool)
    }

    fn probe_tool(&self, agent_id: &str, tool: &str) -> AdmitDecision {
        self.probe(agent_id, tool)
    }
}

impl EvidenceRecord {
    fn seal(
        class: EpistemicClass,
        kind: &str,
        payload: Value,
        premises: &[String],
        seq: u64,
    ) -> Result<Self, SinkError> {
        if !is_token(kind) {
            return Err(SinkError::Prose);
        }
        assert_structured(&payload)?;
        if class == EpistemicClass::Inferred && premises.is_empty() {
            return Err(SinkError::PremisesRequired);
        }
        if class == EpistemicClass::Observed && !premises.is_empty() {
            return Err(SinkError::Prose);
        }
        for p in premises {
            if !p.starts_with("sha256:") || !is_token(p) {
                return Err(SinkError::PremisesRequired);
            }
        }
        let payload_sha256 = payload_cid(&payload);
        let body = serde_json::json!({
            "class": class,
            "kind": kind,
            "payload": payload,
            "premises": premises,
            "seq": seq,
        });
        let cid = format!("sha256:{}", hex_encode(&Sha256::digest(canonical(&body))));
        Ok(Self {
            cid,
            payload_sha256,
            seq,
            class,
            kind: kind.to_string(),
            payload,
            premises: premises.to_vec(),
        })
    }
}

/// True for names that must never appear in the model tool list.
pub fn is_shutdown_tool(name: &str) -> bool {
    matches!(
        name,
        "shutdown"
            | "kill"
            | "deadbolt"
            | "halt"
            | "fleet_halt"
            | "kill_agent"
            | "kill_switch"
            | "deadbolt_kill"
            | "deadbolt_pause"
            | "deadbolt_clip"
            | "stop_agent"
            | "terminate"
    )
}

/// Default SQLite path. `~/.deadbolt/deadbolt.db`.
pub fn default_db_path() -> PathBuf {
    state_home().join("deadbolt.db")
}

/// Default append-only JSONL path.
pub fn default_events_path() -> PathBuf {
    state_home().join("deadbolt-events.jsonl")
}

/// Default Witness sqlite path. JSONL is the sibling `.jsonl`.
pub fn default_witness_path() -> PathBuf {
    state_home().join("deadbolt-witness.db")
}

fn state_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".deadbolt")
}

fn expand_path(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}

struct ClosedWitness;

impl EvidenceSink for ClosedWitness {
    fn append(&self, _record: &EvidenceRecord) -> Result<(), SinkError> {
        Err(SinkError::Unavailable)
    }
}

struct DrillDir(PathBuf);

impl DrillDir {
    fn new() -> Result<Self, DeadboltError> {
        let dir = std::env::temp_dir().join(format!(
            "deadbolt-drill-{}-{}",
            std::process::id(),
            now_secs()
        ));
        fs::create_dir_all(&dir).map_err(|_| DeadboltError::DrillFailed("tempdir"))?;
        Ok(Self(dir))
    }
}

impl Drop for DrillDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= TOKEN_MAX
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '/' | '-'))
}

fn require_token(s: &str) -> Result<(), DeadboltError> {
    if is_token(s) {
        Ok(())
    } else {
        Err(DeadboltError::NotFound)
    }
}

fn assert_structured(value: &Value) -> Result<(), SinkError> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(s) if is_token(s) => Ok(()),
        Value::String(_) => Err(SinkError::Prose),
        Value::Array(items) => items.iter().try_for_each(assert_structured),
        Value::Object(map) => map.values().try_for_each(assert_structured),
    }
}

fn payload_cid(payload: &Value) -> String {
    format!("sha256:{}", hex_encode(&Sha256::digest(canonical(payload))))
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&sort_value(value)).unwrap_or_default()
}

fn sort_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                if let Some(v) = map.get(&k) {
                    out.insert(k, sort_value(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sort_value).collect()),
        other => other.clone(),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn json_list(list: Option<&[String]>) -> Option<String> {
    list.map(|items| serde_json::to_string(items).unwrap_or_else(|_| "[]".into()))
}

fn usd_token(usd: f64) -> String {
    let raw = format!("{usd}");
    if is_token(&raw) {
        raw
    } else {
        "0".into()
    }
}

fn opt_list(raw: Option<String>) -> Option<Vec<String>> {
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

fn migrate_leases(conn: &Connection) -> Result<(), SinkError> {
    for sql in [
        "ALTER TABLE leases ADD COLUMN tools_allow TEXT",
        "ALTER TABLE leases ADD COLUMN dest_allow TEXT",
        "ALTER TABLE leases ADD COLUMN spend_cap_usd REAL",
        "ALTER TABLE leases ADD COLUMN spend_usd REAL NOT NULL DEFAULT 0",
        "ALTER TABLE leases ADD COLUMN irreversible TEXT",
        "ALTER TABLE leases ADD COLUMN approvals TEXT NOT NULL DEFAULT '[]'",
    ] {
        let _ = conn.execute(sql, []);
    }
    Ok(())
}

fn json_tokens(pairs: &[(&str, &str)]) -> Value {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert((*k).to_string(), Value::String((*v).to_string()));
    }
    Value::Object(map)
}

fn save_lease(conn: &Connection, lease: &Lease) -> Result<(), rusqlite::Error> {
    let clips = serde_json::to_string(&lease.clips).unwrap_or_else(|_| "[]".into());
    let clip_cids = serde_json::to_string(&lease.clip_cids).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO leases (agent_id, parent_id, state, expires_at, clips, clip_cids, swarm_task_id, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(agent_id) DO UPDATE SET
           parent_id=excluded.parent_id,
           state=excluded.state,
           expires_at=excluded.expires_at,
           clips=excluded.clips,
           clip_cids=excluded.clip_cids,
           swarm_task_id=excluded.swarm_task_id,
           updated_at=excluded.updated_at",
        params![
            lease.agent_id,
            lease.parent_id,
            lease.state,
            lease.expires_at,
            clips,
            clip_cids,
            lease.swarm_task_id,
            now_secs()
        ],
    )?;
    Ok(())
}

fn load_lease(conn: &Connection, agent_id: &str) -> Result<Option<Lease>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, parent_id, state, expires_at, clips, clip_cids, swarm_task_id, tools_allow, dest_allow, spend_cap_usd, spend_usd, irreversible, approvals FROM leases WHERE agent_id=?1",
    )?;
    let mut rows = stmt.query(params![agent_id])?;
    let Some(r) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some(lease_from_row(r)?))
}

fn load_all(conn: &Connection) -> Result<Vec<Lease>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, parent_id, state, expires_at, clips, clip_cids, swarm_task_id, tools_allow, dest_allow, spend_cap_usd, spend_usd, irreversible, approvals FROM leases ORDER BY agent_id",
    )?;
    let rows = stmt.query_map([], lease_from_row)?;
    rows.collect()
}

fn lease_from_row(r: &rusqlite::Row<'_>) -> Result<Lease, rusqlite::Error> {
    let clips: Vec<String> = serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default();
    let clip_cids: BTreeMap<String, String> =
        serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default();
    Ok(Lease {
        agent_id: r.get(0)?,
        parent_id: r.get(1)?,
        state: r.get(2)?,
        expires_at: r.get(3)?,
        clips,
        clip_cids,
        swarm_task_id: r.get(6)?,
        tools_allow: opt_list(r.get(7)?),
        dest_allow: opt_list(r.get(8)?),
        spend_cap_usd: r.get(9)?,
        spend_usd: r.get(10)?,
        irreversible: opt_list(r.get(11)?),
        approvals: serde_json::from_str(&r.get::<_, String>(12).unwrap_or_else(|_| "[]".into()))
            .unwrap_or_default(),
    })
}

fn descendant_ids(conn: &Connection, root: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut out = Vec::new();
    let mut queue = VecDeque::from([root.to_string()]);
    let mut seen = BTreeSet::from([root.to_string()]);
    while let Some(id) = queue.pop_front() {
        let mut stmt = conn.prepare("SELECT agent_id FROM leases WHERE parent_id=?1")?;
        let children: Vec<String> = stmt
            .query_map(params![id], |r| r.get(0))?
            .filter_map(Result::ok)
            .collect();
        for child in children {
            if seen.insert(child.clone()) {
                out.push(child.clone());
                queue.push_back(child);
            }
        }
    }
    Ok(out)
}

/// One exported evidence row. Tokens only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportRow {
    /// `observed` or `inferred`.
    pub class: String,
    /// Machine kind token.
    pub kind: String,
    /// Content id.
    pub cid: String,
    /// sha256 of the payload.
    pub payload_sha256: String,
    /// Premise cids. Non-empty when `class` is `inferred`.
    pub premises: Vec<String>,
    /// Lease key.
    pub agent_id: String,
    /// Tool token, when the record has one.
    pub tool: Option<String>,
    /// `allow` or `deny`, when the record is a decision.
    pub decision: Option<String>,
    /// Deny or ok code, when the record has one.
    pub code: Option<String>,
    /// Unix seconds from the store row.
    pub ts: i64,
}

/// JSONL when `json` is true. Otherwise one `key=value` line per row.
pub fn format_export(rows: &[ExportRow], json: bool) -> String {
    let mut out = String::new();
    for row in rows {
        if json {
            if let Ok(line) = serde_json::to_string(row) {
                out.push_str(&line);
                out.push('\n');
            }
        } else {
            out.push_str(&format!(
                "class={} kind={} cid={} payload_sha256={} premises={} agent_id={} tool={} decision={} code={} ts={}\n",
                row.class,
                row.kind,
                row.cid,
                row.payload_sha256,
                if row.premises.is_empty() {
                    "-".to_string()
                } else {
                    row.premises.join(",")
                },
                row.agent_id,
                row.tool.as_deref().unwrap_or("-"),
                row.decision.as_deref().unwrap_or("-"),
                row.code.as_deref().unwrap_or("-"),
                row.ts
            ));
        }
    }
    out
}

fn status_of(lease: Lease) -> AgentStatus {
    AgentStatus {
        agent_id: lease.agent_id,
        state: lease.state,
        expires_at: lease.expires_at,
        parent_id: lease.parent_id,
        clips: lease.clips,
        swarm_task_id: lease.swarm_task_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailSink;

    impl EvidenceSink for FailSink {
        fn append(&self, _record: &EvidenceRecord) -> Result<(), SinkError> {
            Err(SinkError::Unavailable)
        }
    }

    fn gate() -> (tempfile::TempDir, Deadbolt) {
        let dir = tempfile::tempdir().unwrap();
        let db = Deadbolt::open_at(dir.path(), true, 60);
        (dir, db)
    }

    #[test]
    fn post_kill_shell_deny_is_killed_not_store_unavailable() {
        for _ in 0..20 {
            let (_dir, db) = gate();
            db.ensure_agent("A").unwrap();
            db.ensure_agent("B").unwrap();
            db.kill("A").unwrap();
            assert_eq!(
                db.admit("A", "shell"),
                AdmitDecision::Deny {
                    code: DenyCode::Killed
                }
            );
            assert!(matches!(db.admit("B", "shell"), AdmitDecision::Allow));
        }
    }

    #[test]
    fn killed_deny_kept_when_sink_append_fails() {
        let (_dir, mut db) = gate();
        db.ensure_agent("A").unwrap();
        db.kill("A").unwrap();
        db.sink = Some(Arc::new(FailSink));
        assert_eq!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        );
    }

    #[test]
    fn allow_write_failure_fail_closed_is_store_unavailable() {
        let (_dir, mut db) = gate();
        db.ensure_agent("A").unwrap();
        db.sink = Some(Arc::new(FailSink));
        assert_eq!(
            db.admit("A", "read_file"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            }
        );
    }

    #[test]
    fn admit_denies_after_kill_in_process() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        assert!(matches!(db.admit("A", "read_file"), AdmitDecision::Allow));
        db.kill("A").unwrap();
        assert_eq!(
            db.admit("A", "read_file"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        );
    }

    #[test]
    fn parent_kill_denies_child_first_admit() {
        let (_dir, db) = gate();
        db.ensure_agent("P").unwrap();
        db.ensure_agent("B").unwrap();
        assert!(db.register_child("P", "C", Some("swarm-fake")).unwrap());
        assert!(matches!(db.admit("C", "read_file"), AdmitDecision::Allow));
        db.kill("P").unwrap();
        assert_eq!(
            db.admit("C", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        );
        assert!(matches!(db.admit("B", "shell"), AdmitDecision::Allow));
    }

    #[test]
    fn child_denied_after_parent_kill() {
        let (_dir, db) = gate();
        db.ensure_agent("parent").unwrap();
        assert!(db
            .register_child("parent", "child", Some("task-1"))
            .unwrap());
        let report = db.kill("parent").unwrap();
        assert!(report.revoked.contains(&"child".to_string()));
        assert!(report.swarm_task_ids.contains(&"task-1".to_string()));
        assert_eq!(
            db.admit("child", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        );
    }

    #[test]
    fn kill_agent_a_does_not_block_agent_b() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.ensure_agent("B").unwrap();
        db.kill("A").unwrap();
        assert!(matches!(db.admit("B", "read_file"), AdmitDecision::Allow));
        assert!(matches!(
            db.admit("A", "read_file"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        ));
    }

    #[test]
    fn unwritable_store_fail_closed_denies() {
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("not-a-directory");
        fs::write(&blocked, b"x").unwrap();
        let db = Deadbolt::open_paths(
            &blocked.join("deadbolt.db"),
            &blocked.join("events.jsonl"),
            true,
            true,
            60,
        );
        assert_eq!(
            db.admit("A", "read_file"),
            AdmitDecision::Deny {
                code: DenyCode::StoreUnavailable
            }
        );
    }

    #[test]
    fn expired_lease_denies_without_sliding() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.force_expire("A").unwrap();
        assert_eq!(
            db.admit("A", "read_file"),
            AdmitDecision::Deny {
                code: DenyCode::LeaseExpired
            }
        );
        assert_eq!(
            db.admit("A", "read_file"),
            AdmitDecision::Deny {
                code: DenyCode::LeaseExpired
            }
        );
    }

    #[test]
    fn clip_shell_allows_read_file_and_resume_works() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.clip("A", "shell").unwrap();
        assert_eq!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        );
        assert!(matches!(db.admit("A", "read_file"), AdmitDecision::Allow));
        db.resume("A").unwrap();
        assert!(matches!(db.admit("A", "shell"), AdmitDecision::Allow));
    }

    #[test]
    fn purpose_exceeded_is_inferred_with_premises_and_observed_has_no_prose() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.clip("A", "shell").unwrap();
        let _ = db.admit("A", "shell");
        let events = db.events();
        let inferred: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "purpose_exceeded")
            .collect();
        assert_eq!(inferred.len(), 1);
        assert_eq!(inferred[0].class, EpistemicClass::Inferred);
        assert!(inferred[0].premises.len() >= 2);
        assert!(inferred[0]
            .premises
            .iter()
            .all(|p| p.starts_with("sha256:")));
        for ev in &events {
            if ev.class == EpistemicClass::Observed {
                assert_structured(&ev.payload).expect("observed prose");
                assert!(ev.premises.is_empty());
            }
            assert!(ev.cid.starts_with("sha256:"));
            assert!(ev.payload_sha256.starts_with("sha256:"));
        }
        let jsonl = fs::read_to_string(_dir.path().join("deadbolt-events.jsonl")).unwrap();
        assert!(jsonl.contains("purpose_exceeded"));
        assert!(!jsonl.contains("purpose exceeded"));
    }

    #[test]
    fn drill_ok() {
        Deadbolt::drill().unwrap();
    }

    #[test]
    fn shutdown_names_are_flagged() {
        assert!(is_shutdown_tool("shutdown"));
        assert!(is_shutdown_tool("kill"));
        assert!(is_shutdown_tool("deadbolt"));
        assert!(!is_shutdown_tool("read_file"));
        assert!(!is_shutdown_tool("shell"));
    }

    #[test]
    fn inferred_purpose_exceeded_has_premises() {
        let dir = tempfile::tempdir().unwrap();
        let witness_db = dir.path().join("witness.db");
        let db = Deadbolt::open(&DeadboltConfig {
            db_path: Some(dir.path().join("deadbolt.db").display().to_string()),
            events_path: Some(dir.path().join("events.jsonl").display().to_string()),
            witness: true,
            witness_db: Some(witness_db.display().to_string()),
            ..DeadboltConfig::default()
        });
        db.ensure_agent("A").unwrap();
        db.clip("A", "shell").unwrap();
        assert_eq!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        );
        let text = fs::read_to_string(dir.path().join("witness.jsonl")).unwrap();
        assert!(!text.contains("generated"));
        let mut saw = false;
        for line in text.lines().filter(|l| !l.is_empty()) {
            let row: Value = serde_json::from_str(line).unwrap();
            let class = row["epistemic_type"].as_str().unwrap();
            assert!(class == "observed" || class == "inferred");
            if row["kind"] == "purpose_exceeded" {
                saw = true;
                assert_eq!(class, "inferred");
                assert!(row["premises"].as_array().unwrap().len() >= 2);
                assert!(row["cid"].as_str().unwrap().starts_with("sha256:"));
            }
            if class == "observed" {
                assert!(row["premises"].as_array().unwrap().is_empty());
            }
        }
        assert!(saw);
    }

    #[test]
    fn killed_deny_not_rewritten_when_witness_append_fails() {
        let (_dir, mut db) = gate();
        db.ensure_agent("A").unwrap();
        db.kill("A").unwrap();
        db.witness = Some(Arc::new(FailSink));
        assert_eq!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::Killed
            }
        );
    }

    #[test]
    fn export_includes_killed_decision_for_agent() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.kill("A").unwrap();
        db.admit("A", "shell");
        let rows = db.export("A", false).unwrap();
        let killed = rows.iter().find(|r| r.code.as_deref() == Some("killed"));
        let killed = killed.expect("killed row");
        assert_eq!(killed.class, "observed");
        assert_eq!(killed.kind, "decision");
        assert_eq!(killed.agent_id, "A");
        assert_eq!(killed.tool.as_deref(), Some("shell"));
        assert_eq!(killed.decision.as_deref(), Some("deny"));
        assert!(killed.cid.starts_with("sha256:"));
        assert!(killed.payload_sha256.starts_with("sha256:"));
        assert!(killed.ts > 0);
        let text = format_export(&rows, true);
        assert!(text.contains("\"code\":\"killed\""));
        assert!(!text.contains("killed because"));
    }

    #[test]
    fn export_includes_child_when_flag_set() {
        let (_dir, db) = gate();
        db.ensure_agent("P").unwrap();
        db.register_child("P", "C", None).unwrap();
        db.admit("C", "shell");
        let alone = db.export("P", false).unwrap();
        assert!(alone.iter().all(|r| r.agent_id != "C"));
        let with = db.export("P", true).unwrap();
        assert!(with.iter().any(|r| r.agent_id == "C"));
    }

    #[test]
    fn export_omits_unrelated_agent() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.ensure_agent("B").unwrap();
        db.admit("A", "shell");
        db.admit("B", "read_file");
        let rows = db.export("A", false).unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|r| r.agent_id == "A"));
        assert!(rows.iter().all(|r| r.tool.as_deref() != Some("read_file")));
    }

    #[test]
    fn inferred_rows_in_export_have_premises() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.clip("A", "shell").unwrap();
        db.admit("A", "shell");
        let rows = db.export("A", false).unwrap();
        let inferred: Vec<_> = rows.iter().filter(|r| r.class == "inferred").collect();
        assert!(!inferred.is_empty());
        assert!(inferred.iter().all(|r| !r.premises.is_empty()));
        assert!(inferred.iter().all(|r| r.kind == "purpose_exceeded"));
    }

    #[test]
    fn disabled_does_not_attach() {
        let db = Deadbolt::open(&DeadboltConfig {
            enabled: false,
            ..DeadboltConfig::default()
        });
        assert!(!db.should_attach());
        assert!(matches!(db.admit("A", "shell"), AdmitDecision::Allow));
    }

    #[test]
    fn policy_deny_off_list_tool() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                tools_allow: Some(vec!["read_file".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        assert!(matches!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        ));
        assert!(matches!(db.admit("A", "read_file"), AdmitDecision::Allow));
        let rows = db.export("A", false).unwrap();
        let inferred: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == "purpose_exceeded")
            .collect();
        assert!(!inferred.is_empty());
        assert!(inferred
            .iter()
            .all(|r| r.premises.iter().any(|p| p.starts_with("sha256:"))));
    }

    #[test]
    fn dest_deny() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                dest_allow: Some(vec!["api.stripe.com".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        assert!(matches!(db.admit("A", "shell"), AdmitDecision::Allow));
        assert!(matches!(
            db.admit_dest("A", "shell", Some("github.com")),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        ));
        assert!(matches!(
            db.admit_dest("A", "shell", Some("api.stripe.com")),
            AdmitDecision::Allow
        ));
    }

    #[test]
    fn spend_cap_pause() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                spend_cap_usd: Some(5.0),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        let under = db.spend_add("A", 1.0).unwrap();
        assert!(!under.paused);
        assert!(matches!(db.admit("A", "shell"), AdmitDecision::Allow));
        let over = db.spend_add("A", 5.0).unwrap();
        assert!(over.paused);
        assert_eq!(db.status(Some("A")).unwrap()[0].state, "paused");
        assert!(matches!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::SpendCap
            }
        ));
    }

    #[test]
    fn irreversible_needs_human_then_approve_allows_once() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                irreversible: Some(vec!["shell".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        assert!(matches!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::NeedsHuman
            }
        ));
        db.approve("A", "shell").unwrap();
        assert!(matches!(db.admit("A", "shell"), AdmitDecision::Allow));
        assert!(matches!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::NeedsHuman
            }
        ));
    }

    #[test]
    fn incident_json_has_killed_decision() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.register_child("A", "C", None).unwrap();
        db.admit("A", "shell");
        db.kill("A").unwrap();
        db.admit("A", "shell");
        let raw = db.incident("A").unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["agent"], "A");
        assert!(v["children"].as_array().unwrap().iter().any(|c| c == "C"));
        assert!(v["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "killed"));
        assert!(v["killed_at"].is_number());
        assert!(v.get("summary").is_none());
    }

    #[test]
    fn dest_allow_does_not_block_write_file_without_host() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                dest_allow: Some(vec!["github.com".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        assert!(matches!(db.admit("A", "write_file"), AdmitDecision::Allow));
        assert!(matches!(db.admit("A", "shell"), AdmitDecision::Allow));
    }

    #[test]
    fn dest_allow_blocks_foreign_host() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                dest_allow: Some(vec!["github.com".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        assert!(matches!(
            db.admit_dest("A", "write_file", Some("evil.example")),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        ));
        assert!(matches!(
            db.admit_dest("A", "shell", Some("github.com")),
            AdmitDecision::Allow
        ));
    }

    #[test]
    fn dest_allow_blocks_network_tool_without_host() {
        let (_dir, db) = gate();
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                dest_allow: Some(vec!["github.com".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        for tool in ["http", "fetch", "browser", "web_search"] {
            assert!(
                matches!(
                    db.admit("A", tool),
                    AdmitDecision::Deny {
                        code: DenyCode::PurposeExceeded
                    }
                ),
                "{tool}"
            );
        }
    }

    #[test]
    fn deny_renews_lease_ttl() {
        let dir = tempfile::tempdir().unwrap();
        let db = Deadbolt::open_at(dir.path(), true, 60);
        db.ensure_agent("A").unwrap();
        db.set_policy(
            "A",
            PolicyPatch {
                tools_allow: Some(vec!["read_file".into()]),
                ..PolicyPatch::default()
            },
        )
        .unwrap();
        let soon = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 5;
        {
            let conn = rusqlite::Connection::open(dir.path().join("deadbolt.db")).unwrap();
            conn.execute(
                "UPDATE leases SET expires_at=?1 WHERE agent_id='A'",
                rusqlite::params![soon],
            )
            .unwrap();
        }
        assert!(matches!(
            db.admit("A", "shell"),
            AdmitDecision::Deny {
                code: DenyCode::PurposeExceeded
            }
        ));
        let after = db.status(Some("A")).unwrap()[0].expires_at;
        assert!(after > soon, "deny must slide TTL: {after} <= {soon}");
    }
}
