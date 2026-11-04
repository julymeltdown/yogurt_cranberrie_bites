# BE42-WORLD-SESSIONS-01

Status: IN_PROGRESS. The first source checkpoint is authored; Cargo and native
validation are NOT_RUN. Formatting and `git diff --check` pass with Rust 1.88.

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
