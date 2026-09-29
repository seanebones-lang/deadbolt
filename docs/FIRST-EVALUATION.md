# First evaluation: real effects in disposable state

This is the shortest demonstration of Deadbolt's HTTP integration. It needs
Python 3.9 or newer and the installed v1.0.2 executable. No Python packages,
Node, Rust compiler, model account or API key are needed.

The v1.0.2 archives include `examples/evaluate.py` and
`examples/deadbolt_client.py`. Keep them in the same directory and record the
release revision with your report. The older v1.0.1 archives do not contain the
evaluation script.

## Run

Complete [installation](INSTALL.md), then from the repository root:

```sh
python3 examples/evaluate.py --binary /absolute/path/to/deadbolt
```

On Windows, use `python` and the path to `deadbolt.exe`. If the executable is
on PATH, omit `--binary`. A successful run prints JSON with `"passed": true`
and exits zero. Save stdout to retain the report. A failure prints
`"passed": false` and exits nonzero; do not treat it as acceptance.

The script starts its own token-protected loopback sidecar with a random token,
creates a temporary database and harmless files, then stops the sidecar and
removes its temporary workspace. Existing Deadbolt state and running services
are not used. A free-port selection race can cause startup failure; retry the
evaluation if another process acquired the selected port.

## What it proves

Eleven checks inspect actual file contents or absence:

- An allowed write creates the expected file.
- An off-policy tool creates no file.
- A configured irreversible write needs approval; one approval permits one
  admission and the next attempt is denied.
- Operator kill blocks both the parent and its registered child.
- An unrelated identity still writes successfully.
- Sidecar outage prevents execution.
- Restart and re-ensure preserve revocation while unrelated work remains usable.

The report includes the executable version and SHA-256. These are observations
from the local run, not a signed or independent attestation.

## Review the integration

Read `dispatch` in [the script](../examples/evaluate.py). The body runs only
after a fresh explicit `allow`. The surrounding program owns identity, policy,
credentials, tool naming and execution. This local example holds operator
authority in one process for demonstration; a production model must not receive
that authority.

This does not exercise a real AI agent, destination policy, spending, backup,
wrong-token handling or all application routes. Existing client contracts cover
additional failure cases. Use the [Hermes showcase](HERMES-SHOWCASE.md) for the
third-party MCP transport and the [pilot worksheet](PILOT.md) for your application.
Kill blocks subsequent admissions; it does not cancel work already running.

## Share useful evaluation feedback

Record the repository revision, executable version/hash, OS, Python version,
command and report. Explain any installation difficulty and identify the
dispatcher you want to protect. Use a public issue for ordinary problems and
[private reporting](../SECURITY.md) for vulnerabilities. Remove credentials,
customer data and private paths before sharing output.
