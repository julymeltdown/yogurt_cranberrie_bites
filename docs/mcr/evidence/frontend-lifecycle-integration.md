# Frontend lifecycle checkpoint integration

Date: 2026-11-04, Asia/Seoul. **READY for Main to publish this bounded candidate.** This record integrates FE40 and the handed-off FE39 working subset. It does **not** complete FE39, qualify live services or accept IN04/IN05. FE39 remains **BLOCKED on missing C04 closure-job RESUME**; its available wall controls and closure progress/status recovery retain owner-run MOCK_READY evidence. Wall PAUSE/RESUME is distinct from the missing closure-job retry mutation.

## Candidate and source ancestry

- Writer: `mcr(fe) lifecycle checkpoint integration`, chat `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c`.
- Assigned isolated checkout: `mcr-worktrees/fe-lifecycle-integration`, branch `codex/mcr-fe-lifecycle-integration`.
- Exact assigned published base: `683484baaf78792292337ad0c197a5d574536db3`.
- Tested code HEAD: **`96b5ab07f83cc9e0f580a80f07cb73a421d00699`**. The final READY commit adds only this evidence document; feature/dependency/contract bytes remain identical to the tested code HEAD. Its exact SHA is supplied in the committed handoff rather than embedded in its own content.
- Original feature checkouts remain stopped and unchanged. No root/shared/auth/contracts/manifests/locks/backend, other feature or UIV01 presentation paths were acquired. No application wiring, role provider, public media endpoint or resume command was invented. Main publishes the exact validated READY SHA; this worker does not push.

Every source commit's changed paths were checked against the exact allocation before serial `git cherry-pick -x`. All applied without conflicts, out-of-scope changes or integration source fixes:

| Source commit | Integrated commit | Purpose |
| --- | --- | --- |
| `92831f469db723e0597b0df78e07cfb7564d9c0b` | `56de7d643cf23e984c637660bb48be8792bc678a` | FE40 public read-only archive; 49 owner tests and durable browser evidence |
| `75856239d273e14a21a17f4c6248af35fe7e7007` | `d7a8ec449a5d930a86d51280fc1c6541ae2820c3` | FE39 wall controls and truthful closure progress/recovery subset |
| `680915dc382927591084d881f75fc94966ae43db` | `2383fda138665772a4530dd0fb21ed5200cad9da` | FE39 qualified-access renewal and identity-scoped full retry delay corrections |
| `54fc54e990cd74329bd731585427d72ac1b7a5a7` | `96b5ab07f83cc9e0f580a80f07cb73a421d00699` | FE39 unresolved resume-contract/blocked-scope record, preserved verbatim |

The changed-path boundary is exactly `frontend/src/features/archive/**`, `frontend/src/features/admin/mural/**`, `frontend/src/features/admin/closure/**`, `docs/mcr/frontend/archive.md`, `docs/mcr/frontend/wall-control-closure.md`, and this new document: **63 final paths** (60 feature/test/evidence files and three documents). `git diff` against each corresponding source checkpoint confirms every feature tree and both imported documents are byte-identical. Cherry-pick metadata retains original authors and exact source commit references.

Specification, project assessment and execution plan bytes match the previously read assignment revision `cd7192f`; their requirements were not amended by this integration. `docs/source` remains byte-identical at Git tree `6a0c3f08c5b3f4d8f56168be1e9f71250bc55659`.

## Owner evidence reused without browser reruns

No browser suite was rerun and no duplicate SL/code-review gate was introduced. Source-owner browser evidence is reused only after verifying all recorded source/contract hashes against this candidate and comparing all retained feature/evidence bytes to the source checkpoints:

| Durable owner report | SHA-256 | Reuse boundary |
| --- | --- | --- |
| `frontend/src/features/archive/__tests__/evidence/browser/report.json` | `ab77e836669b908558e8b38ee1085647673ee89900f610dcc0b91eb0b9895886` | PASS, 17 source/contract hashes all match; 15 image hashes verified; native desktop Chrome `156.0.8078.80`, mobile-touch 360/390/430×844 |
| `frontend/src/features/admin/mural/__tests__/evidence/browser/report.json` | `528c005a8e4315f0bdc40c05dffee6e80c369af282cf7dd0b9e43875a0b36a6f` | PASS, 22 source/contract hashes all match; all nine retained PNG bytes match FE39 source; same Chrome/widths |

These remain owner-run fixture/browser evidence. They are not a fresh browser or physical-device result at the integration commit. The available working-subset semantics remain exactly those in the two imported feature documents, including public/private cache separation, role/privacy withdrawal, one-command idempotency, unknown-result status recovery, truthful partial closure, explicit confirmations and no automatic mutation replay.

## Affected combined checks

All fresh checks ran on tested code HEAD above, using retained dependencies with Node `24.22.2`, pnpm installation `11.7.0`, Vite `8.1.3`, and Vitest `4.1.9`. A temporary authorized `frontend/node_modules` symlink supplied existing dependencies; no install or shared cache/manifest write was performed. It was removed before the boundary/clean handoff check. TypeScript disabled incremental output; Vitest disabled its cache. Build artifacts went only to `/tmp`.

The focused combined test command was:

```sh
frontend/node_modules/.bin/vitest run --root frontend \
  src/features/archive/__tests__ \
  src/features/admin/mural/__tests__ \
  src/features/admin/closure/__tests__ \
  src/features/event-feed/__tests__ \
  src/features/mural/sync/__tests__ \
  src/features/mural/editor/__tests__ \
  src/shared/api/__tests__ src/shared/query/__tests__ \
  --maxWorkers=2 --no-cache
```

| Check | Exit / result |
| --- | --- |
| Focused combined tests above | **0; 460 tests / 15 files pass**. Includes archive/wall/closure and direct feed, mural, canonical HTTP/error and privacy/key consumers. No snapshots updated. |
| `frontend/node_modules/.bin/tsc --noEmit --incremental false --project frontend/tsconfig.json` | 0; whole frontend strict types pass |
| `frontend/node_modules/.bin/oxlint` on `features/{archive,admin/mural,admin/closure,event-feed,mural/sync,mural/editor}` and `shared/{api,query}` | 0; zero warnings/errors |
| `frontend/node_modules/.bin/vite build frontend --outDir /tmp/mcr-lifecycle-app-build` | 0; existing application production build passes, 317 modules |
| Vite API three-entry ES library build described below | 0; archive/wall/closure entry points compile together, 105 modules |
| `python3 scripts/mcr/check-plan.py` | 0; source archive/export/planning provenance PASS |
| `node scripts/mcr/check-ui-origin.mjs` | 0; C08 PASS, 769 scanned paths, no violations |
| `python3 scripts/mcr/check-boundaries.py --base 683484baaf78792292337ad0c197a5d574536db3 --lane frontend --allow-doc docs/mcr/frontend/archive.md --allow-doc docs/mcr/frontend/wall-control-closure.md --allow-doc docs/mcr/evidence/frontend-lifecycle-integration.md` | 0; exact assigned frontend/doc boundary PASS after dependency symlink removal |
| `git diff --check`; explicit changed-path/import/source-byte review | 0; no whitespace/scope conflicts, source archives and source feature bytes preserved |

The application build alone does not prove that unregistered features are mounted. Therefore a separate targeted bundle compiled all three public feature barrels using Vite's `build` API with `root: frontend`, `configFile: false`, React plugin, target `es2022`, `build.lib.entry` mapping `archive`, `wall`, `closure` to their respective `index.ts`, format `es`, external `react`, `react-dom`, `react/jsx-runtime`, and output `/tmp/mcr-lifecycle-feature-build`. It produced all three entries and their CSS/shared chunks. No app root was changed. Build warnings include existing >500 kB application chunks and the CLI's external-output-directory notice; no error occurred. The standalone archive entry includes resolved contract schemas (867.18 kB uncompressed), not a certified production route-size target.

Supplemental local logs: `/tmp/mcr-lifecycle-{tests,types,lint,app-build,feature-build,plan,origin,boundaries}.log`. `/tmp/mcr-lifecycle-source-proof.json` records the same hashes below. Durable feature reports/screenshots remain committed in their original assigned feature evidence paths; this record preserves the combined commands, outcomes and exact tested code state without duplicating those media files.

## Exact candidate source/input fingerprints

Manifest SHA-256 is computed from sorted tracked paths in each tree, each record `path + NUL + sha256(file bytes)`, joined with LF and one final LF. It covers source, tests and retained evidence, and is reproducible without dependencies.

| Feature tree | Files | Git tree | SHA-256 manifest |
| --- | --- | --- | --- |
| `frontend/src/features/archive` | 28 | `5ad1ffcb671e0dd6791ccfb09496a2b6e4d596a9` | `ae173fc3159baba8d55e306e2e4921c152241a47efeaef6659a4ef9cd218192f` |
| `frontend/src/features/admin/mural` | 26 | `b664fd5dda8a51952172ce832fbfc2dea80ca273` | `a69ddfa196253c50888c24e41d5d830efcfc9884fa8e6eb1faa87411062e9c34` |
| `frontend/src/features/admin/closure` | 6 | `6acd9f0e0748fba4998fd30d122fed2b66c949cd` | `7ab56d65c58360342f32d2074be971f0b910bc38b31a874ca585c48b1a0611b8` |

| Contract or direct shared consumer | SHA-256 |
| --- | --- |
| `contracts/domain/v1/admin.schema.json` | `b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8` |
| `contracts/domain/v1/mural.schema.json` | `345871ce89fabff02f06087fcd3e8907651a5712b34ccbd53091158af123b6ca` |
| `contracts/domain/v1/operations.json` | `411fcee8222db2f5a7938866ac37791033e85682f11d024ceb895c245c708500` |
| `contracts/domain/v1/policy.json` | `01c6b9e9a44b81bcf1273ebd7ba8361d3cc035a969399b4dad1f9dedec05839a` |
| `contracts/generated/mcr-domain.openapi.json` | `edd8f89cda100a3f031d6e9b47ff39b615136938faab59259c7351851e139f1b` |
| `frontend/src/shared/api/http.ts` | `441983a9e40cea38ad2f93a5a7101ee22a0d10dc3f50ec9b6ecf878dcd4c70a1` |
| `frontend/src/shared/api/domain-operations.generated.ts` | `9e48ad68c1f1b6ff0ae876fae0355bdc873ff26907a29ec004469ac4aa191484` |
| `frontend/src/shared/query/privacy.ts` | `9f39e9e25b1c322bbe4b2f19a52d930aa369940757e01611ed632aa11cf2f081` |
| `frontend/src/features/event-feed/contract.ts` | `3752e9f21488cd149af6d4d4e5baa0c09f34502dd7366cad1d14491b574ba860` |

## Preserved limitations and final handoff

Fresh inspection confirms `admin.CloseEvent` still has only `expected_revision`, literal `confirmation: CLOSE_EVENT` and `reason`; `closeConcertEvent` still references that exact schema. There is no pinned closure START/RESUME request union. Refreshing a job's progress/status never retries a failed closure step. The imported FE39 document and original author's blocked goal remain authoritative for that unfinished requirement. This integration goal can complete because its bounded job is to integrate and validate the working checkpoints, not silently remove FE39's missing retry scope.

No live BE41 or qualified role/media provider, root route mounting, durable server closure/audit/deletion, real multiuser/world effects, physical iOS/Android, new browser run, IN04/IN05 or UIV01 presentation acceptance is claimed. Those are separate work; this functional checkpoint does not wait for them. No contract amendment, broad retest, production wiring, feature rewrite or extra approval gate was added.

The final READY SHA is the commit containing only this new evidence document atop tested code HEAD. The clean/stopped checkout, absence of dependency symlink, exact final paths, unchanged feature/input hashes and actual goal completion are verified and sent directly to Main/Delivery/Operations. Main may publish this exact candidate promptly; this worker neither pushes nor marks the incomplete FE39 task complete.
