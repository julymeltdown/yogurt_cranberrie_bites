# FE08 public entry and versioned document presentation

Status: **MOCK_READY**. This bounded increment implements the public landing and document presentation against integrated FE07/FE09. It does not accept FE08 as a whole or certify real document delivery, event availability, multiuser behavior, or release readiness.

## Revision and ownership

- Task: FE08, A01 `/` and A07 `/legal/:document`; terms, privacy, safety and contact use the existing document route.
- Worktree: `mcr-worktrees/fe-public`; branch: `codex/mcr-fe-FE08`.
- Base: `376cf2127b5ae5b81fa8743293ec9cc0079f6bfd`.
- Implementation: `34af3d3680da60c98d88ed4b4def142246bbb214`.
- Separately authorized FE07 compatibility tests: `f7a0239c91b625cf0d7a4e416fe46b2b05dac60e`, directly following the implementation.
- Delivery ancestry is the base followed by those two commits and the documentation commit containing this file. The integrator owns review, serial integration and publishing to `julymeltdown/mcr`, `master`; this worker has not pushed.

The implementation reads the specification, assessment, execution plan and source audit. The specification remains the scope authority. Its archived source and all `docs/source/` bytes remain unchanged. The approved mobile-world image was inspected only as design context; no image, character, rendering code or legacy visual library was copied. All public rendering and styles are newly authored. No headless source extraction was needed for this increment.

Existing contracts come from this base's canonical session/access/readiness ports, frozen public route registry, integrated FE09 controller, and FE07 world ownership. No HTTP endpoint, wire DTO, legal approval, event default, operator identity, contact address, retention policy or physical arrival claim was invented. Existing React, React Router, Zod and frontend tooling resolutions are retained; no manifest or lockfile changed.

## Changed paths

Implementation commit:

- `frontend/src/app/FoundationApp.tsx`: replaces the obsolete scaffold with the new landing export.
- `frontend/src/app/PublicBoundary.tsx`: composes the document view at the existing public document route; sign-in and unrelated placeholders retain their routes.
- `frontend/src/app/__tests__/public-entry.test.tsx`: 21 application-level tests.
- `frontend/src/features/public/model/documents.ts`: bounded presentation input, fixed document IDs, strict decoding, version/date/text/link validation and unavailable default.
- `frontend/src/features/public/model/document-provider.tsx`: feature-local read-only provider injection.
- `frontend/src/features/public/ui/Landing.tsx`: world-first public landing and canonical home dispatch.
- `frontend/src/features/public/ui/DocumentPage.tsx`: unavailable/loading/error/retry and versioned content views.
- `frontend/src/features/public/ui/PublicFrame.tsx`: public navigation, skip link and fan disclosure.
- `frontend/src/features/public/ui/public.css`: scoped cream/ink typography, spacing, targets and focus styles.
- `frontend/src/features/public/__tests__/documents.test.ts`: 35 decoder/link/date tests.
- `frontend/src/features/public/__tests__/browser-smoke.mjs`: installed-Chrome public qualification and production fixture-exclusion checks.
- `frontend/src/features/public/__tests__/fixtures/documents.ts`, `preview.tsx`, `preview.html`: isolated, conspicuously synthetic qualification inputs and test entry.

Compatibility commit, explicitly approved as a narrow assignment extension:

- `frontend/src/app/__tests__/shell.test.tsx`: anonymous home isolation and signed-in home replace lifecycle; legal/admin release coverage retained.
- `frontend/src/app/__tests__/browser-smoke.mjs`: replaces the obsolete scaffold headline with semantic new-home assertions and correct unavailable production identity state.

The documentation commit changes only this file. Manual changed-path and import review found no backend implementation imports, no world/network/Phaser imports from production public features, and no edits to router composition, shared contracts, infrastructure, central manifests, lockfiles, auth/session/world implementation, shared styles or assets. Test fixtures may read existing instrumented shell and auth fixtures; this grants no implementation ownership over those modules.

## Behavior and limitations

The landing leads with “One world. Every era.” and an honest description of a shared 2D world being built. It distinguishes anonymous, checking, unavailable, invalid-ready and ready states. Anonymous entry uses the existing sign-in flow with `/world` as the return target. Production without an event stays unavailable. Ready home navigation replaces the history entry with `/world`, using the current canonical generation/user/event and abort scope. It rechecks the session and readiness before dispatch, handles StrictMode without duplicate dispatch, and does not project cached personal information onto the public page. Anonymous public routes allocate neither world host nor world transport.

All four document pages default to unavailable in the production application. `DocumentProvider` is a feature-local presentation seam, not an HTTP/publication schema. Its default performs no network request. Synthetic fixtures are explicit test-only inputs with a trusted composition flag and a visible testing notice, and are absent from the production bundle.

Published presentation accepts an exact, key-bound shape with a bounded version identifier, UTC ISO timestamps and bounded plain-text sections. It displays version, published/effective dates and the explicit Korea Standard Time display convention. React escapes text; no HTML or Markdown execution is enabled. The decoder rejects unexpected keys, invalid state/version/date/text, malformed Unicode, unsafe control/bidi characters, excessive list/text sizes and unreviewed links. Allowed destinations are exact existing static public routes and the four fixed document routes. External and contact schemes require a later reviewed publication/link policy.

Loads are bound to document key, provider and retry attempt; old completion cannot overwrite another document or source. Route/source changes and unmount abort the old read. A 10-second deadline produces a neutral error and aborts the read; explicit retry starts a new attempt. Unknown document slugs are neither reflected nor sent to a provider. Heading focus, current-document navigation and browser back preserve usable document orientation. No private document cache or new identity store was added.

The UI uses no decorative left bars or accented cards. Symmetric control borders, horizontal structural dividers and keyboard focus outlines remain. No real venue guidance, meeting arrival, concert date, live community availability, policy approval or contact channel is claimed.

## Qualification on 2026-11-03

Node `24.22.2`, pnpm `11.7.0`, installed Google Chrome `156.0.8078.80`. Commands ran in the assigned worktree; frontend commands used the retained local Node distribution and `corepack pnpm@11.7.0 --dir frontend`. Every command below exited **0**.

| Command | Result |
| --- | --- |
| `corepack pnpm@11.7.0 --dir frontend install --frozen-lockfile` | 199 packages reused; lockfile unchanged |
| `corepack pnpm@11.7.0 --dir frontend test -- src/features/public/__tests__/documents.test.ts src/app/__tests__/public-entry.test.tsx` | 56/56 focused tests |
| `corepack pnpm@11.7.0 --dir frontend test -- src/app/__tests__/shell.test.tsx` | 39/39 shell tests |
| `corepack pnpm@11.7.0 --dir frontend test` | 410/410 tests, 14 files |
| `corepack pnpm@11.7.0 --dir frontend build` | TypeScript and production build pass |
| `corepack pnpm@11.7.0 --dir frontend lint` | No warnings/errors |
| `node src/features/public/__tests__/browser-smoke.mjs` from `frontend/` | Public Chrome checks pass at 360×800, 390×844, 430×932 and 1280×800, plus 420px reduced height |
| `MCR_FE07_EVIDENCE=/tmp/mcr-fe08-evidence/shell-regression node src/app/__tests__/browser-smoke.mjs` from `frontend/` | Existing shell Chrome regression passes at all four widths and production entry |
| `node scripts/mcr/check-ui-origin.mjs --root <assigned-worktree>` | C08 PASS, 96 files / 89 content files, zero violations; source revision `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` |
| `python3 scripts/mcr/check-plan.py` | PASS; all 13 archive checksums intact; planning checks only |
| `python3 scripts/mcr/check-boundaries.py --base 376cf21 --lane frontend --allow-doc docs/mcr/frontend/public-entry.md` | PASS, followed by manual assignment/import review |
| `git diff --check` | PASS |

The application tests cover canonical-controller readiness, history replace, pre-layout identity revocation, no default event, no public bootstrap/world allocation, unknown documents, stale loads, error/retry, inert markup, hostile links and deadline abortion. These are application/DOM proofs, not backend acceptance.

Chrome checks exercise actual keyboard skip/main focus, visible CTA focus, enabled text contrast, 44px controls, horizontal overflow, reduced-height scrolling, synthetic document loading/version/dates/links/back, ready replacement and revoked mock world ownership. Production checks prove unavailable documents, no synthetic marker/global, no document endpoint or world/Phaser requests, no world socket/canvas/host. The ready world counters use instrumented mock leases; they do not certify actual Phaser or server multiuser behavior.

Local evidence is in `/tmp/mcr-fe08-evidence/`: `unit-tests.txt`, `lint.txt`, `build.txt`, `browser-smoke.txt`, `browser-results.json`, `shell-regression.txt`, `shell-regression/browser-results.json` and the viewport PNGs. Manual visual review included `landing-390.png`, `synthetic-document-1280.png` and `production-home-390.png`. These temporary artifacts are local qualification evidence, not a published or durable release archive. Full-page captures explicitly reset scroll after keyboard checks; this avoids a Chrome fixed-element capture artifact while a separate assertion proves the skip link is outside the viewport after focus moves away.

Build output retains the existing root-chunk advisory: approximately 538.42 kB / 142.91 kB gzip. The lazy public chunk is approximately 14.85 kB / 4.89 kB gzip plus 5.48 kB / 1.62 kB gzip CSS. No warning threshold or bundling configuration was changed.

## Required integrator inputs and open gates

The following requirements were sent early to the integrator while independent UI work continued: canonical document IDs/version/history; publication/effective timestamps and timezone; supersession/withdrawal behavior; approved copy and operator/contact identity; retention/consent compatibility; reviewed destinations and schemes; approval/source provenance, localization, caching and access/error policy; and the actual public HTTP contract. These shared decisions belong to the integrator and relevant backend/publication owner.

Real approved documents/contact/publication/retention delivery: **NOT_RUN**. Backend-connected document success, real session/cookie flow, actual Phaser/socket flow, physical device/accessibility qualification, real multiuser flow, IN01 and whole FE08/release acceptance: **NOT_RUN** in this increment. Preserve this status until those obligations have their own concrete evidence. Planning baseline regeneration is not execution evidence.
