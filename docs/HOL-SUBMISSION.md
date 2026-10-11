# Publish a release and update HOL

DeadBolt already has an [owner-verified HOL listing](https://hol.org/registry/plugins/seanebones-lang%2Fdeadbolt). Software publication and catalog ingestion are separate steps. Complete [release verification](RELEASING.md) before submitting new version information.

## Update the existing listing

1. Publish the immutable versioned GitHub release with verified native downloads, checksums, source SHA, installation instructions and accurate signing status. Mark an RC as a prerelease; keep stable and RC versions distinct.
2. Inspect the current listing, install guide, scan source SHA and launch label. Record which fields need correction instead of assuming a local scanner pass refreshes HOL.
3. Use the owner controls when available, or send the existing HOL support contact the canonical slug, release URL, exact source SHA, versioned installation guide and requested rescan. Link the project site and explain that DeadBolt is a Rust library/local HTTP sidecar/stdio MCP execution gate.
4. Ask which supported manifest or owner fields populate standalone-component website, privacy and license/terms information. Do not invent a Codex plugin manifest or imply certification to satisfy a catalog score.
5. Verify the public catalog after HOL processes the update. Submission, acceptance, scan completion and changed public metadata are distinct results.

## When a contribution PR is appropriate

HOL's [Awesome Codex Plugins contribution guide](https://github.com/hashgraph-online/awesome-codex-plugins/blob/main/CONTRIBUTING.md) requires a real Codex plugin manifest and icon. Its submission PR changes one alphabetically sorted README entry; the maintainer generator owns bundles, `plugins.json` and marketplace output. Do not manually commit generated catalog files. Inspect the current guide before proposing a contribution.

At the October 10, 2026 review, DeadBolt was already in HOL's standalone registry, but was not an entry in that awesome-list README. An existing standalone listing does not establish that it satisfies the Codex plugin bundle requirements. If HOL directs an update to a PR, follow their named repository and update the existing entry or metadata; do not create a duplicate listing.

The [scanner action](https://github.com/hashgraph-online/ai-plugin-scanner-action) can open or reuse a submission **issue**, with an explicitly configured target and token. That is different from the README contribution PR. DeadBolt's normal scanner CI keeps submission disabled and network analysis off. Release submission is intentional, not an automatic side effect of every push.

## Evidence to include

- Canonical slug: `seanebones-lang/deadbolt`.
- Versioned release, exact source SHA, successful CI and scanner run links.
- Native targets, wheel/sdist availability, checksums and Mac notarization logs.
- Versioned install, first evaluation and integration instructions.
- Explicit boundaries: trusted dispatcher coverage, operator-owned policy and credentials, later-admission revocation, and no cancellation of an already running body.
- Any remaining third-party installation or adopter-owned acceptance gates.

The walkthrough videos are versioned media assets, hosted outside the source tree so large binary files do not prevent full static scanning or inflate source installation. The optional SDK dependency closure is exposed through root `requirements.lock`; it is the same pinned set audited by CI.
