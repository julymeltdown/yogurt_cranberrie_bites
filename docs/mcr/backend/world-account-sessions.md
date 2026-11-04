# BE42-WORLD-SESSIONS-01

Status: CONTRACT_READY. Developer-owned component qualification passed on
`a421c0a4e34f8cbee1866fe3a800d6cc8f5feb4a`: Cargo, one lightweight test and
five actual native component cases, including the durable native journal.
This is a scoped committed handoff to Root for publication. Production
Auth-to-World composition and IN01 acceptance remain separate requirements.
Formatting, `git diff --check` and planning provenance checks pass.

The Sessions leaf consumes the common World account-deletion command, verified
Auth source, user advisory lock and permanent fence frozen by the Ticket owner
at `9bbd33e98e4ab6b9240181b2d5311af2121722e9`. It must not create another account identity or deletion authority.
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
withdrawn. One owner discovers at most 3264 generations from its three existing
800-entry bridge maps and 864-entry socket registry, persisted with bounded bulk
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

Fresh developer checks passed for active/pending multi-event removal, unrelated
stream preservation, exact retries and retained precision, permanent reconnect
denial, foreign-owner/unavailable-leg failure, original deadline lock contention,
128+1 resumable SQL withdrawal, durable native-batch resume and actual PostgreSQL
restart. Scope and commands are recorded below; these component tests do not
certify the production Auth transport or the full product deletion flow.

## Earlier qualification cohort and correction

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
gap. The corrected cohort below passed the durable journal, 128+1 native
obligations, authorization loss after durable discovery, dispatcher reconstruction
and actual post-ACK native publication/presence. The 128 additional old-generation
obligations are explicitly metadata fixtures, not a claim of 128 live sockets.
The separate actual active/pending multi-event socket case also passed.

Root explicitly authorized exact read-only dependency imports from Ticket's
`9bbd33e9` common module/0052 and `869a10b0` World module, plus the separately
committed one-line `account_sessions` export `65166553`. The dependency import
is `3fb60670`; it is not a transfer of editing ownership. Ticket's owner later
reported fresh qualification of the unchanged 869 World guards. Session cleanup
qualification and the production Auth bridge remain separate responsibilities.

## Corrected qualification and committed handoff

The exact combined source is `a421c0a4e34f8cbee1866fe3a800d6cc8f5feb4a`,
from base `89f26e6edf916cfd5bcdae481c24091597027448`. Its complete 2385-file
backend/contracts read set is pinned in
`/tmp/mcr-world-sessions-journal-be42/source.json`; runtime source, all 60 Auth
and Concert migrations, installed tools, controller hashes, Cargo JSON output,
executable and native logs are retained in that private evidence directory.
The documentation-only handoff commit does not change those validated files.

The allocator's fresh `BE42-SESSIONS-JOURNAL-MEDIA-202611041510Z` round was
accepted at 2026-11-04 15:11:00.196807 UTC. The single selected build command was:

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  -p concert-service --test world_account_sessions --no-run \
  --message-format=json-render-diagnostics
```

It used the installed Rust 1.88 toolchain, existing retained target,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, the retained contracts optimization profile and
both Rust flag environment variables absent. No packages were installed and no
cache was deleted. The independent guardian started before each child.

Both compiles exited 0. A preparation script initially named the nonexistent
`gateway-service/build.rs` after refreshing the Concert/Notification mtimes;
its initial full metadata receipt was not retained. This limitation and the
successful first compile are preserved in `mtime-preparation-incident.json`
and `attempt1/`. Before the second successful compile, the three exact authorized
Concert/Notification/`api-gateway` build scripts received a complete before/after
SHA256, size, mode, uid/gid and time receipt proving unchanged contents.
The final compile took 30.089 seconds including observation/copy, with no source
fix needed. Its sole copied executable is 29,089,568 bytes, SHA256
`63339b72a78359137c4ba521c562f4c79c8a95e0b7670bb78c5eb8978ab8863b`.

The shared target was unconditionally released at 15:12:57.761123 UTC after
zero handles and compiler PIDs were observed. No later target access occurred.
The copied lightweight command was
`world_account_sessions --skip native_ --test-threads=1 --nocapture` (1 passed,
0 failed, exit 0). Each native case ran separately with
`world_account_sessions --ignored --exact NAME --test-threads=1 --nocapture`:

| Exact case | Result |
| --- | --- |
| `native_sessions_actual_auth_active_pending_multievent_and_exact_retry` | 1 passed, exit 0 |
| `native_sessions_foreign_owner_unavailable_and_original_deadline_remain_retryable` | 1 passed, exit 0 |
| `native_sessions_bounded_persistence_and_lock_deadline` | 1 passed, exit 0 |
| `native_sessions_durable_native_batch_survives_dispatcher_reconstruction` | 1 passed, exit 0 |
| `native_sessions_postmaster_restart_exact_receipts` | 1 passed, exit 0 |

The native controller created only one marker/inode-owned PostgreSQL 16.14
cluster, Unix socket only, with the two explicit `mcr_world_deletion_auth` and
`mcr_world_deletion_concert` databases. It applied the exact source-pinned
migration ledgers, ran the first four cases, actually stopped/restarted the
postmaster, then ran exact receipt replay. Native work took 18.182 seconds.
The new journal case leaves a real native victim durable after source loss,
reconstructs the dispatcher twice, confirms exactly 128 of 129 obligations on
one attempt, withholds ACK until the final obligation, and rejects journal
mutation/deletion. The actual multi-event case arms private policy baselines,
closes both victim sockets, rejects delayed install, and drives survivor input
through the real socket/native loop after ACK, requiring its same-generation
input acknowledgment and checking published presence.

FULL was measured at 15:13:45.488669 UTC: all recorded owned workload PIDs and
the private PostgreSQL root were absent, 165.292 seconds after ACCEPT and
47.728 seconds after target release. Peak charged whole usage was
4,588,978,176 bytes, peak new usage 109,682,688 bytes, minimum free space
23,024,799,744 bytes and maximum native process count 12. The original
8 GiB whole / 1 GiB new / 96 MiB executable limits, 40/48 process ceilings,
600-second freeze / 720-second work / 780-second FULL limits, native 120 seconds
and release-to-FULL 180 seconds were respected. Older per-worker target counters
remain UNKNOWN and centrally charged, not zero or retrospectively proved.
No resources remain held. `full.json` records the exact release/accounting proof.

The implementation patch chain, in order, is `89e3c073`, `75f516bd`,
`625c5ff8`, `ae009267`, `c745f935`, `f1ba3b8b`, `16b062bd`, `a421c0a4`,
then this documentation handoff. The delegated one-line export is independently
committed as `65166553`. Import-only `3fb60670` is validation composition, not
an implementation handoff: Root must preserve the Ticket owner's delivered
common module/0052/World-module changes rather than blindly replaying that
three-file import. Root is the publishing developer; no king/SL review or
acknowledgment is required. The implementation owner verifies actual remote
master ancestry and owned postimages after normal publication.
