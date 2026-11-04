# FE06 / FE34 incumbent runtime preferences

The producer source is INTEGRATED in published master `e2375727`. Its composed
source qualification passed 206 tests, full TypeScript checking and scoped lint.
The composed Settings/Phaser native qualification is NOT_RUN. The new harness has
passed full TypeScript checking, scoped lint and Node syntax checks.
The persistent producer goal remains ACTIVE through the remaining validation,
fixes, committed handoff, publication verification and cleanup. This report does
not accept whole FE06/FE34, W04, IN01/IN05, device behavior, an audio source or art
admission.

## Authority and ownership

The isolated checkout is `mcr-worktrees/fe-runtime-preferences`, branch
`codex/mcr-fe-FE06-FE34-runtime-capabilities`. The fresh intake base was
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`, containing `7c25078f`. The old FE18
branch and its evidence remain preserved; that goal is complete and the original
W04 invitations disposition remains open. The new unbudgeted goal was created
at 1793774015.

World prewrite grant `2ccf628c` permits only preference-specific changes in
`world/runtime.ts` and `features/theme/model/controller.ts`, new
`world/preferences/**`, new `features/theme/__tests__/runtime-preferences/**`,
this document and new `evidence/world-runtime-preferences/**`. All new
prefixes were absent at intake. Root separately granted exactly four MapScene
changes: import the helper, attach it after incumbent scene readiness, expose
its effects on the same EngineLease, and dispose it before existing scene
shutdown/destroy. World recorded the allocation in `61693981`.

Character source, pixels, crops, sizing, manual walk rendering, camera behavior,
background artwork, product UI/CSS, root composition, Auth, transport, physics,
contracts, manifests, lockfiles, CI and preserved source archives are otherwise
read-only. No dependency was added. Social adopter 53d owns the separate Settings
runtime-adoption patch; Root153 owns the production binding and sole serialized
master publication. Validation uses named 21d resource allocations; no extra
scrum-lead or main-chat review gate is required.

## Contract and production behavior

`world/runtime.ts` reexports `RuntimeEffects`, `RuntimeEffectValue` and
`RuntimeEffectLease`. The agreed structural contract is
`acquire({ reducedMotion, lowPower }, signal)`, returning a disposable lease or
promise. `WorldRuntime.effects` is one stable facade. Its capability flags come
from the actual ready incumbent `EngineLease.effects`; unavailable capabilities
fail explicitly. The facade creates no game, connection or identity.

Each acquisition captures the actual runtime lifetime and links caller and
runtime abort. Supersession cancels the former lease. Ignored asynchronous work
is bounded by the existing runtime timeout, and a late lease is disposed on
arrival. Abort, stop and disposal withdraw actual effects synchronously.
Obsolete disposal cannot reset a newer acquisition.

Only an explicit low-power lease wraps the incumbent Canvas renderer's
`preRender`, `render` and `postRender`. All three phases use one at-most-15-Hz
paint decision. Release restores the exact original methods while preserving
newer external replacements. Normal mode installs no wrappers. Scene updates,
controls, camera, movement sampling, heartbeat/network and authoritative physics
are outside this gate. This reduces Canvas paint work; it does not establish
battery savings, device behavior or a universal frame rate.

Reduced motion changes the actual scene TweenManager global scale to zero and
restores its captured value on release. The scene's manual character walk
renderer and all supplied artwork remain unchanged. Helper construction is
inert. Sound has no producer capability: the existing engine uses `noAudio`, and
no eligible source exists. The consumer can apply saved sound=false; a request
to enable sound must remain explicitly unavailable.

`ThemeController.select(theme, signal?)` cancels pending work, detaches owned
caller listeners, disposes ignored-abort candidates and reports the actual
incumbent skin with no stale loading phase. Cancellation after an indivisible
visual commit retains that skin. It does not roll back, reload or restart the
runtime. Older selectors cannot overwrite a newer request or persist a late
choice. The consumer does not own or dispose the shared ThemeController.

The paint deadline belongs to the engine lifetime rather than an individual
preference lease. Reacquiring low power for an OS motion change therefore cannot
permit an extra immediate paint. A deterministic phase-count regression covers
this boundary.

## Source qualification and provenance

The production helpers are newly authored for retained Phaser 3.90.0 Canvas
renderer and TweenManager ports. No legacy rendering was copied. Existing
EngineLease, MapError/abortable, ThemeController and world ownership/resource
boundaries are retained. Source tests borrow existing theme/world fixtures
read-only. Coverage includes normal method identity, paired paint phases,
restoration, caller/runtime abort, supersession, ignored abort, timeouts,
reentrant successor isolation, actual WorldRuntime stop/privacy forwarding,
committed-skin cancellation and stale candidate cleanup.

The original `source-01/report.json` records ten exact source postimages,
commands and exits. Its 172 tests in six files, scoped lint and planning check
passed. Full types initially exited 2 for three declarations: optional getter
undefined under exactOptionalPropertyTypes, test ThemeId narrowing and isolated
cache environment narrowing. The failure log is preserved. `source-02/report.json`
records corrected full types and scoped lint passing on WIP digest `43524a9b`.
No assertion or timeout threshold was weakened. The measured subtotals were
1,716,224 bytes / 12.25 seconds for source-01 and 1,380,352 bytes / 7.03 seconds
for source-02; they omitted only the tiny borrowed symlink allocation. Adding
4 KiB is a conservative ceiling for that omission. Each job removed its own
child, link and private temporary/cache tree before release.

The follow-up paint-deadline commit was handed off as
SOURCE-AWAITING-ROOT-VALIDATION rather than rerunning the already queued batch.
Root's `runtime-preferences-root/source-03/report.json` records actual cohort
`8972b0b634035055eb023dbc2f786e4463b172e5`: 206 tests in 11 files, full types and
scoped lint passed, 2026-11-04 07:16:56.885–07:17:13.006 UTC, 16.16 seconds,
21,012,480 bytes measured peak, with full resource release. Earlier failed
runner attempts remain in Root's evidence. This is source qualification, not
native acceptance.

Producer mappings are `3cd74cae` → `53b67c37` and `dd181c21` → `44d168f2`.
Fresh owner fetch and independent ls-remote both returned
`e237572719602da6a1a08b7fe0579b0e9c96da1f`. All 19 producer postimages through
`dd181c21` matched that remote, and both publisher commits were verified as its
ancestors. The producer branch ordinary-merged that publication. Its sole
add/add conflict was this producer document, resolved by retaining the truthful
new harness preparation; no shared application source was authored during the
merge.

## Composed native preparation

The new `world/preferences/__tests__/native/` harness is TEST ONLY, derived from
Root's `app/world-entry/__tests__/reference-scene-v5/preview.tsx` at `c8a731e5`.
It retains actual production Root, router, canonical Auth, ports, composition,
default Phaser and selected-source character rendering. Frozen C03/C05/source
fixtures supply deterministic HTTP, WebSocket and assets. Preference PATCH uses
the existing Me endpoint, schema and revision. This does not qualify a live
backend, the separately reported live `/me` 504s, production art admission or a
physical device.

The plan uses one 390-pixel browser and one Vite server. The existing Reduce
panel control keeps Settings mounted and the world visible during measurements;
ordinary navigation is tested separately as caller cleanup. Independent renderer
events and Scene UPDATE counts distinguish paints from updates. A data-only real
Phaser tween proves pause/resume without changing visible product pixels. A
hidden inline 120-ms transition under the actual shell must compute zero on OS
or user reduction and restore on release. An attribute alone does not certify
that the production motion CSS rule is loaded.

Other assertions cover save versus apply, OS OR behavior, actual ping/pong,
one game/camera/socket and six selected-source sprites, unavailable sound/theme,
post-settle caller abort, navigation, privacy withdrawal and full teardown. The
fixture lacks alternate admitted theme artwork: pending ignored-abort candidate
disposal is qualified by source tests, not falsely claimed as a native asset
success.

`source-03/report.json` records the separate new-harness qualification on
`0aa1afc597ca393131881e8e10dd69e52883cf22` with all three helper SHA256 pins.
Full project types, scoped helper lint and Node syntax checks exited zero;
no 206-test repeat, browser, Vite or build ran. The named allocation completed
2026-11-04 07:52:22.929–07:52:30.044 UTC in 7.159 seconds, with a measured
1,384,492-byte whole-workflow peak including all retained producer evidence,
new logs, private temporary/cache paths and the borrowed dependency symlink.
Its owned child, link and temporary/cache tree were fully released.

The one composed native job remains NOT_RUN until Root publishes the actual
motion-rule loading and latest adopter cleanup fix. Exact overlapping evidence
from Root's World/Map job will be reused; no duplicate destination screenshot
matrix is requested. Browser/server/profile/cache will be removed before the
application build. All failures, source pins, original UTC/KST capture times,
commands, limits and cleanup will be retained. There is no standalone adopter
native run and no duplicate 206-test producer run.
