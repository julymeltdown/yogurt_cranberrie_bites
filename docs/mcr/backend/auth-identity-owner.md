# Auth-owned current admission read — BE04 prerequisite

Status: **IN_PROGRESS** pending integrator review. The application values have
deterministic validation and the production persistence adapter has real isolated
PostgreSQL evidence. BE04 overall remains **IN_PROGRESS**; authenticated
Gateway/Auth/Concert composition, real email-only Concert participation,
suspension/event policy and real provider login are **NOT_RUN** in this slice.
Production Concert admission remains unavailable.

## Ownership and provenance

Worker base: `5863f915eae1526786859876d22a781b48ca5f7c`, isolated worktree
`be-auth-authority`, branch `codex/mcr-be-auth-authority`. The new unbudgeted
persistent goal includes implementation, real database verification,
documentation, committed handoff and integrator review fixes. Only the integrator
publishes to master; final commit ancestry and candidate-bound evidence are in
the handoff receipt, avoiding a self-referential commit hash here.

The Auth application/adapter are newly authored against retained owner sources:

- `backend/crates/auth-domain/src/user.rs`: canonical UUID and account states.
- `backend/services/auth-service/migrations/0001_init_auth.sql`, `0002_oauth_identities.sql`,
  `0003_governance.sql`, `0006_atomic_auth_and_delivery.sql`,
  `0008_refresh_token_families.sql`: account lifecycle, identity soft deletion,
  refresh families and atomic credential/deletion revocation.
- `backend/services/auth-service/src/adapters/out/cache/session_store.rs`:
  `session_is_live` requires the same user and family, unrevoked and unexpired
  refresh token; a rotated predecessor does not end a live successor.
- `backend/services/auth-service/src/adapters/out/persistence/user_repository.rs`:
  live OAuth relink updates `linked_at` without replacing the identity ID.
  `find_by_provider_subject` lacks a deletion filter and is deliberately unused.
- `backend/services/auth-service/src/adapters/out/persistence/onboarding_transactions.rs`:
  existing OAuth subject validation requires at most 512 UTF8 bytes, a nonblank
  Rust whitespace trim and no C0/C1 control characters. The compatibility relink
  path does not enforce that guard, so this read conservatively checks it again.
- `backend/services/auth-service/src/adapters/out/persistence/auth_transactions.rs`:
  email verification marks the live EMAIL identity verified. OAuth email flags
  are separate from the existence of an actual OAuth subject binding.

Parent-owned prerequisite `e9433c0ec8b33aa63de919d1c4d7336ecc1e50c8` was
cherry-picked separately as `f3c42d2`, adding only
`backend/services/auth-service/migrations/0013_identity_binding_epoch.sql`.
The worker did not edit that migration. Its owner global sequence assigns every
identity INSERT/UPDATE a fresh epoch in `1..9007199254740991`, ignoring caller
values. This covers relink, harmless repeated verification, soft deletion and
ID deletion/reinsertion. Sequence gaps after rollback are expected; exhaustion
refuses writes without wrapping. `linked_at`, wall time and row ID alone are not
binding generations. Production sequence privileges, backup/restore behavior
and invalidation delivery remain separate qualification gates; this slice
claims no replay safety across a restored database.

Retained imported foundation provenance is the C01/BE01 port-rust revision
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, original prefix
`boilerplate-grpc-rust/` mapped to `backend/`. This is not a new upstream fetch
claim. Read specification BE04 and §§3.2, 7.2, 7.4, 8.1, C03 and the existing
BE04 identity boundary document. No Bigyeting extraction, rendering, new
dependency, lockfile resolution or `docs/source/` mutation is included.

Test infrastructure reuse is bounded and recorded: the `owned_pool` guard in
`backend/services/auth-service/tests/concert_identity_owner.rs` was adapted from
`backend/services/concert-service/tests/command_receipts.rs` at the worker base,
with Auth-specific environment/database allowlists and absolute-root validation.
The new smoke script/helper tests adapt the owned-cluster/report pattern from
`backend/scripts/mcr/command_receipts_smoke.py` and
`test_command_receipts_smoke.py` at the same base. They import unchanged
`migration_smoke.py` and `concert_core_smoke.py`. Dependencies are retained
SQLx/Tokio/chrono/UUID/async-trait/thiserror and Python standard library. Regression
coverage includes refusal, cleanup failure, redaction, report preservation,
skipped-test rejection and real owned PG execution. Exact source/helper hashes
are recorded in the external qualification receipt.

## Internal contract and query semantics

`auth-application/src/concert_identity.rs` defines a neutral application port,
validated inputs, explicit cancellation/deadline errors and minimal owner facts.
Input is an authenticated caller's canonical user UUID, session-family UUID and
optional exact identity row UUID/provider/owner epoch. Constructors validate
shape only; they cannot authenticate the caller. Provider references are opaque
internal identifiers, not OAuth subjects or externally signed claims.

`PgCurrentAdmission` reads only Auth's own public tables. A single PostgreSQL
statement supplies one MVCC snapshot and one materialized database clock sample.
The clock uses `clock_timestamp()` during execution rather than request-start or
transaction-start time, including after acquisition or relation-lock waits.
Output has only database `observed_at`, typed account eligibility, session
eligibility and binding eligibility. Debug output redacts requests, binding
references, epoch, pool details and owner facts. Errors contain static categories;
raw database errors never cross this boundary. The SQL projects no provider
subject, email, token/hash, social handle, avatar, profile content or age.

- Account eligibility requires an existing exact user with `status='ACTIVE'`
  and `deleted_at IS NULL`. Missing, pending, deleted and unknown states fail
  closed. There is no invented account suspension field or global role inference.
- Session eligibility requires an unrevoked refresh token in the exact same
  user's family with expiry strictly after database observation time. Other
  users' families and other sessions cannot substitute. This read changes no
  session, role, account or identity row.
- No optional reference means `NotRequested`; the statement omits the provider
  relation entirely, including relation locks. An email-only user needs no X
  account, provider identity row, Profile read or optional email-binding proof.
- An optional reference must match exact row, user, provider and owner epoch,
  with no soft deletion. GOOGLE/APPLE/X additionally require a present subject
  satisfying the retained onboarding validation. SQL checks protected content
  without returning it. EMAIL needs no OAuth subject; it requires the owning
  verified flag and nonempty email. These are internal binding eligibility
  results, not verified-age, social-consent or exported verified-email claims.
- Missing, foreign, unlinked, soft-deleted, relinked or malformed claims return
  `NotBound`. Even a valid binding cannot override an ineligible account or ended
  family. A valid OAuth subject may have `email_verified=false`; that flag is
  not an OAuth subject verification or social consent primitive.

`eligible()` combines only these Auth-owned facts. It does not establish event
membership, operator permissions, event restrictions or suspension clearance.
No actor/event/operation scope is echoed as cryptographic proof. These values
have no serialization implementation and are neither a public wire contract nor
a signed admission capability. Fresh trusted owner reads and reference issuance,
authenticated composition, request-bound signatures and separate Concert event
policy remain integrator work. Profile owns content/lifecycle, not suspension.
No server, router, RPC, HTTP endpoint or accepting fallback is installed.

## Timing, cancellation and snapshot limits

One caller-supplied absolute monotonic deadline covers pool acquisition, local
statement-timeout setup, owner snapshot and read-transaction completion. It is
never extended after an await. The two-second maximum is a conservative internal
resource ceiling, not a product revocation SLO or proof lifetime. Callers can
provide shorter deadlines. Cancellation is shared, irreversible and checked
before I/O, between awaits, after completion and on a five-millisecond polling
interval while the future is suspended; executor scheduling can add latency.

The adapter sets transaction-local PostgreSQL `statement_timeout` from remaining
time. Local timeout/cancellation drops the read future promptly; it does not
promise instantaneous PostgreSQL cancellation. Already dispatched work can
continue until its bounded statement timeout, connection closure or lock release.
SQLx schedules rollback for a dropped transaction; a connection must finish
cleanup before reuse. The native one-connection-pool regression verifies a fresh
successful read and restoration of the session's original statement timeout after
timeout/cancellation and lock release. It does not claim an exact connection PID
must be retained. Setup/response delays and server execution are distinct from
the locally enforced original deadline.

Facts describe the database snapshot, not a revocation lease. A committed writer
after the snapshot cannot retroactively alter an already completed read. Native
tests hold uncommitted deactivation/deletion (including revocation trigger
effects), observe the prior coherent committed state, then commit and require a
fresh read to deny. Production revocation/invalidation and stream termination
need their own end-to-end evidence.

## Verification and reproducibility

Rust `1.88.0`, retained lockfile, `--locked --offline`, two build jobs, retained
target `/Users/lhs/Desktop/mcr-build/backend`. No Docker/global service/default
source database is started. Native PostgreSQL `16.14` uses a new private
marker/inode-owned cluster, Unix socket and databases `mcr_auth_identity` and
`mcr_auth_identity_exhaustion`. Ownership, database, socket, port, marker and
actual data directory are verified before repository tests. Missing owner
environment is a hard test failure, never a skipped success. The exhaustion test
advances only its dedicated disposable database sequence to the maximum; it
never resets a sequence or touches production data.

Validated commands (exit 0):

- `CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 cargo +1.88.0 build --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p auth-service --bin auth-service --message-format=json`
- The same environment/flags with `cargo ... test -p auth-application --test concert_identity -p auth-service --test concert_identity_owner --no-run --message-format=json`.
- Execute the exact `concert_identity` artifact returned by Cargo JSON: **8 deterministic tests**.
- The same locked/offline environment with `cargo ... test -p auth-application --lib`: **23 retained application tests**.
- `python3 backend/scripts/mcr/auth_identity_smoke.py --binary /Users/lhs/Desktop/mcr-build/backend/debug/auth-service --repository-test <absolute Cargo-reported concert_identity_owner artifact> --pg-bin /opt/homebrew/opt/postgresql@16/bin --report <new absolute receipt>`: **22 real PG tests**, SQLx all 13 retained/prerequisite migrations, exact ledger checksums, reapply, restart and owned cleanup.
- `python3 -m unittest discover -s backend/scripts/mcr -p 'test_*smoke.py'`: **89 deterministic helper tests**, including seven new Auth harness tests.
- Rustfmt check of the four new Rust files, `python3 scripts/mcr/check-plan.py`,
  `git diff --check`, exact changed-path/ownership and cross-directory import review.

The retained generic boundary CLI cannot express the two explicitly assigned
`backend/scripts/mcr/` files: `--allow-composition` rejects script paths with
exit 2, and ordinary lane mode classifies scripts as integrator-owned. This is
reported rather than modifying the central checker. The handoff instead records
an exact nine-path assignment comparison, unchanged prerequisite migration,
two one-line exports and the retained import scanner/manual import review.

Native cases cover email-only provider-table independence; active/deleted/unknown
accounts; missing/foreign/expired/revoked families; retained production
`session_is_live`/family revocation and live rotated successors; exact and foreign
provider references; absent/blank/Unicode-whitespace/control/oversized OAuth
subjects; owner email verification; soft-delete/restore and actual retained
repository relink; repeated equal-`linked_at` updates, supplied-epoch overwrite,
ID reuse and exhaustion rollback; committed snapshot races; pre-cancellation,
pool/statement cancellation and deadline waits; expiry after pool/observed lock
waits; original deadline across phases; owner unavailability and safe diagnostics.

Development receipt `auth-identity-review.json` records the 22-test dirty-tree
qualification. The final handoff supplies a separate clean committed-candidate
receipt under `/Users/lhs/Desktop/mcr-build/`, including full source revision,
dirty-path list, source/helper/migration/binary/test hashes, commands/exit codes,
test diagnostics, restart and owned-directory absence. A reviewed clean-candidate
rerun is required before integration; no release readiness is inferred.
