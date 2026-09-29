# Incident

`deadbolt incident` writes JSON. The file has no generated prose. This page is the taxonomy. It is not copied into the file.

## What we log

Two classes. Do not collapse them.

Observed. A fact the gate recorded: `ensure`, `register_child`, `kill`, `pause`, `resume`, `admit_allow`, `admit_deny`, `policy`, `spend`, `approve`. The incident file keeps the observed admit rows under `decisions`. Each row has `class=observed`, `cid`, `ts`, `tool`, and `code`. `premises` is empty on an observed row. `killed_at` is the lease update time when the state is `killed`. It is not a person's name. The gate does not record who held the token.

Inferred. A conclusion from premises, not a second observation. `purpose_exceeded`, `lease_expired`, and `no_lease` are inferred. Each row has `class=inferred`, `kind`, `cid`, `tool`, `ts`, and `premises`. Premises are cids, including the attempt that was refused. An inferred row with empty premises is refused on export.

The file also has `agent`, `children`, `first_seen`, and `policy`. Policy is the snapshot: `tools_allow`, `dest_allow`, `spend_cap_usd`, `spend_usd`, `irreversible`. Unset lists are null. That means open, not "we did not look."

## What we do not log

Prompts are not evidence. Tool arguments are not stored. A generated summary is not a row, and a `generated` class is refused on export. A sentence about what the agent meant is not in the file. If a lawyer needs prose, they write it from the JSON. Deadbolt does not.

## Customer notice workflow

The operator sends the notice. Deadbolt does not.

1. `deadbolt incident --agent ID --out incident.json`
2. Read `killed_at`. The stop is the caller that held `DEADBOLT_TOKEN`, or the in-process gate. The file does not name a person. Do not invent one.
3. Attach `policy`. That is the blast radius that was set: tools, dest, spend cap, irreversible, and spend already recorded.
4. Read `children` and each child's own policy and decisions. Children do not inherit policy automatically; lineage alone does not establish identical blast radius.
5. Send the JSON. Do not replace it with a rewrite and call the rewrite the record.

If your customer agreement or incident policy requires a 24-hour notice, use that requirement and its actual trigger. Deadbolt does not establish a legal reporting deadline or deliver a notice.

## Limit

This is your agent. It is not a lab research swarm on the public internet. The file cannot show a caller that skipped a cooperative admit. It can show what `mcp-proxy` and a build-in gate refused, who was killed, which children were on the lease, and what policy was set.
