# Frequently asked questions

## Is Deadbolt a standalone product?

Yes: it is a Rust library and executable with bundled SQLite. It does not import
Harness or Witness and needs no LLM provider or API key. Optional Witness output
is implemented locally. Harness is a consumer, not a prerequisite.

## Can I add it to any project?

Any language can use the local HTTP protocol. Rust can embed the library.
Trusted stdio MCP servers can run behind the proxy. You still need an adapter
at the executor's real dispatch boundary. A project that lets agent code invoke
tools directly cannot be protected merely by installing a client module.

## What does kill stop?

Kill makes later admissions deny for that ID and its registered descendants.
It does not cancel a tool already running, stop a model, discover other agents,
or revoke an unrelated agent. Your executor can add cancellation separately.

## Can an agent just choose another ID?

It can if your integration permits it. The trusted executor must assign stable
run identities and bind calls to them. The HTTP token authenticates access to
the sidecar, not the agent identity. Anyone with that token can assert any ID
and use all exposed routes, including policy changes.

## Are the clients enforcement or a sandbox?

They return decisions; your dispatcher enforces them. Deadbolt is not an OS
sandbox. Protect the operator CLI, credentials and state from model-controlled
code, and ensure every tool and spawn path uses the dispatcher.

## What is protected by the MCP proxy?

Routed `tools/call` messages. Individual JSON-RPC 2.0 objects are supported;
batches and malformed envelopes are rejected. Remote HTTP/SSE is not supported.
Initialization, resource reads, other methods and child startup are outside the
gate. The child inherits OS access and environment; it must be trusted.

## Why does a newly created agent allow tools?

Unset allow-lists are open. Set tools, destination, irreversible classifications
and spend limits before dispatch. Omitted policy fields retain stored values;
they do not reset previous policy. An empty allow-list permits no matching tools
or hosts. Defaults are not a complete application-specific policy.

## Does a destination allow-list constrain shell commands?

Not by itself. Destination metadata is provided by the caller. A missing host
is denied for recognized network classes (`http`, `fetch`, `browser`,
`web_search`), not a local `shell` or `write_file`. Custom network tools need a
trusted adapter with an explicit destination; this is not packet inspection.

## Is spend automatically measured?

No. The trusted caller reports spend. The MCP adapter discovers numeric
`amount`, `usd` or `cost` arguments by convention. A reported spend crossing
the cap pauses the lease; the next admission returns `spend_cap`. It does not
independently read a provider's bill.

## What happens after 60 seconds?

The default lease TTL is 60 seconds. Live admissions, including policy denials,
renew it. Silence past the TTL expires it; killed and expired leases do not
slide. Use a new run ID after expiry. Configure a suitable TTL in the embedded
`DeadboltConfig`/`open_at` API; the standalone CLI does not expose a TTL flag.

## Can I resume a killed or expired agent?

Kill is terminal for an ID and resume cannot undo it. Resume clears pause and
clips and renews the TTL for a non-killed lease, including an expired one; it
does not replace policy. That renewal is an explicit operator action, not an
automatic client recovery. Use a new executor-assigned ID for a distinct new
run. Do not silently allocate a new ID to evade a denial.

## Does approval last for a session?

No. One approval permits one admission of the specified irreversible tool.
Concurrent connections cannot consume the same approval twice. Admission uses
it even if the subsequent body or evidence write fails; approve again only
after operator review. Other policy checks still apply.

## Do child agents inherit policy?

No. Register the child before starting its runner, require a live registration,
and configure its own policy. Registration preserves an existing same-parent
child and refuses reparenting/self-parenting. An inactive parent cannot create
new live children. Parent kill revokes descendants; the executor owns cancellation
of any work already running.

## What if the sidecar or database fails?

Default behavior is deny `store_unavailable`. The Python/Node clients also deny
on timeout, malformed responses or non-2xx HTTP status. Never turn that denial
into a fallback that runs the tool. Embedded consumers can explicitly disable
fail-closed behavior; enforce the intended configuration in the trusted executor.

## Can multiple processes share a store?

Yes, on the local SQLite store: mutations and approval consumption use immediate
transactions, and evidence sequence allocation is shared through SQLite. This
is not a distributed service, cluster consensus or a network filesystem design.
Validate your own workload; no throughput or latency benchmark is claimed.
Connections use a five-second SQLite busy timeout. Slow storage
or contention can still deny availability. Prefer a long-lived sidecar or cloned
Rust gate, and see [concurrent-writer troubleshooting](TROUBLESHOOTING.md#concurrent-writers-and-store_unavailable).

## Are evidence records immutable or compliance proof?

No. They contain structured observed/inferred facts and content hashes, and
can be exported per agent. An operator with filesystem access can alter them.
SQLite and JSONL are separate writes, so a crash can leave them out of sync.
Retention, access controls, legal review and incident delivery remain yours.

## Can I use it commercially?

The repository is Apache-2.0; follow its license and NOTICE obligations.
It does not include or relicense Harness. See [LICENSE](../LICENSE) and
[NOTICE](../NOTICE). No certification or jurisdictional compliance is claimed.

## Where are the packages and support channels?

Use a published native archive or reviewed source/Git installation. The candidate
Python client can be installed from local source or a built wheel; the Node client
is bundled source. The Rust and Python package names returned registry 404s on
2026-10-10. Follow [installation](INSTALL.md) rather than registry commands.
Use GitHub issues for ordinary bugs and integration requests. Report security
issues privately as described in [SECURITY.md](../SECURITY.md). There is no
published support SLA or universal framework compatibility guarantee.
