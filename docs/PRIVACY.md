# DeadBolt core privacy and local data

DeadBolt is a local execution gate maintained by NextEleven LLC. This policy
covers the Rust engine, local sidecar and bundled clients. Third-party models,
MCP servers, editor applications, hosted demos and distribution sites have their
own data handling; consult their policies before connecting them.

## What the core stores

The SQLite store contains agent identifiers and lineage, lease state and
expiry/deadlines, supervision binding, tool/destination policy, operator-approved
offline exceptions, reported spend, approvals, credential hashes and revocation,
and structured decision/operator evidence. Exact-action grants store a digest,
nonce, tool, identity and expiry, rather than the reviewed private arguments.
Arguments transit the local admission call and may appear in your application's
own review files, tool outputs or logs. Protect those separately.

Default files are `~/.deadbolt/deadbolt.db` and
`~/.deadbolt/deadbolt-events.jsonl`. SQLite may also create journal/WAL companion
files. Optional Witness output uses `~/.deadbolt/deadbolt-witness.db` and its
JSONL sibling. Configuration or an embedding host can select other paths.
Operator exports and incident reports are written only when requested.
Credentials are written to operator-selected files; their bearer values are
not included in core evidence or list/status output. The SQLite hashes and
state are still authority-bearing data and must be protected.

## Transfers and retention

The engine contains no account login, analytics or telemetry uploader. Its HTTP
listener accepts only local Unix sockets or authenticated loopback TCP. It does
not upload its local evidence to NextEleven. A configured downstream tool can
perform network operations; DeadBolt does not intercept those connections or
control what another application logs or uploads. Destination declarations are
policy inputs, not network enforcement.

The core does not automatically expire evidence or remove revoked identities.
Retention is controlled by the operator. Use documented backup/export procedures
and protect backups with the same care as live state. Deleting a store removes
its policy, history and terminal revocations; never reset state to evade a kill
or silently restore an older backup that drops later revocations. Uninstalling a
binary or client does not imply that its separate state was deleted.

## Operator responsibility and contact

Keep state, credential files, backups and review material outside model/workload
access. Same-user processes may access files without separate OS isolation;
Unix file modes alone are not a sandbox. Windows requires appropriate directory
ACLs. The operator chooses what tools to connect and what application data those
tools may access. Do not include credentials or private message bodies in a
public issue or support report.

Privacy/support questions: nextelevenstudios@gmail.com. For security reporting,
follow [SECURITY.md](../SECURITY.md). Core scope and execution limits are described
in [TRUST.md](TRUST.md).
