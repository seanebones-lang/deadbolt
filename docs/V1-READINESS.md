# Deadbolt v1 readiness

Updated: 2026-09-29. Version 1.0.0 is published at source
`61861c05cadf65022723776de869eabc71475de6` after PRs #1–#3 were merged.
The exact merged source passed [source CI](https://github.com/seanebones-lang/deadbolt/actions/runs/36587225122)
and [native packaging, Hermes acceptance and upgrade](https://github.com/seanebones-lang/deadbolt/actions/runs/36587225214).
All four published archives were downloaded again and matched the verified
artifacts byte for byte. The [release](https://github.com/seanebones-lang/deadbolt/releases/tag/v1.0.0)
is the authority for released downloads; later source changes are separate.

## Next milestones

1. An unfamiliar developer independently reproduces installation and the Hermes
   acceptance. Record confusing steps and any failure before adding features.
2. A bounded external application pilot inventories and protects one real dispatcher;
   see [the evaluator brief](EVALUATOR.md) and [pilot checklist](PILOT.md).
3. A second application exercises HTTP or Rust integration on its actual deployment.
4. Validate systemd on a disposable Linux host before promoting the template from
   experimental; extend OS/architecture coverage only with fresh acceptance.
5. Prioritize signing, notarization and registry distribution when evaluation
   identifies them as installation barriers.

## The v1 promise

Deadbolt is a local admission gate owned by a trusted executor. Before each
tool or registered-child dispatch, that executor checks the gate and starts the
body only on an explicit allow. Operators can revoke an identity and its
registered descendants, apply policy, and inspect structured incident evidence.

The supported integration surfaces are the Rust library, local HTTP sidecar
with Python/Node source clients, and individual stdio MCP tools/call messages.
There is no model-provider dependency. A security boundary requires the executor
to own identities, dispatch paths, credentials, policy and state.

V1 must not promise cancellation of running actions, arbitrary-code sandboxing,
remote MCP support, automatic child-policy inheritance, independent spend
measurement, per-agent authorization from the shared sidecar token, or
tamper-proof evidence. Keep these limits prominent in installation and adoption
material, not only in security documentation.

## Completed v1 work sequence

| Order | Work | Completion evidence |
| --- | --- | --- |
| 1 | Freeze the supported v1 behavior and compatibility contract | Document public Rust API, HTTP/CLI response behavior, identity lifecycle, state upgrades, supported platforms and experimental deployments |
| 2 | Prove enforcement in one selected application | Actual tool-body markers show allow executes and deny does not; include approval, descendants, outage and restart cases |
| 3 | Make installation reproducible and convenient | Versioned release archives for validated platforms, checksums, source archive, exact source identity and installation instructions tested against those artifacts |
| 4 | Exercise the operator lifecycle | Disposable-state install, policy, kill, incident export, restart, backup/restore and upgrade drill with revoked identities still denied |
| 5 | Review the candidate and ship deliberately | Clean candidate, passing candidate CI, accurate changelog, final review, merge, immutable release tag and release notes |

Avoid adding a dashboard, cloud service, broad framework catalogue or additional
policy features before these gates pass. Those additions do not establish the
first release's enforcement or installation contract.

## Verified starting evidence

- Linux/macOS/Windows and Rust 1.85 CI pass at commit
  `22896305b0ededd8bf5f979f153f36d822be7261`:
  [CI run](https://github.com/seanebones-lang/deadbolt/actions/runs/36581731717).
- Embedded Rust consumer, Python/Node contracts, source installation, native
  quick start and a separate nonroot container consumer have recorded passing
  evidence in [the standalone review](STANDALONE-REVIEW.md).
- Installation, quick start, integration, FAQ, troubleshooting and operations
  guides exist. Review their commands again against the final release artifacts.
- Protocol and concurrency regressions cover previously identified failures.
  This evidence does not establish security certification or every integration.

## Release and adoption gates

| Gate | Current status | Required decision or work |
| --- | --- | --- |
| Independent-host MCP acceptance | Passed for the selected route | Hermes' real MCP transport and reference filesystem server passed 13 checks; see [showcase](HERMES-SHOWCASE.md). Whole-agent Hermes coverage is not claimed |
| Stable API and compatibility policy | Published v1 contract | See [compatibility](COMPATIBILITY.md) and versioned release notes |
| Release packaging | Four native targets published | Final exact-source packaging and checksum/content verification passed for Linux x86_64, Windows x86_64 and Mac arm64/x86_64 |
| Version/changelog | 1.0.0 published | The final tag is immutable; later changes need a separate release |
| Native systemd deployment | Unvalidated | Validate on a disposable systemd host, or clearly keep this deployment template experimental for v1 |
| Registry distribution | Unpublished | Source/binary distribution can support v1; publish crates.io only if selected and prepared. Python/Node remain source clients unless separately packaged |
| Final source review and publication | Completed | PR #3 merged after candidate checks; exact-main checks passed before v1.0.0 publication |
| Dependency advisories | Passed on 2026-09-29 | cargo-audit 0.22.2 found no matching vulnerabilities or warnings in 84 locked dependencies against RustSec database commit f23b768236fe2880e4cfa167da662cad8ca79240 |

The first application's acceptance is a claim about that application. It does
not imply that other developers can bypass integration work or that any arbitrary
project becomes contained by installing the executable.

Hermes was selected because its MCP client provides an independent integration
surface. Its transport-level acceptance uses no paid model calls. A model-driven
conversation and whole-agent dispatcher adapter are separate later milestones.
The candidate also passes four Python/Node consumer contracts locally, including
a quiescent state-directory backup/restore that preserves killed denial and an
unrelated allowed identity. Hermes MCP acceptance also passed on Linux in the
candidate run. The upgrade test exercises state created by original source
`b8a13036567ca1b71ed14c6b36cbf84df80fe647` against the RC executable. It preserves
revocations, lineage, policies, approvals, spend and existing evidence; it does
not imply every historical source revision or a downgrade is supported.

## Application acceptance record

Use a disposable workspace and harmless observable bodies. Record the source
revision, platform, transport, executor and exact commands. Prefer body execution
counts and files over decision JSON alone.

- Allowed body runs once through the dispatcher.
- Off-list tool and foreign destination produce no body side effect.
- An irreversible tool cannot run before approval; one approval permits at most
  one admission and a subsequent attempt cannot run.
- Kill blocks the same identity and registered descendants, while unrelated
  identities still work. Reusing or re-ensuring the killed identity does not
  restore execution.
- Service/executor restart preserves revocation.
- Sidecar outage, wrong token, unusable decision and unavailable state prevent
  the body from running.
- The application assigns stable identities and configures children before
  dispatch. No actual route caches an allow or skips admission.

Use the broader [operations acceptance checklist](OPERATIONS.md) for the chosen
transport. Keep operator credentials and state outside model-controlled access.

## Release handoff

The release should give a developer one clear path: install a pinned artifact,
verify version/checksum, run the drill, select an integration, configure policy,
prove deny prevents their body, and operate/recover the state. Include a runnable
demonstration and a concise account of boundaries in the release notes.

If a gate remains open at the end of the day, publish a clearly labelled release
candidate only after its own checks pass. Do not call an incomplete candidate
stable merely to meet the date.
