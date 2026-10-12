# Migrating to the 2.0 developer candidates

This candidate consolidates explicit destination exceptions, immutable run
deadlines, optional supervisor-only liveness and a Python owned-process reference.
It is a developer-evaluation prerelease. Stable v1.0.3, the original
v1.1.0-rc.1 packages and MCPB/plugin bundles remain distinct immutable artifacts.
A source merge does not upgrade a package, running sidecar or editor plugin.

## Choose a matching engine and client

Corrected source uses engine `2.0.0-rc.2` and Python `2.0.0rc2`. Its additional
changes are marked unreleased below; publication and client-host acceptance
remain separate gates. [Build the source pair from one reviewed revision](PYPI.md#evaluate-the-corrected-source-candidate).

The published pair uses Rust `2.0.0-rc.1` and Python `2.0.0rc1`. Install a reviewed
candidate wheel in your own virtual environment and install its engine separately:

```sh
python -m pip install 'n11-deadbolt-client==2.0.0rc1'
/absolute/path/to/deadbolt --version
/absolute/path/to/deadbolt drill
```

Expected engine version: `deadbolt 2.0.0-rc.1`. Install it from Cargo with
`cargo install n11-deadbolt --version 2.0.0-rc.1 --locked --bin deadbolt`, or use
the Homebrew preview cask on macOS. For native downloads, verify the archive
checksum and its `BUILD.json` source revision. Existing registry commands pinned
to 1.x still install 1.x. Do not mix newer documentation with an older binary
and assume the guarantee exists. No old package/tag is overwritten.

## Rust source changes

Unreleased corrected source changes **fresh** `ensure_agent`/`issue_agent` roots
to supervisor-only renewal, also through CLI `ensure`/`issue` and HTTP setup.
Existing rows keep their original mode and timestamps. Provision a separate
supervisor credential and job-scoped heartbeat before initial idle expiry; see
[supervision setup](SUPERVISION.md). To deliberately retain workload activity
renewal, use Rust `ensure_activity_agent`/`issue_activity_agent`, CLI
`--activity-renewal`, or HTTP `activity_renewal: true`. Python `ensure` accepts
the strict keyword `activity_renewal=True`; Node uses `{activityRenewal: true}`.
The client alone cannot change an older engine's behavior. The bundle retains
its explicit mode selection for both original rc.1 and corrected engines.

Unreleased source adds `DeadboltError::FingerprintMismatch` for a changed CLI
action review. Update exhaustive operator-error matches when adopting that
source; its string token is `fingerprint_mismatch` and it never grants permission.
It also adds `DeadboltError::ChildInactive` for unsuccessful CLI child
registration, with token `child_inactive`. Update exhaustive error matches.

A new enum variant and added public fields require a major Rust version. This is
not a drop-in minor upgrade for applications using exhaustive matches or struct
literals. Change an exhaustive denial match to include the fixed-deadline case:

```rust
match code {
    deadbolt::DenyCode::DeadlineExceeded => stop_the_protected_action(),
    // Keep the existing denial branches; every denial withholds the body.
    other => handle_existing_denial(other),
}
```

The two application functions above are placeholders for your trusted routing.
If you only need the machine token, `code.as_str()` handles every shipped variant
without reconstructing a match. Unknown HTTP/client denial codes must remain
refusals, never permission.

`AgentStatus` adds `hard_deadline_at: Option<i64>` and
`supervision_root: Option<String>`. `ExportRow` adds `offline_tools`,
`hard_deadline_at`, `supervision_root`, `expires_at` and `renewed` optional fields.
Update test/mock literals with `None` when representing an unchanged legacy row.
Prefer reading values returned by `status` and `export` in application code.
Do not synthesize status to authorize an executor. The candidate's downstream
migration fixture compiles an original v1.0.3 consumer, demonstrates the expected
E0004/E0063 errors against the candidate, and compiles the migrated consumer.

## Destination policy becomes explicit

When a lease has `dest_allow`, every tool needs an executor-supplied allowed
host, unless the operator explicitly lists it in that lease's `offline_tools`.
Tool names no longer decide whether destination checks apply. Unconfigured
leases retain open destination policy. Migrated destination policies default to
an empty exception list: a previously accepted `shell` without a destination can
now deny `purpose_exceeded`.

Inventory actual protected routes before upgrading. Declare their true
network destinations or explicitly review a genuinely offline implementation;
do not exempt general shell/Python execution as an expedient. The setting is
policy, not interception. Pair it with OS egress isolation where needed. See
[destination setup](DESTINATION-POLICY.md) and [trust boundaries](TRUST.md).

## Preserve existing run modes deliberately

Existing rows migrate to admission-renewed idle leases, without a hard deadline
or supervision root. Old admission credentials preserve their scope. Existing
identities, policies, approvals, evidence, spend and terminal kills are retained.
Child registration preserves lineage. Migration never silently enables a new
run guarantee.

A fixed deadline requires fresh operator `issue`/`POST /issue`. Supervised
liveness requires fresh operator `supervise`/`POST /issue-supervised`. Registered
children inherit the fixed deadline and supervision root. A heartbeat maintains
only still-live matching members and cannot extend the fixed cutoff. Keep
operator, admission and supervisor credentials on their documented planes.
See [deadlines](LEASE-DEADLINES.md) and [supervision](SUPERVISION.md).

## Unreleased HTTP authentication default

Unreleased source also requires private Unix state and an explicit quiesced
permission migration for old broad-mode DB/events/Witness files and sidecars.
See [private-state migration](PRIVATE-STATE.md). Kills, grants, credentials,
deadlines and evidence are retained; files are never silently chmodded.

The new candidate requires an operator token for Unix and TCP sidecar startup.
Configure `DEADBOLT_TOKEN` privately before starting either listener; the trusted
server keeps it, and the dispatcher receives only its scoped admission credential.
Existing anonymous Unix integrations must explicitly choose
`--legacy-unauthenticated-unix` during a deliberate migration. The legacy flag
cannot override a configured token, allow tokenless TCP, or erase the store's
persisted first-credential boundary. It does not restore anonymous access after
credential expiry/revocation. The Rust option defaults false and complete config
literals must include it. [HTTP setup](HTTP-AUTHENTICATION.md) provides the recipe.
Published rc.1/1.x remain unchanged. State and operator CLI access are still OS
trust boundaries; HTTP credentials do not sandbox same-user code execution.

## Unreleased registered-tree authorization tightening

These source changes are not in the published 2.0.0-rc.1 artifacts. Select a new
immutable candidate before relying on them. Each descendant admission checks all
ancestor policies dynamically, including later operator edits. Tool/destination
ceilings and clips apply together; ancestor review requirements must be satisfied
by a grant for the actual admitted child. A parent's grant never transfers. Each
MCP child still needs its own explicitly configured tool allowlist.

A parent's spending cap now includes its registered subtree. Existing ledgers
are preserved and summed once, including terminal descendants. Review incident
`spend_usd` (own ledger) and `subtree_spend_usd` (budget scope): old recorded child
costs may immediately exhaust a parent cap after upgrade. Quiesce dispatch and
review the intended tree budgets before resuming. Report costs only on the
identity that incurred them; do not repeat child reports on parents. Resume alone
cannot reopen an exhausted cap. Accounting remains based on trusted reports,
without cost reservation or cancellation of in-flight work.

Verify an allowed child effect, a child denied by each ancestor restriction,
unchanged child grants after denial, and sibling reports reaching a shared cap.
Do not assume a child's own active status means its full ancestry permits work.
Older engines cannot preserve these ceilings during rollback.

## Stop, back up, upgrade, evaluate

1. Stop protected dispatch and the sidecar. Do not let two versions administer
   the same store concurrently.
2. Back up SQLite with its backup API or the quiescent procedure in
   [operations](OPERATIONS.md); preserve JSONL, optional Witness state and
   credential files in operator-protected storage. Backups contain authority.
3. Install the selected candidate, verify its version/hash/source identity,
   and restart using the same protected state paths. Check existing killed
   IDs, policies, child lineage and credentials before reconnecting workloads.
4. Reproduce harmless allowed and denied marker effects through your actual
   dispatch paths. Validate missing destinations, replayed approvals, kill,
   store loss and any deadline/supervision modes you provision.
5. Restore dispatch only after those checks pass. Keep an explicit route
   inventory: successful source tests do not protect an unconnected tool.

Older engines ignore the new fields and do not enforce these guarantees.
Quiesce execution before rollback; reconcile all kills/revocations recorded after
any backup. Restoring an old snapshot must not resurrect a revoked identity.
A successful migration test is not permission to run older code on a new store.

The gate blocks new admissions. In-flight cancellation belongs to an executor
that owns the work. The [Python reference](EXECUTOR-CANCELLATION.md) is POSIX-only;
Linux and macOS with Python 3.13+ support its required wait interface. It refuses
unsupported hosts before execution. Windows native gate support does not imply
Windows process-group cancellation support. OS sandboxing and authoritative
provider cost collection remain separate integration responsibilities.
