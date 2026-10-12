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
