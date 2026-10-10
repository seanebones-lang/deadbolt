# Next integration decisions

Reviewed 2026-10-10 against the current source candidate. This is an engineering
plan and evidence record, not a certification or a promise of complete containment.

## Product direction

Keep DeadBolt as a small local admission gate owned by the trusted executor.
Use framework-native adapters to place the check around actual effects. Keep
model reasoning, prompt-injection classifiers, OS sandboxing and identity
providers as separate controls. The gate can supplement those controls without
becoming another agent framework or asking a model to grant its own permissions.

The original direction is useful, but “a universal agent security layer” would
overstate this implementation. Unwrapped routes can execute without admission;
the trusted host must inventory them. Sharing a process and credentials with
untrusted generated code does not create a security boundary.

## What the upstream research changed

| Source | Observed mechanism | DeadBolt decision |
| --- | --- | --- |
| [OpenAI SDK guardrails](https://openai.github.io/openai-agents-python/guardrails/) | Agent-input checks can run in parallel with actions; tool checks have a different boundary | Put deterministic admission at the actual protected body, after SDK validation and review |
| [Python SDK v0.23.1](https://github.com/openai/openai-agents-python/tree/c4a1d047b22488234b2ba81056e1b9b8db2127a5) | Decorated function invokers prepare arguments for callable approval; replacing that invoker can lose this behavior | Build tools through the SDK decorator instead of monkey-patching an existing tool's callback; test default-argument approval with the real runner |
| [OpenAI human review](https://openai.github.io/openai-agents-python/human_in_the_loop/) | Approved runs can resume later; approval state belongs to trusted server storage | Recheck lease/policy after resume. Keep exact-action review application-owned; do not translate an SDK approval into a broad DeadBolt grant |
| [OpenAI Codex sandbox orchestration](https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/sandboxing.rs) | Approval requirements and sandbox execution are distinct controls | Describe admission and isolation separately; avoid promising protection for arbitrary code |
| [MCP annotations](https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/) | Tool annotations are hints and untrusted servers can misdescribe behavior | Do not automatically grant permissions based on discovered tool metadata |
| [OpenAI Guardrails](https://github.com/openai/openai-guardrails-python) | Composable input/output validation around provider use | Optional semantic checks can complement admission; do not add a model/provider dependency to the deterministic gate |
| [OpenAI Evals](https://github.com/openai/evals) | Framework for evaluating model/system behavior | Keep effect-based acceptance for deterministic enforcement; use model evaluations separately if semantic classifiers are later added |
| [TypeScript Agents SDK](https://github.com/openai/openai-agents-js) | Separate runtime/tool/approval implementation | A Python acceptance result does not establish TypeScript compatibility; verify a separate adapter before claiming support |

The verified Python SDK release commit is
`c4a1d047b22488234b2ba81056e1b9b8db2127a5`. Documentation and main branches can
change; adapter acceptance pins the released package `openai-agents==0.23.1`.

## Implementation sequence

1. **This candidate: installable Python client and optional SDK decorator.**
   Acceptance must run the installed wheel outside the source directory, use the
   real SDK runner/sidecar, verify actual file effects, preserve approval checks,
   and exercise revocation after resume, unavailable sidecar and competing
   one-shot calls. The base wheel must import without the SDK dependency.
2. **Separate operator authority from workload admission.** Design credentials
   bound to a fixed agent/run, with only admission/probe rights, independently
   revocable credentials and no policy/ensure/approve/control privileges. Preserve
   legacy operator routes explicitly, migrate state deliberately, and test role
   enforcement at every route. This is not implemented by the Python wrapper.
3. **Bind sensitive grants to exact actions.** Define a versioned canonical
   representation of trusted tool identity, validated arguments, destination,
   nonce, expiry and run ID. Store the reviewed fingerprint and consume it
   atomically. A changed body, recipient, destination, amount, stale grant or
   replay must fail. Do not hash arbitrary JSON and call that authorization;
   authenticating the reviewer and owning execution arguments are necessary.
4. **Read-only setup inspection.** Show effective policy, lease, credential role
   and configured coverage without renewing a lease or consuming approvals.
   Keep “ready” distinct from evidence that every application route is gated.
5. **Independent integration pilot.** Use one adopter-owned dispatcher to measure
   installation and wiring friction, route completeness, local overhead under
   contention, recovery and confusing denial handling. Add TypeScript or other
   framework adapters against demonstrated demand and tested SDK contracts.

The two protocol changes need separate compatibility and state-migration
reviews. Building several features at once would make their authority and
approval semantics harder to verify. Free Apache-2.0 distribution stays intact;
signing, registry publication, customer adoption and production acceptance are
separate release gates.
