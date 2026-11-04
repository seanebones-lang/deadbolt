# DeadBolt Python client

`n11-deadbolt-client` connects a trusted Python executor to a separately installed
DeadBolt local sidecar. Each protected callback gets a fresh admission decision;
a denial leaves its body uncalled. The base client has no runtime dependencies.

## Version and installation

This source builds **Python client 2.0.1.post1**, paired with **engine 2.0.1**.
The postrelease corrects package documentation only. Its three Python runtime
modules are byte-identical to client 2.0.1. There is no new engine version,
behavior change or state migration. Existing 2.0.1 artifacts remain immutable.
Source preparation does not publish a package; check the exact version on
[PyPI](https://pypi.org/project/n11-deadbolt-client/2.0.1.post1/) before installing.

In your application's virtual environment, once this version is published:

```sh
python -m pip install 'n11-deadbolt-client==2.0.1.post1'
# Optional SDK adapter, Python 3.10+:
python -m pip install 'n11-deadbolt-client[openai-agents]==2.0.1.post1'
```

Base client support starts at Python 3.9. The optional extra pins
`openai-agents==0.23.1`. Installing either route makes no model requests and
requests no API credentials. Your application configures its own provider.

Install **engine 2.0.1 separately** using the checksum-verified native archive,
Cargo or Homebrew route in [installation](https://github.com/seanebones-lang/deadbolt/blob/main/docs/INSTALL.md).
The wheel contains no engine, starts no sidecar and protects no unwrapped route.
Verify the engine with `deadbolt --version` (expect `deadbolt 2.0.1`) and
`deadbolt drill` (expect `deadbolt drill ok`). No registry account is needed to install.

The already-published 2.0.1 wheel has the correct runtime modules and package
version, but its frozen long description recommends 2.0.0. The postrelease
corrects that documentation without replacing either version's bytes. A pinned
2.0.1 application does not need a behavior migration for this metadata correction.

## Connect one protected callback

After trusted operator setup starts the sidecar and issues a scoped admission
credential, configure `DEADBOLT_SOCK` and `DEADBOLT_ADMISSION_TOKEN` in your
executor. Keep operator/supervisor credentials and private state outside
model-facing processes. The operator configures tools, destinations and required
approvals before protected dispatch.

```python
import deadbolt_client as deadbolt

result = deadbolt.dispatch("run-001", "write_note", lambda: write_reviewed_note())
if not result["executed"]:
    show_denial(result["decision"])
```

`write_reviewed_note` and `show_denial` are application functions. Never cache
an allow or execute after a denial or transport failure. Use exact-action
wrappers where approval must bind to content rather than just a tool name.
`ensure` confirms record presence, not a live admission grant.

Fresh version 2 roots default to supervisor renewal. Provision a separate
trusted heartbeat before idle expiry, or deliberately select activity mode.
Neither mode extends a fixed deadline or revives killed/expired state. Existing
leases retain their stored mode; installing a client does not migrate them.

## Limits

Only execution routes that call DeadBolt are protected. Destination policy is
not OS network interception. The executor owns cancellation of already running
work. The optional `deadbolt_supervisor.run_owned` POSIX reference needs Python
3.13+ and its required wait interface; it refuses unsupported hosts, including
Windows, before execution. Ordinary callback wrappers do not cancel running bodies.

## Build and verify

A reviewed checkout of this postrelease source builds client 2.0.1.post1;
the older `v2.0.1` tag retains client 2.0.1. Record the exact source revision.
The engine remains 2.0.1 in both. The package acceptance script builds the sdist,
builds its wheel outside the checkout, checks metadata and installs without
optional dependencies to test unavailable-sidecar denial. Publication additionally
requires runtime-module equality with the checksum-pinned original 2.0.1 wheel.

```sh
python -m build --outdir release-artifacts/python
python tests/python_package_acceptance.py
```

Build/check tooling is separate from runtime requirements; see the repository's
contribution and release instructions. A local wheel is not publication evidence.

## Guides and support

- [Python setup and SDK examples](https://github.com/seanebones-lang/deadbolt/blob/main/docs/PYTHON.md)
- [Upgrade and Rust API migration](https://github.com/seanebones-lang/deadbolt/blob/v2.0.1/docs/MIGRATING-V2.md)
- [Exact-action approvals](https://github.com/seanebones-lang/deadbolt/blob/v2.0.1/docs/ACTION-APPROVALS.md)
- [Supervision](https://github.com/seanebones-lang/deadbolt/blob/v2.0.1/docs/SUPERVISION.md)
- [Owned-process cancellation](https://github.com/seanebones-lang/deadbolt/blob/v2.0.1/docs/EXECUTOR-CANCELLATION.md)
- [Local data and privacy](https://github.com/seanebones-lang/deadbolt/blob/v2.0.1/docs/PRIVACY.md)
- [Private security reporting](https://github.com/seanebones-lang/deadbolt/blob/v2.0.1/SECURITY.md)

Maintained by NextEleven LLC, licensed Apache-2.0. Support:
nextelevenstudios@gmail.com. Never include credentials or private application
data in public issues or support reports.
