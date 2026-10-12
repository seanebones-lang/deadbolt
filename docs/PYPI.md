# DeadBolt Python client

`n11-deadbolt-client` connects a trusted Python executor to a separately installed
DeadBolt local sidecar. It wraps a protected callback in a fresh allow/deny
decision: a denial leaves the callback uncalled. The base client has no runtime
dependencies. The optional `openai-agents` extra adapts function tools to the
tested OpenAI Agents SDK contract.

This source package is **2.0.0rc2**, a developer-evaluation candidate matching
engine **2.0.0-rc.2**. RC2 publication is a separate release gate; the published
pair described below is still client **2.0.0rc1** and engine **2.0.0-rc.1**.
The engine is not included in the wheel. Installing the client
does not start a sidecar, connect an account, or protect unwrapped tool routes.
Existing 1.x packages and editor/MCP bundles remain separately versioned.

## Install the published matching pair

In your application's virtual environment, pin the PyPI candidate explicitly:

```sh
python -m pip install 'n11-deadbolt-client==2.0.0rc1'
# Optional SDK adapter, Python 3.10+:
python -m pip install 'n11-deadbolt-client[openai-agents]==2.0.0rc1'
```

Install the engine with `cargo install n11-deadbolt --version 2.0.0-rc.1 --locked --bin deadbolt`,
the Homebrew preview cask on macOS, or a verified native archive. No registry
account is required. A release wheel is also available for manual installation:

Select the versioned native engine and client wheel from the
[2.0.0-rc.1 GitHub release](https://github.com/seanebones-lang/deadbolt/releases/tag/v2.0.0-rc.1).
Verify their SHA-256 checksums and the native archive's `BUILD.json` source
revision. Install the wheel in your application's virtual environment:

```sh
python -m pip install /absolute/path/to/n11_deadbolt_client-2.0.0rc1-py3-none-any.whl
/absolute/path/to/deadbolt --version
/absolute/path/to/deadbolt drill
```

Expected engine version: `deadbolt 2.0.0-rc.1`. For an optional SDK adapter,
install the tested extra from the same wheel:

```sh
python -m pip install '/absolute/path/to/n11_deadbolt_client-2.0.0rc1-py3-none-any.whl[openai-agents]'
```

The extra pins `openai-agents==0.23.1`; it does not request API credentials or
make model requests during installation. Your application configures its own
provider separately. A source-built wheel follows the same version contract;
registry publication and native downloads are separate release steps.

## Evaluate the corrected source candidate

For RC2, build both components from the same reviewed source revision. In that
checkout, build the engine and client wheel:

```sh
cargo build --release --locked --bin deadbolt
python -m build --outdir release-artifacts/python
```

Install the resulting `n11_deadbolt_client-2.0.0rc2-py3-none-any.whl` in your
application's virtual environment and use that checkout's `target/release/deadbolt`
(`deadbolt.exe` on Windows). Confirm `deadbolt --version` reports
`deadbolt 2.0.0-rc.2` before connecting it. Keep the exact source revision and
artifact hashes with your test results. Do not pair this wheel with the RC1
native downloads or treat a local source build as a published, signed release.

RC2 makes fresh leases supervisor-renewed by default. Provision a separate
supervisor heartbeat before handing a run to the executor; activity renewal is
an explicit operator opt-in. See the [current supervision guide](https://github.com/seanebones-lang/deadbolt/blob/main/docs/SUPERVISION.md)
for the credential boundary, idle TTL and fixed deadline. Existing RC1 leases
retain their stored renewal mode; installing a new wheel does not migrate them.

## Connect a protected callback

After trusted operator setup has started the sidecar and issued a scoped
admission credential, configure `DEADBOLT_SOCK` and
`DEADBOLT_ADMISSION_TOKEN` in the executor. Keep the operator token out of
model-facing processes. Configure each lease's tools, destinations and approvals
through the operator before handing it to the dispatcher.

```python
import deadbolt_client as deadbolt

result = deadbolt.dispatch("run-001", "write_note", lambda: write_reviewed_note())
if not result["executed"]:
    show_denial(result["decision"])
```

`write_reviewed_note` and `show_denial` are your application's functions.
Do not cache an allow or run the body after any denial or transport failure.
Use the exact-action wrapper for content-bound one-shot approvals. The optional
supervisor credential belongs only to trusted liveness management and cannot
authorize a callback.

## Guarantees and integration limits

The 2.0 engine adds explicit offline destination exceptions, fixed run deadlines
and optional supervisor-only idle renewal. These require the matching engine;
installing a newer client alone cannot add them to an older executable.
Destination declarations are admission policy, not OS network interception.
Every protected execution path must pass through fresh admission.

`deadbolt_supervisor.run_owned` is an optional POSIX reference for cancelling
processes the trusted executor starts. It supports Linux and macOS with Python
3.13 or newer and the required wait interface. It refuses unsupported hosts,
including Windows, before executing work. Ordinary callback wrappers do not
cancel already running bodies. The base sidecar client supports Python 3.9+.

## Guides and support

- [Python setup and SDK examples](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/docs/PYTHON.md)
- [Upgrade and Rust API migration](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/docs/MIGRATING-V2.md)
- [Exact-action approvals](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/docs/ACTION-APPROVALS.md)
- [Supervisor-only liveness](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/docs/SUPERVISION.md)
- [Owned-process cancellation and its limits](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/docs/EXECUTOR-CANCELLATION.md)
- [Local data and privacy](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/docs/PRIVACY.md)
- [Security reporting](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0-rc.1/SECURITY.md)

DeadBolt is maintained by NextEleven LLC and licensed under Apache-2.0.
Support: nextelevenstudios@gmail.com. Do not include credentials or private
application data in a public issue or support report.
