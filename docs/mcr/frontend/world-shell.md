# FE07 persistent shell handoff

State: **MOCK_READY**, pending integrator review. The shell and routing are
implemented; feature screens, product authentication and a real multiplayer world
are not claimed by this task.

## Ownership and provenance

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/fe-shell`.
- Branch: `codex/mcr-fe-FE07`.
- Base: `69a5624cd153a216f2398cebabf062400dce23eb` (frozen C05 checkpoint).
- Initial implementation: `d7c720ce6322515280a302f39c8f9a66c790ffa8`.
- Reviewed-fix implementation candidate: `18740f2895fce2d2f5fe3be1695bc007a4f3a1d0` (descends from the initial handoff `8ba0081`).
- Only `frontend/src/app/**`, `frontend/src/main.tsx`, and this exact document
  are assigned. Root router, route composition and main entrypoint edits were
  explicitly delegated by the integrator for FE07.
- Newly authored rendering, CSS, lifecycle ports and tests. No copied upstream
  code or new dependencies. Retained React Router 7.18.1, React 19.2.7 and the
  accepted FE05 Button/Feedback primitives are used directly.
- Contract references: specification §§5–6 and FE07; frozen C03 core 0.1.0 and
  C05 realtime 1.0.0 at the base. FE01 supplies the canonical memory SessionPort
  and generation/abort semantics. FE02 remains the private-query/cleanup owner.
  No duplicate wire schema, backend implementation import or C04 DTO is added.
- Character source pack revision 3 and its original bytes remain untouched.
  No world art, legacy fallback, synthetic atlas, slicing or image generation.
  No backend, contract, package, lockfile, configuration, shared UI/style, catalog
  or unassigned feature changes; no migrations or live-data operations.

## Routes and product behavior

`main.tsx` constructs one router outside StrictMode rendering. `createAppRoutes`
composes the six exact primary destinations: `/world`, `/feed`, `/meet`,
`/gifts`, `/wall`, `/me`. The 41 required shell route paths include all required
nested world, feed/notice, meetup/profile, giveaway/pickup, personal/inbox and
report paths from plan §6. The nine public paths (including archive) and nine
admin paths have separate lazy modules. Optional chat, wall history and souvenir
routes are not activated. This registers boundaries; it does **not** accept those
IA features or implement their business behavior.

The current public landing is preserved unchanged at `/` until FE08. Public
auth/legal/archive placeholders truthfully say that their page is being prepared;
they do not claim account deletion, callback success or a completed account action.
Public/admin exits unmount the world boundary. Admin access requires the injected
current server-authorization port; participant data confers no admin role. Even
an allowed check currently opens an honest unavailable-tools state.

The authorized composition has a compact event header, labeled six-link bottom
navigation, era entry, persistent world host and nested panel outlet. These are
route links (`nav`/`aria-current`), not an ARIA tab widget with hidden tab panels.
Keyboard navigation uses native Tab/Shift+Tab/Enter and visible focus. Panels have
peek/half/full controls, one primary content outlet, safe parent/back actions and
Escape outside editable fields. Scroll positions survive panel/world navigation
within the same identity/event shell. The scroll map retains at most 100 recently
visited destinations, matching the navigation-memory entry limit, and clears on
identity/event replacement. POP navigation restores a known opener;
new destinations focus their heading. Panel content scrolls independently, with
44px minimum primary controls, safe-area padding and VisualViewport resize handling.
The 70% world-height design target is approximately met by the default mobile
composition; no canvas rendering/performance claim follows.

The style uses spacing, typography, quiet surfaces and ordinary symmetric control
borders. There are no decorative left accents, left rails, offset shadows,
repetitive accented cards, invented attendees or stock/presence counters.

## Session, bootstrap and route safety

`ShellPorts` injects the accepted FE01 SessionPort, a session-readiness observable,
a bootstrap projection loader, optional world lifecycle owner and admin checker.
The production constructor installs **no credentials**, participant fixture,
restoration request or world port. Its default is honestly unavailable, retaining
the public landing and the requested deep-link URL while entry is being prepared.
No URL/query parameter enables fixture authentication.

FE09's future adapter must validate the complete C03 Bootstrap before mapping the
small non-secret `ShellBootstrap` view projection. The shell additionally binds
contract version, participant and event to the canonical current session, validates
the event title/phase and checks world availability. DRAFT/ARCHIVED events do not
allocate a world owner. This client check does not replace server authorization.
No bearer, cookie or ticket is copied into view/lifecycle context, DOM, URLs, logs or storage; the canonical SessionPort remains the sole memory credential owner.

Unknown/checking/anonymous/unavailable/error readiness cannot start bootstrap or
world resources. Bootstrap is bounded to 12 seconds, aborts on cleanup or identity
change, and fences both initial dispatch and results even if the loader ignores
abort. An obsolete rendered scope cannot attach to a replacement session or
dispatch a loader; the invocation microtask checks canonical identity and the
captured session signal again. Safe presentation
categories cover 401/403/404/410/429/503-style outcomes through `ShellLoadError`;
unknown exceptions become generic unavailability. There is no automatic mutation,
refresh, cookie or bootstrap retry. Explicit retry keeps the direct route intact.
Nested rendering failures retain the host and show safe error copy; thrown bodies
and diagnostics never render into product UI.

`safeReturnTarget` accepts known participant destinations and canonical UUID
targets. It rejects external/protocol-relative URLs, backslashes, encoded forms,
control characters, traversal, fragments, auth/admin/API routes and unknown query
parameters. A bounded report target query is checked lexically; its semantics and
authorization remain C04/FE36 obligations. Domain filter query allowlists require
their future feature handoff. Login receives a sanitized `returnTo`; the URL remains
unchanged through legitimate session/bootstrap loading until that auth feature is
connected. A known in-memory parent visit permits history back; a direct/foreign
entry replaces with its safe parent, then list, then world. No foreign-history
guess or arbitrary stored return URL is used.

## World lifecycle seam and limits

The mounted `WorldHost` is outside the changing panel outlet. Its owner is scoped
to canonical `(generation,userId,eventId)`; tab/nested navigation, panel resizing
and bearer refresh do not recreate it. Session abort synchronously disposes the
old lease before a replacement can allocate. This includes invalidation during
the synchronous mount call. StrictMode setup/cleanup and unmount use the same
idempotent cleanup. Public/admin exits release the owner.

The future FE14/FE16 adapter must own **both** game and socket authority, release
them synchronously in `dispose`, and make asynchronous allocation obey its abort
signal. A throwing mount must clean partial allocation; dispose must not throw.
The shell passes blocked-input and presentation visibility state and makes the
foreign host inert while a panel is open. Backgrounding pauses input/presentation;
it does not promise a surviving network connection. The future replication owner
must qualify reconnect/resync and all frozen C05 rules, including exact captured
expiry, terminal asset/backpressure recovery and revocation.

No actual Phaser instance, ticket, WebSocket transport, avatar renderer, map,
capability subscriptions or room takeover is implemented here. Counter tests prove
the seam's lifecycle ordering only, **not** real game/socket behavior. No private
query/cache or feature draft is owned by this shell. Future feature screens must
use FE02's scoped query and cleanup ports; they remain responsible for safe draft
retention/cleanup, permissions, expiry and data invalidation.

## Verification and evidence

Toolchain: Node **24.22.2**, pnpm **11.7.0**. A fresh worktree-local frozen install
completed with exit 0; `node_modules` is not symlinked from another worktree.

Commands below ran from this worktree (the browser driver runs from `frontend/`)
with `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin` first on PATH:

| Command | Result |
|---|---|
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile` | Exit 0 |
| `corepack pnpm@11.7.0 --dir frontend test` | Exit 0; 276 tests, including 65 FE07 route/lifecycle/access/focus tests |
| `corepack pnpm@11.7.0 --dir frontend build` | Exit 0; strict TypeScript and Vite production build |
| `corepack pnpm@11.7.0 --dir frontend lint` | Exit 0; no warnings |
| `node src/app/__tests__/browser-smoke.mjs` | Exit 0; installed Chrome 156.0.8078.80, four viewports and built-production entry |
| `python3 -B scripts/mcr/check-plan.py` | Exit 0; planning/source archive checks unchanged |
| `node scripts/mcr/check-ui-origin.mjs --root /Users/lhs/Desktop/mcr-worktrees/fe-shell` | Exit 0; no copied UI or prohibited imports |
| `git diff --check` | Exit 0 |
| `python3 -B scripts/mcr/check-boundaries.py --base 69a5624 --lane frontend --allow-doc docs/mcr/frontend/world-shell.md` | Exit 1; only explicitly delegated root composition paths flagged: `main.tsx`, `app/router.tsx`, `app/routes.ts` |

The lane guard has no per-assignment root-composition delegation option. Its three
findings are reported, not suppressed or misreported as PASS. A separate exact
changed-path check verifies this assignment's allowed paths and cross-directory
imports. Integrator follow-up: reconcile those delegated composition paths in the
review record/guard workflow; no worker edit to the guard is authorized or needed.

The reproducible browser driver launches the installed Chrome with an owned
temporary profile, never downloads a browser or attaches the user's profile.
Its ephemeral development/production servers, contexts and browser are closed in
`finally`. Production fixtures are absent from `dist`; the explicit development
fixture HTML is not linked/imported by the application entry and rejects non-DEV.
The built production page was checked with `?fixture=ready`: no bypass, no world
host, no fixture global and no native WebSocket. Original public landing verified.

Browser evidence is outside Git at `/tmp/mcr-fe07-evidence`:

- `browser-results.json`: per-viewport assertions, instrumented counters and limits.
- `world-{360,390,430,1280}.png`: default world layout.
- `deep-gift-{360,390,430,1280}.png`: direct nested shell entry.
- `gifts-{360,390,430,1280}.png`: panel layout and scrolling.
- `production-entry-390.png`: honest unavailable production entry.

Chrome checks cover 360×800, 390×844, 430×932 and 1280×800: native keyboard focus,
history/opener restoration, six routes without host replacement, deep links,
panel sizes, scroll and query-only target navigation, simulated 420px viewport
height, 44px controls, no horizontal
overflow, account/event replacement and denied admin exit. Instrumented peak game
and connection counters are 1; final counters are 0 after exit, with all six
StrictMode/replacement allocations disposed. Screenshots were visually inspected
at mobile and desktop sizes. This is desktop Chrome at mobile widths, not device
emulation evidence of Safari or mobile OS behavior.

Real authentication/cookies, two-user HTTP/WS flows, Phaser, source-pack rendering,
camera/position preservation, live revocation, native mobile keyboard, physical
safe areas, iOS/Android devices, screen readers, full accessibility acceptance,
load/FPS and release qualification are **NOT_RUN**. FE08/FE09/FE12/FE14/FE16/FE17,
feature owners and the integration gates retain these obligations. No scope cut
or real IA acceptance follows from these placeholders.

## Delivery

Implementation and documentation are committed separately on the task branch;
the integrator alone reviews, integrates and pushes master. The final handoff SHA
is discoverable with `git log -1 --format=%H -- docs/mcr/frontend/world-shell.md`.
The goal stays active pending integrator review and any concrete FE07 fixes.

## Exact delivered paths and hashes

```text
frontend/src/app/AdminBoundary.tsx
frontend/src/app/PublicBoundary.tsx
frontend/src/app/WorldHost.tsx
frontend/src/app/WorldShell.tsx
frontend/src/app/__tests__/browser-smoke.mjs
frontend/src/app/__tests__/fixtures/ports.ts
frontend/src/app/__tests__/fixtures/preview.html
frontend/src/app/__tests__/fixtures/preview.tsx
frontend/src/app/__tests__/routes.test.ts
frontend/src/app/__tests__/shell.test.tsx
frontend/src/app/access.tsx
frontend/src/app/navigation.tsx
frontend/src/app/ports.ts
frontend/src/app/router.tsx
frontend/src/app/routes.ts
frontend/src/app/shell.css
frontend/src/main.tsx
docs/mcr/frontend/world-shell.md
```

The code/test file SHA-256 manifest (sorted `path<TAB>sha256<LF>` rows) is
`d1db85ff966150d371f2867e9b5eeb7ff6a156500c6f2119e47f970f3ab28766`. Its complete per-file values are in
`/tmp/mcr-fe07-evidence/candidate-hashes.json`. Key reviewed files:

```text
frontend/src/app/router.tsx
a2162fe86eafdfdc2eca32b52a906ea2fa9d05a8f9519436190a59a56622003d
frontend/src/app/WorldShell.tsx
c708b0b1dcb265b1197af474e2eb37d7f2feb2d2cd1e14276d58b7d39d0248db
frontend/src/app/WorldHost.tsx
e1ed8a70849c8c687e850e120c29627557cc2dfd1da8c91b18117fd6af722b10
frontend/src/app/access.tsx
9657e3804c38477dfa28de954d034512854d921c04b581a3d4713299c836ead6
frontend/src/app/navigation.tsx
2efd2d1236536e663f3fae00f74ecaacaea091a3474c45c47c845afa0a2ff1cc
frontend/src/app/routes.ts
e49ceffdc2e6e52bfba8e361b727504268fab264763c124878a556ee7f677829
frontend/src/main.tsx
76d64df8e54c8490048f190774ebc7c0769bfa95ee8435489ae0d9360bd5d195
```

## Review fixes and regression evidence

The original delivered commits stayed unchanged while the fixes were prepared.
The fix candidate above and this separate handoff commit are descendants.

1. The render-to-passive-effect race reproduced for both bootstrap and admin.
   A sibling layout effect replaces the canonical identity after a loader scope
   renders but before passive setup. The isolated real hook harness deliberately
   omits a parent session subscription, so a coincidental parent rerender cannot
   hide a stale dispatch. Before the fix each old-scope loader was called once;
   after the fix both are called zero times. Early setup and the invocation
   microtask check canonical scope and the captured abort signal. Existing
   12-second timeout, cancellation and late-result fences remain covered.
2. A 101-detail navigation regression reproduced retained old scroll state beyond
   the intended limit. The map now evicts beyond 100 entries and refreshes recent
   entries. The real route test verifies the oldest entry is absent, the recent
   entry restores, identity replacement clears it, and the host stays singular.
3. Self-review reproduced a minimized panel staying hidden when only a report
   target query changes. A new location key now reopens it before paint, with
   heading focus and no host remount; DOM and actual Chrome checks cover this.

The targeted race/scroll command first exited 1 with all three expected failures;
the query-target regression also first exited 1. The fixed targeted command
`corepack pnpm@11.7.0 --dir frontend test src/app/__tests__/shell.test.tsx -t "stale.*passive|bounds scroll memory|new query target"` exits 0 (four tests).
The complete suite exits 0 with 276 tests, including 65 FE07 tests; strict
build/lint, source-origin scan and the installed-Chrome driver all exit 0.
The final Chrome run is version 156.0.8078.80 at the same four viewports, with
refreshed screenshots/results under the existing evidence directory. Owned
servers and the temporary-profile browser close through the driver finally block.

No additional authorization, feature, server, real-world-runtime or device
acceptance is claimed. The goal remains active for the integrator recheck.
