# BE42-AUTH-ORDERING-01 — canonical Auth deletion ordering

Status: **IN_PROGRESS**, developer qualification **PASS**; committed handoff
awaits the named publisher. This leaf does not accept BE42 or the original
canonical-authority goal.

## Assignment and provenance

Owner207 uses the existing isolated worktree and branch
`codex/mcr-be-BE42-auth-ordering`, based on published
`5f72fcf00fa7ba12599d6417d2f4744c5c49fbcb`. The only writable paths are
`backend/services/concert-service/tests/auth_deletion_owner_ordering.rs` and
this document. Root explicitly returned the initially assigned Auth test path
and transferred the Concert test path because Concert already has the required
Auth/Friend dev dependencies. No manifest, lockfile, SQL migration, main,
configuration, contract, production implementation or existing support changes.
Root remains the publishing developer. Developer validation and fixes are owned
here; no king/SL review, approval or acknowledgment stage applies.

The existing unfinished `BE-CANONICAL-OWNER-AUTHORITY` goal remains historically
`BLOCKED`; it has not been reset, completed, replaced or budgeted. This leaf is
aligned with its durable owner guards, actual producer settlement, response
lifetime and restart requirements. Other canonical acceptance work remains open.

The immutable source originals in `docs/source/` are preserved. The case reuses
the existing CF02 support read-only at the base above, with these SHA256 values:

| Source under `backend/services/concert-service/tests/` | SHA256 |
|---|---|
| `canonical_owner_restore_support/capture.rs` | `eb49fed2b3bc6601ff28cde28af01b3ddc4c62cd6e3a6819015c0b79b29e461f` |
| `canonical_owner_restore_support/environment.rs` | `1ef151b65ff647f73ee5b5ed5be5d6f46e9cdc900bd00db174bc7ed4fab130f5` |
| `canonical_owner_restore_support/fault.rs` | `6b6f48654e82427875ae5e2f433e038ba7fd824de94eddd18a0f8b9c9468293b` |
| `canonical_owner_restore_support/fixture.rs` | `705e66e4e477dee1d816e4cceb1994432b19359e3daeee80beb4a88db2bc3cbb` |
| `canonical_owner_restore_support/notification.rs` | `26e64fd01b92d52f1a36f8cfffb12ccc4db71e2608ad9a15a28bd32bed055953` |
| `canonical_owner_restore_support/owners.rs` | `5239e82f46860a355b33b9f4181c6e0feaae12af273235597eb4c961f71f777d` |
| `canonical_owner_restore_support/release.rs` | `9710b903ab15d50b9442c0b0ef11efbeb17aeff64c7d2dc2491ce2beb678b492` |

The delivery/body helper functions derive from `canonical_owner_composition.rs`
at the same base, SHA256
`10488ad03bfbe5466dfd7b93a73eebf27966d77cd5edb8195eb7e02a4760dce8`.
Retained dependencies are the existing Concert dev dependencies, platform
response-scope/mTLS guards, SQLx and Tokio; no new dependency is acquired.

## Executable behavior

The one ignored native test, `auth_deletion_owner_ordering`, has two explicit
phases, `MCR_BE42_ORDERING_PHASE=SOURCE` and `RESTARTED`. It requires the existing
owned CF01/CF02 five-database PG16/mTLS environment. The operator runs the same
frozen, hash-pinned executable twice against the same isolated cluster, with
an actual PostgreSQL stop/start between invocations and distinct test PIDs.
Reuse the existing canonical qualifier's `Cluster`, ownership checks and PKI;
do not attach a configured/default database or restore a prior archive.

The source phase creates two accounts through actual Auth signup, transactional
email verification and password login. It reads the generated verification
challenge from the owned database; it never SQL-inserts a user, verification,
password, session, claim, age or operator grant. Tokens remain in memory.

For the first account, deletion must be observably blocked by the actual
prepared source SHARE transaction, including a real PostgreSQL lock wait whose
blocking PID is zero. Inner serialization and an unread outer body retain the
guard. Actual outer-body EOF calls the guarded delivery service, releases the
owners and allows the canonical deletion transaction to commit. Repeated
delivery/deletion preserves the original result and saga. Auth0017 security
generation must advance, all refresh rows must be revoked, and password login
must refuse the deleted account. Eight saga targets remain pending rather than
claiming Concert cleanup succeeded.

For the second account, the actual Concert transaction is captured and tracked
twice through both real authenticated owner ports; its immutable registration
deduplicates. While that transaction is in progress, the real terminal service
must withhold success, leave `ROOT_FINISHED` durable and retain prepared owners.
An actual Notification child is created before the existing missing-target
refusal. The negative-only completion fault cannot manufacture a positive ACK.
The test cancels only its blocked deletion future, rolls back its own actual
SQLx transaction and records a private bounded identity/receipt handoff.

The restarted phase requires a changed postmaster start time and fresh test
PID, exact original Auth/Friend prepared GIDs, the same durable scope and an
empty new registry. The real authenticated settlement resolver observes the
original native full-XID abort and Track/seal proof; owner recovery releases
the guards, permitting canonical deletion. The original child remains pending
under a negative Complete response, then the real child worker settles it after
the fault is removed, even though the account is now disabled. No new actor
authority, caller-selected transaction outcome, forced prepared settlement or
fabricated response-delivery success is introduced. All five databases must
have zero prepared transactions before owned teardown.

## Qualification and limitations

The specification requirements are BE42 and §§7.2, 8.3–8.4. The frozen typed
deletion command/receipt ports and eight-target inventory remain unchanged.
Actual user/session/tombstone writes use the existing Auth repositories and
Auth0017; no alternative identity or saga store is introduced.

The unchanged Rust source at
`bca984881e423b54f4c76a63825e14f9dc422136` passed compilation and both native
phases. Only this results document changes after qualification. No source fix,
compiler retry, native retry or assertion relaxation was required.

| Command / evidence | Exit | Result |
|---|---:|---|
| `rustfmt +1.88.0 --edition 2021 --config skip_children=true --check backend/services/concert-service/tests/auth_deletion_owner_ordering.rs` | 0 | Assigned Rust formatting; read-only support untouched. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning provenance only; all 15 source originals additionally match the base byte for byte. |
| `git diff --check` and exact assigned-path audit | 0 | Only the two transferred paths change. |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --test auth_deletion_owner_ordering --no-run --message-format=json` | 0 | One frozen test executable, build wrapper 47.386 s. |
| Frozen executable, `--ignored --exact auth_deletion_owner_ordering --test-threads=1 --nocapture`, phase `SOURCE` | 0 | 1 passed, test body 6.81 s; actual prepared-blocker, unread-body/EOF and native in-progress assertions pass. |
| Same exact frozen executable and argv after owned PostgreSQL stop/start, phase `RESTARTED` | 0 | 1 passed, test body 3.77 s; original guards, native settlement, deletion and child recovery assertions pass. |

The frozen executable is 50,122,016 bytes, SHA256
`0caeb297d6d60300feed2c409b9a921bcb7a5d453cd91da17bfe96458a5af63f`.
The copied native wrapper took 26.084 s in total, including owned cluster/PKI
setup, both tests, physical restart, migration receipts and cleanup. The test
PIDs were 97389 and 98253. PostgreSQL postmaster changed from 96876 to 98153.
Both test invocations checked exact original prepared GIDs, and all five
database migration ledgers matched source SHA384 manifests before/after restart:
Auth 18, Friend 18, Concert 40, Profile 9 and Notification 6 entries. This includes
the actual unchanged Auth0017 tombstone/security-generation migration.

Actual assertions confirmed that inner serialization and an unread outer body
did not release deletion's source blocker; successful outer EOF did. The live
native participant withheld terminal success, and its immutable Track retry
deduplicated. Physical restart preserved the original owners and child. Actual
authenticated producer recovery observed native rollback; deletion then advanced
the security generation, revoked refresh rows and denied password login. Negative
child completion retained its durable pending intent; real worker recovery later
completed it after the account was disabled. The failed response remained
undelivered, and all eight deletion targets remained visible rather than falsely
reporting cleanup completion.

The finite allocation was
`BE42-AUTH-ORDERING-BACKEND-202611041348Z`, actually accepted at
13:49:47.408080Z on 2026-11-04. The compiler group exited and target had zero
handles at unconditional target release 13:51:32.561132Z, before any copied test.
Full release was 13:52:41.959055Z, 69.398 s after target release. All build, test,
native and postmaster PIDs/groups were absent; owned PG/TMP/PKI paths were absent.
All five databases had zero prepared transactions before owned teardown.
There was no target access after its release.

Fresh free space at preflight was 30,794,301,440 bytes. Whole-round high water
was 2,117,955,584 bytes within 3 GiB, including the entire current checkout, all
retained CF02/retirement/restore/BE33 carry, evidence, copied executable, temporary
files, PG and PKI. Positive target growth was 45,797,376 bytes within 1 GiB.
The retained Rust 1.88 profile used dev/test debug 0, incremental 0, both
`RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` unset, and contracts-build dev opt-level 2.
No install, cache deletion, new target or explicit main build/copy/run occurred.

Private resource/evidence receipts remain outside Git. Their portable SHA256s:

| Receipt | SHA256 |
|---|---|
| Source/tool/SQL/carry preflight | `128a2f31077869bfed27fbc2ba989f80b07c2da5d497a231fb70580a7e936bbf` |
| Frozen checkpoint | `af39a022b301c8291ee3f50b47eb3e81bb1ca6ab25c4d0aeec84064168ba16d9` |
| Build result | `7c7090fc79c8652d1bd4b7c028ecfca39550d3b7e4cecf6d017e8323c2e074f4` |
| Native result | `3afe4a4b13d4feeacf8a924be9457c60ed597f3688f825bf44c9a3d682c1b11b` |
| Whole-round high water | `1677ee661681d73c8b27922d9120ab136ade3e06985269a3868ce770c9ea8109` |
| Target release | `756fc57ec707cbfae05d3004e9ac4e1b2ab0e693101b09598ebbea19bf93b68e` |
| Full release | `d5b6e308c023ce1ec4deead8a7b73e89f29d7f30acf313f0bd7074aad3f0ff76` |

This is an in-process guarded-service/native-database qualification. Its
Gateway-signed parent and response body are explicitly test transport; public
Gateway authentication, real network HTTP delivery, workload-process death,
production mounting and deployment are not certified. The restart is a
PostgreSQL stop/start, not a backup/restore or SIGKILL drill. The reused capture
does not establish real Auth/Profile observation-time acceptance. No age claim
or privileged authorization is fabricated.

The Notification child is the real close-before-dispatch cleanup of an
unavailable target, not a successful target-policy/inbox business flow. The four
production Concert deletion adapters, reconnect/reveal denial, socket/presence/
private-cache cleanup, retained-field erasure, reviewed retention policy and
BE42/IN05 acceptance remain open. Auth revocation alone cannot truthfully
acknowledge the Concert Sessions port's World cleanup obligations.
