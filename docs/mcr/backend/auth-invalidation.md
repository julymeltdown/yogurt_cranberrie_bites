# C05 Auth invalidation producer

Status: **IN_PROGRESS**. This increment implements the owner-local journal and
replay adapter; authenticated delivery and a World consumer remain unavailable.

## Scope and provenance

Assigned checkout `/Users/lhs/Desktop/mcr-worktrees/be-auth-invalidation`, branch
`codex/mcr-be-auth-invalidation`, clean base
`df66607c3afec927d44f3ffd9fb570d6ef6d5b7a`. The source specification,
`docs/mcr/project-assessment.md`, `docs/mcr/execution-plan.md`, and C05
`docs/mcr/realtime-contract.md` §§connection/authorization and stream filtering
are the requirements. C05 requires authority rereads within five seconds and
sooner on known safety changes. Hints never grant authority, restore a revoked
connection, extend access expiry or create a cross-owner transaction.

The retained port-rust revision is
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`; its history/import manifest remain.
Existing SQL mutation implementations are reused in place, not copied or forked:

- `adapters/out/cache/session_store.rs`: actual `PgRefreshTokenStore` logout,
  family/user revocation, rotation and session storage.
- `adapters/out/persistence/user_repository.rs` and `auth_transactions.rs`:
  credential updates and atomic reset with token consumption/audit.
- `adapters/out/persistence/user_deletion_saga_repository.rs`: direct canonical
  user disable and refresh revocation before deletion acceptance.
- Immutable Auth migrations 0006, 0013 and 0014: credential/role-triggered
  revocation, identity binding epoch and relationship revision respectively.

New code is confined to Auth migration 0016, `src/concert_invalidation/`, its
single root export, exact new test/support and runner files, and this document.
All published migrations 0001–0015, manifests/lockfile, `docs/source/`, other
services, shared contracts/proto, startup/router and frontend remain unchanged.
Dependencies are retained SQLx, Tokio, UUID, Serde and thiserror; no install.

## Capture and transaction order

The journal contains only cursor/epoch/signal identity, canonical user UUID,
optional family UUID, and the closed kinds `SESSION_CHANGED`,
`USER_AUTHORITY_CHANGED`, `CREDENTIALS_CHANGED`, `USER_REMOVED`. It contains no
JWT, refresh JTI/hash, password, email, roles, private content or arbitrary text.
Records have no user foreign key and survive hard deletion. They are retained;
pruning and consumer acknowledgments are outside this increment.

Migration 0016 captures refresh INSERT/UPDATE/DELETE, including rotation,
revocation, expiry edits and removals. A moved user/family binding emits both old
and new bindings. User status/deleted-at/identity changes, credential changes and
hard deletion emit user-level hints. Existing 0006 credential/status/role
triggers still revoke refresh rows; those actual row effects are captured too.
BEFORE TRUNCATE capture enumerates safe bindings while the existing exclusive
table lock is held, preserving the statement rather than silently losing hints.
Repeated or conservative duplicate hints are safe; each persisted signal ID is
stable on replay. There is no historical hint backfill at migration: a new
consumer must reconcile current authority before choosing its initial checkpoint.

The existing user→token locking prefix is preserved. The new user capture
trigger sorts after `auth_revoke_changed_credentials`. Before allocating a hint,
the append function locks existing 0014's relationship singleton, then updates
the new journal head. This avoids a new head→relationship lock inversion when
earlier statements already hold the old revision. No journal code subsequently
locks user/token rows. Owning transactions still require their original lock
discipline; this is not a new global remote-authority fence.

The head uses a transactional row update and holds its lock until the owning
transaction commits or rolls back. Concurrent writers cannot commit a larger
cursor ahead of a smaller uncommitted cursor. Multiple changes in one
transaction become visible together. Cursor allocation rolls back with the
mutation and journal insert, unlike a sequence plus MAX. Exhaustion fails closed
at 9,007,199,254,740,991 without wrapping. Epoch and committed records are guarded
against replacement, update, deletion and truncation. Missing head/revision or
failed journal insertion aborts the owning mutation rather than dropping a hint.
Database-owner/superuser trigger bypass is outside the trust boundary.

## Owner-local replay

`PgInvalidationJournal` has a bounded positive read deadline at most five seconds;
its checkpoint and replay use one read-only repeatable-read snapshot with a
statement timeout. Replay requires the exact owner epoch, an integer cursor from
zero through the committed high-water, and batch size 1–256. Invalid/beyond-head
cursors, changed epoch, missing/corrupt storage, bad kinds/bindings, gaps or a
missing tail fail closed using fixed redacted errors.

`next` is the last returned cursor, while `high_water` is the snapshot's committed
head. A partial page never advances `next` to `high_water`. Empty/caught-up reads
preserve the committed cursor and report no more work. Repeating a read returns
the same signal identities. User hints match that user only; family hints match
that exact `(user,family)`. `affects` is a binding filter, not authorization.
Downstream code must invalidate cached observations and use the existing typed
current-authority reread before further protected effects. Epoch mismatch needs
explicit reconciliation, not automatic cursor reset or authority restoration.

No HTTP/gRPC endpoint, service identity permission, startup task or World
subscription is added. Journal availability does not establish transport latency,
delivery acknowledgments, five-second consumer behavior or live revocation.

## Qualification and migration recovery

The test runner only accepts an absolute copied executable outside build targets
matching its explicit SHA256. It starts a fresh marker/inode-owned PG16 cluster
on a private Unix socket with TCP disabled. Tests verify its system identifier,
version and exact private database before mutation. It never uses application
defaults, real user data, Docker, provider credentials or Cargo during native
execution. It validates all SQLx checksums, actually restarts the postmaster,
then stops/removes only its owned cluster.

Tests use real `PgRefreshTokenStore`, credential repository and reset transaction,
and canonical `DeleteUserUseCase`. They cover journal failure rolling back real
logout/password/reset/deletion, direct owner suspension SQL, concurrent writers
with an observed lock wait, commit ordering, rollback, failed and successful
rotation, user/family binding, pagination past 256, stable duplicate replay,
malformed epoch/cursor/limits, immutability, physical deletion and restart.
Auth's current domain has no typed suspension variant/mutator; its owning
`users.status` update is tested without inventing a new public API.

Forward migration qualification starts with the actual 0015 ledger, injects a
late DDL collision to prove 0016 rollback, removes the fixture obstacle and
reapplies the unchanged migration. SQLx reapply is a no-op; a changed receipt
checksum is refused. Never edit published migration sources or downgrade to a
binary unaware of the journal. Missing/failed storage is repaired through forward
recovery; do not reset the owner epoch/cursor or delete retained records.

Validation commands, exits, exact artifacts, source commit and remaining limits
will be recorded after the assigned MEDIA build/native gate. Application/native
checks are **NOT_RUN** at this source checkpoint; Python runner safety (2 tests),
Rustfmt and assigned-path checks pass. No network-ready/live World acceptance.
