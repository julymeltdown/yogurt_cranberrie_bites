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
