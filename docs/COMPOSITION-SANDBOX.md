# Composition recipe: DeadBolt inside a sandbox boundary

DeadBolt is not an operating-system sandbox, and this recipe does not
pretend otherwise. Isolation — a process boundary the workload cannot
cross — comes from the container (or VM, or OS sandbox) you run the work
in. What DeadBolt adds inside that boundary is the part a sandbox cannot
do: per-action admission against operator policy, a scoped credential the
workload holds instead of the operator's authority, and a kill the
operator exercises from *outside* the boundary while the work is running
inside it.

## The shape

```
operator side                          sandbox (container)
┌────────────────────────┐             ┌──────────────────────────────┐
│ deadbolt CLI           │  shared     │ deadbolt serve (sidecar)     │
│  ensure / policy /     │  state      │ durable workload ──admit──▶  │
│  credential issue      ├────────────▶│        │ effects             │
│  kill / status         │  volume     │        ▼                     │
└────────────────────────┘             │  workspace volume            │
                                       └──────────────────────────────┘
```

- The workload and the sidecar run **inside** the container, as an
  unprivileged user, on a read-only root filesystem with all capabilities
  dropped. Only two volumes are mounted: the gate state and the work.
- The operator never enters the container. Provisioning (lease, policy,
  the dispatcher's scoped admission credential) happens before the run;
  status and kill happen during it — all through the shared state store.
- The workload holds only its scoped admission credential
  ([credentials](CREDENTIALS.md)). It cannot ensure, set policy, approve,
  or kill — the sidecar refuses those routes for workload credentials.

## Image

One image carries the engine and the workload runtime. The runtime stage
mirrors the Harness repository's own container recipe (same base, libs,
non-root uid, state-in-HOME layout); build the agent binary for the
runtime's libc the way that recipe does — in a builder stage — rather
than copying a binary built against a different libc.

```dockerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      libssl3 ca-certificates git python3 procps \
    && rm -rf /var/lib/apt/lists/*
COPY harness  /usr/local/bin/harness    # the gated agent (optional)
COPY deadbolt /usr/local/bin/deadbolt   # the v2.0.2 engine
RUN chmod +x /usr/local/bin/harness /usr/local/bin/deadbolt \
    && useradd --create-home --uid 10001 agent \
    && mkdir -p /workspace && chown agent:agent /workspace
USER agent
WORKDIR /workspace
ENV HOME=/home/agent
```

Run it hardened, with the gate state and the work as the only mounts:

```sh
podman run --read-only --cap-drop=ALL --security-opt=no-new-privileges \
  -v gate-state:/state -v work:/work -v ws:/workspace sandbox-image
```

## Operator runbook

Provision (once, before the workload starts):

```sh
deadbolt serve --bind /state/deadbolt.sock &   # inside, on the state volume
python3 -c "from deadbolt_client import ensure; ensure('run-001')"
deadbolt policy --agent run-001 --tools ledger_write
deadbolt credential issue --agent run-001 --id run-001-cred \
  --ttl-secs 3600 --out /state/run-001.cred
```

The workload (here `examples/durable_worker.py`) runs with only the
admission credential in its environment:

```sh
DEADBOLT_ADMISSION_TOKEN=$(cat /state/run-001.cred) \
  python3 durable_worker.py --agent run-001 --workdir /work
```

Revoke, from any second container (or the host) sharing the state volume
— no shell inside the workload's container, no signal to its processes:

```sh
deadbolt kill --agent run-001
```

The workload's next admission is denied `killed` and it stops; work
already admitted and completed stays completed in its durable ledger.

## Verified 2026-11-14

Run in this sandbox environment (podman, host networking — this
environment cannot create container network namespaces, so the run used
the host network with the services bound to loopback; the process,
filesystem and capability boundaries below are the container's own):

- Boundary: inside the container the workload ran as uid 10001, saw 4
  processes total, could not read host paths outside the mounts (host
  files simply do not exist there), and could not write its root
  filesystem (`Read-only file system`); capabilities dropped, no new
  privileges.
- Gated work: the worker, holding only its scoped admission credential,
  completed three queued tasks through the in-container sidecar.
- External kill: `deadbolt kill` from a **separate** container sharing
  only the state volume revoked the run; the workload's next admission
  was denied `killed` and it stopped with exactly the three completed
  effects in its ledger — no partial fourth.
- The Harness agent also runs in this composition: a scripted read
  executed inside the container and its gate lease landed in the shared
  state volume, where an operator-side `deadbolt status` reads it.

## The boundary, stated plainly

- The container is the isolation; DeadBolt is the control plane inside
  it. Either half alone is a different, weaker claim.
- Whoever can write the state volume holds operator power over the gate
  (policy, kill). Mount it only where the operator acts.
- A network namespace is part of a real deployment's isolation; this
  verification environment could not provide one, so network isolation
  is asserted by the recipe's run flags in production, not by the run
  recorded here.
