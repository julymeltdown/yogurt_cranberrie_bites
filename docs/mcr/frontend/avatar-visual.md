# UIV04-AVATAR presentation handoff

Status: **MOCK_READY; READY for scoped delivery integration.** This is frontend presentation evidence, not FE13 acceptance, approved-character readiness, a live save journey, or IN05 closure.

## Ownership and ancestry

- Exact parent: `b3889feeed0fa7485a2326cf5bd00ffc4bb4f894`.
- Isolated branch/worktree: `codex/mcr-fe-UIV04-avatar`, `/Users/lhs/Desktop/mcr-worktrees/fe-avatar-visual`.
- Production changes: `frontend/src/features/avatar/AvatarEditor.tsx` and `avatar-editor.css` only.
- New tests/harnesses/captures: `frontend/src/features/avatar/__tests__/visual-adoption/**`; this document is the only documentation change. The delivered commit is the commit introducing this document, with the exact parent above. Integrator records its resulting master ancestry separately.
- Existing controller, preview adapter, renderer, contract/catalog, source pack, account entry, base CSS, shared primitives, dependency manifests and lockfile remain unchanged. No backend imports, copied legacy rendering, new dependencies, asset bytes or generated artifacts.

## Reference and presentation

Read the implementation plan, assessment, execution plan, avatar-editor evidence, and UI reference review; viewed the supplied ten-screen reference and source-bound before/after captures. Reference: `docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.

The CUSTOMIZE reference informs the compact black title, ivory symmetric frame, large preview area beside category navigation, black category caption, and explicit red Save/paper Cancel footer. The existing DRESS UP route heading is retained. Reused read-only `ConcertWindow`, `ConcertTabs`, and `ConcertAction` from the shared concert-window kit; all new application presentation is authored here. No left accent rails or borders are introduced.

Presets, all five existing layers, accessory None, every supplied tone, selected values, saved-item-unavailable placeholders, and the original eligibility predicates remain available. Vertical tabs retain the shared roving focus and Arrow Up/Down/Home/End behavior. Category navigation changes only local presentation state; it does not select items, rerender the preview, persist, randomize or reset the draft. Controls are at least 44 pixels; selected tabs use both text decoration and color, and keyboard focus has an outline. Short windows scroll to both footer actions.

The catalog exposes IDs/layers and tone IDs, without item thumbnails or tone colors. Choices therefore use existing ID labels and native selects. Reference-style image tiles and colored swatches remain an explicit metadata/art dependency. No invented thumbnails, swatch colors, asset URLs, contract fields or replacement characters are supplied.

## Behavior retained

The existing controller continues to own preview leases, loading/unavailable states, draft changes, server authorization failures, pending writes, uncertain results, and revision reconciliation. Save uses the original gate and handler; pending Save additionally exposes `aria-busy`. Cancel retains its original handler and availability. The unresolved-result hint explains that local Cancel does not resolve an uncertain server result. Checking the latest outfit and choosing Keep my draft/Use saved outfit do not automatically replay a write. A retained draft uses the adopted server revision only on the next explicit Save.

Privacy cleanup still removes mounted pixels and private draft state. No asset admission or renderer behavior changes. The synthetic C06 manifest is used only to exercise actual production rejection, with zero asset reads; it is not approved product art. Successful editor fixtures use the existing explicitly test-only blank-canvas preview port, labeled in every fixture capture. No character images are fabricated.

## Validation and captures

Machine-readable commands, working directories, exit codes, corrected probe errors and limits are in `frontend/src/features/avatar/__tests__/visual-adoption/evidence/validation.json`. Final source hashes and all 81 PNG hashes are in `evidence/index.json`; per-run source hashes and assertions are in `before/report.json`, `after/report.json`, and `account/report.json`. Final after/account source pins were compared with the working tree. Before production pins describe the exact parent; test harness refinements made after the baseline do not change that captured parent.

All commands use pinned Node 24.22.2 and retained dependencies via a temporary symlink to retained integration dependencies, removed before commit. No dependency installation. Vite uses runner configuration and temporary cache directories; TypeScript disables incremental writes; production build output goes to `/tmp/mcr-uiv04-avatar-build`.

| Check | Result |
| --- | --- |
| Avatar + account-entry + shared primitive tests | PASS, 7 files / 50 tests, exit 0 |
| Strict TypeScript, lint | PASS, exit 0 each |
| Production Vite build | PASS, exit 0, 319 modules; existing large CommunityFeatures/Phaser chunk warning |
| Before component captures | PASS, 9 captures: loaded/unavailable/reconciliation at 360/390/430 |
| After component browser | PASS, 60 full-page captures: 13 states plus all 7 categories at each width; 3 keyboard/save/reconciliation flows; no page errors |
| Short-height access | PASS at 420 pixels high, every after-state/width; 9 viewport captures of draft/reconciliation/unavailable |
| Existing default `/me/avatar` composition | PASS, 3 captures at 360/390/430; disabled Save, no canvas, no PUT, one active/peak instrumented world lease |
| Planning provenance / whitespace | PASS, exit 0; planning check does not certify application acceptance |

Focused added regressions cover every category, keyboard navigation without preview renders/writes, unsupported catalog choices, accessory None and Cancel, uncertain-save draft retention across category switches, explicit reconciliation without replay, a subsequent revision-4 Save, and pending Save/Cancel availability. Existing avatar tests retain conflict/adopt-server and synchronous privacy cleanup coverage. Existing account-root tests retain actual lazy-route composition, private draft continuity, identity/policy fencing and one peak world lease.

Representative inspected captures:

- Before: `evidence/before/loaded-390.png`, `unavailable-360.png`, `candidate-430.png`.
- After: `evidence/after/loaded-390.png`, `candidate-430.png`, `unavailable-360.png`, `draft-360-height420.png`, `category-hair-360.png`, `category-tones-390.png`.
- Existing account route: `evidence/account/default-avatar-unavailable-390.png`.

The account captures use the existing real route/owner/production art-admission composition with synthetic authentication, HTTP and world ports. React StrictMode mounts twice sequentially and disposes the first lease; active and peak game/connection counts stay one. These captures prove the current unavailable composition rather than an approved-character or real Phaser/server journey.

## Remaining qualification

Reviewed source-pack derivatives and admitted product resources remain unavailable at this base. A live approved-art preview/save across world and profile, real device/assistive-technology checks, backend/database, multiuser/load, release and formal acceptance checks are **NOT_RUN**. The broad 96-file suite is **NOT_RUN**, as assigned. Product thumbnail/swatch metadata requires separate ownership and a reviewed contract decision. Integrator must apply this isolated commit and run relevant combined checks on its resulting candidate; this author does not push master or claim whole-project completion.
