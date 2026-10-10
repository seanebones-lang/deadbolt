# Single-agent admission credentials

The `1.1.0-rc.1` source candidate separates a dispatcher's admission access from
operator authority. This is an unreleased protocol addition; stable `v1.0.3`
does not support it.

| Identity | HTTP access | Who holds it |
| --- | --- | --- |
| Operator (`DEADBOLT_TOKEN` / `X-Deadbolt-Token`) | All exposed routes, including setup, policy, spend and status | Trusted setup/administration |
| Admission (`DEADBOLT_ADMISSION_TOKEN` / `X-Deadbolt-Admission`) | Only `POST /admit`, for one stored agent ID | Trusted dispatcher for that run |
| Socket possession without credentials | Legacy operator access only before any admission credential has ever been issued | Existing Unix integrations only |

An admission credential cannot ensure/register agents, alter policy, report spend,
inspect other agents or grant/resume approvals. Kill, pause, approve and credential
management remain operator CLI controls. A model must not choose the dispatcher's
agent ID, protected tool name or destination; derive those from trusted routing.

## Setup

Use the same protected DB/events paths for the sidecar and CLI. Create the lease
and configure policy through the operator before handing a credential to the
dispatcher. The CLI deliberately refuses to issue for a missing, expired or killed
lease; issuance does not renew or revive it. Configure an operator token even on
Unix if you want to keep administering over HTTP after issuance.

```sh
# Lease creation/policy happen through trusted setup using the operator client.
# Supply private state paths and operator token through your existing secret setup.
mkdir -p "$HOME/.deadbolt/credentials"
chmod 700 "$HOME/.deadbolt/credentials"
deadbolt credential issue --agent shop-bot --id shop-run-001 \
  --ttl-secs 3600 --out "$HOME/.deadbolt/credentials/shop-run-001"
deadbolt credential list
deadbolt credential revoke --id shop-run-001
```

The issue command prints only public identifier/agent/expiry metadata. It never
prints the bearer secret and never overwrites an existing output path, including
a symlink. Unix output files have mode 0600. On Windows, use a directory with an
ACL that admits only the intended operator/dispatcher; this command does not set
or verify Windows ACLs. Protect the output directory against concurrent replacement.

Load the file directly into the dispatcher's environment through trusted code or
a secret manager; avoid command history/logs. A Python launcher can do:

```python
import os
from pathlib import Path
os.environ["DEADBOLT_ADMISSION_TOKEN"] = Path(credential_path).read_text()
os.environ.pop("DEADBOLT_TOKEN", None)
```

The bundled Python and Node clients choose workload mode whenever that environment
variable is present, even empty, and use it for every request. Control requests
will be denied rather than falling back to operator authority. Remove the operator
token from workload environments entirely. Empty, malformed, expired, revoked or
wrong-agent credentials deny execution; Python/Node preserve `unauthorized` and
`forbidden` status codes without trusting an error body's claimed allow.

MCP workload mode requires `--serve-sock`; in-process MCP owns operator-capable
state and cannot claim this separation. Scoped MCP does not call `/ensure` and
cannot call `/spend`; tool calls containing spend reporting therefore fail closed.
Have trusted operator infrastructure report costs separately. Proxy children do
not inherit either DeadBolt bearer-token environment variable. This is hygiene,
not a sandbox: same-user processes may still access files or inspect other
processes unless separate OS controls prevent it.

## Lifecycle and migration

Credentials contain 32 bytes from the OS random source via
[`getrandom`](https://docs.rs/getrandom/0.4.3/getrandom/), encoded with a `dbw1-`
prefix. SQLite stores only a SHA-256 hash, public ID, bound agent, fixed expiry,
creation time and optional revocation time. Evidence contains public metadata,
never the bearer. TTL must be 1..=86400 seconds; it does not slide with lease
activity. Credential IDs remain reserved after expiry or revocation. Issue a new
ID/file when rotating and revoke the old credential.

Authentication, lease/policy checks and one-shot consumption share one SQLite
writer transaction. A completed revocation blocks subsequent admissions even
through another sidecar/DB connection. Revocation cannot cancel an admission
already granted or a body already running. Normal lease expiry/kill still applies
even if the credential is valid. A credential can remain valid across a sidecar
restart; operator-managed state and output-file backup are your responsibility.

Startup adds an `admission_credentials` table without changing existing leases,
policies, approvals or evidence. Old Unix integrations keep their behavior until
the first credential is issued. From then on, anonymous Unix HTTP access is
disabled, including when all credentials are expired/revoked. An absent or invalid
workload header cannot downgrade to socket-only operator access. Older binaries ignore this role boundary. Do not roll back to an older Unix
sidecar with anonymous access while workloads can reach it. Quiesce workloads
and restore a pre-credential backup, or require operator authentication and
revalidate the old deployment before admitting work. Continue through
the CLI, or restart the sidecar with an operator token and use authenticated
operator clients. Mixed or duplicate authentication headers are refused.

Issuance and its primary SQLite evidence row commit together using the default
sink; a failed primary evidence write rolls back activation. JSONL/Witness mirrors
remain separate evidence limitations; post-commit mirror errors can require
reconciliation through credential metadata. Preserve the issued file when a grant
may have committed. Do not blindly retry the same ID or modify credential rows.

The role boundary is HTTP authority. Protect the database, CLI, operator bearer
and executor routing with OS/service ownership. It does not isolate an untrusted
process that can write the store, use the CLI or bypass the protected dispatcher.
Admission credentials also do not bind approvals to exact arguments; that is the
next separate protocol milestone.

## Acceptance

Rust tests cover route authority, mixed/duplicate headers, agent binding, expiry,
independent-connection revocation, exclusive/private output, stored hashes,
terminal leases and primary-evidence rollback. Client acceptance exercises actual
Python/Node effects and a real MCP child. Installed-wheel SDK acceptance checks
that revocation prevents another effect through the real OpenAI SDK runner,
without provider requests. Run the existing CI and packaging acceptance suites
before deploying this candidate.
