# Operations and integration acceptance

## Configuration ownership

Use a dedicated state directory per integration. The CLI reads the following
environment settings; server/executor/operator paths must match.

| Setting | Purpose | Default |
| --- | --- | --- |
| `DEADBOLT_DB` | CLI/sidecar/MCP SQLite store | `~/.deadbolt/deadbolt.db` |
| `DEADBOLT_EVENTS` | CLI/sidecar/MCP JSONL evidence | `~/.deadbolt/deadbolt-events.jsonl` |
| `DEADBOLT_SOCK` | Python/Node client endpoint | Unix default socket; non-Unix loopback `127.0.0.1:9782` |
| `DEADBOLT_TOKEN` | Sidecar/client authentication | No default; required for TCP |
| `DEADBOLT_BIN` | Consumer-test executable path | `target/debug/deadbolt` |

`DEADBOLT_SOCK` does not configure the server's bind or the CLI's store: use
`serve --bind` and matching DB/events paths. Embedded Rust uses its
`DeadboltConfig` or `open_at` arguments; it does not automatically adopt every
CLI environment variable. The CLI has no general config-file/TTL option.

Keep the operator and token in trusted infrastructure. Model-controlled code
must not choose fresh IDs, alter policy, write the store or bypass the dispatcher.
Each child needs registration and explicit policy before its runner starts.

Protect every configured state parent with private ownership/modes or ACLs.
The general Rust/CLI store does not automatically repair existing permissions;
the Unix socket's 0600 mode does not protect separately located DB/events files.
Use a private parent (0700 on Unix) and restrictive umask before first startup.
Review independently configured events, Witness and export paths as well.

Current source creates the Docker image's state directory with mode 0700.
Existing named volumes retain their modes; inspect those separately. This
does not isolate a same-UID trusted executor sharing the state volume.

## Linux systemd setup

The v1.0.2 service template passed a disposable Ubuntu 24.04 arm64 container
test with real systemd: it refused startup without a token, accepted tokened
admission and policy calls, and kept a killed identity denied after restart.
This is not a verified native host installation; validate it on your target
distribution before production. Reproduce the bounded test with
`python3 tests/systemd_acceptance.py`. The test uses a privileged disposable
Docker container and no host port or Deadbolt state.

From a built checkout, install the
binary at the unit's expected path, create a dedicated system account and configure
a private environment file. These commands assume standard Linux user-management
and systemd tools:

```sh
cargo build --locked --release --bin deadbolt
sudo install -m 0755 target/release/deadbolt /usr/local/bin/deadbolt
sudo useradd --system --user-group --home-dir /var/lib/deadbolt --shell /usr/sbin/nologin deadbolt
sudo install -d -m 0700 /etc/deadbolt
sudo install -m 0600 dist/deadbolt.env.example /etc/deadbolt/deadbolt.env
```

If the account already exists, inspect/reuse it instead of recreating it. Edit
`/etc/deadbolt/deadbolt.env` as root and set a strong, nonempty `DEADBOLT_TOKEN`
before starting. Do not put the real token in a shell command history or commit.
The template token is empty and TCP will correctly refuse startup until configured.

```sh
sudo install -m 0644 dist/deadbolt.service /etc/systemd/system/deadbolt.service
sudo systemctl daemon-reload
sudo systemctl enable --now deadbolt
sudo systemctl status deadbolt
sudo journalctl -u deadbolt --since '10 minutes ago'
sudo -u deadbolt env DEADBOLT_DB=/var/lib/deadbolt/deadbolt.db DEADBOLT_EVENTS=/var/lib/deadbolt/deadbolt-events.jsonl /usr/local/bin/deadbolt status
```

The unit listens on loopback `127.0.0.1:9782`, runs as `deadbolt` and manages
`/var/lib/deadbolt` mode 0700. Supply endpoint/token to the trusted executor and
use the same paths/account for operator commands. Remote applications cannot
connect directly; put admission in their local trusted executor instead.

## Containers

```sh
cp dist/deadbolt.env.example dist/deadbolt.env
chmod 0600 dist/deadbolt.env
docker compose -f dist/docker-compose.yml up --build -d
docker compose -f dist/docker-compose.yml exec deadbolt deadbolt drill
docker compose -f dist/docker-compose.yml exec deadbolt deadbolt status
```

The default Compose sidecar uses a mode-0600 Unix socket and named state volume;
its token is optional for this same-UID transport. If you configure a token,
supply it to the executor too. Keep `dist/deadbolt.env` private and ignored.

Add a trusted executor service to the same Compose project, attach the declared
`deadbolt-state` volume at `/home/deadbolt/.deadbolt`, run it as UID 10001 and set
`DEADBOLT_SOCK=/home/deadbolt/.deadbolt/deadbolt.sock`. Compose prefixes named
volumes with the project name; it is not necessarily a global volume literally
named `deadbolt-state`. Check `docker compose ... config` before deployment.

```sh
docker compose -f dist/docker-compose.yml exec deadbolt deadbolt kill --agent RUN_ID
docker compose -f dist/docker-compose.yml restart deadbolt
```

Ordinary stop/restart/down preserves the named volume. `down --volumes` deletes
state and revocations; it is a deliberate destructive action, not routine cleanup.
No port is published. A host app on Docker Desktop should use a native sidecar;
container-local loopback cannot be reached by ordinary port forwarding.

## Backups, evidence and retention

Stop new dispatches, stop the sidecar/proxies and close all embedded connections
before a file-level backup. Copy the entire state directory with permissions
preserved, including SQLite WAL/SHM files if present, JSONL and optional Witness
files. Protect backups like the source state. Use SQLite's supported backup API
instead if a live online backup is required; copying an active database file by
itself is not a consistent backup procedure.

SQLite and JSONL are separate writes. A crash or sink failure can leave a record
in only one output. Preserve both and investigate discrepancies; do not silently
rewrite records to imply atomic evidence. Content hashes are identifiers, not
protection against an operator editing files.

There is no automatic retention, rotation, replication or pruning service.
Plan capacity and retention with your operator. Do not casually remove lease
rows or replace a store: that can erase terminal revocations and permit reuse of
previously killed identities.

## Upgrade and rollback

1. Record the reviewed source SHA, binary path and settings; stop new dispatches.
2. Back up the quiescent state as above and preserve the previous binary.
3. Build/install the selected revision, run `deadbolt --version` and `deadbolt drill`.
4. Run the acceptance checks below against disposable state, then start the service.
5. Check that existing killed IDs still deny and unrelated IDs work before reopening dispatch.

There is no general backwards-migration guarantee. Preserve pre-upgrade state;
rolling back to an older binary must be validated against its matching backup
and must not erase revocations recorded since that backup. Do not resume execution
on a stale snapshot without resolving that revocation gap.

## Acceptance before another developer ships

| Case | Required observable result |
| --- | --- |
| Fresh lease + permitted tool | Tool body runs exactly through the checked dispatcher |
| Off-list tool/destination | Denial and no observable side effect |
| Irreversible tool | Deny, one approved admission, then deny again |
| Kill agent A | A and registered descendants deny; unrelated B still works |
| Restart service/executor | Existing terminal revocations remain effective |
| Lease silence past TTL | `lease_expired`; no silent refresh or replacement ID |
| Sidecar absent or token wrong | No tool body runs |
| Unavailable database/evidence output | Default allow path fails closed |
| Concurrent one-shot attempts | At most one consumes an approval |
| Malformed decision / non-2xx HTTP | Client/proxy refuses execution |
| MCP batch / invalid envelope | No forwarded tool body |
| Worker saturation / slow request | Bounded resources and safe denial; later normal requests work |

Run the repository checks in [CONTRIBUTING.md](../CONTRIBUTING.md), then validate
these cases inside the actual application. The selected third-party MCP server,
platform, permissions, cancellation mechanism and failure recovery are integration
specific. No throughput benchmark, SLA or universal framework acceptance is claimed.

## Incident workflow

Pause or kill the affected ID through the operator path, record the exact source
revision and state paths, and preserve evidence before changing configuration.
Export the affected lineage:

```sh
deadbolt export --agent RUN_ID --children --out evidence.jsonl
deadbolt incident --agent RUN_ID --json --children --out incident.json
```

Restrict access to these artifacts; they may contain identifiers and operational
context. Review [incident taxonomy](INCIDENT.md) and your own legal/retention
requirements. Deadbolt does not send notices or certify regulatory compliance.
