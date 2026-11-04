# BE42-AUTH-ORDERING-01 — canonical Auth deletion ordering

Status: **IN_PROGRESS**; authored native case, compilation and native execution
**NOT_RUN**. This leaf does not accept BE42 or the original canonical-authority goal.

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

## Intended executable behavior

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

Current source checks: Rust 1.88 formatting with `skip_children=true`, assigned
path/whitespace inspection, and `python3 scripts/mcr/check-plan.py` pass. The
planning check certifies planning provenance only. Cargo/native results require
a fresh allocator lease and will be recorded after execution.

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
