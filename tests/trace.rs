//! TR-1 to TR-3 acceptance tests (docs/TRACE-REQUIREMENTS.md), exercised
//! through the public gate API only. Worker code outside the crate cannot
//! construct an admission record; these tests use the real gate paths.

use deadbolt::{
    ActionRequest, AdmitDecision, Deadbolt, DenyCode, EffectIdentity, EpistemicClass,
    ObservedEffect, WorkerSummary, EFFECT_WINDOW_SECS,
};
use serde_json::json;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// TR-1: an admission through the gate produces a gate-emitted admission
/// record, out-of-band from the worker. A hostile worker summary claiming a
/// clean run does not alter the gate's records, and a worker summary is
/// never promoted into an admission record.
#[test]
fn gate_emits_admission_record_and_worker_summary_never_promotes() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("run").unwrap();

    assert_eq!(
        gate.admit_dest("run", "send", Some("pay.example.com")),
        AdmitDecision::Allow
    );

    let records = gate.admission_records();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.emitter, "gate");
    assert_eq!(record.class, EpistemicClass::Observed);
    assert_eq!(record.agent_id, "run");
    assert_eq!(record.tool, "send");

    // Hostile stub: the worker narrates a fabricated clean trace, claiming
    // it was admitted for an effect the gate never decided on.
    let fabricated = EffectIdentity::at(Some("evil.example.com"), Some("999"), now())
        .fingerprint()
        .unwrap();
    gate.trace_log().record_worker_summary(WorkerSummary {
        agent_id: "run".into(),
        claimed_outcome: "allow".into(),
        claimed_effect_fingerprint: Some(fabricated.clone()),
    });

    // The gate's admission records are unchanged by the worker's narration.
    let after = gate.admission_records();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0], records[0]);

    // The summary stays labeled Generated, in its own list.
    let summaries = gate.trace_log().worker_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].evidence_class(), EpistemicClass::Generated);

    // And it does not satisfy reconciliation for the fabricated effect:
    // the inverted audit still alarms (TR-2).
    let effect = ObservedEffect::new(
        EffectIdentity::at(Some("evil.example.com"), Some("999"), now()),
        now(),
    );
    let report = gate.reconcile_effects(&[effect]);
    assert!(report.has_alarm());
    assert_eq!(report.alarms[0].effect_fingerprint, fabricated);
}

/// TR-1 acceptance, denial side: a denied decision also produces a
/// gate-emitted record carrying the structured denial reason.
#[test]
fn denied_decision_produces_gate_record_with_denial_reason() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    // No lease for "ghost": admission denies with NoLease.
    assert_eq!(
        gate.admit_dest("ghost", "send", Some("pay.example.com")),
        AdmitDecision::Deny {
            code: DenyCode::NoLease
        }
    );
    let records = gate.admission_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].emitter, "gate");
    assert_eq!(
        records[0].outcome,
        deadbolt::AdmissionOutcome::Deny {
            code: DenyCode::NoLease
        }
    );
}

/// TR-2: an effect produced outside the gate raises the fail-closed alarm
/// on reconciliation, with the effect's observed identity attached. A
/// matching admitted effect reconciles silently. Admission without effect
/// is reported, not alarmed.
#[test]
fn effect_without_admission_is_a_fail_closed_alarm() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("run").unwrap();
    assert_eq!(
        gate.admit_dest("run", "send", Some("pay.example.com")),
        AdmitDecision::Allow
    );

    // The admitted effect, observed in the world, matches silently even
    // though the observer saw it under a fresh request ID.
    let admitted = ObservedEffect {
        identity: EffectIdentity::at(Some("pay.example.com"), None, now()),
        request_id: Some("fresh-retry-id".into()),
        observed_at: now(),
    };
    let clean = gate.reconcile_effects(&[admitted]);
    assert!(clean.is_clean());
    assert_eq!(clean.matched.len(), 1);
    assert!(gate
        .trace_log()
        .reconcile_or_alarm(&[ObservedEffect::new(
            EffectIdentity::at(Some("pay.example.com"), None, now()),
            now()
        )])
        .is_ok());

    // An unwrapped route fires an effect the gate never admitted.
    let rogue = ObservedEffect::new(
        EffectIdentity::at(Some("rogue.example.com"), None, now()),
        now(),
    );
    let report = gate.reconcile_effects(&[rogue]);
    assert!(report.has_alarm());
    assert_eq!(report.alarms.len(), 1);
    assert_eq!(report.alarms[0].as_str(), "effect_without_admission");
    assert_eq!(
        report.alarms[0].identity.destination.as_deref(),
        Some("rogue.example.com")
    );
    // Fail-closed form: the checked variant returns the alarm as an error.
    assert!(gate
        .trace_log()
        .reconcile_or_alarm(&[ObservedEffect::new(
            EffectIdentity::at(Some("rogue.example.com"), None, now()),
            now()
        )])
        .is_err());
}

/// TR-2 distinction: admission without effect is reported, not alarmed.
#[test]
fn admission_without_effect_is_reported_not_alarmed() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("run").unwrap();
    assert_eq!(
        gate.admit_dest("run", "send", Some("pay.example.com")),
        AdmitDecision::Allow
    );
    let report = gate.reconcile_effects(&[]);
    assert!(report.is_clean());
    assert_eq!(report.admissions_without_effect.len(), 1);
}

/// TR-3: fingerprints key on natural identity (destination, amount,
/// window), never a generated request ID. The same effect retried under a
/// fresh request ID produces the same fingerprint and idempotency key;
/// differing destination or amount produce different fingerprints.
#[test]
fn fingerprints_key_on_natural_identity_never_request_id() {
    let ts = now();
    let first = ObservedEffect {
        identity: EffectIdentity::at(Some("pay.example.com"), Some("250"), ts),
        request_id: Some("req-001".into()),
        observed_at: ts,
    };
    let retry = ObservedEffect {
        identity: EffectIdentity::at(Some("pay.example.com"), Some("250"), ts + 5),
        request_id: Some("req-002-completely-fresh".into()),
        observed_at: ts + 5,
    };
    assert_eq!(first.fingerprint().unwrap(), retry.fingerprint().unwrap());
    assert_eq!(
        first.identity.idempotency_key().unwrap(),
        retry.identity.idempotency_key().unwrap()
    );

    let other_amount = EffectIdentity::at(Some("pay.example.com"), Some("251"), ts);
    let other_dest = EffectIdentity::at(Some("other.example.com"), Some("250"), ts);
    let other_window = EffectIdentity::at(
        Some("pay.example.com"),
        Some("250"),
        ts + EFFECT_WINDOW_SECS as i64,
    );
    assert_ne!(
        first.fingerprint().unwrap(),
        other_amount.fingerprint().unwrap()
    );
    assert_ne!(
        first.fingerprint().unwrap(),
        other_dest.fingerprint().unwrap()
    );
    assert_ne!(
        first.fingerprint().unwrap(),
        other_window.fingerprint().unwrap()
    );
}

/// TR-1 + TR-3 through exact-action admission: the gate record carries the
/// exact-action fingerprint as approval data, while the effect-matching
/// fingerprint derives from the action's natural identity — destination
/// and the amount inside its arguments — not from the grant nonce.
#[test]
fn exact_action_admission_record_keys_effect_on_natural_identity() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Deadbolt::open_at(dir.path(), true, 60);
    gate.ensure_agent("run").unwrap();
    let action = ActionRequest::new(
        "run",
        "pay",
        Some("pay.example.com"),
        json!({"amount": "250", "memo": "reviewed"}),
        300,
    )
    .unwrap();
    gate.approve_action(&action).unwrap();
    assert_eq!(gate.admit_action(&action), Ok(AdmitDecision::Allow));

    let records = gate.admission_records();
    let record = records
        .iter()
        .find(|r| r.action_fingerprint.is_some())
        .expect("exact-action admission record");
    assert_eq!(record.emitter, "gate");
    assert_eq!(
        record.action_fingerprint.as_deref(),
        Some(action.fingerprint().unwrap().as_str())
    );
    // The nonce is not the effect key: the effect fingerprint equals the
    // natural-identity fingerprint computed independently.
    let natural = EffectIdentity::at(Some("pay.example.com"), Some("250"), now())
        .fingerprint()
        .unwrap();
    assert_eq!(record.effect_fingerprint, natural);
    assert!(!record.effect_fingerprint.contains(&action.nonce));

    // The observed effect reconciles against this admission.
    let effect = ObservedEffect::new(
        EffectIdentity::at(Some("pay.example.com"), Some("250"), now()),
        now(),
    );
    let report = gate.reconcile_effects(&[effect]);
    assert!(report.is_clean(), "alarms: {:?}", report.alarms);
}
