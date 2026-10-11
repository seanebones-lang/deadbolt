# Exact-action approval (unreleased source candidate)

Approve one specific effect rather than the next call to a tool. This opt-in
addition is not in published v1.0.3 binaries. Use a reviewed candidate commit
and keep the executor, operator CLI, database and review storage trusted.
It needs no model provider, API key or paid service.

A version-1 request binds the executor-assigned run, routed tool identity,
actual destination, validated JSON arguments, unique nonce and fixed deadline.
Only a matching operator-approved request can pass `POST /admit-action`.
Changing the recipient, body, amount, destination, run, tool, nonce or deadline
refuses admission. Successful admission consumes the grant once, atomically
with its primary SQLite evidence. Latest lease, kill, pause, tool/destination
policy, spend cap and workload credential state still apply.

## Review and execute

Start the sidecar and provision a lease as described in [credentials](CREDENTIALS.md).
The trusted executor prepares review data locally, without granting permission:

```python
import json, os
from deadbolt_client import prepare_action

action = prepare_action("customer-run-1", "send_email_v1", {
    "to": "reviewed@example.com",
    "subject": "Reviewed subject",
    "body": "The exact message for review",
}, dest="mail.example.com", ttl_secs=300)
# Store private review data exclusively, accessible only to the operator.
with os.fdopen(os.open("review.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w", encoding="utf-8") as f:
    json.dump(action, f, ensure_ascii=False)
```

On Windows, protect review storage with an owner-only ACL; `0o600` does not
configure a Windows ACL.

The operator can require exact approval before preparing any grant:

```sh
deadbolt action require --agent customer-run-1 --tool send_email_v1
deadbolt action inspect --file review.json
```

Inspect displays the full private arguments and a `sha256:...` fingerprint.
Review the details in a private terminal. Copy the displayed fingerprint into
the approval command; do not automatically approve arbitrary incoming files.

```sh
deadbolt action approve --file review.json --fingerprint sha256:PASTE_THE_REVIEWED_DIGEST
```

Approval rechecks the supplied fingerprint against the file, refusing a file
changed since review. It also enables exact-only policy for that run/tool.
The file is still review data, not a bearer credential. The trusted executor
uses its existing admission credential, then invokes the actual effect with
the snapshot supplied by the helper:

```python
from deadbolt_client import dispatch_action

outcome = dispatch_action(action, lambda approved: send_email(**approved))
if not outcome["executed"]:
    # Present the denial for operator handling. No implicit retry or fallback.
    show_denial(outcome["decision"])
```

`send_email` and `show_denial` are application functions. This example does not
provide a mail transport. Do not read changed arguments from external mutable
state inside the body. Route tool identity and destination in trusted code;
a model must not choose a tool alias to evade exact-only policy. Version the
tool token when its effect/schema changes. A recipient/amount hidden in a
closure, environment, mutable configuration or context is not covered by a
hash of other arguments: resolve and include every effect-affecting value.

JavaScript (tested on Node 22) has `prepareAction`, `admitAction` and `dispatchAction`; the callback
receives the copied arguments. Python also has `dispatch_action_async`.
Rust exports `ActionRequest::{new,from_json,fingerprint}` and
`Deadbolt::{require_exact_action,approve_action,revoke_action,admit_action,
admit_action_credential,dispatch_action}`. Detailed Rust denials come from
`admit_action`; `dispatch_action` returns an existing `DenyCode` and never calls
the body on an error. JSON parse boundaries must use `ActionRequest::from_json`
to reject duplicate keys, rather than a generic last-key-wins parser.

## Optional OpenAI Agents SDK adapter

The installed client can protect a JSON-native Python function with a fixed
trusted grant or a synchronous trusted resolver returning just its metadata:

```python
from deadbolt_openai_agents import protected_tool

@protected_tool(
    agent_id="customer-run-1", destination="mail.example.com",
    action_grant={"nonce": approved_nonce, "expires_at": approved_deadline},
)
async def send_email_v1(to: str, subject: str, body: str) -> str:
    return await application_mail_transport(to=to, subject=subject, body=body)
```

Build the review request from the validated named Python arguments, including
applied defaults. The wrapper rebuilds the envelope from those actual arguments;
it cannot execute a changed request using an old grant. Destination and grant
resolvers receive separate copies and cannot mutate execution arguments.
`needs_approval` and the SDK's approval/resume flow still work; SDK approval does
not create a DeadBolt grant. Admission occurs when the protected body runs.

Exact mode currently requires named JSON-native parameters. It refuses context
objects, Pydantic/custom instances, variadic signatures and non-JSON values
rather than silently leaving effect-affecting state out of review. For tools
that use such state, resolve a JSON-native effect request in trusted application
code and protect the effect through `dispatch_action_async` instead. The existing
adapter without `action_grant` preserves its previous context support.

## MCP proxy

Supply an operator-owned JSON metadata file through `DEADBOLT_ACTION_GRANTS`:

```json
{"send_email_v1":{"nonce":"executor-created-nonce","expires_at":1791660000}}
```

Use the actual nonce/deadline of the reviewed request, not these illustrative
values. The proxy snapshots and validates this bounded file before spawning
its trusted child. For each configured tool it builds the exact envelope from
the proxy's fixed run, routed tool name, existing destination extraction and
raw `params.arguments` object (omitted arguments mean `{}`). Review those same
raw arguments; downstream defaults/coercions are not part of this envelope.
Keep server schema and effect behavior stable and trusted. A second call needs
a newly reviewed grant and a fresh proxy session with new metadata.

Both embedded and sidecar proxy modes support exact approval. Workload mode
requires `--serve-sock`. Missing entries use normal admission, which denies
when the tool requires exact approval. Invalid/unreadable configured metadata
fails before child startup. The metadata-path environment variable and both
DeadBolt bearer-token variables are removed from the child environment; this
is not an OS sandbox or protection against same-user filesystem access.

## Lifetime, representation and recovery

- Preparation/approval lifetime is 1..86400 seconds. Admission requires an
  unexpired stored grant. Live checks retain the existing sliding lease behavior;
  the action deadline and credential deadline never slide. Kill stays terminal.
- Nonces are unique per run/store, including consumed, expired and revoked rows.
  Use a new OS-random nonce for each review. No automatic cleanup/reuse is provided.
- Fingerprints are `sha256:` plus lowercase hexadecimal SHA-256 over
  `deadbolt-action-v1` followed by a NUL byte and the UTF-8 RFC 8785 canonical
  JSON envelope. Version 1 includes all seven fields, with null destination.
  [RFC 8785](https://www.rfc-editor.org/info/rfc8785/) defines object sorting,
  Unicode preservation and number serialization. No Unicode normalization is applied.
- Requests are at most 32 KiB, arguments must be an object, depth is at most 32
  and value-node count at most 4096. Duplicate keys, unknown fields/versions,
  lone surrogates, non-finite numbers, negative zero and numbers outside the
  safe interoperable range ±9007199254740991 are refused. Numeric representation
  such as `1` versus `1.0` is canonicalized to the same JSON number. Validate a
  stable tool schema before review/execution; do not branch on lexical number
  spelling or host-language integer-versus-float types. Use decimal strings
  for high precision, or safe integer minor units for monetary values.
- Grant/evidence rows contain metadata and fingerprints, not raw arguments.
  Review files, request bodies and inspect output contain private arguments;
  the application owns their permissions, retention and logging. Fingerprints
  of low-entropy content are not encryption. Default primary SQLite evidence
  is transactional; JSONL and optional Witness/custom sinks are not a global
  transaction. A post-commit mirror/decision-write error can consume a grant
  without executing a body. Reconcile evidence; never retry a consumed grant.
- Consumption means admitted, not completed. Tool errors, connection loss or
  a crash may leave the outcome unknown. DeadBolt does not provide transactional
  exactly-once external effects. Use transport idempotency/reconciliation and
  obtain a new review when retrying. Revocation cannot cancel running work.
- Revoke with `deadbolt action revoke --agent RUN --nonce NONCE`. Exact-only
  policy remains. Only the operator can explicitly restore legacy mode with
  `deadbolt action require --agent RUN --tool TOOL --allow-legacy`; that is a
  permission downgrade and does not erase grants or old broad approvals.
- Keep enforcement enabled/fail-closed. Exact admission itself always fails
  closed, including disabled/fail-open configurations. Legacy host APIs retain
  their deliberate operator override behavior and should not be exposed as
  an alternative route. Old servers return an unsuccessful status for the new
  `/admit-action` route; consumers must deny, never fall back to `/admit`.
- Upgrades add two SQLite tables without modifying existing leases/credentials.
  Back up and test recovery first. Older binaries ignore exact-only requirements
  and cannot enforce this feature; rollback requires stopping protected dispatch,
  revoking credentials as appropriate, and restoring a reviewed compatible state.

This gate remains free under Apache-2.0. It approves protected effects inside
a trusted dispatcher; it does not make arbitrary generated code safe to run.
