# Shared frame and footer — preserved baseline

Status: **IN_PROGRESS**. The original frame baseline is published. Root captured
the first background pass and corrected World/Map shared paint. Remaining panel
follow-up, source-check receipt and final publication verification are pending.
This bounded QA06-HUMAN-FRAME-FOOTER-01 correction supports AR02/AR04 and does
not close the original QA06, AR02, AR03, AR04 or any IN gate.

## Current background-only source pass

World recorded the exact CSS-paint extension before edits at `9153d3e5` /
`022730e5` in `docs/mcr/scrum/world.md`, GRANT C. The owner read that committed
grant and accepted it with actual goal/base/branch/scope readback. The isolated
checkout is unchanged; current source baseline is published
`7c25078f118da53dd9a524d0a58e707cca190f0f`, merged while preserving this owner's
tests, docs and evidence. Goal1793771449 remains ACTIVE and unbudgeted. New work
follows the superseding human direction; its old wording is historical.

Root's first World/MAP proposal is implemented in four explicitly granted
background declarations: `.mcr-chrome-header` and `.mcr-chrome-panel-header`
use dark berry `#251017`, and `.world-shell` uses peach `#fff3ec`. Existing
`#f2eadf` header text is unchanged; calculated sRGB contrast is15.11:1. Dark
`#191817` text against peach is16.28:1 where that pairing is actually used.
These are arithmetic source checks, not browser-computed style evidence.
The existing `.world-shell::after` now has a pastel rose/lavender/mint
`background-image` gradient, within its unchanged perimeter mask. The original
background shorthand retains its non-image defaults; only the image is
overridden. All other CSS declarations, foreground colors, control state backgrounds,
typography, borders, masks, layout and artwork remain unchanged. In particular
`parade-footer.tsx` is READONLY for this stage. No source pack, avatar pixel,
portrait crop, world actor, landscape image or renderer is edited.

The first actual390px eight-panel capture cohort is Root candidate
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`, job
`PANEL-COZY-202611040632Z`,06:33:32.440–06:34:13.328UTC. It contains the initial
three paints (`#614252` headers), not the later perimeter gradient. Direct
owner inspection of its before/after saved images found that the unchanged
focused-title red outline became less distinct. RGB samples from the saved
ME pair identify the ring as `#a2232b`, original background `#191817`, and
candidate background `#614252`. Calculated contrast drops from2.37:1 to1.17:1.
The owner therefore corrected only the two header backgrounds to `#251017`:
ring contrast becomes2.41:1 and header text15.11:1, preserving or improving
both original pairings. Focus styles, text and control foregrounds stay frozen.
The original red ring itself remains below3:1; this is baseline preservation,
not a claim of full focus-contrast compliance. Corrected-source World/Map
verification is recorded below; the c8a images keep their actual old palette.

The independent review is recorded in
[`owner-review.json`](../evidence/reference-frame-footer/background-review-c8a731e5/owner-review.json),
alongside a byte-identical copy of Root's actual report (SHA-256
`59e0f2540a1fdc79f240230048385fda678c1029e9d3df3ab2a209320ae86da1`).
All16 screenshot hashes and bytes were checked against that report. All eight
saved control, document-image and text metric pairs compare equal. The owner
directly viewed all16 PNGs; this inspection reused saved evidence and launched
no browser. Document-image metrics exclude CSS imagery and canvas sprites;
they do not certify every rendered character pixel. World/Map,360/430 widths,
other skins, real devices and live service flows remain unqualified here.

The new `background-contrast.test.ts` adds two CSS-backed contrast regressions
and a negative control reproducing the observed first-pass failure. It checks
the frozen red ring and heading text against their historical background
pairings. It is prepared for Root's next source-check cohort and is currently
**NOT_RUN**. It is separate from screenshot acceptance and full accessibility
verification; no extra broad suite or native allocation has been launched.

The fourth paint was left untouched until World explicitly recorded the exact
`.world-shell::after` background-color/image/size extension at `2ccf628c`.
The owner read and accepted that record before editing. No pseudo-element
geometry, padding, border, position, z-index, pointer behavior or mask changed.
Further panel backdrop work is a distinct later source pass.

`git diff --check` passes for these source edits. This owner has launched no
native/build/browser or install job. The first actual390px World/Map before/after
is reviewed below using Root's separately allocated source cohort. The returned
0612 offer grants no launch. Root's scenery tuple remains a separate, pinned
source scope. Further panel/skin evidence retains its own actual source, time,
viewport and fixture limitations. A color proposal or source image alone is
not proof of the requested appearance in the application.

## Corrected World and Map saved-evidence review

Root job `PARADE-WORLD-MAP2-202611040654Z` captured source
`4e491a4509e9cab6a3e28364db616bdab9c157f5` at
06:54:12.310–06:54:41.696UTC. The owner directly inspected all eight saved PNGs:
World and Map before/after at390×844, then candidate views at360×844 and430×844.
Each screenshot's bytes, SHA-256 and actual dimensions match the original
report. All four assigned shared frame/footer source postimages match exactly.
The independent review and byte-identical report are retained in
[`world-map-review-4e491a45`](../evidence/reference-frame-footer/world-map-review-4e491a45/owner-review.json).

The scenery is visibly softer and cuter: pastel flowering trees, mint lawn,
warm stalls and pink/lavender roofs. Stage artwork, baked labels, character
framing and controls remain familiar. The saved390 pair's six sprite texture,
frame, origin, position and size records, camera projection and labels compare
exactly. Map checks report51 objects and zero differing channels against the
cohort's expected composed image. All four Map targets remain at least44px.
The360/430 captures were directly viewed; they are not new baseline pairs.

Actual390 Map PNG samples retain red ring `#a2232b` at(21,22), while its
background at(230,30) changes from `#191817` to the corrected `#251017`.
Calculated ring contrast is2.37→2.41:1. The pastel perimeter is also actually
rendered. BG-FOCUS-01 is therefore rendered-fix verified for this Map cohort;
the targeted test execution and remaining panel follow-up remain pending.

This uses the actual root/Places/Phaser renderer with synthetic C03/C05
map7005 and NOT_ADMITTED provenance. Other eras, live authority and devices
are unqualified. The report records browser/server/temp cleanup. The earlier
`PARADE-WORLD-MAP-202611040651Z` attempt atc1352d14 failed its separate-Phaser
diagnostic and produced zero screenshots; it remains a failure, not evidence
of success. These source-specific cohorts are retained separately from the
first eight-panel captures and are not relabeled as one simultaneous run.

## Partial source publication receipt

Fresh `git fetch origin master` and `git ls-remote origin refs/heads/master`
both resolve published master to `49ac314e4cc8cae26a032618743274eb21d26567`.
The owner independently verified delivered ancestry for the original frame,
separator correction, warm paint, perimeter gradient and dark-berry contrast
fix. All four assigned application files match current owner postimages byte
for byte. Commands and hashes are recorded in
[`publication-49ac314e.json`](../evidence/reference-frame-footer/publication-49ac314e.json).

This verifies publication of application source. The new regression/review
handoff is integrated asb4258353 but is not yet on that master; the World/Map
review handoff afa20482 also awaits publication. Targeted test execution and
remaining panel follow-up are still pending. The goal and CSS lease remain
active until those gates and final remote evidence/doc verification complete.

## Superseding human direction — 2026-11-04 06:10 UTC

The human now requires noticeably cuter **backgrounds only**. Characters,
ornamental artwork, crops, source packs, atlases, renderers, typography, text,
controls, layout and focus are frozen. Root153 relayed this direction and
World confirmed the exact allocation boundary. The former distressed-frame /
marching-ground visual target and its pending23-case job are superseded.
The historical implementation/provenance and original PNG/JPEG bytes below
remain preserved; they are not an acceptance claim for the revised direction.

The owner initially stopped **before accepting or launching** resource offer
`QA06-FRAME-FOOTER-202611040612Z`. It was returned unused: no browser, server,
cache, temporary profile or dependency link was created. No new background or
ornament write followed the steering. A proposed flower/music footer replacement
was withdrawn; no SVG artwork change is authorized by the background direction.
The existing decorative lease was stopped for superseded visual writes. The
new paint grant is now recorded and accepted as described above.
Root coordinates the minimum World/MAP paint proposal and first actual390px
before/after; later panel backdrops and affected-route checks follow their actual
allocation. There is no King/SL technical approval gate. Goal1793771449 remains
ACTIVE and unbudgeted without reset or false completion; its historical wording
does not override the new human instruction.

Source43b492dc was published as
`bfa0193939c3eebd6c80ade85083c6530c6875e1`. The owner independently fetched,
checked `ls-remote`, verified delivered ancestry and compared all seven exact
postimages with exit0. Root reported19 combined footer/chrome tests, full frontend
typecheck and scoped lint passing in its source cohort; these are integrator
checks, not fresh owner native or visual acceptance. A source audit found one
extra titlebar separator declaration outside the exact inset-ring grant.
Owner correction `0ee51c16a9948c45f52784095a2d4694b0c410cd` restores its original
`border-bottom: 3px double #d7cab3`; Root published it as
`bdc1ef6eb8b391ea980ae684bb5164954d895e13`. The owner independently fetched
master7c25078f, checked `ls-remote`, confirmed bdc1 ancestry and exact corrected
ConcertWindow bytes with exit0. Raw6ca capture provenance remains unchanged.

Seven actual Root captures at6ca52f88, saved06:05:14–06:05:33UTC, are preserved
byte for byte under `../evidence/reference-frame-footer/reused/`. Their manifest
retains originalsource/time/844px viewport/fixture qualifications. Three shared
files match exactly; ConcertWindow differs by that one restored separator.
Pinned WorldShell/embedding/CSS proof shows the embedded1px clipped titlebar's
`border:0` overrides it for those seven root cases. This is explicit rendered
applicability for the historical baseline, not raw four-file equality, new
corrected-source screenshots, standalone-window proof or equal target aspect.
The prepared source harness verifies these facts but has **not been run**.

The following sections record the former baseline and its unexecuted validation
plan. Their proposed native work is historical and must not launch against the
superseded visual objective.

## Ownership and source

Owner chat213 uses isolated worktree `fe-reference-frame-footer`, branch
`codex/mcr-fe-QA06-reference-frame-footer`, base
`6f2d289bdce8154a3aa42b981c6ee6d9f5923448` from published master.
World's exact lease is recorded in `f9e3ea9c` and acceptance in `8ffa4bee`.
Root153 is the serial master publisher. Goal is ACTIVE and unbudgeted through
implementation, checks, fixes, handoff, publication and fresh owner verification.

Only decorative declarations in `concert-window.css`, `shell.css`, and
`shell-chrome.css`, plus the ornament in `parade-footer.tsx`, are changed.
Border widths, all layout metrics, actual focus/current-page/forced-colors
rules, controls and component behavior are preserved. The existing outer shell
perimeter mask remains outside this lease. No MAP semantics, routes, providers,
networking, manifests, contracts, backend, selected art or source archive changed.

## Human reference and artwork provenance

The actual human reattachment is a 1280×853 JPEG, 482737 bytes, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Its exact bytes were hashed and directly viewed before implementation. The
earlier PNG is historical context and is not substituted for that authority.
The source packet at `fc65be64`,
`docs/mcr/evidence/visual/qa06/human-reference-20261104/shared-frame-footer-packet.json`,
identifies repeated nested framing and seven transformed copies of one angular
marcher. Its pinned actual Meet/Me/Gifts/World images were directly viewed.
They are historical source cohorts, not captures of this corrected source.

Before authoring, the owner read `docs/mcr/character-art.md` and inspected the
selected source catalog (`mcr_user_characters_20261103`, revision3, no legacy
fallback). No contact-sheet pixels, avatar cutouts, legacy characters or human
reference pixels were imported, traced or edited. All selected originals and
reviewed derivatives remain byte-for-byte untouched. The explicitly assigned
ornamental SVG is newly authored vector code: nine independent irregular coat,
head and stride contours, a ragged dark ground, sparse print flecks and corner
web strokes. It is not an approved player-avatar derivative and is never used
as a selectable character, fallback, world actor, participant or live-person
projection. The footer remains `aria-hidden`, unfocusable and noninteractive.
No external art dependency, generation service or new asset path is used.

The frames now use one solid edge instead of double borders/inset outline
stacks; sparse irregular window scuffs replace the regular repeated scratches.
The titlebar loses its inset ring. Ordinary symmetric controls and required
keyboard focus remain. No decorative left accent lines are introduced.

## Validation plan and current evidence

Source-only `git diff --check`: exit0. Changed paths inspected against the
four-file exact hunk lease; no imports added to application code. New files are
restricted to the assigned reference-frame-footer test/doc/evidence prefixes.

Two focused DOM regressions are prepared for decorative accessibility/tab
order and multiple-footer SVG reference isolation. Existing shared shell/window
tests cover embedding, close behavior, route persistence, panel sizing and Back
focus. Typecheck and lint remain required. They have not been run at this source
handoff; no browser/install/build/native allocation is inherited.

The new owner runner in
`frontend/src/app/__tests__/reference-frame-footer/native.mjs` reuses the existing
QA06 DEV-only actual Auth/router/production-loader fixture read-only. It plans
one PARADE cohort across ten routes at360/390/430, registered phone aspects,
plus five390 baseline samples for exact unchanged frame/footer geometry.
It records original capture times, source hashes, asset hashes, errors, actual
focus/forced-colors,44px chrome, overflow, cleanup and one-host/socket checks.
The old QA06 three-era matrix may inform unchanged behavior only; it cannot
certify corrected source or current MAP architecture. No obsolete v4/full
three-era/semantic MAP matrix is rerun. The baseline Vite plugin reads only the
four exact original Git blobs; candidate source and all other files use this
worktree. No baseline source file is overwritten.

Original QA06 reference aspect registration is only a scaled presentation guide,
not a pixel oracle. Corrected frames still require direct owner comparison
against the exact supplied JPEG; a screenshot count or unit test is insufficient.
Wall's unavailable guard, old test-map admission and synthetic fixture state
will be labeled honestly. Real iOS Safari/Android Chrome, software keyboard,
hardware safe area, live multiuser/API authority and public save remain NOT_RUN.
No fake activity, media, level, currency, member portrait, provider or consent
state is added to imitate the reference.

BackendSL21d registered a future named workflow after renewed IN01 final release.
No capacity offer has been accepted and no install/build/browser/native job has
started. Retained frozen dependencies will be reused read-only first. Proposed
whole-workflow disk cap0.2GiB, temporary100MiB, retained30MiB, start>=16GiB,
stop8GiB; any dependency install needs an exact separately allocated estimate.
Native evidence, qualified comparison, fixes and final publication receipt will
follow under `docs/mcr/evidence/reference-frame-footer/`.
