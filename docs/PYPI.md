# DeadBolt Python client

`n11-deadbolt-client` connects a trusted Python executor to a separately installed
DeadBolt local sidecar. It wraps a protected callback in a fresh allow/deny
decision: a denial leaves the callback uncalled. The base client has no runtime
dependencies. The optional `openai-agents` extra adapts function tools to the
tested OpenAI Agents SDK contract.

This package is **2.0.0rc1**, a developer-evaluation prerelease matching engine
**2.0.0-rc.1**. The engine is not included in the wheel. Installing the client
does not start a sidecar, connect an account, or protect unwrapped tool routes.
Existing 1.x packages and editor/MCP bundles remain separately versioned.

## Install a matching pair

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
