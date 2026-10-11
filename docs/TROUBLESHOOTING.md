# Troubleshooting

First record the OS, `deadbolt --version`, source commit (`git rev-parse HEAD`),
installation method and exact command. Avoid sharing tokens, private tool
arguments, databases or full environment dumps. The installed binary's Cargo
version may be shared by several unreleased commits.

## Installation and startup

| Symptom | Check | Action |
| --- | --- | --- |
| `deadbolt` command not found | Cargo bin directory is on PATH; `command -v deadbolt` on Unix or `Get-Command deadbolt` in PowerShell | Reopen the terminal or call the Cargo bin executable directly |
| `cargo install n11-deadbolt` or `pip install n11-deadbolt-client` cannot find the package | These package names are not published to crates.io/PyPI | Use the [source/Git installation](INSTALL.md) |
| SQLite/compiler/linker build error | Native compiler and Rust version | Install the OS C/C++ toolchain; on Windows use MSVC Build Tools and SDK |
| `token_required` | TCP server has a nonempty `DEADBOLT_TOKEN` | Configure a private token before starting TCP; provide the same token to clients |
| `bind_refused` | Bind is loopback or a Unix path; port/socket already in use; token file mode | Use `127.0.0.1:PORT` or `[::1]:PORT`; stop the existing service deliberately; token files must be 0600 on Unix |
| Permission denied on a Unix socket | Client and server OS user/UID, socket path and parent directory access | Run the trusted client under the intended same account; container clients use UID 10001; do not make the socket public |
| `bind_refused` at an existing path | The path is a regular file, symlink, or live socket | Select a new path; the listener preserves those entries rather than deleting them |
| Docker host cannot connect | Socket is inside the container's volume | Attach a consumer container to that volume or run a native host sidecar; published ports cannot reach container-local loopback |

## Python imports and mismatched versions

- `ModuleNotFoundError: deadbolt_client`: use `python -m pip` in the same virtual
  environment as your application; inspect `python -c "import sys; print(sys.executable)"`.
- The base wheel works on Python 3.9+, but source building and the optional SDK
  need Python 3.10+. A Python package install does not install or start the sidecar.
- New helpers are absent from v1.0.3 source clients. Match the client and executable
  to your reviewed candidate; record the source SHA, not only the Cargo version.
- `/admit-action` on an old sidecar fails closed. Upgrade the selected executable
  deliberately; never fall back to `/admit` for an exact-reviewed action.

## Decision and operator problems

| Result or symptom | Meaning | Check/action |
| --- | --- | --- |
| HTTP 403 / `forbidden` | Admission credential used for operator routes or a different run | Complete setup with operator authority; pass a credential bound to the dispatcher's actual run |
| HTTP 401 / `unauthorized` | Missing or wrong token | Server/client environments match; restart the server deliberately after rotating the token |
| `no_lease` | No lease for that ID in this store | Ensure once before dispatch; confirm the executor-assigned ID and store paths |
| `killed` | Terminal revocation | Confirm operator intent; use a new ID only for a deliberately authorized new run |
| `lease_expired` | Silence exceeded TTL | Ensure is not a refresh; operator resume can renew a non-killed lease after review, or start an intentional new run |
| `paused` | Operator pause | Inspect status/policy; operator can resume after review |
| `spend_cap` | Reported cumulative spend crossed cap | Inspect spend and policy; resume alone leaves spend/cap policy in place |
| `purpose_exceeded` | Tool clip/allow-list or destination check denied | Inspect tool name, clips, explicit host and stored policy; use your tool's real name |
| `needs_human` | Required approval is missing, changed, expired or already consumed | Inspect policy; legacy approval covers run/tool only. For candidate exact-only tools review a new [exact-action envelope](ACTION-APPROVALS.md), not broad `approve`. Other policy must still allow the action |
| `store_unavailable` | Store/evidence/transport failed or response was unusable | Check service, endpoint, writable state, free disk, deadlines and logs; retain denial while diagnosing |
| Kill seems to do nothing | Different database/agent or dispatch skipped admit | Match DB/events paths, UID and ID; inspect the dispatch path; do not cache allow results |
| Sample exits with code 2 | Expected denial path | Read the emitted code; after kill, this is success for the demonstration |

The CLI controls a local database; `DEADBOLT_SOCK` does not redirect CLI kill to
a sidecar. Set `DEADBOLT_DB` and `DEADBOLT_EVENTS` to the executor's actual paths.
For container state, run the CLI inside the sidecar container. Status reflects
the stored row; expiration is checked at admission, so an `active` row can have
an expiry timestamp already in the past.

## Requests hang or disconnect

The sidecar reads a complete request within five seconds, writes with a
five-second timeout, limits requests to 65,536 bytes and caps active workers at
64 per listener. Excess connections close. Use one request per connection with
`Content-Length`; chunked transfer and duplicate/invalid lengths are rejected.
A truncated, oversized or malformed request may close without a JSON response;
the trusted client must treat that as denial. Avoid uncontrolled client retries
and check for slow local callers or a saturated workload.

## MCP-specific problems

- Put child arguments after `--`; use an executable available in the proxy's PATH.
- Keep child diagnostics on stderr. Stdio protocol messages must be one JSON-RPC
  2.0 object per line; top-level batches are rejected.
- A denied `tools/call` returns a JSON-RPC error with the denial token. A killed
  lease does not prevent `tools/list`, initialization or other non-tool methods.
- Use the server's actual tool names in policy. Automatic destination/cost
  discovery is a convention; use a trusted adapter for custom semantics.
- A sidecar-backed proxy needs `--serve-sock` plus the matching token. Without
  that flag the proxy uses its local database directly.

## Verify the binary you actually run

After source changes, do not assume an old binary is current. Build into a new,
empty target directory and point the consumer tests at that exact binary:

```sh
CARGO_TARGET_DIR=target/verification cargo build --locked --bin deadbolt
DEADBOLT_BIN="$PWD/target/verification/debug/deadbolt" python3 tests/client_contract.py
```

Choose a new directory name when diagnosing stale build artifacts. In PowerShell,
set `$env:CARGO_TARGET_DIR` and `$env:DEADBOLT_BIN` separately and use `.exe`.
Record the source SHA and binary path alongside the results.

## Report a reproducible problem

Include OS/compiler versions, commit, mode (embedded/sidecar/MCP/container),
redacted configuration paths, the decision token, expected behavior and a
minimal synthetic reproduction. Ordinary bugs go to repository issues;
suspected gate/auth bypasses go to private [security reporting](../SECURITY.md).

## Concurrent writers and `store_unavailable`

SQLite permits one writer at a time, including in WAL mode. Each connection
uses a five-second busy timeout. A held writer lock, slow disk or heavy write
contention can exhaust that wait; `store_unavailable` denies execution when
fail-closed enforcement is enabled. Keep the database on a local filesystem,
reduce simultaneous independent writers, and prefer one long-lived sidecar or
cloned Rust gate for an application. Do not disable enforcement to hide contention.

In v1.0.2 (not the original v1.0.1 binary archives), the default store writes a spend update, evidence sequence and
SQLite evidence row in one immediate transaction. Other default evidence writes
also reserve their sequence and insert the row in one transaction. A JSONL write
failure rolls back that SQLite transaction. The JSONL file and SQLite are still
separate resources: a partial file write or failed SQLite commit can leave JSONL
bytes without a committed row. Reconcile against SQLite when investigating errors.

An error is not a general promise that a mutation had no effect. A commit error can
have an uncertain outcome, and an optional Witness-sink failure occurs after the
primary commit. On the default primary sink, a failed primary evidence write now
rolls back `approve`, `resume`, and policy grants. Other mutations can still commit
before evidence emission fails.
Inspect current state before retrying `spend_add`; blind retries can double-count
reported spend. No automatic mutation replay is performed. Record the exact source
revision, OS and operation, and use the issue template with a redacted reproduction.

For a reproduction, enable `DEADBOLT_STORE_DIAGNOSTICS=1` in the trusted operator's
process environment (PowerShell: `$env:DEADBOLT_STORE_DIAGNOSTICS = "1"`). Selected
spend and primary-evidence errors write a static stage and error code to stderr,
for example `stage=spend.begin code=DatabaseBusy/5`. Diagnostics are off by default
and omit SQL, error messages, paths, tokens and payloads. They do not cover every
storage failure, change retry behavior, or replace state inspection. Windows CI
enables this diagnostic flag so a failing repetition retains useful stage evidence.
