# FE-REF-CUSTOMIZE-WHOLE

## Assignment and boundary

The whole-character follow-up reuses the clean isolated Customize checkout on `codex/mcr-fe-REF-CUSTOMIZE-WHOLE`, starting at `3842354b56ed15a961e23b1cc4d31b52f46cec48`. The previous layered presentation and evidence remain published; they are not duplicated. This patch owns only NEW `frontend/src/features/avatar/whole-character/**` and this document. All existing layered editor/controller/contract, entity/runtime, Auth, shared, root, barrels, central manifests and locks remain unchanged by this worker.

The later explicit whole-character assignment adds a genuine 1.1.0 variant. It does not certify the original layered hairstyle/eye/outfit/tone/walk-frame targets as complete or silently remove that specification scope. The old layered 1.0 path stays available under the root discriminant. Root owns dispatch and verified-response refresh into profile/world. The whole editor exposes no unsupported skin, layer, accessory, pose or animation controls.

## Canonical ports and ownership

Contract source is `3398d8331863775ecc0180e5eccebddd179fedd5`; canonical schemas are `contracts/assets/v1/avatar-{config,manifest}.schema.json` and golden fixtures are `contracts/fixtures/assets/selected-static/`. Config has exactly six fields: contract_version 1.1.0, kind avatar_config, manifest_id mcr_selected_static_20261103, manifest_version 1, representation whole_character, preset_id photo01_front_01..06. The reviewed manifest hash is `sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`.

Feature entry points (direct imports; no barrel modifications):

- `whole-character/controller.ts`: `createWholeCharacterEditorController({auth, scope, runtime, onCommitted?})`.
- `auth` is the canonical SessionController privacy/request subset; `scope` is a copied captured PrivacyScope, fixed for this controller's lifetime. `runtime` is a borrowed producer owner, structurally choices(signal)/isCurrent for state. Root provides the complete genuine runtime to the UI.
- `whole-character/WholeCharacterEditor.tsx`: `WholeCharacterEditor({controller})` consumes the genuine `WholeCharacterView` and full runtime. Visible main view and three paged thumbnails each own separate revocable leases; they never move another sink's canvas.
- Controller API: read/subscribe/start/stop/dispose/select(id)/random/reset/reload/retry/resolveConflict(keep-draft|use-server)/save, with runtime and scope exposed readonly.
- onCommitted receives only a verified saved response or an explicitly adopted verified reconciliation receipt. It never publishes a speculative draft.

Runtime is **borrowed** from the root's fixed-scope owner shared by Header/Me/World/Customize. Controller stop/dispose aborts only its own requests and removes state/subscriptions; view cleanup releases only its own leases. It never invokes runtime.dispose. Root separately disposes the shared owner on scope/application teardown. Canonical privacy invalidation revokes controller state synchronously; the runtime independently invalidates leases and image data. The controller never captures a replacement identity or rebinds itself.

## Behavior and transport

The UI presents a centered large front-idle figure on cream graph paper, a black CHARACTERS caption, six genuine choices across two pages of three with accessible arrows and red selection, then Random/Reset/Save. Labels come from the producer's validated manifest plus reviewed source release. No selection is invented before the self participant is verified. Pixel scaling uses the compact reviewed atlas through the shared runtime; the 256px on-screen main view does not automatically download the separate detail atlas.

Select, Random and Reset change only the local draft. Random excludes the current configuration. Reset restores the verified baseline while retaining unresolved unknown/conflict state and is blocked while a server candidate awaits an explicit decision. Save is explicit, disabled for unchanged/pending/unreconciled state, and sends only `{expected_revision, config}`. No mutation is automatically retried. A successful receipt must match the submitted whole config, the exact next avatar revision and the captured identity/event/generation.

GET `/api/concert/events/{eventId}/me` validates self event/user/profile identity. PUT `/api/concert/events/{eventId}/me/avatar` uses the canonical protected core request path. C03 and the existing HTTP client require Bearer credentials, JSON, same-origin browser Origin and X-MCR-Intent: browser. This route has no Idempotency-Key: optimistic expected_revision is its concurrency fence. No feature-created credential client, arbitrary headers or alternate endpoint is added. Backend authorization and manifest validation remain mandatory.

Conflict or uncertain outcome keeps the draft and prevents another save. Check latest loads a candidate; Keep my draft rebases its revision, Use saved character explicitly adopts it. Reset/retry cannot bypass this decision. The controller retains the highest verified revision/config across reloads and stop/start, rejecting both older data and same-revision contradictory data. Stop/start preserves a valid scoped draft and revalidates the server baseline. If both revision and configuration are unchanged and no conflict/unknown result exists, editing resumes without an unnecessary decision. A changed baseline, unresolved outcome, or failed fresh read keeps saving blocked. Logout, policy revocation and final dispose erase draft, candidate and private revision state; late responses cannot call root.

## Provenance and current evidence

All UI is newly authored. Scoped request, immutable state, cancellation and reconciliation principles are audited from existing `features/avatar/{controller.ts,contract.ts}` at the base; no layered wire adapter is reused. New deterministic HTTP fixture scaffolding derives from `features/avatar/__tests__/fixtures/harness.ts`, with canonical selected-static configs replacing synthetic layered metadata. Native fixture scaffolding derives from `features/avatar/__tests__/reference-target/{native.mjs,preview.tsx}`; character rendering is the real producer and admitted originals/derivatives, never synthetic character geometry. Dependencies are retained React/Vite/Vitest/Playwright/Zod/query primitives; no install or package/lock change.

Source commit: `3c4dc76ffc0c336b5aeb76504554353bfb6e672b`, parent validation base `a3a6f5835f19bd556c71a046e60160297e1b9560`; original assignment base remains above. Runtime producer `9fbc89426c6d0781c6f7a1f1a78ef722656407e5` and contract `3398d8331863775ecc0180e5eccebddd179fedd5` are in that ancestry. Reviewed art source/release is `7e07f7328f079991a9aedb7ad83a4b682decb543`. These external revisions were consumed from fresh published master, not republished as duplicated patches.

| Gate | Status and evidence |
| --- | --- |
| Actual runtime + source-derived art in functioning component | INTEGRATED: real Chrome RGBA comparison and six canonical figure choices at 360/390/430px |
| Canonical 1.1.0 wire structure | CONTRACT_READY: published schemas, strict producer config validator, exact complete self projection and matching revision-fenced response |
| Self load/save behavior | MOCK_READY: deterministic canonical HTTP; 13 new state cases and all 54 existing avatar cases pass (67 total / eight files) |
| Strict project types / scoped lint / plan | Exit 0 each |
| Default application and explicit whole-editor production entry builds | Exit 0 both; only existing large-chunk warnings; temporary outputs removed |
| Genuine-art native interaction | PASS: six bounded PNGs, three mobile widths, touch/keyboard, 44px controls, RGBAs, explicit save/unknown-result recovery, warm return, owner/lease/privacy lifetime |
| Live BE service / actual root whole dispatch / World propagation / multiuser / physical devices | NOT_RUN; these separate gates are not established by fixture HTTP or a successful build |

Native source binding is in `whole-character/__tests__/evidence/native/report.json`; the report hashes the exact product, fixture and actual producer sources before browser execution. `evidence/validation.json` pins all delivered source/evidence plus schema, fixture, target and admitted public-art bytes, commands and exit codes. The final publication SHA, fresh remote tip and ancestry receipt are sent to the authorized owners after the non-force push. Source then evidence commits form a serial handoff; any subsequent reconciliation merge preserves peers.

The exact target SHA-256 is `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. `comparison-390.png` displays its screen 5 (x1238/y0/298×530) beside this working whole-character variant at equal width/aspect. The selected six complete source figures differ from the target's layered hair example; this explicit whole-character variant exposes genuine whole choices rather than claiming isolated hair/skin customization. Original ten-screen/full layered visual acceptance remains open. Both comparison sides are visible; no static screenshot replaces the working editor.

Native artifacts: `characters-360.png`, `characters-390.png`, `characters-430.png`, `sixth-character-390.png`, `uncertain-reset-390.png`, `comparison-390.png`. Tests retain the original layered regression assertions. Early native failures were fixture JSON-property-order comparison (fixed to structural equality) and browser focus loss when a pager became natively disabled. Pager boundaries now expose aria-disabled, guard actions and retain keyboard focus; final native coverage verifies repeated Enter at the boundary is a no-op. Peer review found retry bypass and stale candidate regression: retry is restricted to unavailable art, and the highest verified revision/config survives reload/stop/start; final focused recheck found no remaining issues. The self decoder also rejects extra/missing C03 projection fields and distinguishes participant revision from nested avatar revision.

Reproduce from `frontend/` with retained Node24.22.2 and the existing locked dependency tree:

- `node node_modules/vitest/vitest.mjs run src/features/avatar --configLoader runner --no-cache`
- `node node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false`
- `node node_modules/oxlint/bin/oxlint src/features/avatar/whole-character`
- From repository root: `node frontend/src/features/avatar/whole-character/__tests__/native.mjs` and `python3 scripts/mcr/check-plan.py`.
- Vite production builds use private mkdtemp output: default app plus `build.rolldownOptions.input` set to the owned native preview.html so the new editor is explicitly bundled even before root dispatch lands. Output directories are removed in finally. Native uses one ephemeral Vite port, an isolated Chrome profile, a private cache and a >8GiB free-disk guard; its temporary directory is removed in finally.

Only the worker-owned node_modules symlink and private build/browser/cache artifacts are removed at clean handoff. No install, extra worktree, source/original deletion, backend build, forced push, model override, or shared ownership takeover occurs. This source worker may publish NEW qualified scoped commits under the latest explicit human policy; the old Customize commits remain untouched.


Provenance detail: base FE13 controller/contract principles map to new whole-character/controller.ts and model.ts; strict Unicode/self-envelope parsing is adapted from `features/me/model/contract.ts` at the validation base into model.ts. Existing fixture harness maps to new __tests__/fixtures.ts; prior reference-target native/preview scaffolding maps to the new native.mjs/preview.tsx. Source and destination hashes are pinned in validation.json; source files remain unchanged.
