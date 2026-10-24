# October 2026 evaluation evidence

These observations cover different interfaces and should be assessed separately.
They do not change the released DeadBolt 2.0.0 engine, bundle or registry entry.
For a first functional evaluation in your own disposable environment, follow
[the evaluation guide](FIRST-EVALUATION.md).

## ProtocolProbe: setup inventory, October 23

[ProtocolProbe's published DeadBolt 2.0.0 inspection](https://protocolprobe.com/mcp/servers/io.github.seanebones-lang/deadbolt)
reports starting the published MCP package in a read-only session and listing
two tools: `deadbolt_get_integration_guide` and `deadbolt_check_bundle`. It reports
five of five applicable public-record checks met and explicitly records **no tool
calls**. These are the blank-configuration setup tools; no downstream tools are
protected in that mode.

The inspection establishes the reported startup and inventory observations. It
does not establish configured policy, approval, kill, denial or revocation
behavior. Because the checksum tool was not called, it also does not establish
bundle checksum verification. The report is not an enforcement test, penetration
test, independent signature verification or security certification.

ProtocolProbe's automated description review suggested making limits, defaults
and errors clearer. That assessment is a documentation signal, not a confirmed
vulnerability. The [bundle guide](../dist/mcpb/README.md#setup-inputs-and-failures)
now documents the existing setup inputs and failure results explicitly. Tool
descriptions in the published artifact remain unchanged.

Source attribution: ProtocolProbe, DeadBolt server record, snapshot/test date
October 23, 2026, linked above. These observations paraphrase its published facts;
no endorsement or certification is implied.

## NextEleven: Swarm native integration, October 24

The separate Counter-Agent Swarm project exercised real, unmodified Mothership
0.3.0 through stdio MCP against the released DeadBolt 2.0.0 Linux executable.
This was **NextEleven-controlled automation**, with synthetic requests and a
deterministic analyst fixture. The lab ran Linux amd64 userspace with Rosetta in
an aarch64 Linux VM on Apple Silicon. It was not a native x86-64 performance
benchmark or an Ubuntu/WSL server test.

| Scenario | Recorded result |
| --- | --- |
| Kill | Approved native action blocked the next benign request with 503 and no extra upstream request. |
| Deny | Clipping the fixed `web.forward` tool blocked the next benign request. |
| Revoke | Revoking the fixed admission credential blocked the next benign request. |
| Reject approval | Rejection caused no native control effect; benign forwarding remained available. |
| Restart pending | A pending approval survived a Mothership restart and subsequently executed through the native path. |
| Unavailable prerequisites | Missing credentials, a damaged disposable lease table and a stopped gate each denied forwarding. |

Approved control cases also checked same-key replay without another native
control event. The record includes a separate HTTPS kill test with the operator
credential kept on loopback, tester/operator separation and disposable cleanup.
The Linux installed regression suite passed 101 tests with one optional native
pytest skipped; the six native scenarios ran separately. Those counts overlap
and must not be added together as independent security checks.

Read the Swarm [status](https://github.com/seanebones-lang/counter-agent-swarm/blob/main/docs/STATUS.md),
[scoped validation record](https://github.com/seanebones-lang/counter-agent-swarm/blob/main/docs/linux-lab-validation.json)
and [lab instructions](https://github.com/seanebones-lang/counter-agent-swarm/blob/main/docs/LINUX_NATIVE_LAB.md)
for exact versions, hashes, scenario results and limitations. Mothership remains
private; its source, wheel and runtime image are not part of the public tester
kit. As of this checkpoint, an external tester had not run the native lab.

This validates the recorded cooperative integration path, not live AI defensive
judgment, hostile-payload sandbox isolation, production deployment, sustained
attack load or production recovery. Kill continues to block subsequent
admissions; it does not terminate an already running effect. Separate
Swarm component reports, including Pankaj Basnet's report, do not establish
native-engine or adversarial security validation.
