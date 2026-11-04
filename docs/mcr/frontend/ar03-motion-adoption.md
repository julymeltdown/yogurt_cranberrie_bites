# AR03 frontend motion and wardrobe adoption

Status: **IN_PROGRESS**. The implemented consumer core is qualified under explicit
TEST_ONLY local authority. No new public C06 version, inventory catalog or
production admission is asserted. The original AR03 requirement remains open
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

## Required canonical interface handoff

The injection seam is `MotionAuthority<C>` in
`frontend/src/entities/avatar/runtime/motion-adoption/runtime.ts`. It decodes an
externally owned config, maps it to a validated local art selection, provides six
canonical preset choices and replaces a selection through the external decoder.
`MotionPersistence<C>` in `features/avatar/motion-adoption/controller.ts` accepts
authoritative reads and optimistic-revision writes. These are internal consumer
ports, not parallel public schemas. Their present qualification is TEST_ONLY.

The producer and shared-contract owner must supply the following before real
application dispatch can be implemented:

1. A separately versioned manifest family/config decision, exact manifest
   id/version/content hash, representation and supported capabilities. The
   consumer does not choose the canonical version or assign inventory IDs.
2. Authorized preset IDs mapped to the six local preset keys, and authorized
   item/skin IDs mapped to each compatible body/skin, hair, eyes, outfit and
   accessory key. Configs must never carry arbitrary asset URLs. The producer
   must decide whether eight skin/body choices map to body IDs, skin IDs or both.
3. A pinned selected-source atlas/descriptor mapping with 64px cells, foot
   `(32,60)`, four directions and idle/four walk phases; exact UVs including the
   already-applied 120-cell wardrobe offset; skin palettes, cuff masks and
   occlusion; body clip y=42; layer order; and explicit rear-eye invisibility.
4. Provenance and eligibility that distinguish supplied pixels from authored
   faithful reconstruction. Candidate status cannot silently become an approved
   inventory release. The server must enforce compatibility, inventory and
   optimistic revision, including bootstrap, save, public reads and replication.
5. Generated public config/state types and decoders across HTTP, WS and public
   profile boundaries, plus positive/negative same-release fixtures and exact
   producer/consumer tests. Wrong release, item slot, source hash and unsupported
   capabilities must still fail closed.

Current guards are intentionally unchanged: layered `runtime/contract.ts` and
`schema.ts` admit strict 1.0.0; whole-character `contract.ts` admits reviewed 1.1.0
front-idle only; WorldTransport and HTTP/Profile DTOs use those public branches.
The existing layered compositor supports a baked skin tone and rectangular
320×256 item sheets, not semantic palette/cuff maps. No old family is broadened.

The five allocated root function scopes are registry.get/factory admission,
createRootCharacterRegistry, account avatar-controller creation, AccountPanel's
avatar branch and loadProduction renderer injection/characters.read. They do not
grant edits to public decoders or other root types. The typed CharacterProvider
context, account-owner character type, application ports, Profile view dispatch
and generated DTO unions must receive exact ownership coordination if the new
canonical interface requires changes. No unsafe cast will manufacture admission.

## Validation and remaining boundary

Tests are confined to the two allocated avatar motion test prefixes and the
additional `app/character-entry/__tests__/motion-adoption/` prefix. The native
fixture runs the actual retained Phaser engine and map with working Customize,
save/reconciliation, self/peer sprites and shared Profile composition. Its local
record is deliberately named `testOnlyContract:mcr-motion-fixture-v1`, differs
from every public AvatarConfig, and uses an explicitly labeled fixture endpoint.
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
composition at 360/390/430 and all themes. The same-source checkpoint passes all seven commands (20 unit cases, scoped
typecheck, lint, optimized fixture build, planning, UI-origin and native browser).
Three native cases cover 360/390/430, each with all themes and real scene walk/idle
frames. All 120 preset cells match the frozen atlas and all 1,176 pairwise wardrobe
compositions match the unchanged qualified art Canvas fixture. Twelve native JPEGs
were captured; the 360px selector/pose preview was visually inspected. All avatar
interactions retain two initial resource requests and zero additional requests.
These results remain TEST_ONLY component/real-Phaser evidence, not public admission.

Actual application Root/public C06 admission, live service and two-account world,
physical devices and release acceptance remain **NOT_RUN**. Pending contract
authorship is an interface dependency, not a missing art input, a manager review
gate, a scope cut or permission to close the original goal early.
