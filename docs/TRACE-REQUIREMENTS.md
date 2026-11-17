# Trace build requirements

Adopted 2026-11-10 by the owner's decision. Source: external review feedback
from DunimeTech on "DeepSeek Built a Super Agent. We Checked It Against a
Permission Gate" (CoderLegion, 2026-11-10). These are requirements for the
trace build — the gate-emitted evidence layer described in the
company's grant work. A requirement below is met only when
its acceptance evidence exists; a design, a plan, or a passing demo of
something else does not meet it.

**Implementation status (2026-11-16, owner-authorized trace build).**
TR-1, TR-2, and TR-3 are implemented in `src/trace.rs` and wired into the
gate's decision paths (`Deadbolt::decide` for tool admission and
`admit_action_inner` for exact-action admission):

- TR-1: `AdmissionRecord`s are constructed only inside the crate
  (`TraceLog::record_admission` is `pub(crate)`) and are emitted by the gate
  at decision time, for admits and for denies (with the structured denial
  code), mirrored into the gate's own evidence store as `admission`
  records. Worker summaries are stored separately, always labeled
  Generated, and are never promoted or consulted by reconciliation.
- TR-2: `Deadbolt::reconcile_effects` / `TraceLog::reconcile` match
  observed effects against admission records by natural-identity
  fingerprint. An unmatched effect produces an `UnmatchedEffectAlarm`
  (fail-closed; `reconcile_or_alarm` returns it as an error, and the gate
  mirrors a `trace_alarm` evidence record). Admissions without an observed
  effect are reported in `admissions_without_effect`, not alarmed.
- TR-3: `EffectIdentity::fingerprint` is a canonicalization-versioned
  (`deadbolt-effect-v1`) SHA-256 over canonical destination, canonical
  amount, and the effect's time window. Request IDs and grant nonces are
  never inputs; the idempotency key is the fingerprint by construction.
  An identity that cannot be resolved fails closed
  (`TraceError::UnresolvableIdentity`), and reconciliation alarms on it.

Acceptance evidence: `tests/trace.rs` exercises all three requirements
through the public gate API, including a hostile worker summary that
cannot alter gate records or satisfy reconciliation, and
`src/trace.rs` unit tests cover fingerprint stability and sensitivity.
The effect window is the fixed `EFFECT_WINDOW_SECS` bucket in this build;
mandate-scoped windows remain a candidate for a later requirement, not
implemented here.

**Second-round requirements (2026-11-17).** TR-4 and TR-5 below were
adopted on 2026-11-17 from DunimeTech's second-round review of the merged
trace build (CoderLegion, 2026-11-17), and the defects they name were
verified against `src/trace.rs` as merged in PR #116. Neither is
implemented in this build. Each states the defect as found, the
requirement that closes it, and the acceptance evidence required — the
same bar as TR-1 through TR-3.

The trace build has one job: make DeadBolt's admission decisions into
evidence that does not depend on the worker's cooperation or narration.
Witness observes effects in the world; DeadBolt decides admissions. The
trace is the join of those two records, produced by the trusted side.

## TR-1 — The gate emits its own admission record

**Requirement.** Every admission decision is recorded by DeadBolt itself, at
decision time, as a gate-owned record emitted out-of-band from the worker.
The record carries at minimum: the exact-action fingerprint (see
[ACTION-APPROVALS.md](ACTION-APPROVALS.md)), the decision (admit/deny), the
credential and lease identity evaluated, and the decision timestamp. The
worker's own trace, logs, or summary are never the source of this record and
can never modify, complete, or replace it.

**Rationale.** A compromised or merely confused worker will happily narrate
a clean trace about its own actions. If the worker produces the
authorization record, the record is the worker's story about itself; the
record has to come from the gate or it is not evidence.

**Acceptance.**
- A test worker replaced by a hostile stub that emits a fabricated clean
  trace does not alter the admission records produced for the same run.
- Killing the worker immediately after an admission leaves the admission
  record intact and readable.
- A denied decision produces a record with the structured denial reason,
  under the same guarantees as an admission.

## TR-2 — Inverted audit: an effect with no admission is a fail-closed alarm

**Requirement.** Reconciliation matches Witness-observed effects against
DeadBolt admission records (TR-1) by natural-identity fingerprint (TR-3).
Any observed effect with no matching admission record raises a fail-closed
alarm. An unmatched effect is an alarm condition, never a logging gap, a
warning, or a statistic.

**Rationale.** "Was the gate attached in every build?" is a deploy-time
question with a silent failure mode: an unattached or unwrapped route stays
quiet forever. The inverted audit replaces it with a runtime invariant — an
unattached gate alarms the first time anything acts through it, because the
effect arrives with no admission behind it.

**Acceptance.**
- A test effect deliberately produced outside the gate (an unwrapped route
  in a disposable harness) raises the alarm on the next reconciliation,
  with the effect's observed identity attached.
- Effects with matching admissions reconcile silently; the alarm path
  requires no cooperation from the worker that produced the effect.
- Alarm output distinguishes "effect without admission" from
  "admission without effect" (the latter is reported, not alarmed: an
  admitted action may legitimately produce no effect).

## TR-3 — Fingerprints key on the effect's natural identity

**Requirement.** Effect-matching fingerprints and idempotency keys derive
from the effect's natural identity: canonical destination, canonical
arguments (including amount where the effect carries one), and the
applicable time window. They are never derived from a caller-generated
request ID. The grant nonce in the exact-action approval record (see
[ACTION-APPROVALS.md](ACTION-APPROVALS.md)) remains part of the approval
record; it is not the effect-matching key.

**Rationale.** A retry under a fresh request ID must not re-fire an effect
that already happened — that is exactly the failure outcome-unknown
reconciliation exists to prevent. Natural identity is also what makes the
TR-2 matching reliable: the admission and the observed effect describe the
same act in the same terms, or the mismatch is itself the signal.

**Acceptance.**
- The same effect retried under a fresh request ID produces the same
  fingerprint and is deduplicated, not re-executed.
- Two effects differing in destination or amount produce different
  fingerprints.
- Fingerprint construction is canonicalization-versioned, so a change in
  canonical form cannot silently split one effect's identity across two
  fingerprints during an upgrade.

## TR-4 — Reconciler silence is an alarm condition, not health

**Requirement.** Reconciliation (`TraceLog::reconcile` /
`reconcile_or_alarm`) is a host-invoked library call; in the current
build nothing records that it ran, and a host that stops invoking it
produces no signal. Every reconciliation run must stamp a persisted
last-run record in the gate's own evidence store, carrying at minimum
the run's time and outcome. The staleness of the last run must be
readable by the host and the operator, and reconciliation overdue beyond
a defined bound must raise the same fail-closed alarm class as an
unmatched effect (TR-2). A stale reconciler is an alarm condition,
never a quiet state indistinguishable from a clean reconciliation.

**Rationale.** TR-2's invariant holds only while reconciliation actually
runs. A crashed scheduler, an unwired deployment, or a host that simply
stops calling reconcile leaves the trace reading as clean precisely
when nothing is checking it — the same silent failure mode TR-2 removed
for unwrapped routes, one level up. The audit has to be able to
distinguish "checked and quiet" from "never checked," or its silence is
not evidence.

**Acceptance.**
- Each reconciliation run leaves a persisted last-run record (time and
  outcome) in the gate's evidence store, produced by the gate and
  readable without the worker's cooperation.
- With reconciliation running within the defined bound, no staleness
  alarm is raised and the last-run record advances with each run.
- With reconciliation suspended past the defined bound, the overdue
  condition raises the fail-closed alarm, with the last-run time
  attached; alarm output distinguishes "reconciliation overdue" from a
  clean reconciliation and from an unmatched effect.

## TR-5 — The matching window is anchored to the admission, not the wall clock

**Requirement.** In the current build, `TimeWindow::containing` derives
an effect's window from wall-clock buckets (`ts.div_euclid` over the
fixed `EFFECT_WINDOW_SECS` bucket), and that window is an input to the
TR-3 fingerprint. The matching window must instead be derived from the
admission's own timestamp: an observed effect is matched against the
window its admission defines, so a matched admission/effect pair cannot
be split by a wall-clock bucket boundary.

**Rationale.** A bucket boundary is an artifact of the clock, not of the
act. An admission at second 299 of a bucket and its effect at second 301
are two seconds apart and the same act, yet they fingerprint under
different windows and never match: reconciliation reports an unmatched
effect — a false fail-closed alarm — plus an admission without effect,
for a pair that did match. False alarms are not free; they train
operators to discount the alarm channel TR-2 depends on.

**Acceptance.**
- An admission and its observed effect placed just either side of a
  wall-clock bucket boundary (seconds 299 and 301 of a 300-second
  bucket) reconcile as a matched pair: no unmatched-effect alarm and no
  admission-without-effect report for the pair.
- An effect genuinely outside the admission-anchored window still fails
  to match and alarms under TR-2, unchanged.
- TR-3's guarantees are preserved under the anchored window: the same
  effect retried under a fresh request ID still produces the same
  fingerprint and deduplicates, and effects differing in destination or
  amount still produce different fingerprints.

## Evidence labeling rule (applies to the whole trace)

A worker's own statement about what it was allowed to do is never promoted
into the authorization record. In Witness terms: the gate's record is
**Observed**; a worker's claim about its permission is at most
**Inferred/Generated** and stays labeled that way wherever it appears,
including in exported reports. Tools that render the trace must not
visually merge the two classes.

## Preserved boundaries

These requirements sit inside the repository's existing boundaries, not
over them: admission belongs immediately before each protected dispatch;
direct execution and already-running work are outside the gate; operator
authority, credentials, and private state stay outside model-controlled
tools; kill, deadlines, and one-use grants fail closed. Nothing here asks a
model to grant its own permissions, and nothing here turns DeadBolt into a
sandbox or a framework.
