# BE42-AUTH-SOURCE-BRIDGE-01

Status: **IN_PROGRESS**, developer qualification **PASS**; committed handoff
awaits the named publisher. This bridge does not accept BE42, PrivateData
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
| `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p auth-service --test concert_deletion_source --no-run --message-format=json` | 0 | Auth source/pure executable compiled. |
| Same build for `-p concert-service --test deletion_source_rpc` | 0 | Corrected consumer and actual RPC test compiled. |
| Frozen Auth executable, `--test-threads=1 --nocapture` | 0 | 2 pure tests passed. |
| Frozen Concert executable, `--test-threads=1 --nocapture` | 0 | 1 pure test passed; native accurately ignored in this invocation. |
| Frozen Concert executable, `--ignored --exact canonical_auth_deletion_source_rpc --test-threads=1 --nocapture`, `MCR_BE42_SOURCE_PHASE=SOURCE` | 0 | 1 native case passed, 4.13 s test body; actual Auth use cases, source mTLS, negative cases and common Ticket consumer. |
| Identical frozen executable and argv, `MCR_BE42_SOURCE_PHASE=RESTARTED` after physical owned PostgreSQL restart | 0 | 1 native case passed, 0.09 s test body; changed PostgreSQL/test PID, stable source binding and actual receipt replay. |

## Qualified source and resource evidence

Source commit `7da00d747fdca2dc57dfc998c2cc1204a4016f20` contains the eleven-path
bridge. The first Auth build passed; the first Concert build exited 101 because
the specialized client alias applied its identity interceptor twice. The
implementation owner corrected that constructor in
`50a3e0543b47aed6eaa9f8da0ec8019138c1f58c`, within the accepted lease and exact
owned path. Both test executables then compiled and all cases above passed.
The original compiler JSON, diagnostics, source pins and failed receipt remain
in private evidence under `failed-build-1`. No native retry, assertion relaxation
or test-only positive authority was needed. Only documentation changes after
the successful qualification checkpoint.

| Frozen executable | Bytes | SHA256 |
|---|---:|---|
| `concert_deletion_source-tests` | 1,254,704 | `4869acd4723cc3a756b217bdb6d53397e7178c46d3b44c172acb410f81cd4423` |
| `deletion_source_rpc-tests` | 26,646,752 | `3d403679a21fc5b65e7598abe2e7de82ed35432272b6da5527bf4424b2e37316` |

Allocator21d's accepted offer was
`BE42-AUTH-SOURCE-BRIDGE-MEDIA-202611041453Z`, actual acceptance
`2026-11-04T14:54:32.480674Z`. The original compile/freeze cutoff was
15:04:32.480674Z, work cutoff 15:08:32.480674Z and full-release deadline
15:09:32.480674Z. After unconditional target release, the stricter native and
full-release cutoffs became 15:00:41.853875Z and 15:01:41.853875Z respectively.
The original clock was never extended.

The accepted whole cap was 5 GiB, including fresh carry 3,768,156,160 bytes:
the entire checkout, entire shared Git directory, retained evidence/custody,
all historical positive target charges and an additional conservative double
charge for the earlier CF02 peak. Live shared-Git growth remained charged.
Initial free space was 25,424,596,992 bytes, above the 16 GiB start floor;
the 8 GiB runtime floor and 80-process cap held. Rust 1.88, locked/offline,
two jobs, dev/test debug zero, incremental zero, contracts-build dev optimization
two and both Rust flag variables unset were enforced.

The first failed build took 43.936 s and the successful guarded build/freeze
36.301 s. The two copied executables total 27,901,456 bytes, below the 96 MiB
aggregate cap. Automatically produced binaries were charged as target growth;
no main was explicitly built, copied or run. Source/runtime/migration/tool
readsets, Cargo JSON and executable hashes were pinned outside the target.
Fresh absent-compiler and zero-handle proof released the media target at
`2026-11-04T14:57:41.853875Z`, before every copied test and PostgreSQL process.
There was no target access after release.

One owned Unix-socket PG16 cluster created exactly the five
`mcr_canonical_{auth,friend,concert,profile,notification}` databases. Both
SOURCE and RESTARTED verified all 18 Auth and 41 Concert migration receipts
against source/checksums. Friend, Profile and Notification migration execution
is explicitly **NOT_RUN**, not a five-owner schema qualification. Native wrapper
time was 21.469 s. Every database had zero prepared transactions before teardown.
Actual PostgreSQL and test PIDs changed across restart. Auth's eight target
steps stayed pending throughout the actual source and Ticket replay checks.

Private evidence is retained under `mcr-be42-auth-source-_wjw9yve`, including
`checkpoint.json`, `source-pins.json`, both Cargo streams, both native phase
streams, `native-report.json`, `highwater.json`, `target-release.json` and
`full-release.json`. The native report SHA256 is
`27a2c9292b4e6f1eeb10e2629cf88135aa4aa7234803c3ae04fa1e0bf18f401b`.
Whole allocation peaked at 4,161,785,856 bytes, positive target growth at
251,740,160 bytes (retained as historical charge), and new outside allocation
at 135,262,208 bytes, below its 256 MiB cap.

Full release was `2026-11-04T14:59:20.893884Z`, 99.040 s after target release:
owned PostgreSQL/PKI/TMP absent, all saved processes/groups absent and zero
remaining evidence handles. The frozen executables and bounded failure/results
evidence remain retained. No source archive was restored and no new worktree
was created. Actual flag-enabled startup, Root's composition of the three
owning production adapters, downstream business population and full public
deletion/BE42 acceptance remain the respective implementation owners' work.

Delivery ancestry is the published base, excluded local World input,
`7da00d74`, `50a3e054`, then this documentation-only results commit. RootBE
cherry-picks the owner's three commits and obtains the common World dependency
from its responsible owner. Normal master publication and the implementation
owner's remote readback remain pending; no manager inspection or ACK is required.
