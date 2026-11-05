# PI01 — Parade Ink shared foundation

Status: IN_PROGRESS. Source preparation is booked; executable and native checks
are NOT_RUN. This document is the versioned presentation interface for PI02/03
and later screen owners. It does not certify application or device acceptance.

## Authorized objective and custody

Deliver the complete shared Photo 2 foundation and its consumption by the
functioning default application: tokens, measured typography, window frames,
fields, tabs, buttons, dialogs/sheets, feedback/statuses and shell chrome. Own
implementation, relevant source and native 360/390/430 state/accessibility
comparisons, fixes, concise evidence and scoped committed publication directly
through standing publisher412e. Original mappings are FE05/FE06/FE07/FE41 and
QA06/QA07; historical functional completions are preserved. The first visible
Feed/Thread milestone is coordinated directly with its PI02/03 owner213.

Root153 explicitly transferred 15 existing shared presentation files after
former-writer STOP. Exact paths and preimages are in the evidence intake.
New owned prefixes are shared/styles/parade-ink, shared/ui/parade-ink,
app/__tests__/parade-ink-foundation and this document/evidence. There is no
blanket shared UI barrel, router, controller, Auth, manifest or World grant.
Feature213 rendering, viewport40fde and artist231/534 remain separate writers.
Root153 subsequently split shell-chrome/index.tsx: EraControls and
BottomNavigation return-JSX/presentation plus necessary scoped imports were
literally STOP/returned to PI08 owner40c576. PI01 retains ShellHeader,
PanelHeader, identity and all existing shared CSS. Protected incumbent
controller/state/select/route logic remains read-only; PI08 owns new pure HUD
views/CSS. The transferred file is unchanged from f025 in this checkpoint.

The sole checkout remains `mcr-worktrees/fe-runtime-preferences`, now branch
`codex/mcr-fe-PI01-parade-ink-foundation` at canonical base
`f025bdb6fb409f3b7706e475dfabb5f3d21fe168`. The old Map branch and all nine
STOP/returned hunks, history and evidence remain preserved. The existing
persistent goal is the incompatible unfinished FE19 objective with historical
BLOCKED metadata. It is not reset, paused or falsely completed; this separately
authorized whole PI01 objective is recorded durably here. The tool cannot edit
an objective or create another while an unfinished goal exists.

## Reference, measurements and contract v1

Authority is the published `parade-ink-delivery-plan-2026-11-05.md`. Original
Photo 2 SHA256 is
`9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27`.
Its 1280×853 JPEG stays byte-exact. `reference-measurements.json` records source
crop rectangles, samples, normalization, font measurements and adaptations.
The Feed frame crop is [24,60,419,847], normalized from 395 source pixels to
390 CSS pixels. Cropped references are analysis artifacts, never product UI.

One `pi01-photo2-v1` CSS-variable contract lives in
`frontend/src/shared/styles/parade-ink/tokens.css`, imported once through the
existing default-entry base stylesheet. Screen owners consume these variables
instead of independent palettes or token files. Existing `--mcr-*` and
`--concert-*` semantic values alias this appearance. REVENGE/PARADE/DANGER
remain independently selected/persisted World eras with unchanged assets.

| Variable group | Values / meaning |
| --- | --- |
| `--pi-ink`, `--pi-paper`, `--pi-surface` | #171717 / #f9f9f8 / #f9f8f6, sampled charcoal and light surfaces |
| `--pi-crimson`, `--pi-on-dark` | #922532 / white; primary actions, black selected tabs |
| `--pi-rule`, `--pi-soft-rule`, `--pi-muted` | #3c3c3b / #777774 / #545454; symmetric rules and readable metadata |
| `--pi-font-body`, `--pi-font-display` | Bundled Roboto / Oswald, with exact official source/license pins |
| Body / title / control / metadata | 18/22, 26/32, 18/22, 14/18 CSS px; title/display weight700, body400 |
| Frame / keyline / ordinary rule / content inset | 2 / 2 / 1 / 6 CSS px |
| Title height / tab height / tab gap | 48 / 44 / 4 CSS px |
| Hit target / glyph / radius / row gap | 44 / 24 / 2 / 8 CSS px |

The JPEG carries no editable font metadata. Fonts are an explicit measured
implementation choice, not an extracted original. Official unmodified Google
Fonts bytes, source revisions, licenses and metrics are retained. Browser text
metrics and fixed-region differences must still be checked before acceptance.

At360/390/430, shared insets/rules/type remain fixed; flexible columns and
authorized long content wrap. No scaling of the entire page or clipping to
conceal layout differences. Three window actions keep separate44px targets,
with24px visible ink; this explicitly adapts the smaller printed controls.
Tabs have44px hit areas instead of the reference's roughly40px printed row.
Safe areas are additive. Short-height/keyboard layouts retain local scrolling
and reachable actions. Modal keyboard/focus and all existing callbacks remain.

## Stable consumption interface

Existing imports and public props remain unchanged:

- `shared/ui/controls`: Button variants primary/secondary/quiet/danger and Field.
- `shared/ui/Tabs`: automatic-activation keyboard Tabs and existing `.mcr-tab*` hooks.
- `shared/ui/Dialog`: Dialog, DialogProps and DialogCloseReason. Use
  `presentation="sheet"` for sheets; there is no separate Sheet export.
  Existing dismissal/focus behavior is preserved.
- `shared/ui/feedback`: Feedback, Skeleton and ToastRegion semantics.
- `shared/ui/concert-window`: ConcertWindow/Action/Tabs/Navigation and related kit.

The shell owns outer frame/titlebar/body inset; feature owners own content.
PI02/03's `feed.css` owns its tabs/cards/composer overrides and must consume
these variables, removing stale red selected-tab and old paper overrides.
PI04 consumes the same contract for its local rendering. No shared barrel or
functional caller API is added. A contract source pin is sent directly to213,
PI04, PI05, PI08, PI10 and UI533 without review/acknowledgment or a whole-family wait.

No decorative left bars, fabricated levels, DM/Say hi, guestbook, client-only
quantities or local success states are introduced. Controls expose real current
callbacks. Character bytes, World geometry/camera/controller, single game/socket,
Auth identity, privacy/cache/query ownership and era persistence are preserved.

## Validation and publication disposition

NOT_RUN: relevant changed source checks; default-App native comparison and
normal/selected/pressed/focus/pending/disabled/error states; short-height/safe-area;
actual device/live obligations. No old matrix is automatically replayed.
Runtime work requires its own fresh finite21d admission. Ordinary preparation
64MiB TOTAL positive growth is booked through2026-11-05 10:00 Asia/Seoul;
checkout +34MiB Git reserve +8MiB authoring projected55824779 bytes. Retained
carry and prior WHOLE1707606622 are not reset or charged twice as NEW.

Complete only after functioning default consumption, necessary comparisons and
fixes, concise evidence and canonical committed412e delivery. The full Feed/
Thread visual milestone also needs owner213's functioning rendered slice.
Original FE19 and unrelated live/device/physical acceptance remain open.

## Authored source checkpoint

Shared Photo 2 presentation now consumes the contract through default base.css:
light panel surfaces; black title strips, selected tabs and navigation with
white text; crimson primary actions; measured body/display fonts; symmetric
frames; and separate 44px action targets. The old ornamental crowd footer and
pastel perimeter are removed. Existing era, route, controller and private
state ownership is preserved. WindowInk uses inline currentColor SVG inside
the incumbent buttons; callback, label and focus behavior is unchanged.

This checkpoint is a reproducible source/API delivery for feature owners,
not a passed visual milestone. Source, native and device results remain
NOT_RUN. Full-screen body height subtracts the actual 70px frame/header/insets;
that measurement and short-height layout must be checked in the native round.

Source01: five new focused regressions PASS. Full nonincremental types failed
on five test-only Testing Library query options (`exact` belongs to Playwright,
not Testing Library). Those options are corrected; lint/harness syntax remain
NOT_RUN pending a fresh source round. Both independent guards returned FULL
with zero resources. Complete checkout/private+common Git/dependency/Node
accounting now retains a nonshrinking WHOLE high-water of 2,764,328,960 bytes.
The original 056a8c48 production postimages are unchanged.

Source02 PASS at 2026-11-04T23:54:29.605141Z: five focused new cases, full
ordinary frontend nonincremental noemit types, owned scoped lint and native
harness syntax each exit0. Both independent guards returned FULL/resource0.
The immutable source01 failure and both exact source readsets remain retained.
The production 056a8c48 postimages are unchanged; source qualification is not
browser/reference acceptance. Native-port preflight/closure accounting was
added to both owned guards for the upcoming distinct browser round. No existing
application component, shared API or production paint changed after source02.

Source04 PASS at 2026-11-05T00:15:34.488510Z: seven Root-transferred
retained tests, full ordinary frontend nonincremental noemit types including
the current admission fixture, owned lint and native harness syntax, each
command exit0. Source03 retained six passes and the lazy AccountPanel timing
failure; its fix waits for the actual unavailable content before unchanged
scroll/minimize assertions. Original source02 five focused cases remain passed.

Native02 stopped at the canonical admission boundary because the inherited
fixture omitted `/api/auth/enrollment/admission`. Production Auth correctly
refused entry. The new owned fixture responds with the existing contract-valid
synthetic admission and awaits actual `auth.checkAdmission()`; no authority
state is patched. The retry still requires a distinct admitted browser round.
No viewport/reference acceptance is inferred from that failed attempt.

Native03 failed before its first qualified width: Vite React Refresh injected an
absolute self-import for the immutable borrowed AuthorHeading helper, which the
overlay resolver did not recognize. The resolver now recognizes every pinned
absolute overlay path. Its owner-inspected failure capture is retained; no
normal native screenshot or visual qualification resulted. Both guards closed
FULL at 00:16:49.164938Z with zero resource holds.

The Feed implementation owner also found the full-screen height variable losing
to the higher-specificity padding selector. The default half-panel variables
now live on the ordinary `.shell-panel-body` selector, allowing the existing
full-screen override to win. No sizing formula, route or controller changed.
The native driver now checks the normal 844px Feed footer is below 700px,
preventing the observed half-height regression. This changes one production
postimage from 056a8c48: `frontend/src/app/shell.css`. A new immutable pin and
readset must be consumed explicitly; prior overlays remain reproducible.
