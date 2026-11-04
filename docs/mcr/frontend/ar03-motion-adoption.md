# AR03 frontend motion and wardrobe adoption

Status: **IN_PROGRESS**. The implemented consumer core is qualified under explicit
TEST_ONLY candidate authority. The producer's pinned C06 2.0.0 candidate now
exists and is decoded strictly; production admission is still denied. The original AR03 requirement remains open
through actual application adoption and its required validation.

Assignment base: `6a73eff54035a4bf6f3e1ef5b0b53ea7be79bebd`.
Checkout branch: `codex/mcr-fe-AR03-motion-adoption`. The preceding art branch is
preserved. Publication is through the designated frontend delivery integrator,
chat `01a1a153-1227-7e72-801f-a2a6b91981e1`; the implementation owner does not
directly push master. A queued checkpoint does not complete the active goal.

## Implemented consumer core

The new helpers live exclusively in `entities/avatar/runtime/motion-adoption/`,
`world/avatar/motion-adoption/` and `features/avatar/motion-adoption/`, beneath
`frontend/src/`. All code and UI in this assignment are newly authored.

- A strict descriptor decoder verifies the complete frozen SHA-256 and length,
  retains `productionEligible:false`, and freezes nested metadata. It accepts
  local artwork selections only; it rejects public configs, arbitrary URLs,
  unknown or incorrectly slotted keys and surplus properties.
- The deterministic compositor builds a 320×256 sheet with 20 real 64×64 cells,
  compatible with the existing Phaser `texture()` port. It uses the combined
  atlas indices directly, clips dressed bodies at y=42, fills explicit cuff masks
  with selected body palette color 2, and layers garment/eyes/hair/accessory.
  Rear eyes use transparent source UVs. No whole-image tint or repeated idle is
  substituted for motion.
- One user/event runtime owns the shared decode, bounded composition cache,
  leases and private invalidation. One consumer abort cannot cancel another
  consumer's shared decode. Late decode completion after withdrawal disposes its
  image rather than allocating a texture. Idle Profile portraits and bounded
  option previews borrow the same owner.
- The scene adapter samples facing/moving on the incumbent Phaser frame callback,
  shares textures for equivalent configs, and removes old revision/room/scope
  resources. It creates no scene, game, socket, RAF or timer. Editor walk previews
  subscribe to that same scene clock.
- The working Customize component exposes six presets; eight skin/body choices,
  six hair, four eyes, six outfits and four accessories; genuine directional/walk
  previews; explicit save/reset; and server-result reconciliation. The controller
  preserves a dirty draft across panel navigation and known failures, fences
  stale work, and prevents another save after an uncertain result or conflict
  until authoritative reconciliation.

## Frozen artwork and attribution

Input art is the published selected-source handoff at `bb7d21b3`, with art/evidence
`2fb80680`, publication receipt `145d0e21` and original implementation `4c216129`.
Full ten-resource pins and supplied/authored attribution are preserved in
`docs/mcr/art/evidence/motion-candidates-v1/consumer-handoff.json`.

| Resource | SHA-256 | Bytes |
|---|---|---:|
| `combined-release-candidate.json` | `6cf3aad25ceba681f4c0306d1139109ee6c07508de617ecf1c0b08b1d43fb950` | 42,925 |
| `avatar-candidates-64-1cd8f05af8d76d58.png` | `1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd` | 470,010 |

Both files remain in `frontend/public/world/avatars/derived-v1/motion-candidates-v1/`.
The source pack is `mcr_user_characters_20261103`, revision 3. Supplied front
pixels and authored faithful reconstruction retain the prior art provenance;
this consumer patch generates no new artwork and changes no original, static
release, source catalog or `docs/source/` byte.

## Canonical v2 candidate handoff

Producer checkpoint `ae41db9db2493c259c72ef461844a3ff1019a1f6` freezes the
following actual Git bytes. These are candidate contracts, not admitted releases.

| Path | SHA-256 | Bytes |
|---|---|---:|
| `contracts/assets/v2/avatar-config.schema.json` | `3cbfd3185113d7f4913de89b0e04d67046dbab64b3f90c0b5f14a261afc6a78b` | 3,543 |
| `contracts/assets/v2/avatar-manifest.schema.json` | `8fc206f00d263ed9465cad932768075a1a0ca61d927e98ccc2aa8e9e7ef0d54f` | 102,672 |
| `backend/releases/selected-character-motion-v2/manifest.json` | `9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6` | 95,128 |

The manifest is `mcr_selected_motion_20261103`, version 1, representation
`selected_source_motion`, with `production_eligible:false`,
`qualification:CANDIDATE_ONLY`, and `fallback:NONE`. It has no admitted
`content_hash` field. Its immutable const schema binds all source resources,
originals, six preset maps, 28 item maps, direction/phase UVs, palettes and masks.

`runtime/motion-adoption/v2.ts` decodes the exact producer config shape:
`motion_preset` with one of six `motion_v2_photo01_front_01..06` IDs, or
`motion_layered` with five required item slots. Body IDs 0..7 select the semantic
skin palette; hair 0..5, eyes 0..3, outfit 0..5 and accessory 0..3 map to their
matching local keys. Outfit and accessory may be explicitly null. Independent
skin IDs, mixed branches, URLs, wrong slots and surplus/private fields fail.
Wire-text parsing rejects duplicate keys and lexical floats before parsing loses
that information. All 6,720 layered combinations round-trip exactly.

`runtime/motion-adoption/v2-candidate.ts` verifies the exact candidate bytes,
freezes their metadata and creates only `qualification:TEST_ONLY` authority.
It never creates a production admission. Tests verify pinned producer bytes,
compare all 31 producer fixtures, schema and manifest hashes, every selected UV
and body palette, and reject mutated eligibility/fallback fields. The 34 inputs
in the test prefix's `producer-inputs.json` each carry a SHA-256 and byte length.
Published checkout files support clean-clone reproduction after producer
integration; immutable proposal Git blobs support validation before integration.
Both paths verify every pin. Source decoder
and canonical schema provenance remain distinct from production inventory.

## Remaining public interface dependency

The injection seam is `MotionAuthority<C>` in
`frontend/src/entities/avatar/runtime/motion-adoption/runtime.ts`. It decodes an
externally owned config, maps it to a validated local art selection, provides six
canonical preset choices and replaces a selection through the external decoder.
`MotionPersistence<C>` in `features/avatar/motion-adoption/controller.ts` accepts
authoritative reads and optimistic-revision writes. These are internal consumer
ports, not parallel public schemas. Their present qualification is TEST_ONLY.

The contract owner must still supply an actually admitted motion release and
its bootstrap/theme/World hash binding; canonical C03/C05 schemas and generated
config/state unions; server persistence, inventory, authorization and optimistic
revision support; and positive/negative same-release HTTP/WS/Profile fixtures.
The candidate's raw-file hash cannot be invented as a production content hash.

Current guards are intentionally unchanged: layered `runtime/contract.ts` and
`schema.ts` admit strict 1.0.0; whole-character `contract.ts` admits reviewed 1.1.0
front-idle only; WorldTransport and HTTP/Profile DTOs use those public branches.
The existing layered compositor supports a baked skin tone and rectangular
320×256 item sheets, not semantic palette/cuff maps. No old family is broadened.

The initial five allocated root function scopes are registry.get/factory admission,
createRootCharacterRegistry, account avatar-controller creation, AccountPanel's
avatar branch and loadProduction renderer injection/characters.read. They do not
grant general root ownership. World additionally recorded provider Context and
mounted-runtime types/loading predicate, account-owner characters option and
motion-editor disposal, and MeHub renderAvatar dispatch at `5d052775`.
World `02348ee6e80b1e66eb689c70f9b796f6050422e2` transferred only
`whole-admission/index.ts`'s five named decoder functions and necessary config/
manifest imports/types after both historical incumbents returned their claims.
The earlier whole-character contract grant stays unused; its 1.1-only guard is
unchanged. Runtime schemas, generated wire types and contract generators remain
excluded. No production hunk is changed in the v2 candidate checkpoint.

A full-application check caught the existing exact-type invariant at
`whole-admission/__tests__/admission.test.ts:22`: `AvatarConfig` must equal the
generated wire union. An attempted local additive v2 alias was reverted before
handoff; the invariant, public decoder and tests remain unchanged. Adding a
public branch before coordinated canonical generation would violate that
contract. This concrete dependency was routed directly to Root1fc and Root153.
No unsafe cast or weakened legacy test manufactures admission.

## Validation and remaining boundary

Tests are confined to the two allocated avatar motion test prefixes and the
additional `app/character-entry/__tests__/motion-adoption/` prefix. The native
fixture runs the actual retained Phaser engine and map with working Customize,
save/reconciliation, self/peer sprites and shared Profile composition. Its local
record now uses the producer's canonical 2.0.0 configs with the hash-verified
candidate authority and an explicitly labeled fixture endpoint. The older
`testOnlyContract:mcr-motion-fixture-v1` remains only in isolated lifecycle tests.
It is not the application's production Root or a live HTTP/WS admission test.

Use Node 24.22.2 and set `MCR_FRONTEND_DEPENDENCIES` to the retained frontend
directory containing `node_modules/`. The test tools use that installation
without package installs, lockfile edits or dependency symlinks. The native
fixture resolves Phaser's declared browser build. Commands and final results
are recorded under `docs/mcr/evidence/avatar-motion-adoption/`.

Meaningful checks cover descriptor corruption/selection denial, phase mapping,
shared cache/leases, stale scope and room withdrawal, late/aborted consumers,
explicit save, draft retention, known denial, conflict and uncertain/malformed
receipts. Browser qualification compares all 120 preset cells against the pinned
atlas, proves five distinct phases per preset/direction, exercises actual walk
and idle frame indices in Phaser, and compares Profile RGBA to the shared
composition at 360/390/430 and all themes. The candidate checkpoint runs eight
commands: 60 unit cases, scoped types, full-application types, lint, optimized
fixture build, planning, UI-origin and native browser. `checks.json` records the
actual exit codes: all eight passed, with 19 scoped TS files and 758 full-application
TS files producing zero diagnostics, and scoped lint producing zero warnings.
The first published core checkpoint had seven commands and
20 unit cases, retained historically in `checkpoint.json`.
Three native cases cover 360/390/430, each with all themes and real scene walk/idle
frames. All 120 preset cells match the frozen atlas and all 1,176 pairwise wardrobe
compositions match the unchanged qualified art Canvas fixture. Twelve native JPEGs
were captured; the 360px selector/pose preview was visually inspected. All avatar
interactions retain two initial resource requests and zero additional requests.
These results remain TEST_ONLY component/real-Phaser evidence, not public admission.

The actual application registry, C03 and C05 refusal cases run in
`app/character-entry/__tests__/motion-adoption/root-denial.test.ts`. They prove
the candidate cannot allocate a production runtime, even with forged eligibility
and hash fields, while preserving static runtime borrowing and private cleanup.
These refusal tests are not a positive Root journey.

Actual positive application Root/public C06 admission, live service and
two-account world, physical devices and release acceptance remain **NOT_RUN**.
Pending release admission and canonical generation are interface dependencies,
not missing art inputs, a manager review
gate, a scope cut or permission to close the original goal early.

## Published ancestry

Core source `e713e0f6` was integrated as
`18a3e745f4c5eeea76f6cc5f5761304cef23a61f`; follow-up `6f88ec7e` as
`59e5f71138346aa179fcb408330c1bc92ae066eb`. Fetched `origin/master`
`a9517e049f24f434d103273d535943ff464346a5` contains both. The implementation
owner verified that ancestry directly. The v2 candidate checkpoint requires its
own subsequent integrator receipt and does not complete the active goal.
