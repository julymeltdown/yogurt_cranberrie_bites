# Account and avatar root composition

Status: MOCK_READY; bounded root consumer composition with native fixture qualification from `5c95ed4ddb243853b27fb625f726341b9771dadd`. Exact ownership: new `frontend/src/app/account-entry/**` and this document; separately delegated `router.tsx` existing four account route elements, `WorldShell.tsx` one stable account provider wrapper, and `ports.ts` only an optional typed `accountAssets` field/import. No main, constructors/defaults, world/entity, contract, source archive or dependency edits.

The actual frozen route table at this base uses `/me`, `/me/profile`, `/me/avatar`, `/me/settings`. No `/me/edit` or `/me/preferences` routes are introduced. Existing feature links remain valid.

`createAccountOwner` receives canonical Auth plus the already validated shell bootstrap and original privacy lease. It creates one ME controller and one lazy-started FE13 editor controller; it does not create an identity, query client, Phaser instance or world connection. The React provider owns setup/teardown in an effect and keeps the child tree stable. Exact auth/bootstrap/lease/assets bindings fence owner publication; disposal and canonical privacy cleanup erase both controllers. Merely opening the world sends no participant/editor request.

`AccountAssetsPort` contains only explicitly supplied reviewed binding/resources. Manifest and pin come from the exact retained C03 bootstrap document under the original lease. No port, absent manifest or rejected review produces an unavailable preview. The shared reviewed compositor and FE13 reviewed preview adapter enforce pack/binding/hash/skin constraints. There is no fallback art, generated substitute, global configuration or second bootstrap fetch. No product asset port is installed by this patch.

FE13 verified `onCommitted(avatar, scope)` calls `me.refreshAfterAvatar(scope, avatar.revision)`. FE34 rejects stale scope/revision notifications, retains dirty/pending drafts and requires explicit reload instead of silently rebasing them. World appearance remains authoritative server replication; this adapter does not write replicated entities or simulate a world update. Editor view mount/unmount uses its public start/stop lifecycle, including retained safe drafts and fresh explicit reconciliation on return. Ordinary panel navigation keeps the provider/ME controller alive.

Settings persistence uses exact C03 GET/PATCH through existing FE34. Existing runtime settings/FE06 theme adapters are not supplied implicitly; the feature truthfully states when live controls are unavailable. Optional ticket section/visibility remain outside qualified C03 and unaccepted. Root wiring is consumer composition, not production art/live HTTP/multiuser acceptance.

## Delivery and validation

Commit ancestry: base `5c95ed4ddb243853b27fb625f726341b9771dadd` → adapter `6b1b17c` → exact three-root-file composition `790c3680dcd6c5b4113a60b7f2cfb29c99fa1472` → validation/docs follow-up. Root edits are intentionally separate from adapter/test paths. Only Delivery integrates/pushes; this worker never pushes. The final handoff identifies the validation commit and exact fourteen changed paths.

All commands use retained Node24.22.2/pnpm11.7.0. Frontend commands run in `frontend/`; Python/source checks run at root. Exit codes below are 0.

| Check | Command | Evidence |
|---|---|---|
| Dependencies | `pnpm install --offline --frozen-lockfile` | PASS; original resolutions, no manifests/lock edits |
| Affected integration tests | `pnpm exec vitest run src/app/account-entry/__tests__ src/app/__tests__ src/features/avatar/__tests__ src/features/me/__tests__ src/shared/query/__tests__` | 236/236 PASS across 16 files, including 12 account-owner/root tests |
| Compilation/build | `pnpm build` and `pnpm typecheck` | PASS; existing chunk-size warnings remain; account provider/panels split into lazy chunks |
| Static analysis | `pnpm lint` | PASS, no warnings |
| Native browser | `node src/app/account-entry/__tests__/browser-smoke.mjs` | PASS: installed Chrome at 390×844 and 1280×844, StrictMode/reduced-motion, 26 native HTTP fixture requests, zero page errors |
| Planning/source integrity | `python3 scripts/mcr/check-plan.py` | PASS; original source unchanged; planning baseline not task acceptance |
| UI source origin | `node scripts/mcr/check-ui-origin.mjs` | PASS; no copied rendering |
| Assignment/boundary | `python3 scripts/mcr/check-boundaries.py --base 5c95ed4ddb243853b27fb625f726341b9771dadd --lane frontend --allow-doc docs/mcr/frontend/account-entry.md --allow-composition frontend/src/app/router.tsx` | PASS; exact assignment/import diff manually reviewed |
| Patch integrity | `git diff --check` | PASS |

The boundary script recognizes router.tsx as a central composition file; ports.ts and WorldShell.tsx are ordinary lane files under its deliberately limited regex. Their explicit shared grants are independently checked against the three-file root commit, not inferred from the script. No script changes were made to bypass a check.

The browser opens the actual root route composition with canonical Auth login and protected native GET/PATCH against explicitly synthetic Vite endpoints. Actual FE09 validates the bootstrap body and retains its immutable document. Navigation traverses World → ME → profile → World → ME → profile → settings → avatar → parent/history, preserving the same instrumented world host/lease and a dirty profile draft. It verifies one bootstrap read throughout ordinary navigation, optional bio not gating World, preferences saving without live-runtime success, missing approved art/disabled avatar save/no canvas, keyboard focus, panel expansion, mobile overflow, policy-generation teardown/reload, a pending read across foreign identity change, and direct profile deep-link entry in a new document. Production `/login` is separately opened from the built application; no account/provider/panels/Phaser/community chunks or world canvas are fetched/mounted.

The lease fixture explicitly represents no actual game or WebSocket. Peak counts prove shell consumer ownership against an instrumented port only; they do not certify real Phaser/network/multiplayer behavior. No reviewed product art is fabricated for native proof. Owner tests use a mocked shared reviewed-preview entry solely to drive the real FE13 PUT/verified callback and FE34 canonical GET/draft-conflict handling; this is not artwork qualification. Additional tests cover exact manifest/pin/original lease/binding/resource transfer, no account network work until panel entry, mismatched/stale bootstrap rejection, immediate dual-controller/preview disposal, ignored-abort late preview disposal, StrictMode teardown and identity/policy reset.

Local review artifacts: `/tmp/mcr-account-entry-evidence/result.json`, `profile-390.png`, `avatar-390.png` and desktop equivalents. The JSON records source SHA-256 for the three root files, product adapter files and browser harness/fixture; screenshots were visually inspected. They are local synthetic qualification evidence, not durable real-server receipts.

Outstanding acceptance: actual backend Auth/Profile/Concert HTTP journeys; installed reviewed supplied-art atlas/binding/resources; verified avatar change observed by both real connected accounts through authoritative replication; real runtime theme/motion/power/sound application; optional ticket-section/visibility contract; real device visual/performance/load and IN01/IN05. Missing artwork or these external gates did not block bounded source/root lifecycle work, and they remain unaccepted rather than being removed from scope.
