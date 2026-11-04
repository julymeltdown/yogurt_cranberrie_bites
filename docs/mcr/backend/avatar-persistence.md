# BE07 avatar and preference persistence

Bounded persistence handoff: **implemented and native-tested**. Overall BE07
remains **IN_PROGRESS** until delivery composes verified production mutation
and read producers. This is not BE07/IN01 acceptance or artwork qualification.

Base: `1db1680e6a5041250b79ca0ffb1dfd965a279edc`, isolated branch
`codex/mcr-be-avatar-persistence`. Contract sources: C03
`contracts/core/v1/{core.schema.json,operations.json}` and C06
`contracts/assets/v1/{avatar-config,avatar-manifest,common}.schema.json` at this
base. The specification's BE07 card and sections 5.2–5.3 remain authoritative.


Pinned contract SHA-256 values:

| Source | SHA-256 |
|---|---|
| C03 core.schema.json | `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88` |
| C03 operations.json | `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| C06 avatar-config.schema.json | `37b9be22cff7cbde882ab144aec3b75b5edc24f70653f97c9dcec14e87c652bd` |
| C06 avatar-manifest.schema.json | `cb98594db269ef017545806b368d5edd1d0a47b53819809392f8ac57cf30da98` |
| C06 common.schema.json | `b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4` |

## Behavior and integration API

Newly authored domain code under `backend/crates/concert-domain/src/avatar/`
parses exactly `SetAvatarRequest {expected_revision,config}`. Inputs have a
4096-byte bound; revisions are positive JavaScript-safe integers. Duplicate or
unknown properties, missing nullable accessory, URLs, unsupported contract tags,
noninteger revisions and invalid IDs are refused. The immutable C06 inventory
checks exact manifest/version, item layer and skin tone. C06 v1 has no additional
pairwise-combination/entitlement fields; no invented combination or source-sheet
inventory is added. Compatibility means the five frozen slot categories and
shared manifest. Additional restrictions require a reviewed contract change.

`AvatarPersistence` in
`backend/services/concert-service/src/adapters/out/repository/avatar.rs` exposes:

- `from_product(PgPool, Arc<ValidatedProductRelease>)`: projects the existing
  validated release, recomputes avatar content SHA-256 using C06 canonical bytes,
  and refuses synthetic/unreviewed/unprovided metadata in product mode.
- `from_synthetic_test(..., true)`: explicitly marked metadata test seam, never a
  product fallback. It uses the same strict inventory and persistence path.
- `read(key, authority) -> AvatarSnapshot {avatar,participant}`: scoped persisted
  state for the producer's canonical Profile join. It creates no defaults.
- `set_avatar(key, raw_bytes, authority) -> JSON AvatarState {revision,config}`:
  a successful update increments only avatar revision, exactly once. The result
  config is exactly the submitted validated selection.
- `set_preferences(key, raw_bytes, authority) -> AvatarSnapshot`: accepts the
  preference-only C03 PATCH subset with all four preference fields. It advances
  only participant revision and never rewrites avatar identity or revision.

Profile `display_name`/`bio` writes remain canonical Profile owner work. This
module must not be installed as an adapter that silently ignores other valid
UpdateParticipantRequest fields. Delivery must explicitly compose/dispatch that
wider operation and construct the complete PrivateParticipant response from
current canonical Profile data; AvatarSnapshot is internal, not a wire DTO.

Application ports live under
`backend/crates/concert-application/src/avatar/`. `AvatarAuthority::check` must
come from a server adapter validating canonical actor, exact user/event/operation
and HTTP target, current release digest, live bounded authority, and current
server mutation capability. It returns a matching AvatarGrant or a refusal.
Missing, mismatched, expired or unavailable authority fails closed. A client's
capability flag, Bootstrap read proof or test grant cannot establish product
write authority. Production authority/root HTTP/RPC wiring belongs to delivery;
this patch exposes no new route and enables no mutation capability. Product
resource byte/rights/selected-source qualification remains the release owner's
obligation; metadata/hash validation does not establish it.
Realtime avatar-change publication and the real two-account IN01 flow remain
separate composition/qualification work; this module emits no world delta.

## Transaction and error rules

Every transaction locks `concert_events` then `concert_event_admissions`,
`concert_participants`, and `concert_avatar_settings`, matching BE06. Authority
and current release are checked before work, after every await and immediately
before commit/response. Draft/missing events and missing/suspended admissions
are refused. OPEN/LIVE allow writes; READ_ONLY/ARCHIVED permit scoped reads but
refuse mutations. The current selection and resulting selection must both
validate against the captured release. No unavailable item is silently replaced.

Expected revisions are compared under these locks. Competing identical expected
revisions produce exactly one commit and one increment; losers cannot overwrite
current state. Existing database guards independently enforce immutable scope,
creation time and one-step revision advancement. No migration is needed: BE05's
0001 values and BE06's 0004 admissions are used without rewriting them.

A check failure before commit rolls back the SQL update. A failure after commit
is an uncertain response and the client must reconcile by reading current state,
not replay automatically. The local check does not promise instantaneous remote
revocation beyond the upstream authority's bounded observation semantics.

Producer error mapping uses existing frozen C03 reasons:

| Internal error | HTTP / CoreErrorDetails.reason |
|---|---|
| InvalidInput (including catalog/version/item incompatibility) | 422 / INVALID_INPUT |
| Unauthenticated | 401 / AUTHENTICATION_REQUIRED |
| Denied | 403 / PERMISSION_DENIED |
| Missing | 404 / RESOURCE_UNAVAILABLE |
| Conflict | 409 / VERSION_CONFLICT |
| ReadOnly | 409 / EVENT_READ_ONLY |
| Unavailable | 503 / DEPENDENCY_UNAVAILABLE |
| InvalidState | 500 / INTERNAL_ERROR |

Root transport owns media type/body-limit normalization and safe error envelopes.
No private grant, SQL detail, item list, user identifier or fixture provenance
should be included in errors. FE13 preserves the draft on conflict and explicitly
reads GET /me, then asks for a keep-draft/use-server decision before retrying.

## Ownership, provenance and evidence

All avatar implementation/tests and `avatar_smoke.py` are newly authored.
Existing BE05 values, BE06 validated-release and native fixture helpers, and C06
canonical JSON encoding are dependencies, not copied old rendering/code.
`avatar_smoke.py` imports the existing owned native Cluster and bootstrap test
execution helpers. New support/avatar delegates to the existing bootstrap
marker/inode/socket validation, avoiding a weaker duplicate harness.
No frontend implementation, source art, docs/source archive, contracts,
manifests, lockfiles, config, migrations or RPC roots are modified. Delivery
explicitly granted only the three `pub mod avatar;` exports in domain lib,
application lib and service repository. Only delivery integrates/pushes master.

The native harness requires compiler-artifact JSON from the exact service avatar
test target, creates a private PG16.14 cluster/database via Unix sockets, applies
service schema SQL, executes real adapter tests, restarts its postmaster and
verifies persisted values, then cleans only its marker/inode-owned root. Direct
schema application does not qualify service migration-runner/ledger behavior.

Rust implementation ancestry is `1db1680` -> `c321acd` ->
`01670bdadb3827727dbc415b87de3e81d1d90809`. The final documentation/harness
handoff commit descends directly from `01670bd` and is reported to delivery. Build phases use only the leased shared target, Rust 1.88.0,
two jobs, debug=0 and `--locked`; no duplicate Cargo target is created.


## Validation commands

The requested leased compile phase is:

```sh
env RUSTUP_TOOLCHAIN=1.88.0 \
  CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
    -p concert-domain -p concert-application -p concert-service \
    --test avatar --no-run --message-format=json
```

Run only the exact emitted test executables for the three packages. The service
avatar executable has explicit ignored native tests; execute them through:

```sh
python3 backend/scripts/mcr/avatar_smoke.py \
  --build-metadata /absolute/path/to/compiler-artifacts.jsonl \
  --pinned-binaries /absolute/path/to/pinned-binaries.json \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin
```

Executed from the assigned isolated worktree on 2026-11-04 (KST):

| Check | Exit / result |
|---|---|
| `python3 scripts/mcr/check-plan.py` | 0 / PASS, planning only |
| Scoped Rust 1.88 `rustfmt --check --edition 2021` on seven owned Rust files | 0 / PASS |
| `python3 -m py_compile backend/scripts/mcr/avatar_smoke.py` | 0 / PASS |
| Changed-path/export/import ownership audit | 0 / exact 12 paths, three exports, no frontend source imports |
| Leased Cargo command above, final invocation | 0 / all three exact avatar test targets built |
| Copied `concert-domain-avatar-tests` | 0 / 2 passed |
| Copied `concert-application-avatar-tests` | 0 / 2 passed |
| Copied `concert-service-avatar-tests` | 0 / 2 passed, 2 native tests explicitly deferred to owned harness |
| `python3 backend/scripts/mcr/avatar_smoke.py --build-metadata /tmp/mcr-avatar-01670bd/compiler-artifacts.jsonl --pinned-binaries /tmp/mcr-avatar-01670bd/pinned-binaries.json --pg-bin /opt/homebrew/opt/postgresql@16/bin` | 0 / both deferred native tests passed; restart and cleanup PASS |
| `git diff --check` | 0 / PASS |

The initial Cargo attempt exited 101 because the shared target reused a
previous checkout's Concert build-script artifact referencing absent
`post/v1/concert_feed.proto`. Refreshing the local `build.rs` mtime rebuilt it
from unchanged source; final compilation exited 0 in 4.07 seconds. There is no
build-script/proto/manifests content change. All emitted binaries were copied
and hash-verified before the Cargo lease was transferred BE07 -> admission.
Runtime checks use those copies, not whichever binary a later checkout leaves
in the shared target. The harness's explicit pinned-binaries receipt binds
package, manifest, compiler-emitted path, copied path and SHA-256. SIGINT and
SIGTERM enter the same owned cleanup path as failures.

Binary evidence for Rust source `01670bd`:

| Package | Copied executable SHA-256 |
|---|---|
| concert-domain | `07b13925cfb5d92e87887efc479bd8a9a10a9a2fa3e1c8b1d3e3d3598979c1dd` |
| concert-application | `6268089a7708393b425b432ddbb74d1c5e1a7bcfe0dede6e24e0448bf9dad90a` |
| concert-service | `c2ef844c1d1a8483ce8512b9c636e7ab3b19b9792cac2042d62f6e5d66e464f4` |

Native report:

```json
{
  "binary_sha256": "c2ef844c1d1a8483ce8512b9c636e7ab3b19b9792cac2042d62f6e5d66e464f4",
  "cleanup": "PASS",
  "commands": {
    "0001_concert_core.sql": 0,
    "0002_command_receipts_outbox.sql": 0,
    "0003_places_map_releases.sql": 0,
    "0004_bootstrap_admission.sql": 0,
    "native_avatar_restart_persistence": 0,
    "native_avatar_transactions": 0
  },
  "migration_sha384": {
    "1": "bf3d01697c1c6167fd999a92aa4323297f0e9c8b063176e2938f78dec8d1278f06cd84469d583a76314ac8aa520d8a5f",
    "2": "6eae0492a1aedf4b2eaf30b44761a853376623d9cc65cc268941e416882998b40c46ff61f0acb8bd285be33a936f2e6b",
    "3": "cbf152183fd2bddecfdc5c15ffbcb94b70ae86bb94fe28cc5e6acbefd836dd74b4379b6ab1ccc0992217542b4020205a",
    "4": "cb60d4fbd67462ec3f6a899148dbcb6cf6d855171b6c1787f06c9e5274923b58b2fc7298c87353d8f513e85bf571d8dd"
  },
  "native_tests": 2,
  "postgres_version": "16.14 (Homebrew)",
  "response_sha256": "e812465b184e29adb802eeb92576c339f9e6a422147d3d76d04b9dfb2dca2088",
  "restart": {
    "after_pid": 8275,
    "before_pid": 8130
  },
  "source_commit": "01670bdadb3827727dbc415b87de3e81d1d90809",
  "state": "PASS"
}
```

The native transaction test exercises 16 competing avatar saves and 12 competing
preference saves, exactly one winner in each revision race; authority expiry
immediately after each SQL update with full rollback; expiry after commit with
explicit uncertain outcome; invalid layer; foreign actor/event; stale release;
wrong operation; missing admission; closure while waiting for the event lock;
READ_ONLY/ARCHIVED/DRAFT refusal; suspension; unchanged other-user outfit;
unchanged avatar across preference/theme writes; and exact persisted avatar and
preference revisions across postmaster restart. These are real native database
checks over synthetic metadata, not production Auth, artwork or multiuser-world
acceptance. No live HTTP/RPC mutation, current mutation-authority adapter,
real-provider/device/load flow, approved art or IN01/IN05 gate is certified.

## Selected whole-character canonical increment (2026-11-04)

Contract source `3398d8331863775ecc0180e5eccebddd179fedd5`
adds the exact C06 `1.1.0` whole-character branch; layered `1.0.0` remains supported.
The new domain `AvatarAppearance` representation is exclusive. `SetAvatar::parse`,
`config_value` and `AvatarCatalog` retain their call signatures; `parse_config`
provides the same strict decoder to release projection. Six selected presets have
no component IDs, skin tones, URLs or frame choices. Manifest pin:
`mcr_selected_static_20261103` / `1` /
`sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`.

Additive migration `0025_whole_character_avatar.sql` retains old migration bytes,
keys, revision/timestamp triggers and participant foreign keys. It adds
`representation` and `preset_id` to `public.concert_avatar_settings`, relaxes
component-column nullability, and enforces mutually exclusive variant checks.
Layered rows retain required components; whole rows require the six-preset
whitelist and all component columns NULL. Canonical avatar/bootstrap/core reads
share `selection_from_row`; writes/default insertion project only the selected
variant. Unknown or corrupt persisted values fail closed.

The explicitly granted scope extension is only `CoreRepository::avatar` in
`src/adapters/out/repository.rs`, necessary to decode saved variants. World
serialization/SQL and associated tests belong to the World owner; this owner
must not modify those files. Participant producer uses the unchanged parser and
serializer interfaces. `tests/support/bootstrap::selected_release()` reads
`release-selected` inside the private native harness; Python
`avatar_smoke.write_selected_release(root, document)` creates its fixture map/theme
bindings without changing the golden inputs. This is metadata/native evidence,
not full approved product scenery or a public multiuser acceptance claim.

Initial focused Rust 1.88 validation passed: domain avatar 3, application avatar
and bootstrap tests (see `/tmp/mcr-selected-domain-application.log`). Locked,
offline, two build jobs, debug 0, incremental 0, existing MEDIA target only.
Initial service builds exposed stale shared-target build scripts/helper artifacts
pointing at the previous participant checkout. Refreshing current checkout mtimes
(no source byte changes) rebuilt those inputs. The World owner supplied the
representation compatibility patch and affected test adaptations separately.
No default database was touched.

The goal API still reports the historical incompatible BE04 goal as `BLOCKED`;
no new goal was created or the old goal falsely completed. The concrete new
objective/completion criteria remain in `docs/mcr/contracts/selected-static-avatar.md`.

### Fresh canonical component qualification

Tested source: `71e9c6d4242329f1f6d9e3588ce2316e83964331`, including canonical
implementation `86123961`, import correction `3621c04a`, upgrade proof `b297b9a6`,
bootstrap binding tests `7dbd8d5b`/fixture correction `61b42bd2`, and mechanically
adapted PostgreSQL assertion `4bd9a7de`. World production patch `d9e9a00a`
was supplied by its owner and incorporated as `b6a6a210`; nine World test/helper
adaptations `e28549a3` were incorporated as `71e9c6d4`. Their new whole-character
oracle fixture initially used invalid synthetic preset IDs; the owner corrected
it in `48335f67` (incorporated as `b37ae80a`) to canonical preset IDs and matching
room/simulation pins. The contract whitelist was retained.

Validation commands and actual exits:

- Rust 1.88, `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`,
  `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, dev/test debug 0:
  `cargo +1.88.0 test --locked --offline -p concert-domain --test avatar -p concert-application --test bootstrap`:
  exit 0; domain avatar 3, application avatar 2 and bootstrap 6 passed.
- `cargo +1.88.0 test --locked --offline -p concert-service --test avatar --test bootstrap --test postgres --lib --no-run --message-format=json`:
  exit 0. Five affected World integration targets also compiled, exit 0.
- Copied service avatar executable: 2 nonnative passed, 4 native ignored until the
  private harness. Copied bootstrap executable: 5 nonnative passed, 2 native
  ignored. Copied service library with `bootstrap::` filter: 2 passed.
- `python3 backend/scripts/mcr/avatar_smoke.py --build-metadata /tmp/mcr-selected-71e9c6d4-eqrqg2cu/compiler-artifacts.jsonl --pinned-binaries /tmp/mcr-selected-71e9c6d4-eqrqg2cu/binary.json --pg-bin /opt/homebrew/opt/postgresql@16/bin`:
  exit 0; PostgreSQL 16.14, 4 native cases passed, existing-layered upgrade passed,
  restart passed and cleanup passed.
- Pinned contract environment `/tmp/mcr-fe14-format-venv/bin/python` selected-static
  contract tests: 7 passed, exit 0. The generated selected native bootstrap also
  passed authoritative Bootstrap/C06 validation. Core/domain generator checks,
  planning check and `git diff --check`: exit 0.

Exact copied avatar executable SHA256:
`6c7dca5c5d8118db3dda581273a6553eb46276c94100ea3ab8c04f4b8ea83697`.
Bootstrap executable SHA256:
`8fa1269a3251b788ea999c36418a7b0687f606c50939766259e4b6a8132b1f56`.
Compiler receipts, frozen executables and native report are under
`/tmp/mcr-selected-71e9c6d4-eqrqg2cu/` (local evidence, not a deployment artifact).

The real native checks execute the service persistence and bootstrap repositories,
not only SQL fixtures. They prove all six canonical selections, reload and repeated
bootstrap preservation; unknown presets, wrong manifests/versions, mixed fields,
foreign actor/event/release grants and stale revisions fail closed. An authority
withdrawal after SQL UPDATE rolls back. Sixteen competing same-revision writes
produce one winner. Preference updates preserve the avatar. A write waiting on
event closure returns read-only without changing state. Database constraints
independently reject mixed/unknown representations. Postmaster restart preserves
both layered and whole-character state. Migration 0025 preserves the preexisting
layered row including revision, timestamps and every old column.

The private cluster was removed; postmasters 98340 and 98799 were gone after
cleanup. Exact compiler binaries were copied and hashed before native execution,
with no open handles on the MEDIA target. MEDIA was unconditionally returned to
the participant owner afterwards; this chat holds no target or PostgreSQL resource.

Limits: authority/admission are explicit test adapters, and map/themes remain
synthetic metadata fixtures. This does not certify real Auth/participant RPC,
public root HTTP, browser rendering, or a real multiuser World flow. Those owner
journeys remain `NOT_RUN` here. Full AR03 directional animation and layered
customization are unfinished; front-idle selected art is the only new capability.
There is no live-data migration or replacement of stale persisted manifests with
a guessed default. Layered and whole-character releases both retain strict
catalog binding.

Exact additional mechanical test scope was granted for
`backend/services/concert-service/tests/postgres.rs`: only the old nullable
accessory assertion now explicitly matches the layered variant. Other active
feature tests remain with their owners. `docs/source/` and existing layered golden
fixture bytes were verified unchanged. No frontend, Cargo manifest/lockfile,
proto, root router or infrastructure changes were authored in this increment.

### World owner compatibility qualification

The implementation owner subsequently qualified corrected source `48335f67`
(equivalent to the incorporated fixture correction `b37ae80a`): affected build
exit 0, 102 pure tests passed (library 30, runtime 28, WebSocket 16, recovery 15,
replication 12 and invalidation 1). The selected snapshot/delta test passed the
actual pinned C05 oracle. Exact World replication executable SHA256:
`4bfe08e80d6ba652c8b2683073b435dd5742cd2c204eb3c7af5a6384d089014f`.
Their copied-binary native reruns also passed 3 READ_ONLY and 6 invalidation cases,
with cleanup passing. Owner evidence:
`/private/tmp/mcr-world-invalidation-20261104/avatar-world-build2-frozen/world_replication-pure.log`.
These are the World owner's compatibility/component results, not a new claim of
public root, browser, or full multiuser product acceptance. This publishing owner
retained the already qualified canonical source bytes through peer reconciliation;
the remaining differences from tested `71e9c6d4` are the separately qualified World
fixture correction and documentation/peer changes.

## Subsequent actual-transaction effect fence (source checkpoint)

The canonical owner retains exclusive ownership of the application avatar API,
repository/avatar.rs and tests/avatar.rs for this bounded hook, agreed directly
with Participant209; Authority207 confirms no competing writer. Spatial release
validation remains the priority. Rust compilation/native qualification of this
hook is NOT_RUN at this checkpoint, with no target or database held.

`AvatarTransactionIdentity` contains the actual PostgreSQL system identifier,
database OID, full transaction ID, backend PID and backend start timestamp.
`AvatarEffectFence` exposes async `register(identity)` and `check()`;
`AvatarAuthority::effect_fence()` defaults to None for compatibility. The product
constructor and public `PgAvatarRepository::new` require a fence for writes.
Only the existing explicit `from_synthetic_test(..., true)` construction permits
legacy fixture writes without a fence; reads may omit it.

The repository captures identity through the same mutable SQLx transaction
before protected effects, awaits registration, rechecks synchronous authority,
and awaits fence checks after protected waits and immediately before commit.
It never completes/releases a root scope or claims an uncertain commit settled.
The participant/root adapter must bind the exact operation, request body, CAS,
actor/event and immutable release, hold the actual Auth/Friend scope through
commit, and quarantine uncertain results until actual PostgreSQL settlement.
`pg_control_system()` permission is required; failure remains unavailable.
The external 207 SDK and participant adapter remain separately unqualified;
this hook alone is not a live distributed-authority implementation.

The source adds component checks to the existing selected-avatar native case.
A trigger captures the actual UPDATE transaction identity; it must match the
registered identity for avatar and preference writes. Missing registration,
registration rejection and post-effect fence rejection must preserve state and
roll back the trigger evidence. This is developer-native component evidence,
not an invented external owner-grant or live multiuser acceptance claim.

### Effect-fence native qualification

The hook and legacy-checkpoint correction are now component-qualified at exact
source `6c700f040888cc40592444edf6772e784395f156`. Rust1.88 locked/offline jobs2
no-debug/no-incremental compilation passed. Service avatar2 and application
avatar2 pure tests passed. The existing avatar harness ran all4 native cases
successfully on PostgreSQL16.14, including the selected case's actual effect-fence
trigger checks: strict default missing fence, rejected registration, post-UPDATE
rejection/rollback and exact actual transaction identity for avatar/preference
success. Existing layered upgrade and legacy uncertain-commit behavior passed.
Postmaster restart15841→16244 and private cluster cleanup passed.

Command: `python3 backend/scripts/mcr/avatar_smoke.py --build-metadata /tmp/mcr-spatial-6c700f04/compiler-artifacts.jsonl --pinned-binaries /tmp/mcr-spatial-6c700f04/binary.json --pg-bin /opt/homebrew/opt/postgresql@16/bin` (exit0).
Frozen avatar executable SHA256:
`4916cbe90812662114917555452d56b863329b20d4df1c8bb2266a956c456e0a`.
Report `/tmp/mcr-spatial-6c700f04/avatar-native.json`; source/binary receipt
`/tmp/mcr-spatial-6c700f04/binary.json`. BACKEND was released after freezing;
no target/native cluster remains held. Actual external 207 SDK, participant
adapter, root-held distributed guard and unknown-settlement quarantine remain
separate owner obligations. No live authority or multiuser acceptance is claimed.

## Successful-commit witness follow-up — source implementation

Base `0a8c422645458c9397af9289e15089ee3f9cb0ad`; isolated branch
`codex/mcr-be-avatar-witness`. The previously qualified actual-transaction
registration hook does not by itself prove successful avatar/preference commit.
An uncertain raw-commit outcome therefore correctly remains Unknown and must
retain the owner scopes until an actual durable outcome can be established.

Product API root assigned this owner the existing application AvatarEffectFence
trait/DTO, avatar repository hook, existing service avatar tests and this document.
Participant209 exclusively owns new migration0035, its sealed
BoundParticipantAuthority descriptor and participant witness/settlement resolver.
Neither side acquires the other's files. A new unbudgeted goal is tool-confirmed
active for this follow-up; the separate spatial goal was fully delivered.

The coordinated proposed ABI is a non-authoritative witness descriptor containing
producer-instance/scope/operation/session-family UUIDs, exact nonzero 32-byte
parent/guard/body/effect binding
digests and the exact existing 64-character lowercase release digest. A synchronous
`witness_binding` fence method defaults to Unavailable. Missing production witness
metadata cannot certify commit. Read-only and explicitly synthetic unfenced
fixtures remain distinct. The repository will attach its already captured actual
PostgreSQL transaction identity, real actor/event, mutation kind and expected/next
revision, and insert the immutable witness inside the effect transaction before
commit. No plaintext profile/request payload, external boolean or new authority
scope is proposed. The existing repository advances revision for every successful
mutation, including equal values; there is no mutation no-op branch to waive.

Exact schema/insert columns and mutation tokens are being frozen directly with
Participant209 before repository SQL edits. The matching proposed application ABI
now has a source draft: typed descriptor, redacted Debug, exact release/nil UUID
validation and default Unavailable fence method, plus a regression for missing or
malformed metadata. Formatting/diff checks pass; compilation and native checks
remain NOT_RUN for this follow-up. The repository now has the exact frozen
witness insert and native regressions in source; the separate joined commit/abort
qualification and full Product/participant admission remain open.


### Frozen 0035 contract and authored hook

Participant209 froze migration0035 in `48b3ec737e209adb6f505afd3f96f84fd1026707`,
SHA256 `113900167fdc2806a9f5126c508b8ae737bcc9ca718ce7dbf408e913d1c3bebc`.
It remains their owned migration; this owner consumes its exact twenty-parameter
INSERT contract without editing the schema. Mutation tokens are AVATAR and
PREFERENCES. The descriptor also carries nonnil producer-instance/scope UUIDs.
Identity digest uses the existing platform `identity_digest` with CONCERT,
the actual captured tuple, exact effect digest, producer instance and **operation
UUID bytes** as settlement reference. Scope UUID is joined separately and is not
substituted into that digest recipe.

The repository obtains the descriptor once after actual transaction registration,
validates original release/IDs/digests before effects, then inserts exact actor,
event, scope/request/effect bindings, native tuple and expected/next revision in
the same SQLx transaction after UPDATE. Fence checks surround the insert and
precede commit. Exactly one inserted row is required; no conflict shortcut or
external commit boolean is accepted. Reads and synthetic None retain their old
checkpoint behavior. Successful production mutations, including equal values,
require the witness. No request/profile payload is persisted.

Authored native regressions use explicitly synthetic component scope/Track rows
with the real SQL constraints and captured transaction identity; they do not
pretend to be real Auth/Friend registrations. They cover descriptor refusal,
foreign release/body/scope, unpublished or unconfirmed scope, rejection after
UPDATE and after witness insertion, successful avatar/preferences rows,
postcommit uncertainty retaining a durable witness, immutable/cross-transaction
reuse rejection and exact witness restart persistence. Actual sealed
BoundParticipantAuthority/owner-protocol qualification remains Participant209's
separate joined proof. These new native cases remain NOT_RUN at this source
checkpoint, pending coherent migration consumption and a named build lease.

### Frozen-binary controller selection

At base `b7ffb3ce181a09b6413c272b3a570d628b1ec0b3`, the backend resource
coordinator explicitly assigned this owner the sole metadata-selection edit in
`backend/scripts/mcr/avatar_smoke.py`. In pinned mode the original absolute
compiler executable path is now a lexical receipt key; it is never resolved,
statted or executed after the shared target has been released. The frozen copy
still requires an executable-file check and an exact SHA256 match. Default
nonpinned selection retains its existing live executable validation.

A hermetic Python selection check passed (exit0), trapping any resolve or
executable validation of the released target, selecting the frozen copy, stopping
before PostgreSQL, and refusing missing/failed/ambiguous compiler records, a
foreign manifest and a bad snapshot checksum. It also verified legacy executable
validation. `git diff --check` passed. These checks certify controller selection
only; the two-target avatar build and actual native witness cases remain NOT_RUN.

The coherent source checkpoint `fb0db1a9800933197ec75970fa5448dbfaaaf95d`
consumes Participant209's exact `f42d8ae0` sealed descriptor, `753083b4` resolver
and `275c80f6` joined-test source as `1c7f4b69`, `d15af75e` and `fb0db1a9`.
All three owned Rust blobs still match `7322cbcf`; migration0035 retains its
frozen checksum. The joined test remains the participant owner's validation.

Its fresh named BACKEND two-target build exited101 before avatar compilation:
friend-service protobuf generation reported an authority.proto shadow between
this checkout and a retained be-operations-resume build path. Original compiler
JSON, stderr, source hashes and the failure receipt are retained outside the
target in `/tmp/mcr-avatar-witness-fb0db1a9/`. No avatar binary was frozen and
no PG cluster started. The target was explicitly released with no handles.
Participant209 supplied the prior successful remediation: invalidate only the
`contracts-build` package inside a fresh allocated target turn, then repeat the
same locked/offline two-target build. No shared-source edit or new target is
needed. This is a build-provenance failure, not a successful witness validation.

### Actual producer-consumer compatibility observed

Participant209 compiled source `a7ace884d216446872f1c93a23d0e62dcf0068ff`
with the real sealed BoundParticipantAuthority, Root journal and native resolver
(Rust1.88, locked/offline, jobs2, debug0, incremental0; exit0). This owner
independently verified the successful original compiler JSON, frozen executable
SHA256 `fbfcdc36521067cd451dbbb1f71592cf4bcbc1c9adf32a2668e76aae2c8b3e14`
and exact equality of all three avatar Rust files plus migration0035 against
that immutable source commit. The avatar integration-test executable is a
separate target and is not certified by this library compilation.

The actual Auth/Friend/Profile-process diagnostic run recorded AVATAR revision
1→2 in Concert xid923 and PREFERENCES revision1→2 in xid1012, each with its
matching committed same-transaction witness. For both writes the real Root
delivery completed, both owner leases completed and the CONCERT registration
settled COMMITTED. This supplies concrete positive compatibility evidence for
the owned hook through the real consumer, beyond the synthetic scope fixtures.

The **whole joined test failed** on the later full Profile update: Concert
xid1107 committed, Profile xid1144 remained in progress/PENDING and delivery did
not complete. That separate participant/Profile settlement issue remains with
its owners; it is not concealed by the successful avatar/preference prefix.
No complete save/reload/abort journey, public HTTP, operator or IN01 acceptance
is claimed. Three peer shutdowns exited0 and private cluster cleanup passed.

Local peer evidence: `/private/tmp/mcr-participant-join-a7ace884/` contains
`concert-lib-receipt.json` (SHA256
`b05e0ba1c247f597113b5397308f25627b064f37be828ff65c385144df6cdb31`)
and `join-2-diagnostic.json` (SHA256
`ab163a7b3fbcfa8aa70f5acfefb2fbbc9adb2f1d4d01c273efa571206f9b4bed`).
The own two-target avatar build/native rollback/restart qualification remains
pending its fresh allocated retry.

### Qualified avatar witness checkpoint

Source `43c0e265c186f5a655d0b90688e588ec46f40a77` passed the fresh named
BACKEND retry. The sole `contracts-build` package cache reset resolved the
previous shadowed-proto failure; no other package clean was needed. Exact build:

```sh
CARGO_TARGET_DIR=<allocated-BACKEND> CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -p concert-service -p concert-application --test avatar --no-run --message-format=json
```

Exit0 in19.55s. Exact Cargo test artifacts, source hashes and both executable
copies were verified outside the target before its explicit release. No target
handles remained and no second target was used. Application executable SHA256
`e2fb507b2ac9c295d8dee942a89c91cb42f4807737142c5d0dc32e8b36cbf20e`;
service executable SHA256
`cd857eceb225f4b59a94ecdd3cb4657a7378b5ad74a32d924213b7fd0d511d9e`.
The copied application2 and service3 pure tests all passed (exit0).

```sh
python3 backend/scripts/mcr/avatar_smoke.py --build-metadata /tmp/mcr-avatar-witness-43c0e265/compiler-artifacts.jsonl --pinned-binaries /tmp/mcr-avatar-witness-43c0e265/binary.json --pg-bin /opt/homebrew/opt/postgresql@16/bin
```

Exit0; all4 actual PostgreSQL16.14 cases passed. Migration0035 was applied
unchanged. The selected case exercised all authored descriptor/scope refusals,
both effects' exact twenty witness fields and registered native identity,
rollback after UPDATE/after witness, durable postcommit uncertainty, immutable
and cross-transaction reuse rejection. All3 committed witness rows matched
exactly after postmaster restart16065→16203. Existing layered-data upgrade,
legacy transactions, CAS and selected-character persistence also passed.
Private cluster cleanup passed. These are actual executed native cases, not the
four ignored entries printed by the ordinary pure-test invocation.

Local receipts under `/tmp/mcr-avatar-witness-43c0e265/`: original compiler JSON
SHA256 `534d98ac66707a72ecb32b3b0cfc255eb6a3c08712e3fa791954b994b480bc7e`,
`binary.json` SHA256
`8a68451599ff8f627367cc4b06596164cd1286f4c15dcf6efa73d42aaacb50be`,
`avatar-native.json` SHA256
`3d4720126bef0ab726157962ef63a20b2348e74576ee574479a875d02be534e9`.
The controller consumed original compiler paths as metadata and ran only the
frozen copy after release. The prior failed build remains a separate receipt.
The successful real producer prefix above complements this full component
suite; the later full Profile settlement and whole Product/IN01 flow remain
outside this owned hook's completed validation.

### Production-only delivery handoff

The isolated `codex/mcr-be-avatar-witness-delivery` handoff starts from published
base `ced8ec124a3c9fb3443a2bc69d69055d64a6fd7b` and changes exactly eleven
paths: this owner's three avatar Rust
files, `avatar_smoke.py` and this document; plus Participant209's explicitly
handed-off migration0035, participant-effect-witness document, guarded adapter,
coordinator visibility seam, module export and settlement resolver. Peer source
commits are `48b3ec73`, `753083b4`, `f42d8ae0`, and production-only `70d2263f`
(the actual avatar PUT/preference PATCH resolver binding extracted from275).
All production peer bytes match the actual a7ace884 joined prefix.

The later failing full-Profile joined test, its driver and its test-only module
wiring are excluded from this delivery. All five owned executable/controller
source blobs and migration0035 match the qualified43c0e265 snapshot exactly.
`docs/source/` is unchanged; no frontend, manifest, lockfile or root-composition
edit is included. The eleven-path scope audit, plan checker and diff check passed.
Only the designated delivery integrator publishes this handoff to master and
records remote ancestry; the source branch alone is not completed delivery.

### Subsequent joined qualification

Participant209 subsequently fixed its own retained-deadline propagation and
qualified source `6767ee8e805db17291d1d77a25c72d881e9f743a`: the actual native
Auth/Friend/Profile/Participant save/reload/abort test passed1 case in5.28s.
This owner independently verified its immutable copied executable SHA256
`fc35b975663f4aa1b9061bbe2420987651df1b025c79c6a3255b74685ef5df18`,
matching source/binary receipt, and exact equality of the three owned avatar
Rust files plus migration0035 against that Git source. The earlier a7ace884
failure remains valid historical evidence; the later receipt closes that
specific joined-test failure rather than erasing it.

Peer report `/private/tmp/mcr-participant-join-6767ee8e/join-1.json` SHA256
`b1b22654dc2b92187ae27d3ab2936d33764ab63d712ff33a1297c60750649917`
records the actual native test exit0, all three service shutdowns exit0 and
private cluster cleanup PASS. The peer owns delivery of its separate deadline
fix and joined tests; they are not added to this eleven-path handoff. This is
real producer-consumer compatibility evidence for the hook, with initial rows
still fixtures and public Gateway HTTP/operator/IN01 acceptance separate.
