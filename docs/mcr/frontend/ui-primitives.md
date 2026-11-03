# FE05 original UI primitives

## Delivery and scope

Worker branch: `codex/mcr-fe-foundation`. Assignment base: `2b42fd479887a3101f51a83ac9dc81fca2a4fe2f`. Evidence date: 2026-11-03. Delivery status: **MOCK_READY** for isolated component composition; integrator/QA acceptance remains pending.

This implements the FE05 row in `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, following the root/frontend working agreements and `docs/mcr/project-assessment.md` / `docs/mcr/execution-plan.md`. Parent reported C02 qualification and C08 fixture/origin-guard integration before assignment. No shared contract is consumed by these components.

All markup, styles and interaction logic are newly authored. No Bigyeting JSX, CSS, assets, visual dependencies, or headless source modules were copied. No source hashes/extraction entries are needed for this patch because there is no copied source. C01 source-map consolidation remains integrator-owned. Runtime imports use only React/ReactDOM, local UI modules and CSS; existing Vitest, jsdom, Testing Library, Playwright and Vite support the tests.

Changed paths are confined to `frontend/src/shared/ui/**`, `frontend/src/shared/styles/base.css`, and this document. No manifest, lockfile, router, entry, application feature, backend, API/auth logic, theme selection, world runtime, or FE07 shell behavior is implemented. The independent preview is a test fixture, not an application route.

## Composition API

Import named exports and their types from `frontend/src/shared/ui/index.ts`. Import `frontend/src/shared/styles/base.css` once at the browser entry (the current foundation entry already does). Components do not import styles themselves.

| Export | Contract and caller responsibility |
| --- | --- |
| `Button` | Native button props/ref, default `type="button"`; `variant="primary\|secondary\|quiet\|danger"`. `busy` disables activation and exposes `aria-busy`; callers retain a meaningful action label and supply progress feedback when necessary. Explicit `type="submit"` is supported. |
| `Field` | Labelled native text-like input with `label`, optional `hint`/`error`, native props/ref and generated or explicit unique ID. Hint, external `aria-describedby`, and error IDs compose. `required` uses native semantics plus visible text; errors expose `aria-invalid` and an alert. No validation or submission policy is included. Use native select/textarea/checkbox markup when those controls are needed; this input does not pretend to support every form widget. |
| `Dialog` | Controlled `open`, `title`, `children`, `onClose(reason)`; optional concise `description`, stable `initialFocusRef` / `returnFocusRef`, `dismissOnBackdrop` (default true), `closeLabel`. Reasons are `escape`, `backdrop`, `close-button`. The callback must update `open` to dismiss. |
| `Dialog presentation="sheet"` | Same modal/focus/dismissal contract with bottom-aligned responsive geometry and safe-area padding. No duplicate Sheet wrapper, swipe dismissal, snap points, or navigation/history ownership. |
| `Tabs` | Controlled `value` / `onValueChange`, labelled `items` with unique stable `value`, `label`, `content`, optional `disabled`; optional vertical orientation. Update selection synchronously. This is for immediately available panels, not network-on-focus navigation. Invalid/disabled selection displays the first enabled item. |
| `Feedback` | One component for `kind="status\|error\|empty"`, `title`, body and optional `action`. Status uses `role="status"`, error uses `role="alert"`, empty is an ordinary labelled section. Callers choose truthful text and wire recovery actions; no request is retried automatically. |
| `Skeleton` | Static decorative lines with a loading status label; optional `lines` clamped to 1–8. No shimmer or timer. Replace with actual content/error/empty state when known. |
| `ToastRegion` | Keep mounted before notifications arrive. Controlled `messages: {id, message, tone?: 'info'\|'error'}[]` and `onDismiss(id)`; unique stable IDs required. Persistent polite and assertive live lists, explicit dismiss buttons, no timeout or automatic focus on insertion. Info precedes error visually. Do not announce every background update as urgent. |

Example composition (React state and refs belong to the consuming feature):

```tsx
import { Button, Dialog, Field, Feedback } from './shared/ui'

<Dialog open={open} title="Edit draft" initialFocusRef={nameRef}
  onClose={() => setOpen(false)}>
  <Field ref={nameRef} label="Draft name" value={name}
    onChange={event => setName(event.target.value)} error={nameError} />
  {saveError && <Feedback kind="error" title="Could not save">{saveError}</Feedback>}
  <Button onClick={saveDraft} busy={saving}>Save draft</Button>
</Dialog>
```

## Interaction and visual decisions

The modal uses an explicit ARIA dialog and a body portal rather than depending on a mocked native `showModal()` implementation in jsdom. Tests exercise actual `document.activeElement` changes; they neither stub `focus()` nor count focus calls as evidence. Chrome separately verifies real browser inertness, keyboard behavior, stacking, hit testing and geometry.

On opening, focus goes to the supplied target or first enabled visible control (normally Close); an empty focus sequence falls back to the dialog panel. Every Tab press recomputes eligible controls, skips hidden/inert/disabled elements, preserves positive-tabindex order and treats a radio group as one stop. Tab/Shift+Tab cycle inside the top modal. Programmatic background focus is redirected. The page and earlier modals become inert; body scroll and pre-existing inert state are restored after the final modal closes. Newly appended body roots are also isolated. Nested dialogs support ordinary opening, simultaneous mounting, and removing an entire parent/child tree.

Escape affects only the top modal and respects child `preventDefault()` and IME composition. A backdrop dismissal requires both pointer-down and click on the backdrop; dragging from content to the backdrop or cancelling a pointer does not dismiss. An explicit Close button is always available. Focus returns to the invoking element or `returnFocusRef`; a surviving parent receives focus if the child return target is unavailable. Supply a valid return ref when a trigger disappears. Choose an initial ref to a heading (`tabIndex={-1}`) for long structured content or the least destructive action for consequential confirmation; keep descriptions concise rather than flattening structured content into `aria-describedby`.

Tabs use roving focus, Left/Right (horizontal) or Up/Down (vertical), wrap, disabled-item skipping, Home/End and automatic activation. Horizontal tabs leave vertical scrolling keys alone. Panels remain mounted but inactive panels are hidden and removed from navigation/accessibility exposure. Tab from the tablist enters the active focusable panel. Native button Enter/Space activation is retained.

Toast insertion does not take focus. Dismissing a focused notification restores focus to the next/previous dismiss button or the labelled region. Delayed controlled removal does not reclaim focus after the user has moved elsewhere. Place interactive notifications inside an open modal if they must remain usable while the rest of the page is inert; do not use a toast as the sole record of a critical outcome.

Cream surfaces, ink typography/borders, restrained red errors/actions, and non-color selected/error cues form the base style. Native controls have 44px minimum height and buttons/tabs also have 44px minimum width. Visible keyboard focus, 16px input text, wrapping content, scrollable tablists/dialog bodies, safe-area sheet padding, reduced-motion transition removal and forced-color selection outlines are included. This is a base palette, not FE06's three-theme system.

The behavioral reference is the WAI-ARIA APG [modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) and [tabs pattern](https://www.w3.org/WAI/ARIA/apg/patterns/tabs/). These references guide implementation; they do not constitute an accessibility certification.

## Verification

All commands run in the worker checkout. Frontend commands use:

```sh
export PATH=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH
```

| Command (working directory) | Result |
| --- | --- |
| `corepack pnpm@11.7.0 install --frozen-lockfile` (`frontend/`) | PASS, exit 0; isolated install, no manifest/lock changes. |
| `corepack pnpm@11.7.0 exec vitest run src/shared/ui` (`frontend/`) | PASS, exit 0; 22 tests in two files, explicit per-file jsdom environment. |
| `corepack pnpm@11.7.0 typecheck` (`frontend/`) | PASS, exit 0; initial check, with final strict TypeScript check repeated through build. |
| `corepack pnpm@11.7.0 build` (`frontend/`) | PASS, exit 0; strict TypeScript including component/test/fixture files and Vite production landing build. Components are intentionally not wired into the root app. |
| `corepack pnpm@11.7.0 exec oxlint src vite.config.ts` (`frontend/`) | PASS, exit 0; parent owns the equivalent scoped lint-script correction. |
| `MCR_CHROME_EXECUTABLE='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' node src/shared/ui/__tests__/browser-smoke.mjs` (`frontend/`) | PASS, exit 0; isolated headless Chrome 156.0.8078.76, loopback Vite fixture, no browser download or user profile. |
| `node scripts/mcr/check-ui-origin.mjs --root /Users/lhs/Desktop/mcr-worktrees/fe-foundation` (root) | PASS, exit 0. |
| `python3 scripts/mcr/check-boundaries.py --base 2b42fd4 --lane frontend --allow-doc docs/mcr/frontend/ui-primitives.md` (root) | PASS, exit 0; exact assigned document exception. Manual changed-path/import review also performed. |
| `git diff --cached --check` (root) | PASS, exit 0. |

DOM tests cover native button/ref/submit/busy behavior; field associations and required/error changes; tab relationships, focus, orientation and multiple instances; feedback/live regions; toast insertion/dismissal/deferred-removal focus; modal initial focus, forward/backward containment, hidden/disabled/dynamically changed controls, radios, rerenders, StrictMode, nested/simultaneous mounting, parent removal, Escape, pointer cancellation, backdrop and explicit close, background isolation, scroll restoration and fallback return targets.

Chrome tests exercise native keyboard events and rendered components at **360×800, 390×844, 430×932 and 1280×800**, checking tab traversal, actual modal focus cycles/restoration, nested/simultaneous modal stacking and clickable top layer, inert background, sheet bottom alignment and drag-out/backdrop behavior, toast keyboard dismissal, measured 44px targets, page overflow, unclipped modal bounds, and reduced-motion computed styles. No browser page errors were observed. A 390px fixture screenshot was visually inspected; the generated review image is not part of the patch. The fixture bundles the UI through Vite independently of root app composition.

## Limits and remaining acceptance

Physical iOS/Android, Safari/Firefox, on-screen keyboard/visual-viewport behavior, screen-reader announcement delivery, forced-colors rendering and touch hardware: **NOT_RUN**. The tested narrow Chrome viewports are browser layout evidence, not physical-mobile acceptance. No live API, authentication, world connection, hardware, source user data or database was accessed.

The focus manager covers ordinary light-DOM controls in this application; Shadow DOM/embedded frame focus internals are outside this patch's evidence. Keep modal refs stable, do not externally mutate modal-owned inert/scroll state, and review third-party body portals before composition. Long-content zoom, feature-specific destructive confirmations, routing/back behavior, Phaser input suspension and real end-to-end flows require their owning tasks and integration tests. No full FE05/QA02/FE07 acceptance or product readiness is claimed by this component handoff.
