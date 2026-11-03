# FE05 independent UI-primitives review

**No remaining actionable findings** against worker commit
**`8f9ddf7351c3f504efe9265ed1708e8a986d99bd`**.
Review date: 2026-11-03. Worker assignment base:
`2b42fd479887a3101f51a83ac9dc81fca2a4fe2f`.
Integration and FE05 acceptance remain the parent's decision; this review does
not accept QA02, QA07, FE07, physical-device behavior, or product flows.

## Exact reviewed scope

All 13 paths changed by the worker commit were inspected:

- `frontend/src/shared/ui/controls.tsx`
- `frontend/src/shared/ui/Dialog.tsx`
- `frontend/src/shared/ui/modal-focus.ts`
- `frontend/src/shared/ui/Tabs.tsx`
- `frontend/src/shared/ui/feedback.tsx`
- `frontend/src/shared/ui/index.ts`
- `frontend/src/shared/ui/__tests__/dialog.test.tsx`
- `frontend/src/shared/ui/__tests__/primitives.test.tsx`
- `frontend/src/shared/ui/__tests__/browser-smoke.mjs`
- `frontend/src/shared/ui/__tests__/fixtures/preview.html`
- `frontend/src/shared/ui/__tests__/fixtures/preview.tsx`
- `frontend/src/shared/styles/base.css`
- `docs/mcr/frontend/ui-primitives.md`

The patch stays within the frontend primitives/styles and its assigned document.
No backend, manifest, lockfile, shared contract, router, entrypoint, or feature
module was changed. Runtime imports use React/ReactDOM and local modules; no
legacy rendering or cross-stack implementation import was found. The independent
fixture is not an application route.

This review follows specification FE05 and §5.1 keyboard, focus, dialog dismissal,
and 44px-target criteria. It does not impose later feature or device gates on
this primitive increment.

## Resolved preliminary issue

An uncommitted preliminary snapshot inverted modal priority when an outer dialog
and already-open inner dialog mounted together. The reviewer reproduced that
with actual DOM focus and Chromium: the inner overlay became inert, focus stayed
in the outer dialog, and Escape removed both dialogs.

The final commit fixes this with parent-dialog identity, ancestry-aware layer
insertion, top-layer-only initial focus, and depth-based paint order. The added
worker regression covers simultaneous mounting and nested close/restoration.
The independent initial-open probe also passes against the exact final commit:

```json
[
  {"title":"Inner","inert":false,"focusInside":true},
  {"title":"Outer","inert":true,"focusInside":false}
]
```

After Escape, exactly **one** dialog remains: the outer dialog. The preliminary
finding is therefore resolved and is not an open finding on `8f9ddf7`.

## Independent execution

No application tree was edited. UI/style/test blobs were read directly from
`8f9ddf7` into a temporary harness. The final component/style/test bytes were
refreshed from that commit before execution. Installed dependencies were reused;
review fixtures and Vite cache/configuration stayed in the temporary harness.
The copied browser driver only adds `configLoader: 'runner'`; the harness config
selects a temporary cache and explicit React preoptimization. Component code,
styles, fixture behavior, and worker assertions are unchanged.

Runtime: Node **24.22.2**, Vitest **4.1.9**, Chromium **156.0.8078.76**.
No package installation, full build, source repository test suite, live API,
database, user profile, hardware, or production data was used.

| Executed check | Result |
|---|---|
| `node node_modules/vitest/vitest.mjs run --config vite.config.mjs` in the temporary harness | Exit **0**; **24 tests passed** across three files in **1.56s**: the worker's 22 tests plus two independent probes. |
| Copied `browser-smoke.mjs` with explicit local Chromium | Exit **0**; passed **360×800, 390×844, 430×932, 1280×800**; no page errors. |
| Independent native-browser initial-open nested-dialog probe | Exit **0**; inner remains active, outer inert, Escape leaves only outer. |
| `node scripts/mcr/check-ui-origin.mjs --root /Users/lhs/Desktop/mcr-worktrees/fe-foundation` | Exit **0**; **22 files / 15 content files**, no violations, zero reviewed extracts. |
| `python3 -B scripts/mcr/check-boundaries.py --base 2b42fd479887a3101f51a83ac9dc81fca2a4fe2f --lane frontend --allow-doc docs/mcr/frontend/ui-primitives.md` | Exit **0**; all 13 changed paths allowed. Manual path/import review also completed. |

The independent probes cover a complete keyboard journey through roving tabs
inside a modal, and nested dialogs initially open on the first render. The first
verifies ArrowRight activation, Tab into the active panel, Tab into its input,
and Shift+Tab back to the panel; the second independently checks active focus and
background isolation rather than counting focus-method calls.

The worker tests and native browser run corroborate labelled fields and error/
description associations; non-submitting/ref/busy/native buttons; roving tab
focus, orientation, disabled skipping and panel associations; forward/backward
modal containment; hidden/disabled/dynamic controls and radio grouping; Escape,
IME/child cancellation, explicit close and backdrop gesture discrimination;
initial/nested/return focus; background inertness and scroll restoration;
bottom-aligned sheets; persistent live regions and notification focus behavior.
The browser additionally measured visible 44px targets, modal bounds, horizontal
overflow and reduced-motion computed styles at the stated viewports.

## Limits

Physical iOS/Android, Safari/Firefox, on-screen keyboard/visual-viewport behavior,
screen-reader announcement delivery, forced-colors rendering and touch hardware
remain **NOT_RUN**. Narrow Chrome viewports are not physical-mobile acceptance.
Light-DOM tests do not qualify embedded-frame or Shadow DOM internals. No claim
is made about future consumers' labels, asynchronous tab loading, routing/history,
Phaser input suppression, or feature-specific destructive confirmations.

The originating worker checkout was clean at the final read. This reviewer wrote
only `docs/mcr/qa/ui-primitives-review.md` in the assigned QA worktree. Historical
foundation review and later clean-install/lint receipts remain separate and
unchanged. Parent integration must still review combined changes and run its
relevant integrated checks on the delivered candidate.
