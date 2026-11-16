//! Gate-emitted trace records (TR-1 to TR-3 in `docs/TRACE-REQUIREMENTS.md`).
//!
//! The trace build's one job: make DeadBolt's admission decisions into
//! evidence that does not depend on the worker's cooperation or narration.
//!
//! - TR-1: every admission decision is recorded by the gate itself, at
//!   decision time, out-of-band from the worker. [`AdmissionRecord`]s can
//!   only be created inside this crate (`record_admission` is `pub(crate)`),
//!   so worker code can never forge, complete, or replace one.
//! - TR-2: reconciliation matches observed effects against gate admission
//!   records by natural-identity fingerprint. An observed effect with no
//!   matching admission is a fail-closed alarm
//!   ([`UnmatchedEffectAlarm`]), never a logging gap or a warning. An
//!   admission with no observed effect is reported, not alarmed: an admitted
//!   action may legitimately produce no effect.
//! - TR-3: fingerprints and idempotency keys derive from the effect's
//!   natural identity — canonical destination, canonical amount, and the
//!   applicable time window — never from a caller-generated request ID.
//!   Fingerprint construction is canonicalization-versioned
//!   ([`CANONICALIZATION_VERSION`]).
//!
//! Evidence labeling: a worker's own statement about what it was allowed to
//! do is stored separately as a [`WorkerSummary`], is always labeled
//! [`EpistemicClass::Generated`], and is never promoted into an admission
//! record or consulted by reconciliation.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{hex_encode, is_token, now_secs, AdmitDecision, DenyCode, EpistemicClass};

/// Canonicalization version for effect fingerprints. A change in canonical
/// form bumps this version, so one effect's identity cannot silently split
/// across two fingerprints during an upgrade.
pub const CANONICALIZATION_VERSION: &str = "deadbolt-effect-v1";

/// Default effect-matching window, in seconds. Admissions and observed
/// effects are bucketed into windows of this size for TR-2 matching.
pub const EFFECT_WINDOW_SECS: u64 = 300;

/// A fixed time window an effect belongs to. Part of its natural identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    /// First Unix second of the window.
    pub start: i64,
    /// Window length in seconds.
    pub secs: u64,
}

impl TimeWindow {
    /// The window of length `secs` containing `ts`. `secs` must be non-zero.
    pub fn containing(ts: i64, secs: u64) -> Self {
        let secs = secs.max(1) as i64;
        let start = ts.div_euclid(secs) * secs;
        Self {
            start,
            secs: secs as u64,
        }
    }

    /// The default-length window containing `ts`.
    pub fn for_timestamp(ts: i64) -> Self {
        Self::containing(ts, EFFECT_WINDOW_SECS)
    }
}

/// An effect's natural identity: where it went, how much it carried, and
/// the window it belongs to. This — never a generated request ID — is what
/// fingerprints and idempotency receipts key on (TR-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectIdentity {
    /// Canonical destination token; `None` for local effects.
    pub destination: Option<String>,
    /// Canonical amount token (decimal string or safe-integer minor units),
    /// when the effect carries an amount.
    pub amount: Option<String>,
    /// Applicable time window.
    pub window: TimeWindow,
}

impl EffectIdentity {
    /// Build an identity from its natural parts.
    pub fn new(destination: Option<&str>, amount: Option<&str>, window: TimeWindow) -> Self {
        Self {
            destination: destination.map(str::to_string),
            amount: amount.map(str::to_string),
            window,
        }
    }

    /// The identity for an effect observed at `ts`, using the default window.
    pub fn at(destination: Option<&str>, amount: Option<&str>, ts: i64) -> Self {
        Self::new(destination, amount, TimeWindow::for_timestamp(ts))
    }

    /// Validate the natural parts. An identity that cannot be resolved
    /// fails closed: callers must treat an error here as an alarm
    /// condition, never as permission to proceed unidentified.
    pub fn validate(&self) -> Result<(), TraceError> {
        if let Some(dest) = &self.destination {
            if !is_token(dest) {
                return Err(TraceError::UnresolvableIdentity);
            }
        }
        if let Some(amount) = &self.amount {
            if !is_token(amount) {
                return Err(TraceError::UnresolvableIdentity);
            }
        }
        if self.window.secs == 0 || self.window.start < 0 {
            return Err(TraceError::UnresolvableIdentity);
        }
        Ok(())
    }

    /// Canonicalization-versioned SHA-256 fingerprint of this natural
    /// identity. Object order and JSON whitespace do not affect it. No
    /// request ID, nonce, or other caller-generated token is an input.
    pub fn fingerprint(&self) -> Result<String, TraceError> {
        self.validate()?;
        let envelope = json!({
            "amount": self.amount,
            "canonicalization": CANONICALIZATION_VERSION,
            "destination": self.destination,
            "window_secs": self.window.secs,
            "window_start": self.window.start,
        });
        let bytes = serde_jcs::to_vec(&envelope).map_err(|_| TraceError::UnresolvableIdentity)?;
        let mut hasher = Sha256::new();
        hasher.update(CANONICALIZATION_VERSION.as_bytes());
        hasher.update(b"\0");
        hasher.update(bytes);
        Ok(format!("sha256:{}", hex_encode(&hasher.finalize())))
    }

    /// The idempotency key for this effect. Identical to the fingerprint by
    /// construction: a retry under a fresh request ID produces the same key
    /// and is deduplicated, not re-executed.
    pub fn idempotency_key(&self) -> Result<String, TraceError> {
        self.fingerprint()
    }
}

/// An effect observed in the world, as reported by an observer such as
/// Witness. `request_id` may be carried for diagnostics; it is never part
/// of the fingerprint and never influences matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedEffect {
    /// The effect's natural identity.
    pub identity: EffectIdentity,
    /// Caller-generated request ID, if one was supplied. Ignored by
    /// fingerprinting and reconciliation.
    pub request_id: Option<String>,
    /// Unix second the effect was observed.
    pub observed_at: i64,
}

impl ObservedEffect {
    /// An observed effect with no request ID.
    pub fn new(identity: EffectIdentity, observed_at: i64) -> Self {
        Self {
            identity,
            request_id: None,
            observed_at,
        }
    }

    /// This effect's natural-identity fingerprint.
    pub fn fingerprint(&self) -> Result<String, TraceError> {
        self.identity.fingerprint()
    }
}

/// Whether the gate admitted or denied an action, with the structured
/// denial code when denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// The gate admitted the action.
    Admit,
    /// The gate denied the action; `code` is the structured denial reason.
    Deny {
        /// Machine denial code token.
        code: DenyCode,
    },
}

impl From<AdmitDecision> for AdmissionOutcome {
    fn from(decision: AdmitDecision) -> Self {
        match decision {
            AdmitDecision::Allow => Self::Admit,
            AdmitDecision::Deny { code } => Self::Deny { code },
        }
    }
}

/// A gate-owned admission record (TR-1). Emitted by DeadBolt itself at
/// decision time, out-of-band from the worker. The only constructor is
/// crate-private, and [`TraceLog::record_admission`] is `pub(crate)`, so
/// code outside this crate — including the worker being checked — cannot
/// create, modify, complete, or replace one of these records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRecord {
    /// Natural-identity fingerprint of the admitted/denied effect (TR-3).
    pub effect_fingerprint: String,
    /// Exact-action fingerprint of the reviewed envelope, when the decision
    /// came through exact-action admission. The grant nonce is part of that
    /// approval record; it is never the effect-matching key.
    pub action_fingerprint: Option<String>,
    /// Admit or deny, with the structured denial reason when denied.
    pub outcome: AdmissionOutcome,
    /// The credential/lease identity the gate evaluated (the agent id).
    pub agent_id: String,
    /// Routed tool identity evaluated.
    pub tool: String,
    /// Natural identity the decision was about.
    pub identity: EffectIdentity,
    /// Unix second the gate made the decision.
    pub decided_at: i64,
    /// Who emitted this record. Always `gate`; worker narration is a
    /// [`WorkerSummary`], never this.
    pub emitter: String,
    /// Evidence class. Always [`EpistemicClass::Observed`]: the gate
    /// measured its own decision.
    pub class: EpistemicClass,
}

impl AdmissionRecord {
    pub(crate) fn from_gate(
        identity: EffectIdentity,
        action_fingerprint: Option<String>,
        decision: AdmitDecision,
        agent_id: &str,
        tool: &str,
    ) -> Result<Self, TraceError> {
        Ok(Self {
            effect_fingerprint: identity.fingerprint()?,
            action_fingerprint,
            outcome: AdmissionOutcome::from(decision),
            agent_id: agent_id.to_string(),
            tool: tool.to_string(),
            identity,
            decided_at: now_secs(),
            emitter: "gate".to_string(),
            class: EpistemicClass::Observed,
        })
    }
}

/// A worker's own statement about its run. Stored separately from admission
/// records, always labeled [`EpistemicClass::Generated`], and never promoted
/// into an authorization/admission record or consulted by reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSummary {
    /// Agent the summary claims to describe.
    pub agent_id: String,
    /// The outcome the worker claims it was given (its own narration).
    pub claimed_outcome: String,
    /// Effect fingerprint the worker claims its action had, if it names one.
    pub claimed_effect_fingerprint: Option<String>,
}

impl WorkerSummary {
    /// The evidence class a worker summary always carries. It can never be
    /// [`EpistemicClass::Observed`].
    pub fn evidence_class(&self) -> EpistemicClass {
        EpistemicClass::Generated
    }
}

/// Fail-closed alarm (TR-2): an observed effect with no matching gate
/// admission record. This is an alarm condition, not a logging gap, a
/// warning, or a statistic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnmatchedEffectAlarm {
    /// Natural-identity fingerprint of the unmatched effect.
    pub effect_fingerprint: String,
    /// The unmatched effect's observed natural identity.
    pub identity: EffectIdentity,
    /// Unix second the effect was observed.
    pub observed_at: i64,
}

impl UnmatchedEffectAlarm {
    /// Stable machine token for this alarm condition.
    pub fn as_str(&self) -> &'static str {
        "effect_without_admission"
    }
}

/// Trace failure. Every variant fails closed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TraceError {
    /// An effect's natural identity could not be resolved to a valid
    /// fingerprint. Fail closed: alarm, never admission.
    #[error("deadbolt:unresolvable_identity")]
    UnresolvableIdentity,
}

/// The outcome of reconciling observed effects against gate admissions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconciliationReport {
    /// Effect fingerprints that matched an admission record.
    pub matched: Vec<String>,
    /// Fail-closed alarms: observed effects with no matching admission.
    pub alarms: Vec<UnmatchedEffectAlarm>,
    /// Admissions with no observed effect. Reported, not alarmed: an
    /// admitted action may legitimately produce no effect.
    pub admissions_without_effect: Vec<AdmissionRecord>,
}

impl ReconciliationReport {
    /// True when every observed effect matched an admission.
    pub fn is_clean(&self) -> bool {
        self.alarms.is_empty()
    }

    /// True when at least one fail-closed alarm was raised.
    pub fn has_alarm(&self) -> bool {
        !self.alarms.is_empty()
    }

    /// Fail-closed check: `Err` carrying the first alarm when any observed
    /// effect had no matching admission.
    pub fn require_clean(&self) -> Result<(), UnmatchedEffectAlarm> {
        match self.alarms.first() {
            Some(alarm) => Err(alarm.clone()),
            None => Ok(()),
        }
    }
}

/// The gate-owned trace store. Holds admission records emitted by the gate
/// and, separately, worker summaries that are never promoted. Lives in the
/// gate, not the worker: killing or replacing the worker does not alter or
/// erase the admission records already emitted.
#[derive(Debug, Default)]
pub struct TraceLog {
    admissions: Mutex<Vec<AdmissionRecord>>,
    worker_summaries: Mutex<Vec<WorkerSummary>>,
}

impl TraceLog {
    /// An empty trace log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a gate admission decision. Crate-private: only gate code can
    /// emit an admission record (TR-1).
    pub(crate) fn record_admission(&self, record: AdmissionRecord) {
        if let Ok(mut guard) = self.admissions.lock() {
            guard.push(record);
        }
    }

    /// Store a worker's own summary, labeled Generated. This never creates,
    /// completes, or alters an admission record.
    pub fn record_worker_summary(&self, summary: WorkerSummary) {
        if let Ok(mut guard) = self.worker_summaries.lock() {
            guard.push(summary);
        }
    }

    /// All gate admission records, in decision order.
    pub fn admissions(&self) -> Vec<AdmissionRecord> {
        self.admissions
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// All worker summaries, kept separate from admission records.
    pub fn worker_summaries(&self) -> Vec<WorkerSummary> {
        self.worker_summaries
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Reconcile observed effects against gate admission records by
    /// natural-identity fingerprint (TR-2). Only `Admit` admissions match
    /// an effect; a denial admits nothing, so an effect matching only a
    /// denied identity still alarms. Worker summaries are never consulted.
    pub fn reconcile(&self, effects: &[ObservedEffect]) -> ReconciliationReport {
        let admissions = self.admissions();
        let mut report = ReconciliationReport::default();
        let mut matched_admissions: Vec<bool> = vec![false; admissions.len()];
        for effect in effects {
            let fingerprint = match effect.fingerprint() {
                Ok(fp) => fp,
                Err(_) => {
                    // Unresolvable identity fails closed, in the same bucket
                    // as unmatched: alarm, never admission.
                    report.alarms.push(UnmatchedEffectAlarm {
                        effect_fingerprint: "unresolvable".to_string(),
                        identity: effect.identity.clone(),
                        observed_at: effect.observed_at,
                    });
                    continue;
                }
            };
            let hit = admissions.iter().position(|record| {
                record.outcome == AdmissionOutcome::Admit
                    && record.effect_fingerprint == fingerprint
            });
            match hit {
                Some(idx) => {
                    matched_admissions[idx] = true;
                    report.matched.push(fingerprint);
                }
                None => report.alarms.push(UnmatchedEffectAlarm {
                    effect_fingerprint: fingerprint,
                    identity: effect.identity.clone(),
                    observed_at: effect.observed_at,
                }),
            }
        }
        for (record, matched) in admissions.into_iter().zip(matched_admissions) {
            if !matched && record.outcome == AdmissionOutcome::Admit {
                report.admissions_without_effect.push(record);
            }
        }
        report
    }

    /// Reconcile and fail closed: any unmatched observed effect is an
    /// error, not a warning.
    pub fn reconcile_or_alarm(
        &self,
        effects: &[ObservedEffect],
    ) -> Result<ReconciliationReport, UnmatchedEffectAlarm> {
        let report = self.reconcile(effects);
        report.require_clean()?;
        Ok(report)
    }
}

/// Extract the canonical amount token from exact-action arguments, when the
/// effect carries one under `amount`. Strings are taken as-is; numbers use
/// their canonical JSON form, so `1` and `1.0` agree. Anything else carries
/// no amount for fingerprinting.
pub(crate) fn amount_from_arguments(arguments: &Value) -> Option<String> {
    match arguments.get("amount") {
        Some(Value::String(raw)) if is_token(raw) => Some(raw.clone()),
        Some(Value::Number(num)) => {
            let canonical = serde_jcs::to_string(&Value::Number(num.clone())).ok()?;
            if is_token(&canonical) {
                Some(canonical)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> EffectIdentity {
        EffectIdentity::at(Some("pay.example.com"), Some("100"), 1_700_000_000)
    }

    #[test]
    fn fingerprint_ignores_request_id_and_matches_natural_identity() {
        let a = ObservedEffect {
            identity: identity(),
            request_id: Some("req-aaa".into()),
            observed_at: 1_700_000_000,
        };
        let b = ObservedEffect {
            identity: identity(),
            request_id: Some("req-bbb-fresh-retry".into()),
            observed_at: 1_700_000_010,
        };
        assert_eq!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
        assert_eq!(
            a.identity.idempotency_key().unwrap(),
            b.identity.idempotency_key().unwrap()
        );
    }

    #[test]
    fn fingerprint_changes_with_destination_amount_or_window() {
        let base = identity().fingerprint().unwrap();
        let other_dest = EffectIdentity::at(Some("other.example.com"), Some("100"), 1_700_000_000)
            .fingerprint()
            .unwrap();
        let other_amount = EffectIdentity::at(Some("pay.example.com"), Some("101"), 1_700_000_000)
            .fingerprint()
            .unwrap();
        let other_window = EffectIdentity::at(
            Some("pay.example.com"),
            Some("100"),
            1_700_000_000 + EFFECT_WINDOW_SECS as i64,
        )
        .fingerprint()
        .unwrap();
        assert_ne!(base, other_dest);
        assert_ne!(base, other_amount);
        assert_ne!(base, other_window);
    }

    #[test]
    fn unresolvable_identity_fails_closed() {
        let bad = EffectIdentity::new(Some("not a token!"), None, TimeWindow::for_timestamp(0));
        assert_eq!(bad.fingerprint(), Err(TraceError::UnresolvableIdentity));
    }
}
