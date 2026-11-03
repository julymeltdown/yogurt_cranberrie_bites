# Frontend ready integration — 2026-11-04

Result: READY for main's explicitly authorized non-force publication. This record
covers committed handoff assembly and combined verification only. The FE33, FE36
and FE37 source qualifications remain MOCK_READY; no root composition, live
integration, task acceptance or project release is asserted.

## Candidate and authority

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-ready-integration`.
- Branch: `codex/mcr-fe-ready-integration`.
- Exact clean starting base: `cd7192febf9c2764169b55d63a70d7e00d13fd08`.
- Tested application HEAD: `e4b9c0dd969b0251a610cac6123ee25645edd9ec`; this document's commit is its direct child.
  The final handoff supplies that child's exact SHA. Its sole change is this record.
- Persistent goal verified ACTIVE before assembly and through validation; complete
  only after committing and sending the READY handoff.
- Six exact source commits applied serially with `git cherry-pick -x`, exit 0;
  none skipped, no conflicts and no application changes beyond source handoffs.
- Existing owner/SL review dispositions are carried forward. This is no new feature
  review and introduces no main/SL approval gate. Original Delivery's integration
  checkout, dirty evidence and unfinished goal were not transferred or edited.
- Main owns publication; this worker did not push, fetch or modify remote refs.

## Source ancestry and mapping

| Handoff | Exact source commit | Integrated commit |
| --- | --- | --- |
| FE33 | `1ae55459a2f2b32c5ded777cff28886736827930` | `36d703d825d3bdafe244442c00be96f77fce4a4a` |
| FE36 safety | `009d833477e424bed9e152e4e0c87a75e17b2f7d` | `0a45e8af240f4b891c5b46e48959e6967da50919` |
| FE36 account | `cc586d7b8be8d9fdca7e64c9ea8130a54aa0b659` | `a0e8332acc85a6901dd0cf77d4a577a2ca3cfc09` |
| FE36 handoff | `926449d0d239852af3a170e7eaaf4de61ce19f0f` | `f936c071bebb693a7f889951713cf1f32b304bc9` |
| FE37 feature | `09f6c685b0f5686c32a9accfaa65db08c8da720f` | `b786e648dd889e9bdf064c0e4e637a1dc4075a70` |
| FE37 evidence | `7914577e77746b5d8f04d398a1a888173247465f` | `e4b9c0dd969b0251a610cac6123ee25645edd9ec` |

Source bases: FE33 implementation parent `a4f1b86592666ad3be53e4958f784236b19c3730`
(the original assignment was `98c2c793ba5c380b0cca334f4f8647123c9d30a9` with
explicit shared dependency handoffs); FE36 `892ca4de7580eb36c82b54efe44f03941ca68c41`;
FE37 `98c2c793ba5c380b0cca334f4f8647123c9d30a9`. Each integrated commit retains
its source SHA in the cherry-pick trailer. All original source/evidence documents
are byte-identical to their terminal source handoff. Existing dependency and
contract provenance remain in [mural synchronization](../frontend/mural-sync.md),
[safety/account](../frontend/safety-account.md) and [moderation](../frontend/moderation.md).
No code was copied from dirty checkouts; retained dependency resolutions are unchanged.

The exact six-commit ancestor is fast-forwardable from the assigned base. Main
must use the final handoff SHA including this record and preserve serial publication
if another checkpoint reaches master first. No fresh coordinator review is required.

## Verification

Run in the assigned worktree. `NODE` below expands exactly to
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`.
Node reported `v24.22.2`; retained pnpm reported `11.7.0`. Dependencies were read
through the explicitly authorized temporary `frontend/node_modules` symlink to
`/Users/lhs/Desktop/mcr/frontend/node_modules`. No install or dependency/lockfile
update was run. TypeScript used `--incremental false`; Vite/Vitest used the runner
config loader and Vitest disabled cache to avoid shared build-cache writes. The
symlink was removed before the final ownership check.

| Command / check | Exit | Result |
| --- | --- | --- |
| `NODE frontend/node_modules/typescript/bin/tsc --project frontend/tsconfig.json --pretty false --incremental false` | 0 | Strict whole-frontend types pass |
| `NODE frontend/node_modules/oxlint/bin/oxlint frontend/src frontend/vite.config.ts` | 0 | Frontend source/config lint passes |
| Selected Vitest command below | 0 | 799 tests, 28 files pass; 9.20 seconds |
| From `frontend/`: `NODE node_modules/vite/bin/vite.js build --configLoader runner` | 0 | 317 modules; production build passes; >500 kB minified chunk warning remains (CommunityFeatures 809.36 kB, Phaser 1198.04 kB) |
| `python3 scripts/mcr/check-plan.py` | 0 | 13 archived hashes, narrative/export probes and required 112 tasks/61 IA checklist pass; this checker certifies planning only |
| Boundary command below, after symlink removal | 0 | Frontend ownership boundary passes; 72 source handoff paths |
| `git diff --check` | 0 | No whitespace errors |
| Exact source blob and owner-evidence hash checks | 0 | All three terminal source path groups unchanged; FE33 11 source hashes and FE37 13 source hashes plus 9 screenshot hashes match retained reports |
| Import and excluded-path inspection | 0 | Changed imports stay in frontend/public generated contracts; source archive, contracts, backend, root composition, manifests and lockfile unchanged |

```sh
NODE frontend/node_modules/vitest/vitest.mjs run --root frontend --maxWorkers=2 --no-cache --configLoader runner src/features/mural/sync/__tests__ src/world/mural/__tests__ src/features/mural/editor/__tests__ src/features/safety/__tests__/controller.test.ts src/features/account/__tests__/controller.test.ts src/features/admin/reports/__tests__/controller.test.ts src/features/auth/__tests__ src/shared/query/__tests__ src/shared/api/__tests__ src/features/party-authoring/__tests__ src/features/party-discovery/__tests__/contract.test.ts src/world/network/__tests__/replication.test.ts src/world/composition/__tests__/composition.test.ts

python3 scripts/mcr/check-boundaries.py --base cd7192febf9c2764169b55d63a70d7e00d13fd08 --lane frontend --allow-doc docs/mcr/frontend/mural-sync.md --allow-doc docs/mcr/frontend/safety-account.md --allow-doc docs/mcr/frontend/moderation.md --allow-doc docs/mcr/evidence/frontend-ready-integration.md
```

Selected tests cover the three handed-off features and their directly used editor,
canonical Auth/privacy/HTTP, receipt validation, replication and world authority
consumers. No blanket native-browser rerun of unchanged features was performed.
FE33 and FE37 committed browser reports/screenshots are preserved, with matching
source hashes. FE36 owner-run native evidence remains the temporary evidence
location and pinned JSON SHA in its unchanged source document; it was not recreated
or promoted into durable/live evidence by this assembly.

Reproduce exact source preservation with `git diff --exit-code SOURCE HEAD -- PATHS`
for FE33 source `1ae55459a2f2b32c5ded777cff28886736827930` and its sync/world-mural/doc
paths, FE36 source `926449d0d239852af3a170e7eaaf4de61ce19f0f` and its safety/account/doc
paths, and FE37 source `7914577e77746b5d8f04d398a1a888173247465f` and its three admin/doc
paths. All three commands returned 0. Evidence SHA-256 validation read every entry
in each committed report's `sourceHashes` and FE37 `retainedScreenshots` collection.

## Contract pins at the tested integration candidate

- `contracts/domain/v1/mural.schema.json`: SHA-256 `345871ce89fabff02f06087fcd3e8907651a5712b34ccbd53091158af123b6ca`.
- `contracts/domain/v1/admin.schema.json`: SHA-256 `b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8`.
- `contracts/domain/v1/operations.json`: SHA-256 `411fcee8222db2f5a7938866ac37791033e85682f11d024ceb895c245c708500`.
- `contracts/domain/v1/policy.json`: SHA-256 `01c6b9e9a44b81bcf1273ebd7ba8361d3cc035a969399b4dad1f9dedec05839a`.
- `contracts/domain/v1/inherited-map.json`: SHA-256 `2d7510d94d45efd3f77a5586800ab7a353326288bfebb3900157b80225329751`.
- `contracts/generated/mcr-domain.openapi.json`: SHA-256 `edd8f89cda100a3f031d6e9b47ff39b615136938faab59259c7351851e139f1b`.
- `frontend/package.json`: SHA-256 `0f7a05587bcf1af5f4d5d077a0c2d90011adc87e1036a61db9ef3acf37c8b7d3`.
- `frontend/pnpm-lock.yaml`: SHA-256 `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`.

## Limits and separately owned work

- FE33 backend own-result deployment, root scene/authority/capability composition,
  real multi-client moderation/restore, native devices and IN04 remain NOT_RUN.
- FE36 keeps the strict currently frozen four-step account-deletion decoder. The
  proposed BE42 eight-step contract is not adopted without a committed canonical
  contract revision and affected consumer tests. Missing canonical Auth deletion
  port/root lifetime wiring remains open; deletion is unavailable without it.
- FE37 role-provider seam remains fail-closed; qualified server role decisions,
  protected report-media retrieval, root admin routing and real commands remain
  external composition/producer obligations. Mock fixtures do not grant roles.
- No route registration, root navigation, generated contract, approved character
  asset, backend or shared API change was included. No left-accent styling added.
- Live HTTP/database/realtime flows, physical-device/accessibility/load/recovery
  qualification and release gates were not executed. There is no scope cut.
- Main receives the exact ready candidate for prompt publication; original Delivery
  receives its ancestry to prevent duplicate integration. No acknowledgement wait
  is required to finish this bounded assembly assignment.

## Changed paths

All 72 handoff paths below match the explicit assignment. This evidence file is
the sole additional integration-owned path (73 final changed paths).

```text
docs/mcr/frontend/moderation.md
docs/mcr/frontend/mural-sync.md
docs/mcr/frontend/safety-account.md
frontend/src/features/account/__tests__/controller.test.ts
frontend/src/features/account/contract.ts
frontend/src/features/account/controller.ts
frontend/src/features/account/index.ts
frontend/src/features/account/panel.tsx
frontend/src/features/admin/content/index.ts
frontend/src/features/admin/members/index.ts
frontend/src/features/admin/reports/__tests__/browser.mjs
frontend/src/features/admin/reports/__tests__/controller.test.ts
frontend/src/features/admin/reports/__tests__/evidence/browser/360-content-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/360-members-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/360-reports-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/390-content-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/390-members-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/390-reports-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/430-content-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/430-members-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/430-reports-confirmation.png
frontend/src/features/admin/reports/__tests__/evidence/browser/report.json
frontend/src/features/admin/reports/__tests__/fixtures/harness.ts
frontend/src/features/admin/reports/__tests__/fixtures/preview.html
frontend/src/features/admin/reports/__tests__/fixtures/preview.tsx
frontend/src/features/admin/reports/index.ts
frontend/src/features/admin/reports/model/contract.ts
frontend/src/features/admin/reports/model/controller.ts
frontend/src/features/admin/reports/model/validate.ts
frontend/src/features/admin/reports/ui/ModerationReview.tsx
frontend/src/features/admin/reports/ui/moderation.css
frontend/src/features/mural/sync/MuralSyncPanel.tsx
frontend/src/features/mural/sync/__tests__/browser-smoke.mjs
frontend/src/features/mural/sync/__tests__/controller.test.ts
frontend/src/features/mural/sync/__tests__/evidence/browser/360-converged.png
frontend/src/features/mural/sync/__tests__/evidence/browser/360-frozen.png
frontend/src/features/mural/sync/__tests__/evidence/browser/360-unknown.png
frontend/src/features/mural/sync/__tests__/evidence/browser/390-converged.png
frontend/src/features/mural/sync/__tests__/evidence/browser/390-frozen.png
frontend/src/features/mural/sync/__tests__/evidence/browser/390-unknown.png
frontend/src/features/mural/sync/__tests__/evidence/browser/430-converged.png
frontend/src/features/mural/sync/__tests__/evidence/browser/430-frozen.png
frontend/src/features/mural/sync/__tests__/evidence/browser/430-unknown.png
frontend/src/features/mural/sync/__tests__/evidence/browser/report.json
frontend/src/features/mural/sync/__tests__/fixture.ts
frontend/src/features/mural/sync/__tests__/http.test.ts
frontend/src/features/mural/sync/__tests__/preview.css
frontend/src/features/mural/sync/__tests__/preview.html
frontend/src/features/mural/sync/__tests__/preview.tsx
frontend/src/features/mural/sync/__tests__/projection.test.ts
frontend/src/features/mural/sync/contract.ts
frontend/src/features/mural/sync/controller.ts
frontend/src/features/mural/sync/http.ts
frontend/src/features/mural/sync/index.ts
frontend/src/features/mural/sync/projection.ts
frontend/src/features/safety/Dialog.tsx
frontend/src/features/safety/__tests__/browser-smoke.mjs
frontend/src/features/safety/__tests__/controller.test.ts
frontend/src/features/safety/__tests__/fixtures.ts
frontend/src/features/safety/__tests__/preview.html
frontend/src/features/safety/__tests__/preview.tsx
frontend/src/features/safety/contract.ts
frontend/src/features/safety/controller.ts
frontend/src/features/safety/index.ts
frontend/src/features/safety/panels.tsx
frontend/src/features/safety/safety.css
frontend/src/world/mural/__tests__/authority.test.ts
frontend/src/world/mural/__tests__/sink.test.ts
frontend/src/world/mural/authority.ts
frontend/src/world/mural/index.ts
frontend/src/world/mural/phaser.ts
frontend/src/world/mural/sink.ts
```
