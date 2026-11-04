# BE42-WORLD-SESSIONS-01

Status: IN_PROGRESS. Candidate `c745f935` passed Cargo, one lightweight test and
four native component cases. The durable native-journal correction described
below is authored and NOT_RUN; that earlier cohort does not qualify this change.
Formatting and `git diff --check` pass with Rust 1.88.

The Sessions leaf consumes the common World account-deletion command, verified
Auth source, user advisory lock and permanent fence frozen by the Ticket owner
at `2c61aa79`. It must not create another account identity or deletion authority.
The retained Auth deletion contract is
`backend/crates/auth-application/src/concert_deletion/mod.rs` at base
`89f26e6edf916cfd5bcdae481c24091597027448`.

The implementation owns only `world/account_sessions.rs`,
`tests/world_account_sessions.rs`, migration `0053_world_account_sessions.sql`,
this document, and the explicitly transferred deleted-user hunks in
`world/admission_bridge.rs` and `adapters/in/world_ws.rs`. Root stopped writing
those hunks before transfer. Root or the Ticket owner retains shared exports.

Completion requires durable bounded membership withdrawal, actual active and
pending bridge-view removal, native-owner disconnect, socket and private-policy
registry withdrawal, permanent reconnect denial, exact immutable retry receipts,
meaningful developer-owned regression evidence, a committed handoff and confirmed
publication by Root. An unconfigured or empty runtime registry, unknown active
foreign owner, unavailable publication leg or unverified raw command cannot
produce an acknowledgement. The original dispatch deadline is never extended.

Production Auth-to-World composition is assigned to Root. A test-only authority
or Auth test transport does not qualify that production boundary. This leaf
does not grant IN01 activation or account-deletion product acceptance.

Checkout: `/Users/lhs/Desktop/mcr-worktrees/be-world-profile-producer`.
Branch: `codex/mcr-be-BE42-world-sessions`.
The completed Spatial branch and `/tmp/mcr-spatial-v10/` evidence are preserved.

## Implementation and provenance

`SessionDeletion::new` requires a configured authority and between one and 16
distinct actual serving owners. `SessionOwner::new` checks the serving state's
admission Arc identity and takes its real socket registry and native handle.
The dispatcher commits the common fence and exact Sessions command/source
binding before native effects. Each attempt disconnects at most 128 retained
exact native generations and removes at most 128 membership rows. The final
receipt is written only after durable all-event membership absence, bridge and
private-policy withdrawal, native maintenance and a final authority check.
Every await remains inside the original dispatch deadline. Completed receipts
are never updated on retry. No HTTP/RPC endpoint is mounted by this leaf.

Exact native generations now enter the private, immutable
`concert_world_session_deletion_victims` journal before any local view/socket is
withdrawn. One owner discovers at most 1664 generations from its existing
800-entry bridge and 864-entry socket registries, persisted with bounded bulk
SQL. The caller retains the admission gate across discovery and withdrawal;
the synchronous native publication fence never spans a database await. Each
attempt confirms at most 128 native effects durably. A new dispatcher loads
unfinished exact generations from SQL, including when ordinary bridge cleanup
has already removed its cached binding. Unknown journal owners remain incomplete.
The final receipt guard rejects any pending native obligation. A process-local
pending map is insufficient and is no longer used for this duty.

Active and pending bridge entries are removed under the existing publication
mutex; surviving visibility sets lose only the deleted user. The bounded local
removal cache is never evicted to reopen authority. Its 4096-user capacity fails
closed. The common SQL fence remains permanent across restarts. Migration 0053
guards all membership inserts/updates and retains the common user advisory lock.
The producer-side FIRST-lock changes in World `mod.rs` belong to Ticket/Root;
the bridge's own membership lease extension takes the user lock before its
existing event lock. No ordinary credential deadline is extended.

The integration test's Auth repository transport and signup/verify/login/delete
helpers are adapted from Ticket commit
`9bbd33e98e4ab6b9240181b2d5311af2121722e9`,
`backend/services/concert-service/tests/world_account_deletion.rs`; only the
target changes to Sessions. Headless World fixture setup is adapted from
`tests/world_runtime_support/postgres.rs` at base `89f26e6e`; it reuses the
retained `world_runtime_support` geometry helpers. Profile, recipient visibility
and artwork in these tests are explicitly synthetic. Auth repositories,
PostgreSQL, World admission, native runtime and loopback sockets are actual.
Dependencies are the existing service dependencies; no manifest/lock changes.

Pending developer checks cover active/pending multi-event removal, unrelated
stream preservation, exact retries and retained precision, permanent reconnect
denial, foreign-owner/unavailable-leg failure, original deadline lock contention,
128+1 resumable SQL withdrawal and an actual PostgreSQL restart. These authored
tests are not passing evidence until run on the combined source and fresh lease.

## First qualification cohort and remaining gate

Frozen `c745f935e0b78c0c43a6c0f484a7aa987a143f64` compiled with the existing
locked offline Rust 1.88 profile, two jobs, debug zero, incremental off and both
Rust flag environment variables unset. The first compile failed on a missing
test `AtomicBool` import; its source, diagnostics and controller report remain
in `/tmp/mcr-world-sessions-be42/attempt1/`. The owned test fix compiled with
exit 0. The copied 28,767,520-byte executable has SHA256
`5b18ae4a8dd06365b2368e955a38703c7ae325e88f90edfbced539f3cef97f47`.
The copied lightweight case and all four authored native cases exited 0.
Actual PostgreSQL 16.14, real Auth repository operations, loopback sockets,
armed native policy baselines and a postmaster restart were exercised. External
Profile/recipient/art/policy metadata remain deterministic fixtures.

That cohort's target was released with zero handles at 14:44:56.152 UTC;
all recorded workload PIDs and the private PostgreSQL root were absent at the
14:46:12.020 UTC FULL measurement. Native work took 11.044 seconds, within the
original offer. Independent guardians measured peak new growth 149,508,096 bytes,
peak charged whole usage 4,548,583,424 bytes and minimum free space
26,044,616,704 bytes. The allocator explicitly amended the whole ceiling to
8 GiB while retaining the 1 GiB new-growth and 96 MiB executable caps. Older
per-worker shared-target growth is UNKNOWN, centrally charged; this is not proof
of a complete historical counter. No old evidence was rewritten to hide that gap.

Developer inspection then identified the re-created-dispatcher native-batch
gap. The next cohort must test the durable journal, 128+1 native obligations,
authorization loss after durable discovery, dispatcher reconstruction and actual
post-ack native publication/presence. The 128 additional old-generation obligations
are explicitly metadata fixtures, not a claim of 128 live sockets. The separate
actual active/pending multi-event socket case remains required.

Root explicitly authorized exact read-only dependency imports from Ticket's
`9bbd33e9` common module/0052 and `869a10b0` World module, plus the separately
committed one-line `account_sessions` export `65166553`. The dependency import
is `3fb60670`; it is not a transfer of editing ownership. Ticket's owner later
reported fresh qualification of the unchanged 869 World guards. Session cleanup
qualification and the production Auth bridge remain separate responsibilities.
