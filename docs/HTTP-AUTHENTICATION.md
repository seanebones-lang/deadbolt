# Authenticate the local sidecar

**Stable 2.0.0 behavior:** Unix and loopback TCP sidecars require a nonempty
operator token at startup. Published 2.0.0-rc.1 and 1.x Unix engines retain their
older socket-possession default. Select the matching stable 2.0.0 executable before relying
on the changed default; documentation cannot upgrade an installed executable.

## Default setup

Give the trusted server a private `DEADBOLT_TOKEN`, then start it:

```sh
# Your secret manager or trusted launcher supplies DEADBOLT_TOKEN privately.
export DEADBOLT_SOCK="$HOME/.deadbolt/deadbolt.sock"
deadbolt serve --bind "$DEADBOLT_SOCK"
```

Generate a fresh 256-bit random token in trusted setup. Never print it, include it
in an MCP tool definition, commit it, or pass it to an untrusted tool process.
The [quickstart](QUICKSTART.md) has a disposable file-backed demonstration.
Loopback TCP uses the same token contract, with `127.0.0.1:PORT` or `[::1]:PORT`.
Public binds are refused. The Unix socket remains mode 0600.

Operator requests send `X-Deadbolt-Token`. Setup creates the lease and actual
policy, then issues a [single-agent admission credential](CREDENTIALS.md).
Give the dispatcher only `DEADBOLT_ADMISSION_TOKEN`; remove the operator token
from its environment. Supervisor-only renewal uses its separate root-bound
credential. Mixed or duplicate role headers are refused. Wrong/missing operator
authentication returns 401; a workload credential on a control route returns
403. Neither response grants permission to execute a body.

Rust hosts configure `DeadboltConfig.token_env` or `token_file`. Environment
resolution takes precedence; an empty environment value can fall back to a
configured token file. Unix token files must be mode 0600; missing or empty token
sources do not satisfy default startup. The CLI selects `DEADBOLT_TOKEN` and
does not automatically generate one or read a token-file flag. Local CLI/library
administration does not require an HTTP token.

## Explicit legacy migration option

For a deliberately trusted older Unix integration only:

```sh
deadbolt serve --bind /private/path/deadbolt.sock --legacy-unauthenticated-unix
```

The equivalent Rust config is `allow_legacy_unauthenticated_unix: true`; its
default is false. A complete config struct literal needs the new field or
`..Default::default()`. A configured token always wins, even with the flag.
The option never permits tokenless TCP.

Without a token, explicit legacy mode grants full exposed HTTP authority to a
socket holder only while the store has never issued any scoped credential.
Admission and supervisor issuance both close that older anonymous path;
expired/revoked credential rows and restarts do not reopen it. A supplied invalid
role header cannot fall back to anonymous authority. A failed credential-boundary
read fails closed. A legacy request already authenticated before concurrent
first issuance may still finish; this is not cancellation of in-flight requests.
Use tokened setup instead of relying on that transition for new integrations.

## Filesystem and execution boundary

The token authorizes HTTP requests. It does not protect state from a process
that can write the database, invoke the operator CLI or inspect the server's
secrets. Keep state and administration under separate OS/service ownership from
untrusted general code execution. The [trust guide](TRUST.md) describes this
boundary. A shared Unix UID or a writable shared container volume is not an
isolation mechanism merely because the dispatcher holds a scoped credential.

Default `serve()` validates authentication before binding or initializing its
store. The CLI currently opens its configured store before command dispatch,
so refused CLI startup can initialize state; it still does not expose a listener
or authorize a request. Protect state paths before invoking either interface.

## Disposable verification

`tests/unix_authority_acceptance.py --binary /absolute/path/deadbolt` tests the
actual CLI/listener with fresh stores and synthetic secrets: tokenless refusal,
operator and scoped controls, rejected mutation attempts, mixed headers, explicit
legacy access and its persistent credential boundary. It connects no real
accounts. Unix source and extracted-native CI run it; TCP and role/parser
contracts are also covered by Rust and client acceptance tests.

Source/native process checks are separate from a model invoking the tool in an
actual graphical client. Published artifact identity and host acceptance remain
release gates.
