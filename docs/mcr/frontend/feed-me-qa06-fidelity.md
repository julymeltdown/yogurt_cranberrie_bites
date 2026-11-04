# FE27 / FE34 — QA06 Feed and Me fidelity repairs

## Scope and disposition

This delivery repairs the original FE27 Feed and FE34 Me presentation against the supplied ten-screen target. It does not create replacement task IDs or amend their original acceptance criteria. Candidate base: `29b1779e84106fcbe1c59f973cb5992d83c4fc00`; isolated branch `codex/mcr-fe-FE27-FE34-qa06-fidelity`. Worker publication is prohibited; Root153 is the designated serial publisher. Implementation and bounded validation are complete; committed handoff, publication, and independent QA06 disposition are recorded below when available. Whole FE27/FE34, QA06, IN01 and IN05 acceptance remain open.

The original specification requires FE27 chronological feed, filters, cursor loading, linked references and source-stamped notices, stable ordering and safe revoked targets without private activity projection. FE34 requires a new Me hub, profile form and preferences, party/gift destinations, optional profile fields that do not block entry, and identity-scoped private settings. This patch retains those behaviors and checks their existing regressions; it does not claim a fresh live multiuser proof of every criterion.

## Findings and behavior

- Feed had two compose destinations: its real red action and unconditional supplementary shell navigation. PanelScreen now suppresses only `/feed` and `/me` supplementary links when feature children are supplied. Absent-child fallback links and every other panel retain their navigation. `routes.ts` is unchanged.
- A short Feed placed its sticky footer immediately below its first row because percentage height did not resolve through the intermediate wrappers. Feed now consumes the existing `--mcr-panel-available-height` shell variable. No shell sizing, footer, world host, socket or lifecycle code changes.
- Me retains the selected saved portrait, identity, four profile actions, Activity/Parties/Gifts navigation and real loaded-page activity. Secondary Settings, Age & privacy, Blocked users and Account destinations now live in collapsed Profile options, preserving all eight account destinations once after supplementary shell navigation is removed. Me-only activity padding/minimum height and portrait proportions are compacted.
- Cold activity remains explicitly unavailable and never starts an automatic feed read. An explicit Open feed link offers the next action. An authorized loaded empty page remains distinct from unavailable activity and full-history emptiness. The existing strict C04 parser, event/order/subject/privacy checks and two-row projection remain intact; public-profile callers do not opt into the own-profile action. No levels, points, fabricated parties/gifts or synthetic history are added to product code.

## Ownership and provenance

Social cleared Motion83's former Me feature reservation before granting the full `frontend/src/features/me/**` tree. Root153 and World recorded the exact PanelScreen route-links transfer before edits; the earlier conditional `routes.ts` grant was withdrawn unused. Motion83 retains external account/avatar/character integration hunks, none modified here.

Exact changed production paths:

- `frontend/src/app/WorldShell.tsx`: only the PanelScreen supplied-child route-links conditional.
- `frontend/src/features/event-feed/feed.css`: only the existing shell-height variable consumption.
- `frontend/src/features/me/ui/MePages.tsx`
- `frontend/src/features/me/ui/me.css`
- `frontend/src/features/me/ui/reference-profile/ProfileActivity.tsx`

Tests/evidence are confined to the granted Feed and Me feature trees and new `frontend/src/app/__tests__/qa06-feed-me-fidelity/**`. One existing Me visual-adoption expected-link assertion is updated for the preserved destinations. This document is the only new documentation path. No backend, settings, account-entry, character-entry, public contract, generated file, artwork, manifest, lockfile or configuration changes. Cross-directory imports were inspected; there are no backend implementation imports. No legacy rendering logic is copied into product code. `docs/source/` and supplied originals remain byte for byte unchanged; the planning checker verifies all thirteen archive checksums.

Reference: `docs/mcr/design/reference-target-2026-11-04.png`, SHA256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. Feed crop x3/y539/w298/h482; Me x1218/y539/w316/h482. QA06 historical comparisons came from its `codex/mcr-qa-QA06-mobile-visual` checkout at `980fc4d8`, source tree `6d3a321ccf4a7fb768798bc9fd13b79967835dae`. Those are historical comparison inputs, not this candidate's runtime evidence.

The test-only preview adapts QA06's actual application fixture `frontend/e2e/visual/qa06/preview.tsx` (source SHA256 `1c59c73e787566dbedea0e876bbf282b897bb1bf161ad41e6040ca20b492a2bd`). Imports resolve within this checkout. It uses canonical Auth, the real browser router, production default loader, one Phaser instance and the selected whole-character renderer. Synthetic HTTP/C05 availability, MockSocket frames and existing schema fixtures are explicit. Public authors remain separate from the actor; controlled `posts=own` and `posts=long` modes supply an authorized own post and sixteen ordered rows only for test cases. They do not change product data. No new dependencies; existing installed Node24.22.2/Chrome/Playwright/Vite/Phaser tooling and retained lock resolutions are used.

## Evidence and commands

Evidence root: `frontend/src/features/event-feed/__tests__/qa06-fidelity/evidence/`. Each native report records target hash, browser version, candidate base, source/contract SHA256 pins, exact selected asset bytes, requests, geometry, screenshots and cleanup. Source pins identify dirty candidate bytes independently of the base commit; compare them with the delivered commit. The before runner/preview snapshots are preserved byte for byte beside its report because the driver subsequently gained controlled populations and stronger checks. New untracked unit files are committed payload, while tracked source/contract files and the three preview driver files are pinned by the native report.

| Check | Command (from repository root unless noted) | Exit / result |
| --- | --- | --- |
| Before native | `MCR_FIDELITY_PHASE=before node frontend/src/features/event-feed/__tests__/qa06-fidelity/native.mjs` | 0; reproduced duplicate and footer gap at390; no automatic Me feed read |
| After native | `node frontend/src/features/event-feed/__tests__/qa06-fidelity/native.mjs` | 0;360/390/430, short/16-row Feed, compose/back, keyboard tabs, cold/loaded Me, preserved44px destinations, actual profile-route/back, last row above footer, no overflow, one canvas/socket, cleanup zero |
| Relevant regressions (frontend cwd) | `vitest run src/features/event-feed/__tests__ src/features/me/__tests__ src/app/__tests__/qa06-feed-me-fidelity src/app/__tests__/default-shell src/app/__tests__/shell.test.tsx src/app/__tests__/reference-root src/app/account-entry/__tests__ src/app/character-entry/__tests__/activity.test.tsx src/app/social-composition/__tests__` | 0;23files/244tests |
| Typecheck (frontend cwd) | `tsc -b --pretty false` | 0 |
| Lint (frontend cwd) | `oxlint src vite.config.ts` | 0;five pre-existing warnings outside this patch |
| Build (frontend cwd, after typecheck) | `vite build` | 0;existing large-chunk advisory |
| Planning | `python3 scripts/mcr/check-plan.py` | 0;PASS, not implementation acceptance |
| UI origin | `node scripts/mcr/check-ui-origin.mjs` | 0;PASS, static origin only |
| Boundaries | `python3 scripts/mcr/check-boundaries.py --base 29b1779e84106fcbe1c59f973cb5992d83c4fc00 --lane frontend --allow-doc docs/mcr/frontend/feed-me-qa06-fidelity.md` | 0;PASS (final staged payload; exact manual assignment review complete) |
| Whitespace / manual ownership | `git diff --check`; staged changed-path and exact shared-hunk inspection | 0;PASS (final staged payload; exact manual assignment review complete) |

Commands use Node24.22.2 in PATH and the existing dependency installation through a temporary own-checkout symlink; no install or lock mutation. Logs are archived below `evidence/checks/`, with local checkout paths normalized and trailing terminal whitespace removed. Initial red navigation tests reproduced two product failures before repairs. A subsequent existing link-list regression required its expected preserved destinations to be updated. Native driver issues (direct-panel socket expectation, occupied fixed port, wrong fixture endpoint, and leaving Feed before response readiness) were corrected in test-only code; none are represented as product defects or successful evidence. Failed preliminary drivers are superseded by the final pinned reports.

## Limits and remaining ownership

The supplied target's image-rich Feed population is not supplied by the one-row text fixture; absent optional media is a fixture difference, not a license to invent images. Cold Me has no previously loaded authorized activity by design. Selected source art and current saved data are preserved. Reference-only levels/points are outside the specified product and are not fabricated. The shell's ME title remains outside this exact route-links grant. The source fixes are bounded navigation/layout improvements, not a pixel-identical whole-app certification.

Native proof is local desktop Chrome mobile viewport emulation at844px height with reduced motion and synthetic HTTP/C05/MockSocket. No physical device, live service, authoritative movement, real server authorization, multiuser flow, load or release check is certified. Independent QA06 owns direct target qualification and its broader matrix; Root153 owns combined integration and publication. No main-chat or scrum-lead review is an added gate.

Initial native work paused at the8GiB resource floor. BackendSL coordinated retirement of a confirmed unrelated old executable cohort; this task did not delete peer binaries or stop peer jobs. Native resumed only after measured headroom exceeded the floor. Each own native run uses an ephemeral port and temp Vite cache, cleans its own browser/server/cache, and reads sparse public assets from exact Git blobs without materializing duplicate packs. Old FE17/QA05 evidence and its checkout remain untouched.

## Delivery receipt

Pending committed handoff and Root153 publication; keep the persistent goal active until actual remote ancestry/payload verification, required concrete fixes, final clean STOP and exact lease return are complete.
