# FE13 / UIV04 compact Customize presentation

Status: **MOCK_READY** for this presentation increment. Approved-art appearance,
real root/account/world integration, physical devices and full task/release
acceptance remain **NOT_RUN**. No acceptance counts change.

Base: `215da400385c400f059913c826105ca4ec5f134b`, branch
`codex/mcr-fe-customize-layout`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/fe-customize-layout`. This document is committed
with the patch directly after that base; `git log --format=%H --all --
docs/mcr/frontend/avatar-compact-layout.md` resolves its exact delivery ancestry.
The former option-preview checkout remains read-only. Only Main integrates and
pushes `julymeltdown/mcr:master`.

## Presentation change

The inspected ten-screen reference's CUSTOMIZE screen has a large character,
compact category rail, one choice row with side arrows and immediate actions.
The prior loaded 390 capture placed an always-visible duplicate preset grid and
extra paging/description rows between that chooser and Save.

- The three same-owner full-outfit previews are now the primary chooser. Their
  heading and count share a line, with 44px paging buttons at the sides.
- Native preset/ID controls remain in a closed native **Choose by ID** disclosure
  for each category. Opening one is explicit. All ID labels, unsupported options,
  accessory None and existing handlers remain available.
- Only the active tabpanel mounts an option strip. It precedes the disclosure in
  DOM order as well as visual order. Existing named vertical tabs retain their
  Up/Down/Home/End behavior. The rail is 68px wide, with seven 44px controls and
  concise visible names; no new icon vocabulary is required.
- Option labels ellipsize visually to preserve the compact row. Complete button
  accessible names, source-ID titles and disclosed native labels remain intact.
- The editor uses a bounded flex window, with scrolling content and an in-flow
  footer outside that scroll area. Save/Cancel remain 44px targets. The window
  follows its available parent height, capped at 46rem and the dynamic viewport
  minus 4rem. A minimum usable height of up to 20rem lets a shorter parent panel
  scroll externally rather than collapsing the editor body below one control.
  Header/footer do not overlay content or global navigation.

This patch changes only presentation in `AvatarEditor.tsx`, `OptionStrip.tsx` and
`avatar-editor.css`. Controller, HTTP, scope, schema, resource and compositor
files are byte-identical to the base. The three-lease/one-option-flight lifecycle,
main lease, explicit Save, Cancel, failure and uncertain reconciliation semantics
are unchanged. No art, swatch, asset URL, renderer, socket, cache, identity,
dependency, implicit write, randomizer or reset behavior is added. There are no
decorative left lines.

## Provenance and exact scope

The original reference is
`docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
It and the base's `option-previews/evidence/native/loaded-390.png` were viewed.
Newly authored product layout uses the existing ConcertWindow/Tabs primitives.
No legacy rendering or image extraction is copied.

The new `__tests__/compact-layout/` scaffolding adapts the option-preview fixture
from `bcb0428d01db2bdfa9970c4cb56cf2d53a45ab67`, as present at the pinned base.
It imports that read-only `option-previews/data.ts` and the existing FE12
`world/avatar/__tests__/fixtures/png.mjs` geometric generator. Long-label mode
renames only a synthetic preset ID, maintaining schema validity/content hash.
Nested mode imports the unchanged `app/shell.css` and reproduces its half-panel,
header, scroll body and bottom navigation structure. This tests layout nesting;
it is not a claim that a real root route, Auth provider or world flow ran.

C03 participant, avatar write and error fixtures and C06 AvatarConfig/Manifest
v1 remain the same inputs. Dependencies are retained React/Query, Vite 8.1.3,
Vitest 4.1.9, Playwright and TypeScript. There was no installation or lock change.
The supplied originals, `docs/source/`, manifests, contracts, shared primitives,
backend and root composition are unchanged.

Writable existing paths are exactly the three presentation files above, plus
Main's additional narrow grant for
`__tests__/visual-adoption/presentation.test.tsx`: two lines in its first category
loop open the active details only if closed. Its original visible combobox,
unsupported, selection, lease, no-PUT and Cancel assertions are retained.
New paths are `__tests__/compact-layout.test.tsx`, `__tests__/compact-layout/**`
and this document. `evidence/initial-presentation-failure.json` preserves the
obsolete always-visible assertion and initial new-test query failures.

## Verification — 2026-11-04

Each `node` below uses retained
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (24.22.2).
Temporary dependencies were linked from the integration checkout, then the link
was removed for handoff. Runner config loading, no Vitest cache and no TypeScript
incremental output were used. Build/cache output stays in `/private/tmp`.

From `frontend/`:

```text
node node_modules/vitest/vitest.mjs run src/features/avatar/__tests__/compact-layout.test.tsx src/features/avatar/__tests__/option-previews.test.tsx src/features/avatar/__tests__/ui.test.tsx src/features/avatar/__tests__/visual-adoption/presentation.test.tsx --configLoader runner --no-cache
node node_modules/typescript/bin/tsc --noEmit --incremental false --pretty false
node node_modules/oxlint/bin/oxlint src/features/avatar/AvatarEditor.tsx src/features/avatar/OptionStrip.tsx src/features/avatar/__tests__/compact-layout.test.tsx src/features/avatar/__tests__/compact-layout src/features/avatar/__tests__/visual-adoption/presentation.test.tsx
```

From the worktree root:

```text
node frontend/src/features/avatar/__tests__/compact-layout/native.mjs
node frontend/src/features/avatar/__tests__/compact-layout/production.mjs
python3 scripts/mcr/check-plan.py
git diff --check
```

The focused tests pass **26/26**. They cover closed/disclosed ID controls,
category navigation, one chooser, native selection, Save/Cancel, failed and
conflict/uncertain draft recovery, no implicit PUT, paging, bounded old/new
leases, StrictMode and synchronous privacy/unmount teardown. Whole-project
strict types and scoped lint pass. The production build compiles this explicit
test entry; it does not certify root composition. Planning validation passes
only its planning/provenance/export obligations.

Native evidence under `compact-layout/evidence/native/` uses installed Chrome
156.0.8078.80 and the actual editor/controller/shared compositor. Full host
widths **360/390/430**, heights **844/560**, cover loaded and distinct draft
captures, keyboard Tab order/Enter paging, touch selection, 44px controls,
unclipped focus/hit tests, full names for a long valid ID, disclosed native
selection, no browsing PUT, explicit error/uncertain saves and revision-4
reconciliation, same-owner RGBA pixels, bounded ignored-abort churn, zero-canvas
policy teardown and actual reviewed-entry unavailability with zero asset reads.
All images are explicitly **TEST ONLY** geometric fixtures, never product art.

Peer review found a real half-panel issue in the first bounded-height version:
the content viewport could shrink to 16px at 390×560. The minimum-height/outer
scroll correction is exercised with the actual shell CSS and matching structure
at **360/390/430×560**. Tests require a usable inner viewport and fully revealable
paging, ID and Save controls inside every clipping ancestor, above navigation;
all three content viewports measure **192px**. The 390 case also exercises
uncertain recovery there. The peer rechecked the corrected source/report and
reported the P2 resolved, with no remaining actionable findings. Artifact reports retain measured
geometry, source hashes, command outcomes and explicit writes. Initial fixture
corrections included jsdom details visibility queries, a too-long synthetic ID
and ignoring `display:contents` as a nonexistent clipping box.

The loaded 390×844, short uncertain 390×560, long-ID 360×560 and nested
action/recovery captures were visually inspected. The nested host is explicitly
a structural fixture; its decorative presentation is not product acceptance.

`evidence/checks.json` pins source and evidence SHA-256 values; native and
production reports pin the tested product sources and bundle outputs. Approved
artwork still needs the pending human extraction-method decision. Physical
mobile/browser combinations, live backend multiuser behavior, actual root-route
composition, final reference/user approval and release acceptance are **NOT_RUN**.
