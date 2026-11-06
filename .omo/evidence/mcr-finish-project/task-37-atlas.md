# Task 37 - character atlas production status

Verdict: **GAP_REMAINS**. The atlas and reviewed manifest exist and are hash-pinned, and the frontend has a real consumer path. They are **not served by the current shipped runtime configuration**, so row 37 / GAP-6 is not closed and no served-renderer acceptance is claimed.

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w6-atlas`
- Branch: `codex/mcr-exec-w6-atlas`
- Base: `cc8ad1b5bbdca9f4af40a05ca92331328b13ebac`
- Row: `/Users/lhs/Desktop/mcr/.omo/plans/mcr-finish-project.md` task 37, "Verify the character atlas production status"
- Evidence path: `.omo/evidence/mcr-finish-project/task-37-atlas.md`

## Status matrix

| Check | Result | Evidence |
| --- | --- | --- |
| Selected source pack registered | PASS | 20 selected JPEGs, revision 3, original hashes and dimensions match the catalog |
| Produced motion atlas | PASS | 1280x1856 PNG, 470010 bytes, SHA-256 `1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd` |
| Candidate descriptor | PASS | 42925 bytes, SHA-256 `6cf3aad25ceba681f4c0306d1139109ee6c07508de617ecf1c0b08b1d43fb950`, `productionEligible: false` |
| Visual review receipt | PASS, admission still pending | `VISUAL_ACCEPTED`; frozen receipt says `production_runtime_admission: PENDING_TYPED_AND_RUNTIME_QUALIFICATION` |
| Reviewed manifest | PASS as staged artifact | v2, `production_eligible: true`, `fallback: NONE`, content hash `sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018` |
| Frontend consumer | WIRED, TESTED WITH FIXTURES | provider -> reviewed registry -> protected bootstrap authority -> motion runtime -> `/world/avatars/derived-v1/motion-candidates-v1/*` |
| Current stack serves it | **NO** | compose runs concert health-only; the reviewed bootstrap exists only as a runtime test fixture |
| World / profile / gift surfaces use the reviewed view when admitted | PASS in component wiring | `CommunityFeatures`, `root-features`, `routes`, social and account panels select `ReviewedCharacterView` |
| Legacy or synthetic character art reachable | NO | new regression test plus source/public asset scan |
| Production activation | NOT_RUN | `reviewed-handoff.json`; `qualification.md` |

## Hash and dimension comparisons

All paths are repository-relative; the original-sheet table uses paths relative to `frontend/public/world/avatars/`. `sha256` values are lowercase hex with the PEM-style prefix used by the recorded artifacts.

### Selected originals

20/20 selected sheets match the catalog byte-for-byte, including byte length and decoded JPEG dimensions. `photo_03` remains archived, byte-identical to `photo_02`, and is not selectable.

| Source ID | File | Bytes | SHA-256 | Dimensions |
| --- | --- | ---: | --- | --- |
| photo_01 | supplied-v1/photo-01.jpg | 168045 | `67448360482a0dd68172d6089378a002190142bb8bd804818c89a86370154311` | 1280x853 |
| photo_02 | supplied-v1/photo-02.jpg | 166375 | `cb66e99ef86893e542728864933a694663cc4bdee9440df52de64a9323a46489` | 1280x960 |
| photo_04 | supplied-v1/photo-04.jpg | 189238 | `87cdfbff215758c12d7042a7c435a17a6cdccb7637b0542dd9e0508d832e2a63` | 1280x960 |
| photo_05 | supplied-v1/photo-05.jpg | 185536 | `681f566ab7ef081fa7f8a39ea0c3367cc822ea31f1bcfe3065f2755679950027` | 1280x960 |
| photo_06 | supplied-v1/photo-06.jpg | 193901 | `a5fa9ecebf1ce7f36d1cb3eb50c5a21d75fe817797d9f1cd330d252b3677845f` | 1280x960 |
| photo_07 | supplied-v1/photo-07.jpg | 172536 | `5ff980e6268beb3162c23f1d8d0478d09d2180ce45f5df9c4303eac80b9c82ce` | 1280x960 |
| photo_08 | supplied-v1/photo-08.jpg | 111594 | `9b9e5ba41b8f286ecf73ca2e859a03eefa294ff3750c013f75310189343445d9` | 1280x960 |
| photo_09 | supplied-v1/photo-09.jpg | 142915 | `e4e857fb1a7f98d1a52538f47738b8fe01f0e352be0cd71e7b2f9e36ae4008c8` | 1280x960 |
| photo_10 | supplied-v1/photo-10.jpg | 98841 | `397ffa665515ebe8c573f1a5164358c5cb698633a1680f545994404026ecc96d` | 1280x960 |
| photo_11 | supplied-v1/photo-11.jpg | 242601 | `528bc065159be519e076c84648561845eb97d08c1fa090345e80489f5d69821f` | 1280x960 |
| photo_12 | supplied-v1/photo-12.jpg | 273737 | `ac2d1aa9153aa1217afa0817679fba0c00d09f3290f191d4706ad5a22833db94` | 1280x960 |
| photo_13 | supplied-v1/photo-13.jpg | 263938 | `292a1080e08ac42474222ea1c1cda2d063885b7da72f2f9ea8c606b25c5217b3` | 1280x960 |
| photo_14 | supplied-v1/photo-14.jpg | 288728 | `6bceb582fc1301589c968a40335ad7dc183f01c60b47ad45f223a375d3c49d95` | 1280x960 |
| photo_15 | supplied-v1/photo-15.jpg | 293533 | `d0a17abd38eb55e46db985c168d0183073b5d703478d00e8d49ca8e14d4af825` | 1280x960 |
| photo_16 | supplied-v1/photo-16.jpg | 248204 | `48200de941ebc7b9474752b0475c7c0fe4983fd16442afce83fbe56a1d474507` | 1280x960 |
| photo_17 | supplied-v1/photo-17.jpg | 175218 | `21c7dbe159570072cb7406d6d1bf3521ba44da4bd392dd9f058f4e2bb026750c` | 1280x960 |
| photo_18 | supplied-v1/photo-18.jpg | 222649 | `419a2b2527dab6b63bf2cee251651eefeb6792a546bcd4737a99d6f5678aa514` | 1280x960 |
| photo_19 | supplied-v1/photo-19.jpg | 240144 | `856135f8c8b970f711b1ccdf1c071151a54037e07827bae6b518465e18d6c1ad` | 1280x960 |
| photo_20 | supplied-v1/photo-20.jpg | 193103 | `1b741978ef59714c6ecd7f906abf6f909ee77d2dc1906c97cc95b958aa98243a` | 1280x960 |
| photo_21 | supplied-v1/photo-21.jpg | 255136 | `3f0d763f18913cdd4e6af8830404ef7163315cd30a275054c76e63c6fa09831d` | 1280x960 |
| photo_03 (archived) | supplied-v1/photo-03.jpg | 166375 | `cb66e99ef86893e542728864933a694663cc4bdee9440df52de64a9323a46489` | 1280x960 |

### Produced artifacts

| Artifact | Bytes | SHA-256 | Dimensions / frames |
| --- | ---: | --- | --- |
| `frontend/public/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json` | 42925 | `6cf3aad25ceba681f4c0306d1139109ee6c07508de617ecf1c0b08b1d43fb950` | candidate, `productionEligible: false` |
| `frontend/public/world/avatars/derived-v1/motion-candidates-v1/avatar-candidates-64-1cd8f05af8d76d58.png` | 470010 | `1cd8f05af8d76d58243e2f6e143961953b4d4291fdd553df8c8d33aabd700dbd` | 1280x1856 PNG, 576 physical 64x64 cells |
| `backend/releases/selected-character-motion-v2/manifest.json` | 95128 | `9a555f726a9e421f14ed876117be87bb1bb8fa128ac401ab38f8927760776fd6` | candidate v1, `production_eligible: false` |
| `backend/releases/selected-character-motion-v2/reviewed/manifest.json` | 97407 | `1342f2b640a8135c2f128713fe1f59adc0b6da8b925a0ac84808b2d8d1ec884d` | reviewed v2, content hash `sha256:7f2891c8...f9018` |
| `docs/mcr/contracts/avatar-motion-v2/review/visual-review.json` | 8109 | `27dd1f7f1d31b792f4da415cf6daf6eafc97e91023443a27ac70480f75ce5956` | `VISUAL_ACCEPTED`; runtime admission pending |
| `frontend/public/world/avatars/derived-v1/static-release.json` | 1898 | `cc53c6852742e9cbb9858a2d0961411e1a9e131d1e30c0f2b73b265e03f91677` | selected static release, 6 front-idle characters |
| `frontend/public/world/avatars/derived-v1/photo01-front-idle-64-e124c88815b22220.png` | 36030 | `e124c88815b22220df2f2dc59e2f26b3ceb845e57b77674fd8e55383116d7efe` | 384x64, 6 frames |

Frame accounting for the motion atlas:

- 1280x1856 = 20 columns x 29 rows = 580 cells; 576 are declared physical frames.
- Six complete presets x 20 frames = 120 frames: four directions x five phases (idle + four walk).
- 30 preset frames are `SUPPLIED_FRONT_PIXELS_SOURCE_ARTICULATION`; 90 are `AUTHORED_RECONSTRUCTION_WITH_ARTICULATED_SOURCE`.
- Wardrobe: 28 items (8 body, 4 eyes, 6 hair, 6 outfit, 4 accessory) x 20 frames = 560 frames.
- 120 preset frames occupy unique indices 0-119. The 560 wardrobe placements occupy 336 unique cells in the 120-575 range, so 456 of the 576 physical cells are referenced and 120 cells are unused by these two sources.
- The reviewed manifest's 20 `source.originals` entries match the selected pack's source IDs, paths, hashes and byte lengths exactly; `photo_03` is absent.

## Admission path

1. The selected pack (`mcr_user_characters_20261103`, revision 3) supplies 20 original JPEGs. `source-catalog.ts` allowlists exactly those IDs; unknown or archived IDs throw.
2. The candidate descriptor and atlas are a reviewed derivative, not byte-identical source sheets. The descriptor is explicitly `productionEligible: false`; front frames use supplied pixels, side/rear frames are authored reconstructions on an articulated source.
3. `docs/mcr/contracts/avatar-motion-v2/review/visual-review.json` binds the candidate manifest, atlas hash, source pack and inspected resources with `VISUAL_ACCEPTED`, `originals_modified: false`, `art_generated: false`. The frozen receipt separately says `production_runtime_admission: PENDING_TYPED_AND_RUNTIME_QUALIFICATION`.
4. `backend/releases/selected-character-motion-v2/reviewed/manifest.json` is the reviewed successor (`mcr_selected_motion_reviewed_20261104`, v2). It pins the candidate manifest, atlas, review receipt and all 20 originals; `fallback: NONE`.
5. Backend admission is structural and byte-based, not a caller boolean:
   - `backend/crates/concert-domain/src/avatar_motion_v2/reviewed.rs` includes the exact reviewed manifest via `include_str!`, compares the document with the pinned value, recomputes the canonical content hash, verifies the review receipt, and exposes separate structural and visually-reviewed types.
   - `backend/services/concert-service/src/bootstrap/release.rs::validate_avatar` routes contract 2.0.0 to `VisuallyReviewedCatalog::from_manifest`; `validate_motion_files` requires the canonical reviewed manifest path and verifies every referenced resource from a closed repository-relative layout.
   - `backend/services/concert-service/src/adapters/out/repository/avatar.rs` and `bootstrap/spatial.rs` revalidate the reviewed catalog and resources for product construction.
6. Frontend consumption:
   - `frontend/src/entities/avatar/runtime/motion-adoption/reviewed-production.ts` fetches the protected `/api/concert/events/{eventId}/bootstrap`, decodes the reviewed manifest, validates the avatar config and mints a `REVIEWED_PRODUCTION` authority.
   - `frontend/src/app/character-entry/reviewed-registry.ts` admits it with the canonical session/privacy scope; `provider.tsx` selects `admitReviewed` only for `representation: 'reviewed_selected_source_motion'`.
   - `frontend/src/entities/avatar/runtime/motion-adoption/runtime.ts` owns the runtime; `art.ts::loadMotionArtwork` fetches exactly `/world/avatars/derived-v1/motion-candidates-v1/combined-release-candidate.json` and `/world/avatars/derived-v1/motion-candidates-v1/avatar-candidates-64-1cd8f05af8d76d58.png`, verifying byte length, SHA-256 and 1280x1856 dimensions before decode.
   - `ReviewedCharacterView` is used by the world, social/profile, account and gift-owner portrait paths (`frontend/src/app/CommunityFeatures.tsx` lines 45-50 for gifts/parties; `root-features.tsx`, `routes.tsx`, `social-composition/panels.tsx`, `account-entry/panels.tsx`).
7. Existing tests already exercise the loader with actual artifact bytes: `frontend/src/features/avatar/__tests__/reviewed-production/owner.test.tsx` asserts the two requested asset paths exactly, and `frontend/src/entities/avatar/runtime/__tests__/reviewed-production/resource.test.ts` verifies the descriptor/atlas and every source original hash.

## Why it is not served today

- `infra/mcr/compose.yml` (lines 217-224) states: "Concert runs its health-only operational mode: the product gRPC/release composition requires an operator release bundle this stack does not ship." It sets `CONCERT_RUNTIME_MODE`, migration, database and metrics settings only. None of the six `CONCERT_BOOTSTRAP_FILE`, `CONCERT_AVATAR_MANIFEST_FILE`, `CONCERT_MAP_MANIFEST_FILE`, `CONCERT_REVENGE_MANIFEST_FILE`, `CONCERT_PARADE_MANIFEST_FILE` or `CONCERT_DANGER_MANIFEST_FILE` settings are configured. Without `CONCERT_GRPC_ADDR` and those six files, the bootstrap serving path is not enabled.
- The only bootstrap document in the repository with `mcr_selected_motion_reviewed_20261104` is `backend/releases/selected-character-motion-v2/reviewed/runtime-fixture/bootstrap.json`. It is used by `backend/services/concert-service/tests/avatar_motion_v2_runtime.rs` as a native test fixture, not by a configured product release.
- `backend/releases/operator-bundle/quality-v6/bootstrap.json` and `backend/releases/photo2-world-display-v1/bootstrap.json` contain the static whole-character family (`mcr_selected_static_20261103`, 1.1.0), not the reviewed motion family. They are also not configured by compose.
- `docs/mcr/contracts/avatar-motion-v2/reviewed-handoff.json` records `production_activation: NOT_RUN` and lists operator registry activation and real public HTTP/WS admission as remaining gates. `qualification.md` says: "Public release activation is NOT_RUN."
- `frontend/src/entities/avatar/character-source-pack.json` still declares `production_atlas.status: "NOT_PREPARED"`, `runtime_manifest_id: null` and `four_direction_idle_and_walk: "NOT_PROVIDED"`. `docs/mcr/character-art.md` still says the production atlas and FE12 composition are `NOT_RUN`. Those status fields are stale relative to the reviewed artifact and are not silently rewritten by this verification; they are part of the remaining documentation/activation gap.

Consequence: the app can render the atlas when a protected bootstrap serves the reviewed manifest, and the component tree covers world/profile/gift owner portraits. The current shipped stack does not serve that bootstrap, so no end-to-end served-renderer acceptance is established. A deployment that points the six release settings at a validated bundle containing the reviewed manifest would exercise the path, but such a bundle is not present or configured here.

## Legacy and synthetic art guard

New regression test: `frontend/src/entities/avatar/__tests__/production-art-guard.test.ts` (5 tests).

It fails if any of the following reappears:

- A selected-source hash, byte length or JPEG dimension changes.
- `photo_03` or an unknown source ID becomes selectable (`getCharacterSourceUrl` must throw).
- The candidate atlas/descriptor hash, dimensions, 576/120/560 frame counts, 6 presets or 28 wardrobe IDs change.
- The reviewed manifest's pins, content hash, 20-original set, fallback `NONE` or schema const change.
- The C06 fixture becomes admissible: the test proves its texture is `NOT_PROVIDED`, its provenance is `ORIGINAL_FIXTURE`/`UNREVIEWED`, and `validateBinding` rejects a forged binding.
- A production source references an avatar family outside `supplied-v1`/`derived-v1`, references `photo-03`, or imports the C06 fixture as a value.
- A public avatar file appears outside `supplied-v1`/`derived-v1`, an unknown supplied file appears, or a name matches legacy/placeholder/fallback/old-avatar/default-avatar.
- The loader stops requesting the exact derived descriptor and atlas URLs.

Existing complementary coverage: `resource.test.ts` verifies all 20 originals and the frozen derivative bytes; `owner.test.tsx` asserts only the two approved asset requests; the reviewed registry/provider tests admit only a protected reviewed manifest and expose an honest unavailable state on failure.

## Validation commands

Environment: Node `v24.22.2`, pnpm `11.7.0`, worktree base `cc8ad1b5b`.

| Command | Exit | Result |
| --- | ---: | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile --offline` | 0 | 199 packages reused from the local store; no network |
| `corepack pnpm@11.7.0 typecheck` | 0 | `tsc -b --pretty false` clean |
| `corepack pnpm@11.7.0 exec oxlint src/entities/avatar/__tests__/production-art-guard.test.ts` | 0 | no findings |
| `corepack pnpm@11.7.0 vitest run src/entities/avatar/__tests__/production-art-guard.test.ts` | 0 | 5/5 tests pass |
| `corepack pnpm@11.7.0 vitest run` (16 avatar/atlas files listed below) | 0 | 16 files, 235 tests pass |
| `node -v; corepack pnpm@11.7.0 --version` | 0 | `v24.22.2`, `11.7.0` |

The related run included `src/entities/avatar/__tests__/production-art-guard.test.ts`, `src/entities/avatar/runtime/__tests__/reviewed-production`, `src/features/avatar/__tests__/reviewed-production`, `src/app/character-entry/__tests__/reviewed-production`, `src/world/avatar/__tests__/motion-adoption` and `src/entities/avatar/whole-character/__tests__/contract.test.ts`.

Authoring correction retained: the first guard-test run was 4/5 because the public-asset walk compared paths including `world/avatars/`. The test was corrected to compare paths relative to the avatar root; the rerun passed 5/5. No product code was changed for that failure.

## Capture

No new capture was produced. The current stack does not serve a reviewed-manifest bootstrap, so a fresh "actual served release" capture would be misleading. A historical 390x844 capture from the actual main fixture, with the reviewed atlas rendered in World, is retained at:

- `docs/mcr/evidence/frontend-combined-adoption/reviewed-round01/reviewed/2026-11-04T15-46-44-984Z-reviewed-world-before-390.png`
- 230307 bytes, SHA-256 `64e8f8d82349c5eedb6b54489ef56619283e15744ac9f610750450ff9c7b0eec`
- Qualification recorded beside it: `ACTUAL_MAIN_SYNTHETIC_HTTP_C05_REVIEWED_C06_NOT_LIVE`

## Row disposition and remaining work

Row 37 remains **OPEN / GAP_REMAINS**:

- Artifact production, hash verification and frontend consumer wiring are proven.
- Actual serving and end-to-end world/profile/gift rendering from a configured release are not proven.
- Required next action: publish/configure a validated product release containing `mcr_selected_motion_reviewed_20261104`, enable the six `CONCERT_*` release settings in the serving stack, then run an actual 390px native capture across world, profile and gift-owner portrait surfaces and update the stale source-pack/character-art status fields. Until then, do not claim the atlas is served or close GAP-6.
