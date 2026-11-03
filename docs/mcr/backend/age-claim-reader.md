# BE26-CLAIM-READER: source-finalized projection and port limitation

## Result and delivery boundary

The existing synchronous BE25 `ClaimReader` cannot safely return a PostgreSQL
lease with the required connection-loss semantics. This handoff therefore
provides independently usable **transaction-scoped claim projection**, and a
small coordinated port proposal below. It does **not** claim a successful legacy
`ClaimReader` bridge. `PgClaimProjection::read` deliberately returns
`PolicyError::Unavailable`, even when a qualified authority is configured.

The bounded assignment's independent implementation and native qualification are
delivered; production claim-reader/BE25/BE27 composition remains **IN_PROGRESS**.
No whole BE26, BE27, IN03, public-provider or release acceptance is implied.

- Delivery base: `5a0c6fb717e1b304694e9ec74224b196f990e91b`.
- Branch: `codex/mcr-be-BE26-claim-reader`.
- Implementation: `e5d04b0`; scoped review fixes and qualified clean code:
  `793b5e32b63c8089ab92b9da591e9824ebfcdf71`.
- Subsequent documentation-only handoff commits change no executable input.
- Main integrates/publishes; worker does not push master.

Exactly five assigned new files are delivered:

- `backend/services/concert-service/src/adapters/out/age_provider/claim_reader.rs`
- `backend/services/concert-service/tests/age_claim_reader.rs`
- `backend/services/concert-service/tests/age_claim_reader_support/mod.rs`
- `backend/services/concert-service/tests/age_claim_reader_support/native_smoke.py`
- `docs/mcr/backend/age-claim-reader.md`

Existing age-provider modules/schema, BE25/BE27 application code, contracts,
manifests, lockfiles, root registration, central migrations and HTTP remain
unchanged. Tests path-include the exact new module; root runtime composition is
**NOT_RUN**. The old checkout and immutable evidence remain untouched.

## Why a detached synchronous lease is insufficient

BE25 returns a `ClaimSnapshot` containing private claim data and a boxed
`ClaimLease`. `current` and `IntroPolicy` call the lease's synchronous `check`
before their final projections. Retaining a SQLx transaction inside that box
would retain a local connection object, but not establish that its remote
PostgreSQL session still exists. A lost connection, server termination or idle
transaction timeout can release source locks while that local object survives.
A concurrent revocation/replacement may then commit before local synchronous
projection. A timestamp, mutex, heartbeat flag or last successful RPC cannot
close that interval. Merely making a pre-projection check asynchronous still
leaves source loss between its response and projection.

The native connection-loss test demonstrates the relevant ordering: projection
holds a local private value; its owned source backend is terminated; source
revocation succeeds because the locks are gone; the projection scope then fails
and withholds the value. No insufficient successful legacy lease is fabricated
to run this experiment. Existing `IntroAuthority::ReadLease` has the analogous
source-lifetime constraint and needs qualified composition too.

## Safe independent scope

`PgClaimProjection::project(query, closure)` owns one SQL transaction for the
entire private operation. It validates and preserves the original canonical
actor, subject, event, session, request, requested time and request deadline.
These values are correlation only. `ProjectionAuthority::authorize_in` must
resolve them against current real authority and retain participating source
fences through final COMMIT; it is invoked before the read and again after
projection. Returned `AuthorityDecision` data is checked against the exact query
and must remain consistent. It is not a lease or an authority token.

`UnavailableProjectionAuthority` is the production default and always fails.
No canonical Auth producer or cross-service fence protocol is provided here.
The test authority reads and share-locks a table explicitly named
`be26_reader_fixture_authority`; it is synthetic, isolated Concert test data,
not a second production Auth identity. Provider availability comes from that
locked source decision, not a cached flag in the adapter. A real implementation
must qualify its provider-policy source as well as its canonical identity fence.

Lock order follows BE26: authority, event, subject-owner guard, canonical claim.
The owner guard is held `FOR SHARE`, blocking the existing BE26 writer's
`FOR UPDATE` through finalization. The selected claim row is also share-locked.
Participating production replacement/erasure/revocation writers and database
grants must enforce this same owner-guard protocol; direct inserts bypassing it
are not qualified. A missing owner guard is inserted within this transaction;
rollback or cancellation removes that uncommitted guard.

The exact query is global to the canonical subject, matching BE26:

```sql
select * from concert_age_claims
where user_id=$1
order by verified_at desc,expires_at desc,claim_id desc
limit 1 for share
```

No event, expiry, revocation, replacement or status predicate precedes canonical
selection. BE26's `EXPIRED` erasure tombstone maps to the existing BE25 assessment
semantics. Existing BE25 arithmetic and half-open bounds make the age decision;
the adapter does not reimplement birthdate arithmetic. A provider outage does not
invalidate a still-valid claim. Unknown/unavailable verification cannot satisfy
restricted discovery or sharing through those same BE25 rules.

The borrowed `ProjectionAge` exposes status, safe timestamps, availability,
query correlation and an in-scope bounds decision. It exposes no DOB, exact age,
claim ID or source provenance and implements no Clone/Debug/Serialize. Its
assessment borrow cannot outlive the closure. Private database mapping data does
not leave the scope. The closure is trusted application projection code: it must
not publish, log, cache or perform external side effects with intermediate
values, nor return an authorization capability for later use.

The closure's result stays private until post-projection authority validation,
effective claim/birthday/request deadline checks, source acknowledgment and known
COMMIT. Final SQL checks the minimum effective deadline. Post-COMMIT clock checks
also discard results when SQL/COMMIT itself crosses a temporal boundary.
Successful COMMIT establishes source ordering; the final temporal check does not
extend it. Revocation after a completed read governs subsequent reads; no response
is a reusable authorization grant or a promise to retract already delivered data.

The outer original request deadline covers pool waits, SQL, authority, projection
and finalization, clamped to at most five seconds to bound resource occupancy.
SQL timeout clamps preserve shorter existing limits. Errors, cancellation,
projection failure or connection loss drop the transaction and withhold the
result. SQLx queues rollback on transaction drop; native checks establish that
the transaction disappears and the authority row can be locked again. Lost final
acknowledgment is an unavailable read, not a successful projection.

## Smallest coordinated port proposal

Do not loosen `ClaimLease::check` or wrap `PgAgeClaims::status`. Add an application
projection port alongside the old fail-closed read port, using the existing
private domain assessment and framework-free types. Its concrete method shape
matches this qualified adapter:

```rust
async fn project<T: Send, F>(
    &self,
    query: ClaimQuery,
    projection: F,
) -> Result<T, PolicyError>
where
    F: for<'a> FnOnce(ProjectionAge<'a>) -> ProjectionFuture<'a, T> + Send;
```

`ProjectionFuture` is a boxed Send future, and `ProjectionAge` is the borrowed,
private policy view. Move those framework-free view/port declarations into the
assigned application owner; keep PostgreSQL and `ProjectionAuthority` in the
adapter. This generic port is not an object-safe replacement for existing trait
objects; current application consumers are already generic and can adopt an
explicit new scoped entrypoint without falsely enabling the old one.

The BE25/BE27 owner must then move the **entire authorized direct/page projection**
inside the source-finalized scope. A page uses one viewer-age observation and
retains collection and every applicable intro/resource authority through the
final source acknowledgment, including an empty page. The exact collection or
intro ID/subject/revision/read-kind targets must also be bound and fenced by the
joint read authority. Coordinating that target scope and its common lock order
with BE27 is required; this claim-only API does not silently qualify intro rows,
blocks, memberships, restrictions, profiles or pagination.

Calling the old `IntroPolicy` inside this closure with a timestamp-only fake
`ClaimReader` or `ReadLease` would remain unsafe. Merely appending an async check
to each independently released row does not establish a final page authority
scope. Do not publish intermediate results or commit separate row scopes while
assembling a still-unpublished page. The production owning-source protocol must
make final projection and revocation/owner loss orderable; ordinary detached Auth
RPC observations cannot provide that guarantee.

Required shared changes remain proposals, not edits in this handoff:

1. Application `age_policy` projection port/view declarations and explicit scoped
   BE25/BE27 consumers with joint collection/resource authority semantics.
2. Reviewed `age_provider` module registration and real canonical Auth/provider
   source composition retaining the required fences through finalization.
3. Registered BE26 schema/grants, qualified joint read lock order, and real HTTP/
   multiuser/provider qualification. No new schema or migration is proposed here.

BE27 can continue independent intro persistence and deterministic policy work;
its production restricted-age composition must remain unavailable meanwhile.

## Provenance and executed evidence

All reuse comes from the exact delivery base, not a new upstream import. The new
file adapts BE26's private `owner_claim` mapping and timeout clamp; the guard and
native harness adapt BE26's owned fixture infrastructure. Orchestration and
projection lifetime handling are newly authored. Existing retained `async-trait`,
`chrono`, `sqlx`, `tokio`, `uuid`, application and domain dependencies suffice;
no manifests or lockfile resolutions changed.

| Read-only source at `5a0c6fb` | SHA-256 |
| --- | --- |
| Application `age_policy/mod.rs` | `7a47d219bfa90a787fd1f2705556fcab63cee79d38eb100c695c74dc6a528591` |
| Application `intro_policy/mod.rs` | `a1c86a896f17d787cb2a8b46f211d819d01144b6bb0f8c567c15ff239365474b` |
| Domain `age_policy/mod.rs` | `8e0c8a2dba17a90075a52ab1a77d62f14a0aacd142e42a46a6c334802dd5f2f3` |
| BE26 `age_provider/mod.rs` | `772c0297b60ef07a4d03817ceec096b8ca32029fb79cdf9a9fd04208d65d7e76` |
| BE26 `age_provider/schema-v1.sql` | `1c2e786250e6a4e61b26c4232211ef48d2b4cb7a2e51b258c57b697d1e6a46d5` |
| BE26 `tests/age_claims_support/mod.rs` | `7bd9cb4396b83f308e3b4218c67d2ba3137906c279da02a9514f33016d61dcb9` |
| BE26 `tests/age_claims_support/native_smoke.py` | `447d670a1d3e7db5720810c5d8cbd3c18ef84463ebea081b4c0b0ca4aabaea07` |

Private evidence root:
`/private/tmp/mcr-be26-claim-reader-5a0c6fb-20261104/` (`0700`),
reports/logs `0600`, copied binary `0500`. Clean qualified code is
`793b5e32b63c8089ab92b9da591e9824ebfcdf71`.

```text
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 1 -p concert-service --test age_claim_reader --no-run --message-format=json
PYTHONDONTWRITEBYTECODE=1 python3 backend/services/concert-service/tests/age_claim_reader_support/native_smoke.py --artifact-receipt /private/tmp/mcr-be26-claim-reader-5a0c6fb-20261104/build-3/receipt.json --pg-bin /opt/homebrew/opt/postgresql@16/bin --report /private/tmp/mcr-be26-claim-reader-5a0c6fb-20261104/native-1.json
```

Both exited `0`. Rust 1.88.0, retained protoc, locked/offline dependencies, one
job, no debug information or incremental compilation; minimum observed free
bytes `15405146112`, above the required 8 GiB floor. Only the released
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target` was reused. Build 3
copies and hashes the exact compiler-emitted executable before target release.
After qualification, `lsof +D <target> -t` exited `1` with no handles; Main was
explicitly notified that the target was released for World runtime work. No
subsequent Cargo or target writes are authorized to this worker.

Native run: **11 passed, zero failed, zero ignored** on PostgreSQL
`16.14 (Homebrew)`. All 13 current central schema files plus the unchanged BE26
local schema applied via single-transaction psql, all exit `0`. Isolated database
`mcr_concert_be_age_claim_reader`, TCP disabled, reserved port `61030`, private
socket/owner marker checks. Cleanup passed; `/private/tmp/mcr-pg-43hi0nue` is
absent. No live store or global settings were changed. A process restart is not
part of these ephemeral read-scope tests; no claim of lease persistence or replay
across restart is made. Central SQLx migration-ledger registration is **NOT_RUN**.

The native suite verifies default/legacy-port denial, exact query and returned
decision bindings, provider availability versus persisted validity, canonical
expired/revoked/replaced/unsupported records without fallback, a persisted Seoul
100-to-101 boundary crossed during projection, observed owner-lock serialization
with replacement and revocation, revoked-owner denial, owned source-backend
termination before finalization, cancellation/drop/rollback and authority-lock
release, and the original request timeout after confirmed projection entry and
observed authority lock. A deferred test-only PostgreSQL trigger delays COMMIT
past a 250 ms claim expiry: the audit insert is demonstrably committed, but the
private result is rejected. These are synthetic authority/provider/calendar
fixtures, not canonical Auth or a real provider flow.

| Evidence | SHA-256 |
| --- | --- |
| `build-3/receipt.json` | `72dc0bab98be9f198073775cc4a8fcfd887bca898c02d2b168340f103b739f56` |
| `build-3/age-claim-reader-tests` | `6d941695c494bbc6cca964ec00e533e5ae8f84bc2fe65be995aee02da50f042a` |
| `native-1.json` | `86290dd7072bbf84eafa261ddfe44510f9d3e0c3ca956e47c0ba7854ccb7258a` |
| New native harness | `dbe4d2a13b0add5bc6013dd875375ead0ea1e61607d9b7e8f34b525b6aee7e84` |
| Unchanged migration helper | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` |

`qualification-binding.json` confirms all **1,465** backend/contract source pins
and **122** prior immutable BE24/BE25/BE26/world evidence pins remained unchanged,
with owned-root removal. The planning/provenance checker, owned Rust 1.88
`rustfmt --check`, and `git diff --check` exited `0`. Planning validation alone
does not certify application behavior.

Preserved attempts: build 1 exited `101` for an unannotated never-returning test
closure; explicit `project::<(), _>` resolved its inference. Build 2 compiled
successfully before review finalization. Targeted peer review then identified
late COMMIT temporal expiry and a deadline fixture that could pass before entry.
Commit `793b5e3` fixes both and adds the observed-entry/lock and delayed-COMMIT
checks. Final build 3/native 1 qualify those changes. Focused peer source review
closed with no remaining actionable findings; it does not qualify the production
Auth, shared application port, or joint BE25/BE27 composition.
