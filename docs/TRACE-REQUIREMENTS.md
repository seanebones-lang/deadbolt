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

**Third-round requirement (2026-11-18).** TR-6 below was adopted on
2026-11-18 from DunimeTech's third-round review (CoderLegion,
2026-11-18), and the gap it names was verified against `src/trace.rs`:
`TraceLog::reconcile` is a pure pass over the in-memory logs — it
persists nothing and stamps nothing — and TR-4 as drafted has the
reconciler write its own last-run record, judged by no one. TR-6 is not
implemented in this build. It states the defect as found, the
requirement that closes it, and the acceptance evidence required — the
same bar as TR-1 through TR-3.

**Fourth-round requirement (2026-11-19).** TR-7 below was recorded on
2026-11-19 as a candidate requirement from DunimeTech's fourth-round
review (CoderLegion, 2026-11-19), and the gap it names was verified
against TR-6 as adopted: TR-6(b) has the out-of-band observer raise the
overdue alarm, but specifies no delivery mechanism for that alarm — an
alarm raised by emitting a message is silent when the observer is dead,
so the regress TR-6 closed for the reconciler reopens one layer up.
TR-7 is not implemented in this build. It states the defect as found,
the requirement that closes it, and the acceptance evidence required —
the same bar as TR-1 through TR-3.

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

## TR-6 — Liveness is judged out-of-band

**Requirement.** The TR-4 last-run record is evidence, not a heartbeat.
(a) It is written only as the final step of a completed reconciliation
pass, and it binds to what the pass consumed — at minimum the counts of
admissions and effects reconciled and a digest of the admission/effect
set — so a stamp cannot exist for a pass that did not run, and a fresh
stamp over no work is itself an alarm condition. (b) Staleness is
evaluated by a component other than the reconciler, reading the
persisted record against the observer's own clock. The reconciler never
reports its own liveness and never raises its own overdue alarm.

**Rationale.** TR-4 closes the "never checked reads as clean" hole by
having reconciliation stamp a last-run record — but as drafted, the
reconciler is the author of the only evidence that it is alive, and
nothing judges that evidence. A reconciler that stamps without running,
or stamps over an empty set, manufactures the appearance of health; a
reconciler asked to evaluate its own staleness makes the judged and the
judge the same component. Liveness has to be a byproduct of completed
work, bound to what the work consumed, and judged out-of-band by an
observer with its own clock — the same out-of-band principle TR-1
applies to admission records, applied to the reconciler itself.

**Acceptance.**
- A reconciliation pass interrupted before completion leaves no new
  last-run record; the previous record stands, and the out-of-band
  observer raises the overdue alarm from it once it goes stale.
- A last-run record whose bound counts or digest do not match the
  admission/effect set it claims to cover — including a fresh stamp over
  no work — raises the fail-closed alarm (TR-2's class), with the record
  attached.
- With the reconciler never invoked, the observer raises the overdue
  alarm from the persisted record and its own clock alone, with no
  cooperation from the reconciler.
- The reconciler has no code path that evaluates its own staleness or
  raises its own overdue alarm; the overdue decision is made only by the
  out-of-band observer.

## TR-7 — The overdue alarm terminates in absence

**Requirement.** TR-6's overdue alarm is a condition, not a message.
(a) The alarm terminates in an absence-triggered check at the
monitoring layer: that layer checks for the record the observer's
completed evaluations leave behind, and a record missing past its
deadline is itself the alarm condition — no component has to
successfully send anything for the alarm to exist. (b) This holds at
every level of the chain: the reconciler does not report its own
liveness (TR-6), the observer does not report its own liveness either,
and any component's silence is read by the next layer out as the
overdue condition, never as health. A dead observer emits nothing, and
under this requirement nothing is what raises the alarm.

**Rationale.** TR-6 moved the staleness judgment out of the reconciler
but left the observer raising the overdue alarm as an emission — and a
dead observer emits nothing. The hole TR-6 closed for the reconciler
reopens one layer up, at the observer; any design whose alarm depends
on a live component successfully sending it has the same hole at
whatever layer sends last. The regress terminates only when the alarm
is an absence: a record that should exist and does not, judged by a
checker reading against its own clock. Silence is the signal at every
level, and no component is left reporting its own health.

**Acceptance.**
- With the observer killed before it can emit anything, the monitoring
  layer raises the overdue condition from the missing, past-deadline
  record alone, with no cooperation from the observer.
- No component in the chain — reconciler, observer, or monitoring
  layer — has a code path that reports its own health or raises its own
  overdue alarm; every liveness judgment is made by the next layer out,
  against that layer's own clock.
- An evaluation record inside its deadline reads as live at the
  monitoring layer; the same record aged past its deadline raises the
  overdue condition with no new input from the observer.
- The overdue condition raised by absence is the fail-closed alarm
  (TR-2's class), carrying the identity of the record that was expected
  and the deadline it missed.

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
