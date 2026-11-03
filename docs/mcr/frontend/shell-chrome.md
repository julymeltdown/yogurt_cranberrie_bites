# FE07-CHROME / UIV02 — reusable shell presentation

## Status and ownership

**MOCK_READY** for the assigned production components and explicit module
composition. Default WorldShell adoption, full FE07 and IN01 are **not ACCEPTED**.
There is no scope cut or change to the 112-task/61-required-IA denominator.

Author branch `codex/mcr-fe-FE07-chrome`, isolated `fe-shell-chrome` worktree,
exact parent `d3440e9d1acbd1882a082f9c89d1be1e807bec34`. The author SHA is the
direct child recorded in Git and the READY handoff. All delivered paths are new
under `frontend/src/app/shell-chrome/` plus this document. Existing root,
theme/controller, shared UI, account/world, manifests/locks/contracts/backend,
source documents and art remain unchanged. Prior FE41 checkout remains stopped.
Main/Delivery integrates and publishes this handoff.

## Components

`frontend/src/app/shell-chrome/index.tsx` exports:

| Component | Behavior / owner boundary |
|---|---|
| `ShellHeader` | Compact black/ivory header with MCR brand, optional caller-authorized event title, Inbox and Settings. Brand goes to `/world`; Inbox to `/notifications`; Settings to `/me/settings`. Stable `shell-brand`, `shell-inbox`, `shell-settings` IDs. No fabricated avatar, nickname, level, balance, unread count or private profile. |
| `BottomNavigation` | The incumbent `routes.tabs` supplies all six paths/labels; `resolvePanel` supplies the active tab for validated deep links, including notices and Inbox. Each destination uses `ShellLink`, current-page semantics and the incumbent `tab-*` ID. Six current MCR `ConcertIcon` glyphs plus visible labels. No ordinary anchor/document reload is substituted for internal navigation. Invalid paths advertise no active authorized destination. |
| `EraControls` | Accepts the existing `ThemeController` or narrow `EraPort` (`read`, `subscribe`, `select`) from its owner. Native buttons form a labeled group. `aria-pressed` follows actual confirmed `selected`; `pending` has separate loading indication and status. Loading never announces a changed world before controller confirmation. An absent/disposed/withdrawn port disables all three choices, clears pressed state and says **Eras unavailable.** |
| `PanelHeader` | Optional compact replacement for the existing header only: caller-owned Back, expand/reduce and minimize/show actions; retained `panel-title`, heading ref and focusability. Icon controls reduce crowding while preserving accessible action names and 44px targets. Caller retains title source, focus/scroll restoration, sizing, Escape handling and lifetime. No new drawer or navigation controller. |

New scoped CSS uses black, ivory and restrained red; compact uppercase display
type; symmetric double frames and ordinary control borders. No decorative left
accent, offset rail, generic card stack, legacy image or replacement character
is introduced. Shared `concert-window` icon/style primitives are reused; the
Settings glyph is newly authored inline SVG with an accessible enclosing link.
No animation framework or dependency is added. All essential controls remain
present at small sizes rather than disappearing to approximate the reference.

`EraControls` creates no world, Auth, controller or preference store. It observes
one supplied controller and issues only explicit selection. The existing owner
handles qualified resources, atomic skin replacement, rollback, persistence and
late-result cancellation. Fixed local presentation text does not render raw
controller error messages, URLs or secrets. On storage failure it reports the
actual new selection alongside failure, not a false successful save. Error copy
invites an explicit era choice to retry. A narrow custom port must retain this
existing state contract and publish errors through `ThemeState`; rejection text
is never rendered. Native buttons preserve Enter/Space and touch activation.

No private identity or release is captured in these components. The parent must
withdraw its port (`controller={null}`) synchronously in its canonical authority
cleanup and dispose the existing controller before transferring identity/event
ownership. React's external-store binding follows the current prop; the view
holds no asynchronous selection result that could repopulate after withdrawal.
Components never dispose a caller-owned controller themselves.

## Exact minimal root insertion (reserved owner work)

No default root file was changed by this assignment. In
`frontend/src/app/WorldShell.tsx`, **inside the existing NavigationMemory**:

1. Replace the current `shell-header` markup with
   `<ShellHeader eventTitle={bootstrap.event.title} />`.
2. Replace the plain `shell-era` link row with
   `<EraControls controller={currentThemePort} />`. An absent producer must use
   null, not a default selected REVENGE state. Retain any actual read-only event
   notice owned by the existing shell, placed accessibly without obscuring controls.
3. Replace current `shell-tabs` markup with `<BottomNavigation />`. Do not add
   another NavigationMemory, Router, provider, history stack or link mapper.
4. Optionally replace only `PanelFrame`'s header JSX with `PanelHeader`, passing
   the existing `heading` ref, resolved title, `size`, `back`, and current
   `onSize` callbacks. Preserve the parent `main`, `panel-title` relationship,
   focus effect, positions map, Escape behavior, peek/full state and outlet.
   For example, expanding still toggles full/half and showing a minimized panel
   returns to half. Do not independently recreate these state transitions.

The existing `.world-shell` four-row grid can retain its header/era/stage/footer
ownership. New module CSS is scoped; it does not modify `shell.css`, global
tokens, content panels or feature headers. The root writer must check combined
safe-area/visualViewport/keyboard behavior with its final composition. It must
avoid stacking the old header or era row around the replacements, and must keep
the existing one WorldHost keyed by canonical scope.

**Theme producer gate:** current `frontend/src/app/ports.ts` and
`frontend/src/world/composition/index.ts` do not expose the existing
`features/theme/model/controller.ts` through production shell ports. The root/
world/theme owner must supply the incumbent controller with its qualified map
releases and canonical identity/event lifetime. This module does not create one,
instantiate a second runtime or invent resource/preference defaults to make
buttons appear available. The production `/world/theme` route remains separately
owned. An optional wider theme scope should keep the existing confirmed-state
owner; this module does not write document/global theme state independently.

## Source/reference provenance

New rendering and CSS are authored for MCR. Only current MCR public types,
router helpers, shared icons and the supplied theme controller are reused from
the pinned parent. No legacy Bigyeting rendering/visual package is copied.
Immutable `docs/source/` and supplied character originals remain untouched.

Viewed unchanged reference:
`docs/mcr/design/ui-reference-audit-2026-11-04/user-reference.jpg`, SHA256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Also viewed the actual existing 390px account capture
`features/avatar/__tests__/visual-adoption/evidence/account/default-avatar-unavailable-390.png`
and `app/mural-entry/__tests__/evidence/loaded-390.png` under `frontend/src/`.
They show the concrete outer-chrome mismatch: generous plain brand/event row,
single theme link and text-only footer around already adopted concert windows.
The new captures replace those module surfaces with compact framed header,
three direct era controls and icon/label navigation. Remaining differences from
the reference are explicit: no approved character/venue artwork, no fictional
levels/currency, synthetic terrain and fixture content, and no default root
adoption. These are not a finished concert-world screenshot.

## Validation and measured layout

Retained Node `v24.22.2` at
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin`; retained dependencies
temporarily symlinked from `/Users/lhs/Desktop/mcr/frontend/node_modules`.
No installation, manifest/lock change, shared cache or incremental buildinfo.
The symlink is removed before commit. All new files are staged explicitly.

| Command | Exit / result |
|---|---|
| `python3 scripts/mcr/check-plan.py` | 0; planning only |
| From `frontend`: `MCR_FE14_PYTHON=/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin/python node node_modules/vitest/vitest.mjs run --configLoader runner --no-cache src/app/shell-chrome/__tests__/chrome.test.tsx src/features/theme/__tests__/controller.test.tsx src/world/theme/__tests__/replacement.test.ts src/app/__tests__/routes.test.ts` | 0; 81 tests in 4 files, including C06 bundle validation, none skipped |
| `node frontend/node_modules/typescript/bin/tsc --noEmit --incremental false --project frontend/tsconfig.json` | 0; full frontend strict types |
| From `frontend`: `node node_modules/oxlint/bin/oxlint src/app/shell-chrome` | 0; scoped lint, no diagnostics |
| `node frontend/src/app/shell-chrome/__tests__/native.mjs` | 0; existing application + module fixture production build into `/tmp/mcr-shell-chrome-build`, temporary cache; Chrome 156.0.8078.80, 30 captures, zero browser errors |

The new 17 DOM tests cover valid/invalid deep-link tab state, actual router
navigation/state, Inbox/Settings, direct-parent replacement and NavigationMemory
Back focus. Actual ThemeController tests cover pending versus committed pressed
state, safe failure, persistence failure, disposal, withdrawal, old callbacks,
subscription cleanup and caller-only panel actions. Existing focused route,
theme and replacement tests retain their own semantic coverage.

The explicit **module composition fixture is not WorldShell**. It keeps one
actual FE14 Phaser runtime, actual ThemeController/loader/qualified synthetic
map resources, canonical Auth and FE17/MockWorldTransport beside the new modules
and the incumbent NavigationMemory/Router. It renders **Avatar artwork
unavailable**, never character fallback art. Native checks exercise Enter
selection, touch tap selection/retry, actual rendered palette changes and
rollback, one persistent game/mock socket/camera/room/outfit, all six internal
destinations without document reload, Inbox/Settings, Back focus, direct-detail
parent, panel sizing, short viewport and complete disposal. Tests wait for actual
renderer frames before pixel verification/capture, not only controller state.

| Viewport | Header | Ready era row | Footer | Total chrome | World-stage allocation |
|---|---:|---:|---:|---:|---:|
| 360×800 | 54 | 50 | 66 | 170 | 594.5px / 74.3% |
| 390×844 | 54 | 50 | 66 | 170 | 638.5px / 75.7% |
| 430×932 | 54 | 50 | 66 | 170 | 726.5px / 78.0% |
| Each width at 430px short height | 54 | 50 | 66 | 170 | 224.5px / 52.2% |

Measurements include the fixture's 35.5px qualification banner. The regular
ready World view exceeds the approximately 66%/70% world-area targets in this
module composition. Short-height allocation intentionally falls below that
target to retain controls; a panel can further cover the stage. These are
layout allocations, not a claim that an open panel leaves 70% unobscured world,
nor proof of default shell performance. Pending/error/unavailable status text
takes additional space and remains visible. World tools occupy an in-world
44px strip. No essential navigation is removed to inflate the percentage.

All visible chrome actions measure at least 44×44px, every footer label fits,
and no horizontal document overflow occurs. At 430px viewport height, the
focused fixture field and footer remain reachable. Reduced motion is requested
and checked; chrome adds no animation. Browser viewport resizing approximates
keyboard space, not a physical phone keyboard or safe-area qualification.
Manual inspection includes 360px world/pending/short captures, 390px rendered
Parade/error/expanded panel and 430px unavailable state.

Initial diagnostic runs: one native runner expectation incorrectly requested
Reduce after the caller had returned a minimized panel to half; the runner
sequence was corrected without a production change. Enabling the existing
optional Python bundle test from repository root failed its relative script
path (retained in `focused-initial-root-cwd.log`); rerunning from frontend with
the retained validated Python environment passes all 81 tests. No reserved fix
is required. Vite's existing large-chunk warning remains nonfatal.

Evidence in `frontend/src/app/shell-chrome/__tests__/evidence/`: native JSON,
30 PNGs, command logs and `validation.json` with new source, existing source/
contract/reference and evidence SHA256 pins. Raw logs remain byte-preserved;
staged whitespace verification excludes only raw `*.log` trailing terminal lines.

Default/root integration, production theme release, approved art, live server/
multiuser world, physical device/keyboard/safe-area, load and release gates are
**NOT_RUN**. Full frontend suite was intentionally not run for this bounded
change. Functional fixtures and visual captures do not certify FE07/IN01.
