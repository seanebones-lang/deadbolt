# Private persisted state

**Published RC2 behavior:** new Unix SQLite and JSONL state files are created
with mode 0600, before SQLite writes or creates WAL/SHM/journal files. New parent
directories use 0700. Published rc.1 and older engines remain unchanged.

The same boundary protects configured core DB/events paths, `open_at`, and the
optional or directly opened Witness sink. Independently located files are each
checked. SQLite opens the prepared database without CREATE or URI handling.
`:memory:` and `file:` special targets are unsupported; use ordinary persisted
filenames. Witness retains its sibling-extension convention: `nodes.db` writes
`nodes.jsonl`. A Witness filename already ending `.jsonl` collides and is refused.
Core DB/events and SQLite sidecar outputs must also be distinct.

Existing Unix files, including present `-wal`, `-shm` and `-journal` files, must
be regular, owned by the effective user, single-link and mode 0600. Leaf symlinks,
FIFOs, directories, foreign ownership, hardlinked aliases and broad permissions
are refused before SQLite recovery or migration. No existing file is silently
chmodded or replaced. JSONL is validated through its opened descriptor with
no-follow/nonblocking flags. Existing SQLite metadata checks avoid an extra
open/close descriptor that could release another connection's POSIX locks.

Linux POSIX extended/default ACLs and macOS extended ACLs on the final parent
or state files are refused, including inherited read grants that mode bits alone
would miss. This is conservative: even a harmless extended ACL needs an explicit
operator migration to a simple dedicated state directory. Other Unix ACL models
and all Windows ACLs remain operator-managed. Review ancestor ACLs as well;
directory owners and replacement resistance remain prerequisites.

The resolved final parent must belong to the effective user and not be writable
by group/others. An owned 0755 parent is supported because its files are private.
Writable non-sticky ancestors are refused. Sticky system temporary directories
may contain an owned private state directory. Operator-controlled parent aliases
are resolved before leaf checks, supporting macOS `/var` aliases and deliberate
directory symlinks. Keep all directory owners trusted: these checks do not
provide race-safe confinement against an account able to replace parents or
change state after startup. Same-user state access remains operator authority.

Windows retains operator-managed filesystem ACLs. DeadBolt does not set or verify
Windows confidentiality through Unix mode bits. Export files, action review files,
operator environment/token sources and credential-output parents remain separate
operator responsibilities; this boundary does not claim to protect every file
an integration uses.

## Upgrade older Unix state

1. Stop every process using the store, including sidecars, MCP wrappers and
   embedded connections. Do not discard a WAL or reset a lease to fix startup.
2. Back up the quiesced store and evidence privately. Inspect the exact configured
   paths, parent owners/modes and any present SQLite sidecars. Refuse unexpected
   symlinks, hardlinks, special files or owners; investigate rather than chmodding
   an unreviewed target.
3. As the intended service user, set the verified regular DB/events/Witness files
   and present sidecars to 0600, and dedicated state parents to 0700. Apply this
   only to the reviewed paths, without recursive chmod of unrelated files.
4. Start the matching new engine with the same paths. Verify retained identities,
   kills, policies, grants and evidence. The permission prerequisite does not
   reissue runs, rearm consumed grants or change a deadline.

Default fail-closed startup problems surface as `store_unavailable`; optional
Witness failures retain the existing unavailable-sink behavior. Deliberate
`enabled=false` or legacy `fail_closed=false` overrides remain their documented
contracts, rather than becoming a filesystem recovery mechanism. Scoped/exact
and MCP admission retain their stricter fail-closed behavior.

## Verification

### Unreleased source after RC2: damaged stores are not new stores

The source engine initializes an empty SQLite database only when its configured
JSONL file is also empty. It accepts complete known unversioned schemas from
stable 1.0.3 and the 1.1/2.0 previews for migration. Initialization, migrations
and the SQLite schema-generation marker are committed in one writer transaction.
After that marker is present, every required table must exist; partial or newer
unknown schemas deny `store_unavailable` rather than recreating missing state.
An enabled sidecar exits unsuccessfully before binding a listener when the initial
store check fails; a later live-store outage still denies admissions.
Published RC2 artifacts do not yet implement this check.

If a lease row is missing but SQLite evidence retains its issuance or kill,
`issue`, `ensure` and child registration refuse to create it again. A denied
attempt for an identity that was never issued does not reserve that run ID;
normal operator setup after a `no_lease` result remains possible.
Existing rows retain normal idempotent `ensure` and terminal-kill behavior.
Do not delete state to make startup succeed. Stop protected dispatch, preserve
the database, WAL and evidence, and restore a consistent reviewed backup or
provision a separate fresh state directory with fresh run IDs. An empty database
paired with old JSONL is not treated as an empty new installation.

These checks detect incomplete schemas and retained history, not valid same-user
edits or deletion of every copy of history. An older unversioned store can also
be made to resemble a supported older schema by someone with operator write
access. Operator state remains outside the containment boundary; protect it with
separate OS identities and permissions. Older engines ignore the new schema
marker: a rollback does not preserve the damaged-state refusal guarantee.

The Unix private-state regression suite covers fresh core/Witness files, live
WAL/SHM modes, unsafe existing copies, symlinks, FIFO nonblocking refusal,
hardlinks, parent aliases, collision refusal and reopen preserving a kill.
`tests/private_state_acceptance.py` checks the actual executable under subprocess
umasks 000/022, including default, custom, relative and expanded paths. Existing
multiconnection and upgrade tests separately cover single-use grants, evidence,
credentials and state continuity. These are local/source/native checks, separate
from GUI-host model invocation and independent-human acceptance.
