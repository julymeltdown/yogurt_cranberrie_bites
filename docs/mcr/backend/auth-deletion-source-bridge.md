# BE42-AUTH-SOURCE-BRIDGE-01

Status: **IN_PROGRESS**. Source is authored; compilation, native qualification
and publication are **NOT_RUN**. This bridge does not accept BE42, PrivateData
erasure or the unfinished canonical-authority goal.

## Ownership and provenance

Owner207 uses the existing isolated worktree on
`codex/mcr-be-BE42-auth-source-bridge`, based on published
`3ce5ceda75c0df8b372c51c3b9d3d53ad2717cba`. RootBE transferred exactly eleven
paths: the new `backend/proto/authority/v1/deletion_source.proto`; the Auth and
Concert `build.rs` generation hunks; Auth's new
`src/adapters/in/grpc/concert_deletion_source.rs`, its module export and guarded
`src/main.rs` mount; Auth's `tests/concert_deletion_source.rs`; Concert's new
`src/adapters/out/relationship_authority/deletion_source.rs`, its module export
and `tests/deletion_source_rpc.rs`; and this document. No other manifests,
lockfiles, SQL, workers, business leaves, frontend or shared composition changes
are owned here. RootBE is the publishing developer and owns runtime composition.
The implementation owner performs validation and fixes; no king/SL/QA review,
approval or acknowledgment stage applies.

The actual existing `BE-CANONICAL-OWNER-AUTHORITY` goal is aligned with this
Auth-owned provenance and authenticated consumer assignment. Its historical
`BLOCKED` metadata remains unchanged; it has not been reset, replaced, completed
or given a token budget. Other required acceptance work remains unfinished.

All production bridge code and the additive protocol are newly authored.
Existing dependencies are platform mTLS/caller/method/deadline guards, Tonic,
Prost, SQLx, Chrono, SHA256, Tokio, UUID and Serde; no dependency changes.
Identity helper logic in the Concert test derives from
`backend/services/concert-service/tests/auth_deletion_owner_ordering.rs` at
`7490f15e6ac78606c2a2821610a26e4e740d5a1d`, SHA256
`0751e2c53533b1b352c4f121e02d38e6f167f683441e74602b6177cc3037793c`.
Its destination is the new `tests/deletion_source_rpc.rs`. The existing
`canonical_owner_composition_support/environment.rs` is used read-only for
owned-store/PKI checks, SHA256
`7d1b8a3a51be7c63a6aaeaa0a82a25dbb62f9e7ae46737a89ea0905166dd49f6`;
its SQL-seeded `identities` helper is never called.

The explicitly imported local input commit
`0b939a5aa9c89ddf8b4295b1bfe0166234804124` is not this delivery. It contains
World's `src/world/account_deletion.rs`, migration
`0052_world_account_deletion.sql` and the single module export, copied unchanged
from World source revision `9bbd33e9`. Their respective SHA256 values are
`9682fce1433f9de0e2a3af52f22d69ced6eeeea8bafb51e65cd68711e69902aa`
and `214e7db3cd1c506d810fd768a247bf85f777f08fb03232a509f38fb02ebd9b52`.
The publisher takes the World owner's delivery separately and cherry-picks only
this owner's eleven-path commits. No World SQL or implementation is authored,
validated as a full leaf, or transferred by this bridge assignment.
`docs/source/` remains byte for byte unchanged.

## Source contract and trust boundary

`authority.deletion.v1.AuthDeletionSourceService/VerifySource` confirms only
canonical deletion provenance. It returns no cleanup success, worker ACK,
receipt or completion. Its request names the exact saga, user, actor, target
and retention seconds/nanoseconds, a fresh sixteen-byte nonce and the original
absolute call expiry. The response echoes these exactly and returns a stable
source binding. Target values are Tickets, Sessions, Sharing and PrivateData;
unspecified/unknown targets and malformed/noncanonical UUIDs are refused.

The Auth endpoint requires a CA-validated Concert workload certificate, its
registered credential and the default-deny Concert-only method policy. It
requires actual peer certificate extensions even at the handler boundary.
Background work supplies no fabricated Gateway actor delegation. Mounting is
explicitly opt-in with `AUTH_CONCERT_DELETION_SOURCE_ENABLED=true`; missing or
`false` leaves it unmounted. Opt-in requires mTLS material and the configured
Concert credential registry, including existing rotation support. Malformed
configuration fails startup. Root owns enabling and composing the consumers.

Within one bounded repeatable-read transaction, Auth checks its real saga,
matching target step, deleted user, revocation of every refresh token,
tombstoned identities and Auth0017 security generation. It corroborates the
immutable source tuple with the canonical successful deletion-request audit:
same subject/actor, saga ID, creation timestamp and retention. Audit JSON retains
the original nanoseconds; Rust compares its timestamp at SQLx's PostgreSQL
microsecond storage precision. The requested tuple must nevertheless equal the
actual stored tuple exactly, so a one-nanosecond request change is denied.
The source SELECT locks the saga/user/step for the transaction. It performs no
data mutation and never advances a step. Unsupported historical or synthetic
sagas lacking canonical audit provenance fail closed. This protocol does not
claim protection against privileged rewriting of the entire Auth database.

SHA256 binds the canonical command, original audit identity and original
creation timestamp. Nonces, expiry and worker attempt/status are excluded so
the source binding remains stable across repeated checks and process/database
restart. The hash is an identity value, never portable authorization.
Error responses contain fixed text and no subject IDs, credentials or SQL.
Responses have `cache-control: no-store`; messages are limited to 2048 bytes.

The Concert `AuthDeletionSource` implements the existing World
`DeletionAuthority`. Construction requires HTTPS with Auth peer verification
and the Concert identity interceptor. There is no plaintext, local-positive,
cache or retry path. Only a successful authenticated response with the exact
tuple, nonce, expiry, nonzero 32-byte binding and no-store metadata can create
`VerifiedDeletionSource`. It preserves the caller's original monotonic deadline
unchanged. Parent deadlines may shorten I/O; neither service renews the budget.
Pool acquisition, lock waits, queries and response production share the
original bounded clock, maximum five seconds. Expired or failed checks cannot
produce authority. The source bridge supplies all four target proofs but does
not implement their business cleanup or terminal Auth acknowledgment.

## Developer qualification

The new Auth test contains two pure policy/wire-shape tests. The new Concert
test contains one plaintext-constructor negative and the ignored real-store
`canonical_auth_deletion_source_rpc`. The native test requires an explicitly
owned PG16 Unix-socket cluster and PKI. It never attaches ambient databases.

`MCR_BE42_SOURCE_PHASE=SOURCE` creates and verifies actual Auth accounts and
logs in through the production use cases, then performs canonical deletion.
No SQL-positive user, credential, claim, session or source proof is seeded.
Real mTLS calls check all four exact target proofs, repeated source stability,
changed tuple/nanosecond/live-user refusal, caller/credential/certificate
refusal, unknown/missing commands, original deadline expiry and a real blocked
source row. Negative-only corruption of an existing saga's retention must not
rebind its original user/audit facts. The actual common Ticket deletion adapter
consumes the real authority; all eight Auth steps remain unchanged.

The same frozen executable runs `RESTARTED` after an actual owned PostgreSQL
stop/start and in a different test process. It requires a changed postmaster
start time, the identical canonical source binding, actual Ticket receipt replay
and eight still-pending Auth steps. Its bounded private handoff contains the
command/binding and process/start identities, never tokens or a source archive.
Auth and Concert migration receipts and zero prepared transactions are checked;
unused Friend/Profile/Notification databases are **NOT_RUN**. This is source
qualification with an empty-ticket consumer, not complete ticket population,
session disconnection, sharing removal, PrivateData erasure, worker mounting or
the eight-target release flow.

| Command | Exit | Current result |
|---|---:|---|
| `rustfmt +1.88.0 --edition 2021 --config skip_children=true --check` on the four new Rust files | 0 | Source formatting only. |
| `git diff --check` | 0 | Whitespace only. |
| `python3 scripts/mcr/check-plan.py` | 0 | PASS; planning provenance only. All 15 source originals also match the base byte for byte. |
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --test concert_deletion_source --no-run --message-format=json` | NOT_RUN | Requires allocator OFFER and ACCEPT. |
| Same build for `-p concert-service --test deletion_source_rpc` | NOT_RUN | Requires the same bounded allocation. |
| Frozen Auth and Concert pure tests | NOT_RUN | Run after target release. |
| Frozen Concert native test, `--ignored --exact canonical_auth_deletion_source_rpc --test-threads=1 --nocapture`, phases `SOURCE`, then `RESTARTED` | NOT_RUN | Actual store/RPC/restart qualification remains open. |

One bounded validation round is requested from allocator21d. No Cargo, PG,
native execution, archive restore or new worktree may start on an expired or
unaccepted offer. Evidence will record source and binary pins, commands and
exit codes, runtime limitations, target/full release and published ancestry.
