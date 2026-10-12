# DeadBolt Python client

`n11-deadbolt-client` connects a trusted Python executor to a separately installed
DeadBolt local sidecar. It wraps a protected callback in a fresh allow/deny
decision: a denial leaves the callback uncalled. The base client has no runtime
dependencies. The optional `openai-agents` extra adapts function tools to the
tested OpenAI Agents SDK contract.

This release source pairs client **2.0.0** with engine **2.0.0**.
Use a matching engine/client pair and check the [release page](https://github.com/seanebones-lang/deadbolt/releases)
and [installation guide](https://github.com/seanebones-lang/deadbolt/blob/main/docs/INSTALL.md)
for published artifacts and package-manager availability. The wheel does not
include an engine, start a sidecar or protect unwrapped routes. Earlier published
packages retain their original immutable files.

## Install from PyPI

When version 2.0.0 is available on PyPI, select it explicitly in your application's
virtual environment. The wheel/source archive in the GitHub release is an
alternative with separately verifiable checksums:

```sh
python -m pip install 'n11-deadbolt-client==2.0.0'
# Optional SDK adapter, Python 3.10+:
python -m pip install 'n11-deadbolt-client[openai-agents]==2.0.0'
```

Install the matching native engine separately. No registry account or API key
is required for installation. The package's description is a frozen release
snapshot; current installation status is documented here.

## Install a checksum-verified release wheel

Download the native ZIP, wheel and `SHA256SUMS` from that release. Verify the
hashes and the native archive's `BUILD.json` revision. In your application's
virtual environment:

```sh
python -m pip install /absolute/path/to/n11_deadbolt_client-2.0.0-py3-none-any.whl
/absolute/path/to/deadbolt --version
/absolute/path/to/deadbolt drill
```

Expected: `deadbolt 2.0.0`, then `deadbolt drill ok`. For the optional SDK adapter (Python 3.10+):

```sh
python -m pip install '/absolute/path/to/n11_deadbolt_client-2.0.0-py3-none-any.whl[openai-agents]'
```

The extra pins `openai-agents==0.23.1`. Installation does not request API
credentials or make model requests. Your application configures its provider.

## Build the matching release source

Use the `v2.0.0` release tag once published, or a reviewed 2.0.0 source checkout,
to build both components. Record `git rev-parse HEAD` and compare it with the
release notes and the native archive's `BUILD.json`:

```sh
cargo build --release --locked --bin deadbolt
python -m build --outdir release-artifacts/python
```

Install its `n11_deadbolt_client-2.0.0-py3-none-any.whl` in your environment and
use its `target/release/deadbolt` (`deadbolt.exe` on Windows). Keep the revision
and artifact hashes with your results. A local build is not a signed download.

Version 2 fresh roots default to supervisor renewal. Provision a separate supervisor
heartbeat before initial idle expiry; activity renewal is explicit operator
opt-in. See [supervision](https://github.com/seanebones-lang/deadbolt/blob/main/docs/SUPERVISION.md).
Existing leases preserve their stored mode; installing a wheel does not migrate them.

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

- [Python setup and SDK examples](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/docs/PYTHON.md)
- [Upgrade and Rust API migration](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/docs/MIGRATING-V2.md)
- [Exact-action approvals](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/docs/ACTION-APPROVALS.md)
- [Supervisor-only liveness](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/docs/SUPERVISION.md)
- [Owned-process cancellation and its limits](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/docs/EXECUTOR-CANCELLATION.md)
- [Local data and privacy](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/docs/PRIVACY.md)
- [Security reporting](https://github.com/seanebones-lang/deadbolt/blob/v2.0.0/SECURITY.md)

DeadBolt is maintained by NextEleven LLC and licensed under Apache-2.0.
Support: nextelevenstudios@gmail.com. Do not include credentials or private
application data in a public issue or support report.
