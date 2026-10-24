# Integration dependency locks

The OpenAI Agents, Hermes and FastMCP acceptance environments pin every resolved
Python dependency and its accepted SHA-256 distribution hashes. CI installs these
three files with `pip --require-hashes`; it refuses an unlisted artifact or an
unresolved dependency. Conditional platform dependencies remain in the locks.
This verifies downloaded bytes against reviewed pins, not the safety of the
package itself. Dependency audits remain separate checks.

To install a test environment, use its complete lock rather than an individual
SDK version. For example:

```sh
python -m pip install --require-hashes -r tests/openai-agents-requirements.txt
```

When reviewing a dependency change, regenerate the relevant lock with `uv pip
compile --generate-hashes --universal --python-version 3.12 --no-header
--no-annotate`, using the existing pinned file as input and output. Review both
version changes and newly added conditional dependencies, then run the package
audit and installed consumer checks on the supported CI platforms. For an
intentional FastMCP upgrade, update `tests/fastmcp-requirements.in` first and
compile it to `tests/fastmcp-requirements.txt` with those same flags.

These locks cover the integration environments. They do not make the separate
audit/bootstrap tooling, npm installations or provider downloads hash-locked.
Cargo uses its checked-in `Cargo.lock`; released native archives and wheels have
their own versioned artifact checksums and publication gates.

## October 24 integration-environment maintenance

The FastMCP recipe and acceptance lock now select 4.1.2 in response to
GHSA-7xgj-8392-5q4r and GHSA-vqqp-x5qm-5qp4. The OpenAI Agents and Hermes locks
select certifi 2026.10.19 for PYSEC-2026-4320 / GHSA-6q48-6c7w-9gw2. The
OpenAI Agents lock and Python build-system pin select setuptools 84.1.0 for
GHSA-xmj3-9gf7-h52w. The Hermes acceptance lock selects MCP SDK 1.30.0 within
the existing 1.x line, addressing the additional SDK advisories reported by its
audit, including GHSA-hvrp-rf83-w775, GHSA-jpw9-pfvf-9f58 and
GHSA-84m7-p3x7-pcfv. The test uses stdio; this version update is not evidence
that an HTTP/session or experimental-task vulnerability was exercised there.
Universal distribution hashes were regenerated while preserving the other
resolved versions. No audit exceptions were added.

These updates maintain optional integration and build dependencies; they do not
identify a defect in the DeadBolt Rust engine or republish an existing artifact.

Local checks on macOS arm64, Python 3.12.13: fresh hash-required installs and
`uv pip check` passed for all three locks; `pip-audit==2.10.1 --no-deps
--disable-pip` reported no known vulnerabilities for each complete lock at check
time. The client wheel built with the updated build-system pin and installed
outside the source tree. Against released DeadBolt 2.0.0, FastMCP passed 20
consumer checks, OpenAI Agents passed 19 tests and the pinned Hermes transport
fixture passed 13 effect checks. No model/provider calls were made. Linux and
Windows consumer validation remains the separate CI gate.
