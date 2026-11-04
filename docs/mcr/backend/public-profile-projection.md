# Canonical public profile projection

Status: **IN_PROGRESS**. The dedicated internal protocol is published at
`493b05cb18052ead6690bdbf000ae1b967b49e4d`, with protocol commit `86cb76e5` in its
verified remote ancestry. The Profile producer is compiled and independently qualified as recorded below.
The caller-intent recovery increment is independently qualified at source
`52000b758da0fd96ab0a4bccbedb6feb31a0d8c2`. The Concert adapter compiles against
SDK2 and its closed C03 schema tests pass: **CONTRACT_READY**. Its actual local
prepared child, joined canonical Auth/Friend flow and final HTTP response
qualification remain **NOT_RUN**; this is not joined acceptance.
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
`0008_concert_self_profile.sql`; this public producer exclusively owns
`0009_concert_public_profile_children.sql`. Concert
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


### Initial independent producer receipt

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

### Qualified recovery increment and compiled adapter contract

The independently qualified producer above is not joined restart acceptance.
The qualified recovery increment adds caller-preallocated `child_id` and
`completion_secret` to ReadCurrent. The root must durably register that exact
private intent before dispatch. New owner migration 0009 is exclusively allocated
for minimal child control receipts; no Profile content or plaintext key is stored.
A receipt captures the actual full transaction ID, immutable cluster/database and
role identity before PREPARE. Cancellation before dispatch reserves a terminal
nonce so a late read cannot create a child. Completion accepts only an exact
scope/key receipt and either its durable terminal state or proof that its actual
recorded transaction aborted; missing prepared state alone is insufficient.
PostgreSQL documents [transaction status and its unknown-state limits](https://www.postgresql.org/docs/16/functions-info.html).

The recovery increment's signed transport and nine native PostgreSQL phases
passed at `52000b75`, including lost read replies, rollback-before-ack loss,
idempotent retries, pre-dispatch cancellation, and active/committed/misbound
receipts. The original-budget test also recovers an actual expired pre-PREPARE
transaction from its durable receipt. Durable root child registration and Concert restart reconstruction
remain coordinated with the canonical authority and root owners; no automatic
TTL/Drop cleanup or joined capability is enabled.

The compiled consumer increment also adds
`PublicProfileReader::recover_one(&ChildRecoveryReference) -> Result<(), Status>`.
It uses the configured Profile channel or actual Concert pool and returns no
content or admission. PROFILE_PUBLIC preallocates its exact child/key, persists
and attaches that reference before dispatch, and rejects a differently echoed
capability. LOCAL_PUBLIC captures full native identity and persists its exact
nonce-derived reference before PREPARE. Its live checks require actual database
identity and matching prepared XID; idempotent settlement requires that exact
transaction's aborted status. The root authenticates the encrypted saved reference
and records child completion durably only after this proof. Missing, expired or
unknown metadata cannot independently release a child or close a root scope.

| Remaining original assignment criterion | Current state and owner |
| --- | --- |
| Dedicated canonical arbitrary-subject Profile read, minimal content, privacy exclusion, native restart primitive | Independently qualified and published `d4ee53f5` via `b09db61e` |
| Pre-dispatch intents and idempotent Profile owner completion | Independently qualified at `52000b75`: 9 transport and 9 native phases pass; public Profile developer owns joined follow-through |
| Exact C03 assembly, canonical avatar union and shared local policy | CONTRACT_READY at `52000b75`: compile and 3 schema tests pass against SDK2; actual local SQL/held policy and joined response qualification NOT_RUN |
| Durable child intent API and native canonical Auth/Friend producer availability | SDK base `888cb6bd` and additive SDK2 `c4ad561c` consumed. Authority owner published native-qualified stores at `dc70d620`; frozen artifact hashes independently verified, actual public-reader joined run NOT_RUN |
| Root private ledger, HTTP completion, actual joined Feed/C04 response and restart cleanup | Root developer owns ledger/HTTP; public Profile developer validates this adapter in the actual joined flow with consumer owners |
| Final combined validation, documentation, qualified committed publication and remote ancestry | Independent producer and compiled-contract receipt below; publication and actual combined joined validation remain tracked separately, without an additional main-chat or scrum-lead approval stage |

The separately assigned SELF producer registrations are authored in isolated
commit `d88751fd`, consumed by that owner as `e2ca87e0`. The SELF developer
reports production Profile binary and test compilation exit 0 without warnings
at `5c921e28`, plus eight deterministic and three current native PostgreSQL /
migration-CLI / restart cases passing with exact cleanup. A fresh Git comparison
confirmed that all three registration files at that compiled source are byte
identical to `d88751fd`; no registration correction was requested. These are
peer-owned qualification results for the SELF composition, not a run of the
public recovery or Concert tests. The SELF owner subsequently published `4e9c57e3838a8f9b3de30e0962e4d2ff01133937`
with an actual Auth/Friend/Profile MAIN mTLS read/Track/abort probe passing.
Concert decision / root HTTP acceptance remains separate. The public recovery
compile source `52000b75` predates those SELF registrations. The public Profile developer
remains sole writer of the four existing registration paths; no ownership
transfer was implied.


SDK2 `c4ad561cd813f0f7f1e14f21cb2ee90592e09eea` was fetched from actual
master and merged without conflicts. Its actual `HeldChildGuard` reference
method, subject intent/capture methods and required lifecycle hooks match this
reader's source. The authority owner's 40 passing Concert library tests and 202
platform tests qualify that SDK checkpoint; they are not a new run of this
reader's tests. This removes the SDK source prerequisite for the combined
Profile/Concert compile. The fresh allocated two-artifact build below qualified reader compilation.
Joined native acceptance remains NOT_RUN.


### Recovery and adapter qualification receipt (2026-11-04)

Exact compiled and executed source: `52000b758da0fd96ab0a4bccbedb6feb31a0d8c2`.
The fresh named BACKEND allocation had 66 GiB free and no open target handles.
Both builds used Rust 1.88.0, locked offline dependencies, jobs 2, debug 0 and
incremental 0. The initial Concert build exposed an adapter client-constructor
type-inference error (fixed in `52000b75`) and a stale contracts-build output
from another checkout. A scoped `cargo clean -p contracts-build -p auth-service`
in the allocated target removed only those package outputs; no source, global
cache, installation or second target changed. The final Concert build exited 0
in 14.87 seconds with inherited World warnings and no adapter warnings. The final
Profile rebuild after that refresh exited 0 in 6.26 seconds without warnings.

```sh
CARGO_TARGET_DIR=<leased-backend-target> CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -p profile-service --test concert_public_profile --no-run --message-format=json
CARGO_TARGET_DIR=<leased-backend-target> CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -p concert-service --test public_profile_projection --no-run --message-format=json
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/public_profile_projection_smoke.py --cargo-json <profile-json> --cargo-json <concert-json>
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/public_profile_projection_smoke.py --pinned <private-frozen-directory>/pin.json
```

Both exact emitted test executables and 1,747 source hashes were frozen before
the target was explicitly released with no handles. Native execution used only
those copies, after release. PostgreSQL 16.14 was socket-only in an exact-owned
private cluster, `max_prepared_transactions=256`, with current Profile migrations
0001 through 0009. No target was reacquired for the native run.

| Check | Actual result | Exit |
| --- | --- | --- |
| Signed arbitrary-subject transport, exact caller intent, malformed owner data, caller/proof/replay/scope rejection, cancellation and original deadline | 9 passed | 0 |
| Closed C03 fields, required nullable bio and canonical 1.0/1.1 avatar schema/parser/serializer | 3 passed | 0 |
| Native privacy/minimal wire, held mutation exclusion, original budget/pre-PREPARE recovery, 101 children/overflow, pre-dispatch cancellation, lost read/completion replies, missing-state refusal, before-restart persistence, after-restart settlement | 9 individually selected phases passed | 0 |
| Exact-owned cleanup | zero prepared quarantine; owned server stopped; exact owned root removed | 0 |

The nine ignored native cases were explicitly selected and executed. The three
Concert tests execute schema/parser/catalog/serializer behavior; they do not
execute the actual local prepared SQL methods. No actual canonical Auth/Friend
joined public read, root lifetime through final HTTP bytes, production TLS for
this dedicated public RPC or real multiuser Feed/C04 flow is certified here.
The task remains IN_PROGRESS, with no capability enabled.

| Frozen item | SHA-256 |
| --- | --- |
| `pin.json` | `dcfb701e5b135685b0f73422bf197911dc900bd50e1f988ea18707e034af6877` |
| `native-report.json` | `2905ee78298d3b55f1fcc9cde38c2e6195869e5aa82066a4268c441d8ca04e99` |
| `concert_public_profile` executable | `1e65db732850ea31d0e9b312984517b3a517b0b0866aea0ca494f19b1c93369f` |
| `public_profile_projection` executable | `097168f4c3a4522739d7f243a3530e24594f4693e90a5b0f418ebf5195c15690` |
| `0008_concert_self_profile.sql` | `056fe8d0eb15f352fc8cbd6b07f4562ebca937fa857cd03fe129f319716cf544` |
| `0009_concert_public_profile_children.sql` | `756bbfe4cf70d13765f580c28e5ba1bd147ebfd5a8970df4cb1d1829dbe90afa` |

| Phase | Passed | Exit | Log SHA-256 |
| --- | --- | --- | --- |
| signed-profile | 9 | 0 | `7eac03124dc9d8d3c8e7f76e9ef2d4d485c40a855714b20bc08e8f2c1035aa2f` |
| closed-c03-union | 3 | 0 | `98e926bd68eac539cedf395535c6d457fea75256c6b7306b4ab057bb5ee37693` |
| native-privacy | 1 | 0 | `6466e76d05193051803bc4a1fe333a312a024408589dd3adc09617e21dccf1ee` |
| native-held-exclusion | 1 | 0 | `4dac38cfb1a91c27f411fcf60662838f12ae0949044b2086c2f493d86bccb3ae` |
| native-original-budget | 1 | 0 | `121c4a1bb9d16942340d8b0e30327d7b40205dbe58005b16404ba4257e25ad05` |
| native-capacity | 1 | 0 | `ade576dd6ddba84a804098710da2e3a126495f35417fcb7d4c76c479edc8a175` |
| native-pre-dispatch-cancel | 1 | 0 | `3bdb945b006af3cf942606aa20d152a0bc3e86ca5bf64abb9e9c19a01208bb61` |
| native-lost-ack | 1 | 0 | `1349ef601726bc10ca60a80cc218c305f0a19007a2a162452909351daea26b12` |
| native-absence-not-proof | 1 | 0 | `790c898ae82812aca612b5421c9984b2225d9e52d62add8682ecfd5be60d5e7e` |
| native-owner-before-restart | 1 | 0 | `70b55030918478ad5d61e146d5cb303c41be6f33b9ca76f489e4f3715d9d16a3` |
| native-settlement-after-restart | 1 | 0 | `90c4a41b7e91de232cd93374b1e7f8b002455f4684155d6a7d7b43529d87d0c8` |
