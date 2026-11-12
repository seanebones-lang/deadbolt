# Trace build requirements

Adopted 2026-11-10 by the owner's decision. Source: external review feedback
from DunimeTech on "DeepSeek Built a Super Agent. We Checked It Against a
Permission Gate" (CoderLegion, 2026-11-10). These are requirements for the
future trace build — the gate-emitted evidence layer described in the
company's grant work. **None of this is implemented in the 2.0 series, and
nothing here changes 2.0.x behavior.** A requirement below is met only when
its acceptance evidence exists; a design, a plan, or a passing demo of
something else does not meet it.

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
