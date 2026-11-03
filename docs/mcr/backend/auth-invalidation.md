# C05 Auth invalidation producer

Owner-local producer: **CONTRACT_READY**, with implementation and validation
complete. Parent C05 remains **IN_PROGRESS**; authenticated delivery and a World
consumer are **NOT_RUN**. The clean three-commit author handoff is integrated;
all eight assigned files matched the author exactly before this delivery note.
The fresh Python harness checks pass (2 tests). Existing qualified native
evidence is reused without a duplicate general review or taking either active
worker's build target. See the [integration receipt](../evidence/auth-invalidation-integration-2026-11-04.json).

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
consumer must fence old publications, memberships and queued work first, capture
its owner checkpoint, perform current typed authorization reads, and apply
intervening signals before becoming ready. Reading authority before capturing
the checkpoint could skip a concurrent change. A restart resumes a durable
applied cursor or performs this full invalidating bootstrap.

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
The later shared contract at `9204e1e57524badddfc7d16431ad44dcd290e967`
was reviewed read-only; it was not imported into this worktree. Its authenticated
RPC adapter must choose an operation deadline at most one second, bound response
size, map the local kinds to its closed reasons/scopes and enforce serving-owner
identity. Those are separate integration obligations; the local adapter accepts
a configured positive deadline through five seconds and does not certify them.

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

## Validation evidence

Validated source is clean commit
`b501c120aa81305d128d0d459f49531075e08ed1`, following root-export commit
`a92e311a` from the assigned base. The final handoff adds only this evidence and
the bootstrap documentation/comment correction; executable Rust statements,
SQL and tests remain identical to that validated source. No runtime rebuild is
claimed for the comment-only follow-up.

Evidence root: `/private/tmp/mcr-auth-invalidation-snphm8k4`. These are retained
local artifacts, not a published CI run. `binaries.json` pins Cargo JSON-selected
executables, source and copied-artifact digests. No other build target was used.

| Command / evidence | Exit / result |
| --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p auth-service --test concert_invalidation --test unit --no-run --message-format=json` | 0; `build.jsonl`, `build.log`; both exact test executables copied outside the target |
| Copied `unit --test-threads=2` in the runner's minimal environment | 0; 124 passed; `unit-result.json`, `unit.stdout`, `unit.stderr` |
| `python3 backend/scripts/mcr/auth_invalidation_smoke.py --test-binary /private/tmp/mcr-auth-invalidation-snphm8k4/concert_invalidation --sha256 f30aea8bc2c0894151e6b6ed3ba648b83eef36f4b90d0b29ada6f950a5ac6159 --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report /private/tmp/mcr-auth-invalidation-snphm8k4/native-report.json` | 0; PASS; 2 pure tests and all 3 native tests passed |

The native report records PostgreSQL 16.14, all 16 exact migration receipts,
334 contiguous committed records, an observed concurrent writer lock wait,
postmaster restart from PID 16248 to 16370, stable replay after restart, and
successful owned-cluster cleanup. The private root
`/private/tmp/mcr-pg-avpqlhn2` was removed. The report includes SHA256 for all
runner/adapter/test inputs and SHA384 for all migration sources.

Copied unit executable SHA256:
`6d8b6446e071a82d52abbe3b5b2f5b729d294dcb948efc4dde151844f79a0075`.
Migration 0016 SHA384:
`1a1e6ac9bd0891d48f6163ccda8e09664f582a71a7942b2feae53f273eb9b8fa89dab6cead3b28148581ac357c9af782`.

Final gates all exited 0:

- `python3 -m unittest discover -s backend/scripts/mcr -p test_auth_invalidation_smoke.py -v`: 2 passed.
- `rustup run 1.88.0 rustfmt --check --edition 2021 backend/services/auth-service/src/concert_invalidation/mod.rs backend/services/auth-service/tests/concert_invalidation.rs backend/services/auth-service/tests/concert_invalidation_support/native.rs`.
- `python3 scripts/mcr/check-plan.py`: PASS, planning provenance only.
- `python3 scripts/mcr/check-boundaries.py --base df66607c --lane backend --allow-composition backend/services/auth-service/src/lib.rs --allow-doc docs/mcr/backend/auth-invalidation.md --allow-harness backend/scripts/mcr/auth_invalidation_smoke.py --allow-harness backend/scripts/mcr/test_auth_invalidation_smoke.py`: PASS, exactly 8 assigned paths.
- `git diff --check df66607c`: PASS. Manual import review finds only existing
  Auth/workspace dependencies; the root export is the only composition edit.

`final-audit.json` verifies the exact assignment, byte-identical published
migrations 0001–0015 and `docs/source/`, unchanged runtime source after the
validated commit, both frozen executable digests, absent private cluster, and
no remaining matching process or open MEDIA/frozen-artifact handle.
`final-checks.json` retains final gate outputs and exits. The standing exclusive
MEDIA reservation is unconditionally released with the handoff; no further
Cargo use is authorized without a new lease.

| Local artifact | SHA256 |
| --- | --- |
| `native-report.json` | `4e5538f1fda16cfc85f2f815f993a385a4c639171f12e8f7d7097bd0bccb2157` |
| `binaries.json` | `36bceab42d268a5fba4636c072e93dc6a933e730b35d173c74eaa4e477f3cc6d` |
| `build.jsonl` | `af9d07f3c6a11f67c36d8e8bd98140f3735620a294809b9980e52004b5aa1a97` |
| `unit-result.json` | `d7caf4a945269bc511c1266e23e9118b66fe1a6fc9b89b8e57e16ddb3e6124e9` |

Real authenticated RPC, World invalidation/removal, multiuser delivery latency,
load and release checks remain **NOT_RUN**. No network-ready or live World
acceptance is claimed. Review fixes within this assignment remain the owner’s
responsibility; new shared transport/consumer work needs explicit allocation.
