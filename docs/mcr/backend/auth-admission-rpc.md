# Authenticated current-admission RPC — BE04 prerequisite

Status: **IN_PROGRESS**, pending integrator review. This slice composes an
authenticated read of Auth-owned account, session-family and optional exact
provider-binding facts. Whole BE04 and the project remain **IN_PROGRESS**.
Gateway JWT validation, private Profile/event policy, signed Concert admission,
world transport and production Concert authority are **NOT_RUN** here. The
production `UnavailablePolicy` is unchanged.

## Source and delivery boundaries

Assignment base: `a44f8fa711b1ac73ce97d9e18d13dfc2eeee2382`, isolated worktree
`be-auth-admission-rpc`, branch `codex/mcr-be-auth-admission-rpc`. The base
contains the parent's frozen proto/method-policy/verified-read-authority
prerequisite on reviewed `e725ae4`. The parent publishes the functioning
prerequisite and handler together; the original fail-closed stub was not a
working owner RPC. Parent test-only correction
`e79fd177c44ca4c18621ce23560eb7007e3e48fe` was separately cherry-picked as
`e774b1a`. It isolates elapsed proof-monotonic time while signed wall expiry is
still live; it changes no runtime behavior and is not worker-authored platform
code. The implementation commit and delivered ancestry accompany the handoff.

Read authorities are the BE04 specification, `docs/mcr/identity-admission-decision.md`,
the frozen `auth-admission-rpc-contract.md`, Auth proto, production guard,
`VerifiedReadAuthority` and `AuthorityRpcDeadline`. Retained foundation provenance
is C01/BE01 port-rust `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, original
`boilerplate-grpc-rust/` mapped to `backend/`; no fresh upstream fetch is claimed.
There is no Bigyeting code, new dependency/lock resolution or `docs/source/` edit.

Nine exact assigned paths change: new feature handler and test, two native smoke
files, this document, and four bounded existing composition exceptions. AppState
adds one `Arc<dyn CurrentAdmissionPort>` field and injects only the real
`PgCurrentAdmission(pool.clone())`. The grpc module exports the feature; the
generated server replaces only the `ReadConcertAdmission` stub with its handler
call. Method-policy changes add only a Gateway/actor-field test. The existing
owner adapter, platform, proto, policy declarations, env/defaults, other use cases,
Gateway, Concert and frontend remain untouched by the worker implementation.

## Authenticated handler behavior

`ReadConcertAdmission` runs behind the production `guard()` and
`CallDeadlineLayer`, as in Auth's existing binary registration. The guard
authenticates a registered Gateway credential and checks signed caller,
audience, full RPC method, exact protobuf frame/body digest, declared actor and
replay before installing the private-constructor `VerifiedReadAuthority`.

The handler independently requires verified Gateway caller, `VerifiedActor` and
that private context. Both actor values must agree with the exact canonical
non-nil request user UUID. The family must also be a canonical non-nil UUID,
matching the shape of Auth JWT `sid`; this shape check does not validate a
browser JWT or prove family ownership. The real owner query does the latter.
Bare metadata, a supplied UUID, constructor validation, or an echoed actor/event/
operation cannot stand in for verified authority. The handler never re-parses a
raw proof token. Optional binding parses the exact row UUID, closed provider and
bounded positive owner epoch; it creates or attaches no identity.

The minimal response contains only database observation micros and typed
account/session/binding facts. Existing negative owner facts remain a successful
internal fact response. Errors map to fixed InvalidArgument, Cancelled,
DeadlineExceeded or Unavailable statuses; a dependency error never becomes a
positive fallback. A requested binding cannot be replaced by `NotRequested`
facts, nor can an omitted binding receive a binding assertion. Responses have
`cache-control: no-store`. No actor, event, purpose, provider subject, email,
token/hash, role, legacy avatar, Profile content or age is returned or logged.

Omitted binding remains email-only/provider-independent: the unchanged owner SQL
does not reference the provider relation. The native RPC test holds an exclusive
provider-table lock and still obtains an honest successful read. Exact,
foreign-user, deleted and relinked references are checked by the same reviewed
owner repository. Optional binding is not provider consent, verified age or a
signed admission capability.

## One original deadline and cancellation limits

The handler calls `VerifiedReadAuthority.deadline(ReadBudget::MAX_DURATION)`.
Its stop is the minimum of original transport stop, guard-entry resource stop,
captured proof-monotonic stop and signed wall expiry. Missing original deadline
or missing strict context fails closed. Legacy clock-skew tolerance remains
unchanged for other methods; a legacy-valid proof past its actual signed expiry
gets no usable strict read context. The test uses the public signer with an
issuance time 31 seconds earlier, rather than waiting or bypassing constructors.

One absolute budget covers the whole owner await. No fresh two-second budget is
started after buffering, verification, acquisition or earlier waits. The resource
ceiling is not a product revocation SLO. Before dispatch, after the await and
before returning facts, the handler rechecks both signed wall and captured
monotonic time. The whole await is enclosed by the same `timeout_at` stop. A
controlled owner which blocks a poll and returns late success is rejected by the
post-await check, even if it ignores the budget itself.

A drop guard signals the shared owner cancellation flag on timeout, cancellation
or handler completion. This is not instantaneous database cancellation. SQLx
rollback, transaction-local statement timeout, socket closure or lock release
can complete later. Native RPC qualification uses a one-connection pool, times
out behind a real table lock, releases the lock and requires a successful fresh
RPC plus restoration of the original session timeout. It does not require a
particular connection PID to survive. Facts describe a completed database
snapshot; subsequent revocation needs a fresh read or separately qualified
invalidation/epoch fence.

## Test implementation and provenance

The generated-server socket tests register the actual `AuthGrpcServer` through
the production guard and deadline layer on owned ephemeral loopback listeners.
Controlled owner ports count dispatches and expose captured budget cancellation;
they are test injections into the exact AppState field, not parallel accepting
servers or production adapters. Native tests leave production injection intact
and read real PostgreSQL. Socket children are shut down/aborted only through their
owned handles. Test credentials and local-parity signing keys are fixtures, not
production TLS/credential qualification.

The owner guard in `concert_admission_rpc.rs` adapts `owned_pool` from
`concert_identity_owner.rs` at the assignment base, with RPC-specific environment
and the sole `mcr_auth_admission_rpc` database. It verifies private absolute root,
marker token/inode, socket/database/role/port and actual data directory. Config
fixtures read the unchanged `support/auth_fixture_config.rs`; signing key
generation is cached only within the test executable. Generated server/guard
setup follows the retained platform method-policy network tests at the same
base. The production handler and controlled cases are newly authored.

The new smoke files adapt the existing Auth owner smoke/report pattern and import
unchanged `migration_smoke.py`, `concert_core_smoke.py` and `auth_identity_smoke.py`.
They use Python standard library and retained Rust dependencies. The harness
lists actual executable tests to derive native counts, requires zero failures and
ignored tests, refuses missing ownership and existing/symlink reports, sanitizes
failures, and revokes success if cleanup fails. The retained 22-case owner suite
runs in a second fresh owned cluster. No Docker/global service or source-default
database is started by the native harness. Source/helper/binary/test hashes and
both owned-directory absence results are in its receipt.

## Verification and limits

Rust `1.88.0`, `--locked --offline`, `CARGO_BUILD_JOBS=2`,
`CARGO_PROFILE_DEV_DEBUG=0`, shared retained target
`/Users/lhs/Desktop/mcr-build/backend`. Build/test artifacts are selected from
Cargo JSON, never guessed by executable hash. Commands and exact exits/log hashes
accompany the handoff and clean-candidate receipt.

- Generated guarded-server controlled suite: **11 passed**, covering registered
  Gateway, missing/wrong/sibling caller, missing/malformed/signature/audience/
  method/body/actor proof, replay, malformed canonical IDs/reference, missing/
  invalid/elapsed original deadline, strict proof expiry, fixed owner failures,
  inconsistent binding facts and late/noncooperative completion fences. Denials
  prove zero owner dispatch where appropriate; replay proves no second dispatch.
- Review fix R1 corrects the live-proof timing fixture only. The public signer
  issues fixed 30-second, integer-second claims; the test completes server/key/
  channel setup, then bounds a pre-issuance phase wait to two seconds. It issues
  one proof with 1.25–1.5 seconds of live authority and sends one RPC with its
  original five-second deadline. An explicit owner-entry notification must arrive
  within 750ms while the proof is live; the owner stays pending until cancellation.
  Early denials fail without retry. Assertions require exactly one dispatch,
  `DeadlineExceeded` at or after actual signed expiry, cancellation, and a response
  before the two-second resource ceiling. This proves proof expiry wins over both
  longer limits. The misplaced wait in the already-expired legacy-skew case is
  removed. The previous 11-case pass did not expose its near-second-boundary
  dispatch race. The corrected `deterministic_` suite passed **11/11**, exit **0**,
  with the five native cases filtered, using the command below. Its separate log
  is `/Users/lhs/Desktop/mcr-build/auth-admission-rpc-R1-controlled.log`.
  No native database or global Python suite is rerun for this test-only fix;
  the parent owns combined-candidate native and contract verification. The
  retained 97-case Python result below remains evidence at the original contract
  base, not a claim about the parent's subsequently revised C04 contract pin.
- Native generated-server suite: **5 passed**, plus the retained **22 owner
  tests** in a separate cluster. SQLx all 13 Auth migrations, exact ledger,
  reapplication, PostgreSQL restart/data persistence and owned cleanup pass.
  Counts derive from executable listings; no native test is skipped.
- Platform authority tests: **103 passed**, including the corrected strict-context
  regression; focused `actor_delegation::read_authority`: **6 passed**.
  Native platform method-policy network guard: **2 passed**.
- Retained Auth application library: **23 passed**; application value tests:
  **8 passed**; Gateway verification transport: **13 passed**. Python smoke
  helpers: **97 passed**, including eight new RPC harness tests.
- Auth's full library attempt recorded **48 passed / 1 fixture acquisition
  failure**: unchanged
  `config::tests::external_migration_mode_rejects_schema_damage_without_owner_privileges`
  unconditionally starts Docker through the retained shared fixture and assumes
  its `postgres:postgres` URL. Docker is unavailable and no native override
  exists. The remaining library cases are run with that exact test filtered,
  transparently recorded; this does not claim the Docker-only privilege/schema
  guard passed. Changing the shared fixture is outside this assignment.
- Rustfmt of authored Rust/test additions, planning provenance, whitespace,
  exact nine-path ownership, bounded composition diff and cross-directory import
  inspection are required. The generic boundary CLI cannot express the two
  authorized script exceptions; manual exact-path/import qualification is used
  without changing or claiming to bypass broader guards.

Reproduction uses these commands from the assigned worktree, with the environment
and retained target above:

```sh
cargo +1.88.0 build --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p auth-service --bin auth-service --message-format=json
cargo +1.88.0 test --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p auth-service --test concert_admission_rpc --test concert_identity_owner --lib --no-run --message-format=json
cargo +1.88.0 test --manifest-path backend/Cargo.toml --target-dir /Users/lhs/Desktop/mcr-build/backend --locked --offline -j2 -p auth-service --test concert_admission_rpc deterministic_ -- --test-threads=1
python3 backend/scripts/mcr/auth_admission_rpc_smoke.py --binary /Users/lhs/Desktop/mcr-build/backend/debug/auth-service --rpc-test <absolute Cargo-reported RPC artifact> --owner-test <absolute Cargo-reported owner artifact> --pg-bin /opt/homebrew/opt/postgresql@16/bin --report <new absolute receipt>
python3 -m unittest discover -s backend/scripts/mcr -p 'test_*smoke.py'
python3 scripts/mcr/check-plan.py
```

The companion retained-check receipts record exact Cargo commands for platform,
Auth/application and Gateway tests, including the explicit Docker-only exclusion
and the unsuccessful full-library command; no aggregate all-green claim hides it.

The final clean committed receipt records both PG16.14 cluster identities and
absence after cleanup, full candidate SHA/dirty paths, source/proto/helper/artifact
hashes, native case names, command exits, ledger/restart and limitations. Earlier
dirty-tree receipts are development evidence only. Parent review and its combined
candidate rerun remain required; the bounded goal stays active until concrete
review fixes are resolved. No publication is performed by this chat.

Production mTLS, role/sequence privileges and exhaustion operations, backup/
restore epoch replay safety, multi-replica replay enforcement, product revocation
SLOs and stream invalidation remain unqualified. The parent owns central RPC/
actor-proof/deadline composition and the eventual Gateway-to-Concert admission.
