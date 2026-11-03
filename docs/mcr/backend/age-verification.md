# BE26: scoped verified-age claim implementation

## Delivery boundary

This handoff implements the application/provider port, isolated PostgreSQL schema,
receipt-backed initiation/callback processing, own-status reads, and bounded
expiry-detail erasure. It is **IN_PROGRESS** at the whole-task level: production
Auth/provider composition, registered migrations, HTTP behavior, and end-to-end
BE26 acceptance remain unqualified. No IN03, IN05, or release acceptance is implied.

Delivery base: `3810016a72a82358e785790cf67d7b1b292db80e` on
`julymeltdown/mcr` master. Worker branch: `codex/mcr-be-BE26-age-claims`.
Implementation commits: `2434ae9`, `3566ef3`, `7304895`, `e99d059`, and
`e9ac29a8be927ec329dbcf1c7faaee2d3a91aaac`. The latter is the clean source revision
bound to the final executable and native evidence. Subsequent documentation-only
commits do not change those executable inputs. Only the designated integrator
publishes to master.

All changed paths are new and explicitly assigned:

- `backend/crates/concert-application/src/age_verification/mod.rs`
- `backend/crates/concert-application/tests/age_verification.rs`
- `backend/services/concert-service/src/adapters/out/age_provider/mod.rs`
- `backend/services/concert-service/src/adapters/out/age_provider/schema-v1.sql`
- `backend/services/concert-service/tests/age_claims.rs`
- `backend/services/concert-service/tests/age_claims_support/mod.rs`
- `backend/services/concert-service/tests/age_claims_support/native_smoke.py`
- `docs/mcr/backend/age-verification.md`

Existing application exports, ports, manifests, lockfiles, contracts, router,
startup, and central migrations were not modified. Tests include the exact new
modules by path, with explicit aliases for existing application/repository modules;
this compiles their real bytes without claiming root registration.

## Behavior and trust boundaries

Initiation generates 32 random bytes using `ring::rand::SystemRandom`, exposes a
strict 64-character lowercase hexadecimal token once, and stores only its SHA-256
digest. PostgreSQL creates the state with a ten-minute deadline. The server-owned
configuration permits an exact allowlisted HTTPS endpoint and fixed
`AGE_PRIVACY` return target. Credentials, explicit ports, fragments, queries,
escapes, IP literals, and malformed host/path configuration are rejected. No
caller-controlled redirect or provider endpoint is accepted.

The trusted `Verifier` port must independently validate provider output. Its proof
must bind verification ID, canonical user, event, session, verifier, state digest,
and source revision. The application validates the binding, challenge deadline,
proof time/lifetime, and calendar data. `UnavailableVerifier` and
`UnavailableAuthority` deny production use by default. Fixture proofs are not
provider evidence. A BE04 identity observation or UUID alone grants no mutation
authority.

Every SQL phase independently invokes the injected transaction authority. Its
contract requires a current canonical Auth/session/event/eligibility decision and
an owner fence retained through commit. Lock order is authority fence, event
share lock, owner guard, verification state, then BE15 receipt budget. Original
request deadlines bound the complete public operation, including pool waits and
provider work; finalization rechecks proof expiry after lock waits and before
commit; SQL checks both the injected clock and database wall time and
preserves shorter pre-existing SQL timeouts.

Callback progression is `PENDING -> RESERVED -> EXCHANGING -> DONE`, with an
expiry tombstone. BE15 creates the callback Pending receipt and reserves exactly
one attempt in one transaction. Only that invocation can durably dispatch the
reservation. Provider exchange occurs outside SQL, after a known dispatch commit.
The final transaction reauthorizes, checks the reservation/binding, replaces prior
verified claims, inserts the new private claim, finishes the matching Pending
receipt, and clears the state digest atomically.

A timeout, provider failure, invalid proof, process interruption, or uncertain SQL
outcome cannot authorize another automatic exchange. Same-key replay returns the
existing receipt; different keys cannot consume a reserved state. An unknown
outcome can remain Pending even if no exchange actually occurred. Recovery is a
reauthorized receipt lookup, not permission to retry external work. No provider
reconciliation/retry worker is implemented.

Initiation replay returns the safe BE15 receipt but cannot reconstruct its raw
state or authorization URL. It returns `authorization: None`; the original
one-shot authorization is neither persisted nor rotated. **The C04 HTTP
`VerificationPending` response projection for this case is an explicit open
integration gate.** The HTTP owner must qualify receipt recovery/explicit new
initiation UX and compatible status behavior before claiming C04 completion;
fabricating a URL or rebinding the original key is unsafe.

Claims retain only private derived age/next-birthday data, verification/expiry
times, status, and minimal verifier/source provenance. Raw birth year/birthday,
proofs, state, provider response tokens, and authorization URLs are not persisted
or logged by these modules. Request canonicalization uses the existing BE15
keyed fingerprint rather than a raw receipt body. Secret-bearing types do not
derive `Debug` or `Serialize`. Own-status responses expose no exact age or proof.

Own-status selects the canonical newest claim by verified time, expiry, and claim
ID **before** validity/status assessment. A newer expired, revoked, unsupported,
or replaced record cannot fall back to an older valid record. Existing BE25 Seoul
calendar arithmetic derives age and the exact birthday transition; ages exceeding
the supported range become unsupported. Provider availability is separate from
existing claim validity, so an outage does not erase a valid claim.

Bounded owner-authorized `Access::Erase` cleanup clears expired state digests and derived claim
details, retaining canonical tombstones and revoked/replaced meaning. Erasure
requires the current canonical owner/session fence but remains available in
`READ_ONLY` and `ARCHIVED` events; it does not grant verification authority. This is not
a global retention scheduler or a deletion/backup policy. Own-status holds its
authority fence through the read transaction; it deliberately does not implement
BE25 `ClaimReader`, whose lease must remain valid at the policy use site.

## Provenance and dependencies

The selected headless Bigyeting inputs were inspected in the read-only source at
revision `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`. Callback one-use and binding
concepts were reviewed; orchestration/schema are newly authored, and no provider
integration or rendering was copied.

| Source path | SHA-256 |
| --- | --- |
| `src/application/use_cases/naver_verification.rs` | `ec5e0c24b544569e4df57c1bb80ff1cd8c8d2bd8e9843e6fe679d9d520e2d618` |
| `src/application/ports/naver_verification.rs` | `eb865ceda244cfecefd71e0cdae3b50b51327eae5f808f22af38a9713b34a243` |
| `src/adapters/out/repository/naver_verification_state.rs` | `ddea01ac7868e47fdd3557e7ce2f1f9910dce5d238c2374050243448a1ec82c2` |
| `src/adapters/out/repository/current_age.rs` | `74df0f4f60a9ce262aeec2af6a33deb897279889634cb14ed6cfe74579e11a50` |

Existing MCR BE25 `concert-domain/src/age_policy/{mod.rs,arithmetic.rs}` and
`concert-application/src/age_policy/mod.rs` supply canonical selection and Seoul
arithmetic. BE04 supplies `AuthorityBinding`; BE15 `commands` and
`adapters/out/repository/receipt.rs` supply registered operations, keyed
fingerprints, authorization callbacks, atomic reservation, and safe receipts.
The SQL timeout clamp follows the existing media adapter at the delivery base.
The owned native guard follows the BE24 media test guard; its harness imports the
unchanged `backend/scripts/mcr/migration_smoke.py` owned-cluster helper. These
exact inputs, all backend sources and contracts are pinned in the build receipt.

Dependencies use retained resolutions: `async-trait`, `chrono`, `uuid`, `sha2`,
`sqlx`, `tokio`, and service-side `ring`. No dependency or lockfile changes.
`docs/source/` remains byte-for-byte unchanged; the planning provenance check
also verifies the original source archive checksums.

## Executed qualification

Private evidence root: `/private/tmp/mcr-be26-age-claims-3810016-20261104/`
(mode `0700`); reports/logs `0600`; copied executable `0500`.
Build source is clean `e9ac29a8be927ec329dbcf1c7faaee2d3a91aaac`.
`build-7/receipt.json` binds 1,453 backend/contract file hashes and both commands:

```text
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 1 -p concert-application --test age_verification
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 1 -p concert-service --test age_claims --no-run --message-format=json
```

Both exited `0`; application tests: **10 passed**. The retained owned target was
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`, reused only after
Cargo/target-handle clearance. Rust 1.88, explicit retained protoc, jobs `1`,
debug `0`, incremental `0`, locked/offline dependencies, and an 8 GiB disk floor
were enforced. Minimum observed free bytes: `15046385664`. No other processes,
global settings, shared build targets, or live databases were changed.

```text
PYTHONDONTWRITEBYTECODE=1 python3 backend/services/concert-service/tests/age_claims_support/native_smoke.py --artifact-receipt /private/tmp/mcr-be26-age-claims-3810016-20261104/build-7/receipt.json --pg-bin /opt/homebrew/opt/postgresql@16/bin --report /private/tmp/mcr-be26-age-claims-3810016-20261104/native-5.json
```

Exit `0`: **16 native tests passed**, one restart verifier intentionally ignored
in the first phase, then **1 restart verifier passed** after an actual PostgreSQL
stop/start. PostgreSQL `16.14 (Homebrew)` used private Unix sockets, disabled TCP
listeners, random reserved port `57573`, and isolated database
`mcr_concert_be_age_claims`. Postmaster PID changed `25546 -> 26138`, with distinct
start times. Owned root `/private/tmp/mcr-pg-_8dsxrzl` was removed; cleanup passed.

All 13 current central SQL files plus local `schema-v1.sql` applied using
single-transaction `psql`, each exit `0`. This does not exercise a registered
SQLx migration ledger. The native run covers digest-only initiation/replay,
same/different-key callback races, atomic receipt/claim completion, foreign
session/state/proof denial, missing provider/authority, provider failure,
transaction rollback, revocation during exchange, deadline cancellation,
canonical replacement/expiry/revocation, persisted Seoul 100-to-101 birthday
boundary, expiry constraints, private-detail erasure, proof expiry during an
observed owner-lock wait (preserving the prior valid claim and Pending receipt),
and closed-event erasure with revoked-owner denial, plus durable restart reads.
The test named `commit_failure_rolls_back_claim_and_preserves_unknown_command`
injects a pre-insert SQL failure: it proves transaction rollback, not a literal
lost COMMIT acknowledgment or provider-network fault experiment.

| Evidence | SHA-256 |
| --- | --- |
| `build-7/receipt.json` | `04a1bf2b1082693a4dd890455ad077f72f9e38e9361f8065d8b0132ac8cf774c` |
| `build-7/age-claims-tests` | `9ba4f99958c299e52094f20f4fae9745802bb6919e15270759a475d0384ef5b4` |
| `native-5.json` | `359d496e57db6a774ba2b0018338c6572b154b5642412168c46b6608ce38681c` |
| Native harness | `447d670a1d3e7db5720810c5d8cbd3c18ef84463ebea081b4c0b0ca4aabaea07` |
| Unchanged migration helper | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` |
| Local schema | `1c2e786250e6a4e61b26c4232211ef48d2b4cb7a2e51b258c57b697d1e6a46d5` |

`final-binding-v2.json` confirms no drift in all 1,453 source pins or 46 frozen
earlier BE24/BE25 evidence/bundle pins and confirms owned-root removal.
`python3 scripts/mcr/check-plan.py`, owned Rust 1.88 `rustfmt --check`, and
`git diff --check` all exited `0`. The planning check certifies provenance and
planning structure only.

Earlier attempts remain preserved: build 1 app tests passed but service test
compilation exited `101` because a path-included application error derive required
an undeclared service-side `thiserror`; it was replaced with explicit standard
error implementations. Build 2 passed compilation before commit qualification.
Build 3 at `2434ae9` and `native-1.json` passed 12 native tests plus actual restart;
build 4/native 2 add the birthday and retention checks. Targeted peer review then
identified proof expiry during final lock waits and event-phase blocking of
privacy erasure; commit `7304895` fixes both. Native 3 and 4 each had 15 passes and
one failed new cleanup fixture: respectively missing event revision increment
and unsupported `CLOSED` enum. Both failures occurred before cleanup behavior was
exercised; reports remain preserved and owned-root cleanup passed. Commits
`e99d059` and `e9ac29a` correct the fixture to use revision increments and actual
`READ_ONLY`/`ARCHIVED` phases. Final build 7/native 5 qualify those fixes with clean
source and 16 passing native tests plus restart. No failed or superseded
artifact was rewritten into a passing receipt.

## Required integrator work

1. Register the application module in its existing crate root; compose the
   service's `crate::age_verification` alias/import and outbound module. These
   shared-file changes are reserved to the integrator, not implied by test paths.
2. Review/register the isolated schema with the central migration number,
   runtime migration ledger, grants, and deployment lifecycle. Local `SCHEMA_V1`
   is an owned artifact, not a registered root migration.
3. Implement and qualify a production verifier with independent provider proof
   validation and source revision, plus the actual canonical Auth/session/event
   authority fence through commit. Keep defaults unavailable until then.
4. Compose C04 HTTP start/callback/own-status and receipt recovery, including the
   one-shot initiation replay projection, authorization/rate limits, secret-free
   transport diagnostics, and genuine provider/browser flow evidence.
5. Bridge BE25 policy reads only with a valid lease maintained through policy
   use; a timestamp after this SQL transaction ends is insufficient. Qualify
   cleanup scheduling, account deletion, retention, and backup handling separately.

These are delivery integration requirements, not requests for main-chat or
scrum-lead approval and not evidence of end-to-end task acceptance.
