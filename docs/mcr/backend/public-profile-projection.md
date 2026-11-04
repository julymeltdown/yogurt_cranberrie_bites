# Canonical public profile projection

Status: **IN_PROGRESS**. The dedicated internal protocol is published at
`493b05cb18052ead6690bdbf000ae1b967b49e4d`, with protocol commit `86cb76e5` in its
verified remote ancestry. The Profile producer is compiled and independently qualified as recorded below.
Concert adapter compilation and joined Feed response qualification remain
**NOT_RUN**. This delivery contains only the qualified Profile producer, its
registrations, tests and controller; the Concert adapter remains on the isolated
`codex/mcr-be-public-profile` branch.
No production capability flag is enabled by this work.

The original assignment base was `8430cd57`; current source incorporated avatar
owner delivery `b13d79e6d9138c7208137cb5912d3db573b25d1d` and reconciled published
master before the protocol push. Existing moderation work was already delivered
separately and is not duplicated here. The historical unfinished moderation goal
remains truthfully incompatible; the public-profile assignment is tracked here
with implementation, transport/native/joined validation, documentation, committed
handoff, non-force master publication, and fixes still required.

## Boundary and contracts

`PublicProfileReader::read(&GuardedSubject) -> Result<PublicAuthor, FeedError>` is
the concrete consumer API under
`concert_service::adapters::out::public_profile`. Its constructor accepts an
isolated Concert pool, `Arc<ValidatedProductRelease>`, Profile endpoint, explicit
Concert service credential, optional TLS material and dedicated
`RelationshipReadSigner`. `policy(&GuardedSubject)` and its `local_subject` alias expose actual event/pair
revisions, phase and valid-until under that same local prepared child. They do not
require public Profile content or a present avatar for the authenticated viewer.
The local child is reused per exact scope-binding/subject and both complete owner
epoch/revision pairs, with actual owner and prepared-state checks on every use.
Root retains the strong child; the reader keeps only weak references and bounded
per-key acquisition slots. Repeated author enrichment reuses held content and
never reserves the same uncommitted local pair twice. No settled DTO or expired
hold acts as authorization.

No identifier-only Feed `Context`, self-profile grant,
World observation, cached allow or default avatar constructs this capability.
The caller must keep the actual root guard through all Post/author/cursor/C04
serialization and final response assembly.

The frozen C03 allowlist is exactly `user_id`, `display_name`, required nullable
`bio`, and `avatar`. Profile contributes only its current canonical UUID,
nickname-as-display-name, bio and observation timestamp. Empty stored bio maps to
null; content is never truncated or replaced. Current private, deleted,
tombstoned, malformed or missing Profile data is unavailable. Exact age,
provider claims, email, social handles/links, legacy avatar URL, counts and private
meeting state are never selected or serialized. The dedicated RPC is unrelated
to `/profile.v1.ConcertWorldProfileService/ReadCurrent` and SELF_PROFILE.

Source pins at protocol checkpoint:

| Input | SHA-256 |
| --- | --- |
| `contracts/generated/mcr-domain.openapi.json` | `ef678d3445e5ee5ed3b90594ef1decc1d53b342166b10a91c4d9c9b3f20aa057` |
| `contracts/assets/v1/avatar-config.schema.json` | `615eba53a0cc4810c7fd1a1c71235a18e6a237bf40f5bd931070b8b1b3c35bb4` |
| `backend/proto/profile/v1/concert_public_profile.proto` | `c2d2827cf30812e8ec1d264f8cc0023bc7adccc5310a347c765627c04903d2bb` |

Concert consumes the existing owner `selection_from_row`, `AvatarCatalog`,
`config_value` and product-art provenance validation. The canonical 1.0 layered /
1.1 whole-character union is retained unchanged. The selected manifest is
`mcr_selected_static_20261103` revision 1, hash
`sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`.
Missing, cross-event, malformed or out-of-catalog selections fail closed. Synthetic
layered schema tests are compatibility evidence, not approved product artwork.

## Current authority and held visibility

The canonical authority owner supplies the sealed `GuardedSubject`, current
Auth/Friend checks and both complete owner epoch/revision pairs. This projection
never collapses that vector into a scalar or substitutes a point-in-time owner
observation. Profile's dedicated signed RPC additionally requires exact Concert
caller, actor, event, subject, session family, request, PUBLIC_PROFILE purpose and
an original bounded deadline. Unknown or wrong caller/key/audience/method/body,
replay, missing original deadline, expired scopes and cancellation fail closed.

`ReadCurrent` reads PUBLIC content while holding the Profile tombstone table in
SHARE mode, then the subject Profile row FOR SHARE. This excludes visibility and
soft/hard deletion updates, and direct concurrent tombstone inserts. Table-before-
row ordering matches the retained deletion saga's tombstone-insert then Profile-
update path; no retained writer or privacy behavior is modified. The hold is
prepared before returning the content. A unique opaque child ID losslessly binds
all five UUIDs and a nonce; a separate random completion key is required to release
it. The PostgreSQL GID stores a domain-separated SHA-256 completion-key digest
using PostgreSQL's built-in binary hash function; the plaintext completion key is
never interpolated into a SQL statement or prepared identifier. `Check` returns only hold state and never accepts that completion key.
`Complete` settles a terminal read using an independently bounded signed control
call, including after the original data-read expiry; it returns no profile data.
It cannot renew an admission or data-read budget.

Concert independently protects its event phase, local normalized relationship
pair, sorted admissions/participants and subject avatar in the same transaction.
Local pair blocking in either direction, suspended/missing admission or participant,
DRAFT/ARCHIVED event, wrong event/subject, missing avatar and invalid revision are
unavailable. A missing pair is reserved as in the owning policy, and its empty
metadata insert is rolled back at terminal settlement. Local locks are prepared
and retained as a second root child through final assembly. No ordinary read
transaction is silently treated as a crash-safe remote fence.

Prepared state survives session/process loss and database restart, so unknown
outcomes retain exclusion until explicit settlement. There is no TTL, Drop cleanup
or automatic expiry release. These are coordinator-managed prepared transactions;
PostgreSQL documents their durable state and retained locks in
[PREPARE TRANSACTION](https://www.postgresql.org/docs/16/sql-prepare-transaction.html).
The original wall and monotonic deadlines still govern all data access and final
checks. Profile read/each control RPC is capped at two seconds; the canonical root
budget is never extended by this producer. SQL statement budgets refuse a zero-
millisecond remainder and are scoped locally to transactions.

Each producer bounds outstanding projection children to 101 transactional advisory
slots, covering 100 targets plus the actor. `max_prepared_transactions >= 256` is required on the actual PostgreSQL host
for a full 100-subject root sharing a cluster with both owners. The setting is
cluster-wide: deployment sizing must additionally cover concurrent roots and other
prepared protocols. The authority owner's 16-root admission cap is separate from
this subject count. Resource exhaustion is unavailable, never a smaller page or
cached permission. Unknown prepared state requires coordinator/operator
reconciliation; long-lived quarantine can block mutations and must not be left
unmanaged. The native controller settles only its own known children before exact
owned-cluster cleanup.

## Paths, ownership and provenance

New Profile owner logic is under `profile-service/src/concert_public_profile/**`
and `src/adapters/in/grpc/concert_public_profile.rs`; tests are
`tests/concert_public_profile.rs` and `tests/support/public_profile/**`.
The dedicated proto is `backend/proto/profile/v1/concert_public_profile.proto`.
New Concert projection logic is only `src/adapters/out/public_profile/**`, its exact
module export and `tests/public_profile_projection.rs`. The controller and its
regression tests are `backend/scripts/mcr/public_profile_projection_smoke.py` and
`test_public_profile_projection_smoke.py`.

The Profile `src/lib.rs`, `build.rs`, `src/main.rs`, and grpc `mod.rs` registrations
have one writer in this assignment. The separate SELF producer supplies new module/
proto/constructor pins for narrow additions to those files. SELF owns its new
`0008_concert_self_profile.sql`; this public producer owns no migration. Concert
build/generation and final root composition stay with the root composer. Auth/
Friend guard, lease proto, platform method policy and exchange registration stay
with their canonical authority owner. No manifest/lockfile, frontend, World,
Feed-owned subtree, avatar implementation or bootstrap code is changed here.

The test transport fixture adapts retained
`profile-service/tests/support/world_identity/mod.rs` at the source base into a new
public-only fixture; existing helpers for explicit native owner identity/seed are
reused read-only. The controller reuses `migration_smoke.Cluster`'s exact-owned
socket/marker/inode checks. Production query and hold logic is newly authored.
Dependencies are retained SQLx, Tokio, tonic/prost, chrono, UUID, async-trait,
serde_json, platform authority and Concert jsonschema/domain libraries. No legacy
rendering or new visual dependency is copied. `docs/source/` remains untouched.

## Validation and remaining gates

Executed before any Cargo lease:

- Dedicated proto descriptor generation: exit 0.
- Explicit Rust 1.88 rustfmt syntax/format pass: exit 0. The unrelated default
  stable rustfmt installation lacked its rustc-driver library; no toolchain repair
  was attempted.
- Controller artifact-selection/source-pin regressions: 5 passed, exit 0.
- `python3 scripts/mcr/check-plan.py`: PASS, exit 0; planning provenance only.
- `git diff --check`: exit 0.
- Protocol non-force master push and remote ancestry verification: exit 0.

On 2026-11-04, a fresh BACKEND-only allocation was explicitly accepted. Fresh
preflight showed 75 GiB available and no target handles. The focused build used
Rust 1.88.0, locked offline dependencies, jobs 2, dev/test debug 0 and incremental
0. It exited 0 with no warnings after correcting three stale generated type
names in the new fixture. Exact Cargo JSON artifacts were frozen outside the
target, and the target was explicitly released with no handles before native
execution. No MEDIA target, installation or global cleanup was used.

```sh
CARGO_TARGET_DIR=<leased-backend-target> CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -p profile-service --test concert_public_profile --no-run --message-format=json
python3 backend/scripts/mcr/public_profile_projection_smoke.py --profile-only --cargo-json <private-cargo-json>
python3 backend/scripts/mcr/public_profile_projection_smoke.py --pinned <private-frozen-directory>/pin.json
```

The compile and native commands exited 0. Native PostgreSQL was 16.14 with
`max_prepared_transactions=256`, socket-only on an exact-owned private cluster,
and current Profile migrations 0001 through 0007. Receipt `mcr-public-profile-frozen-5dtf4gee`
records 1,710 source hashes, migration hashes, command exit codes and log hashes.
Its test source equals checkpoint `507ba93e` (the source at execution was
`332fded9` plus exactly the subsequently committed fixture fix). Producer
publication was assembled from `b417ed3b`, `44ffebf6`, `02268707` and `507ba93e`
on current master so the unrelated uncompiled Concert adapter is excluded.
The retained Profile and dependency source trees, manifests and lockfile were
compared against the qualified checkpoint and are unchanged.

The controller accepts Cargo JSON through `--cargo-json`, selects exact package,
test target and debug-zero artifacts, copies/hashes them into a private directory,
and writes `pin.json` while the target lease is held. The owner then releases the
target and runs `--pinned /absolute/private/path/pin.json`; no Cargo or target access
is used during private socket PostgreSQL execution. Source/artifact drift is a hard
failure. Tests cover signed arbitrary subjects and scope tampering, caller/proof/
replay rejection, private-field denial, cancellation and noncooperative deadlines,
current privacy/tombstones, held visibility/direct-insert exclusion, owner shutdown,
100-target-plus-actor child capacity, lock waits, actual PostgreSQL restart and expired-read terminal
settlement. Successful cleanup must show zero prepared quarantine and removal of
only the exact owned root.

Required remaining work: consume the actual
canonical owner guard and root-generated Profile client; qualify the actual joined
response with event/subject mismatch, current block/restriction/suspension/deletion,
local Concert block/admission changes, both avatar branches and deadline/cancel;
verify final response serialization retains all children; record exact source/
artifact/command evidence and limitations; commit and non-force publish only the
qualified exclusive implementation; verify remote ancestry and resolve fixes.
Production TLS, browser and full multiuser release acceptance are not certified by
unit or loopback owner tests.


### Independent producer receipt

| Check | Actual result | Exit |
| --- | --- | --- |
| Signed arbitrary-subject RPC, malformed owner data, caller/proof/replay/scope tampering, cancellation and original deadline | 8 passed | 0 |
| Native current PUBLIC/private/deleted/tombstoned owner reads and exact minimal wire | 1 passed | 0 |
| Held privacy mutation and direct tombstone-insert exclusion until settlement | 1 passed | 0 |
| Original-budget lock wait and recovery | 1 passed | 0 |
| 100 targets plus actor, fail-closed overflow and recovered capacity | 1 passed | 0 |
| Prepared state before actual PostgreSQL shutdown | 1 passed | 0 |
| State survives restart; expired data read denied; fresh terminal settlement | 1 passed | 0 |
| Exact-owned cleanup | zero prepared transactions; owned server stopped; exact owned root removed | 0 |

All six ignored native cases were explicitly executed by the controller; their
ignored status in the initial transport run is not counted as native evidence.

Frozen executable SHA-256: `459c4c6b2d7870b9874b8eb79390af14ecee77236ddac34aa5a6abf81c0be580`.

Native report SHA-256: `d7693d2dda4b133f48abab0fbc2056924f16a6437f34596a9df5df9c950468ca`.

| Phase | Log SHA-256 |
| --- | --- |
| signed-profile | `58d289b1cbdb11ff6f3b370b37e8471a692ff0c577664f26bcdac108cd55dd0b` |
| native-privacy | `155e903a4c6d06186383f3d9ec150cb2fd83dd378966c2cde8a7dcb87835cf89` |
| native-held-exclusion | `69de5dd2a15e182bac19d5d592d5b32ddcb2ce8c02ae86caa685134530d33edc` |
| native-original-budget | `1f1552f5e08304dd23e69eb650dd6ef5bfd32d4c388166deb221179680094952` |
| native-capacity | `0dc886b5c8de2b4563a437c6f3d2d82a6025684ae28d6025ea1b88681972615f` |
| native-owner-before-restart | `dadad92b659eea5d77dec47d001efa5a2372b13c3c0e8dd9ff6e1765495212bb` |
| native-settlement-after-restart | `1e880cd4e9c93a7fc766aa7ca8f9a12eb2b27f9248a2e700cbd5f4e94fea021f` |

### Recovery increment in progress

The independently qualified producer above is not joined restart acceptance.
A subsequent unpublished increment adds caller-preallocated `child_id` and
`completion_secret` to ReadCurrent. The root must durably register that exact
private intent before dispatch. New owner migration 0009 is exclusively allocated
for minimal child control receipts; no Profile content or plaintext key is stored.
A receipt captures the actual full transaction ID, immutable cluster/database and
role identity before PREPARE. Cancellation before dispatch reserves a terminal
nonce so a late read cannot create a child. Completion accepts only an exact
scope/key receipt and either its durable terminal state or proof that its actual
recorded transaction aborted; missing prepared state alone is insufficient.
PostgreSQL documents [transaction status and its unknown-state limits](https://www.postgresql.org/docs/16/functions-info.html).

The recovery increment's Rust/native execution is NOT_RUN until a fresh target
allocation. Its new tests cover lost read replies, rollback-before-ack loss,
idempotent retries, pre-dispatch cancellation, and active/committed/misbound
receipts. Durable root child registration and Concert restart reconstruction
remain coordinated with the canonical authority and root owners; no automatic
TTL/Drop cleanup or joined capability is enabled.
