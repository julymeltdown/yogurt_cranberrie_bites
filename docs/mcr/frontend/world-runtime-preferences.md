# FE06 / FE34 incumbent runtime preferences

Status: IN_PROGRESS. Production source is implemented; 172 source/regression
checks pass. Corrected full types and scoped lint now pass. Combined actual-engine/adopter
native qualification remains pending. No complete FE06/FE34, W04, IN01/IN05, device,
audio-source, or art-admission acceptance is inferred.

## Assignment and ownership

New isolated `/Users/lhs/Desktop/mcr-worktrees/fe-runtime-preferences`, branch
`codex/mcr-fe-FE06-FE34-runtime-capabilities`, fresh published base
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`, containing `7c25078f`.
New unbudgeted goal1793774015 is ACTIVE through implementation, validation/fixes,
actual incumbent/adopter proof, scoped committed Root153 handoff, publication,
fresh owner remote verification and cleanup. Old FE18 branch/evidence remain
preserved and its goal is complete; original W04 invitations remain open.

Prewrite World grant2ccf628c permits only preference-specific hunks in
`world/runtime.ts` and `features/theme/model/controller.ts`; new
`world/preferences/**`, `features/theme/__tests__/runtime-preferences/**`, this
document and new `evidence/world-runtime-preferences/**`. All new destinations
were absent before writes. Root separately granted exactly four MapScene hunks:
import helper; attach after incumbent scene readiness; expose effects on the same
EngineLease; dispose before incumbent shutdown/destroy. World recorded that
allocation in61693981. Scene/art/characters/crops/sizing/camera/renderer-source,
UI/CSS, root composition, Auth, transport/physics, contracts, manifests, lockfiles,
CI and preserved source archives are otherwise read-only. No dependency added.

Social adopter53d owns only the separate settings/runtime-adoption leaf. Root153
owns production composition/import binding and is sole serialized publisher.
There is no additional SL/King review gate. A new 21d allocation is required for
native/browser/build work; source implementation proceeds independently.

## Public contract and actual behavior

`world/runtime.ts` reexports RuntimeEffects, RuntimeEffectValue and
RuntimeEffectLease. WorldRuntime.effects is a stable facade. Its availability
flags reflect the current ready incumbent EngineLease.effects; requests without
that actual capability fail unavailable. The structural contract agreed directly
with53d is `acquire({reducedMotion,lowPower}, signal)` returning a disposable lease
or promise. Sound is not exposed: the existing engine is configured noAudio and
there is no eligible authorized source. Saved sound=false does not need one;
sound=true remains an explicit unavailable application in the consumer.

Acquisition captures one actual runtime lifetime and links caller and runtime
abort. Supersession cancels the former lease, and stale asynchronous acquisition
is disposed on arrival. Abort, stop and disposal synchronously withdraw actual
engine effects. Obsolete disposal cannot reset a newer acquisition. The existing
runtime timeout bounds ignored work; no game, connection or identity is created.

Only an explicit lowPower lease wraps CanvasRenderer.preRender/render/postRender.
All three paint phases share the same at-most15Hz decision. The original methods
are restored by identity on release; unrelated replacements are preserved. No
wrapper exists in normal mode. Scene UPDATE, controls, camera, movement sampling,
heartbeat/network and authoritative physics are not throttled. This is a canvas
paint-work reduction, not a claim of mobile battery savings or measured universal
FPS. Reduced motion sets the actual nonessential TweenManager global scale to0
and restores the captured incumbent value. Character source/pixels/crops/sizing/
manual walk renderer and background artwork are unchanged. Helper construction
is inert until an explicit preference lease.

ThemeController.select(theme, signal?) cancels pending work, detaches owned caller
listeners, releases ignored-abort candidates and reports the actual incumbent
skin with no misleading loading state. Cancellation after an indivisible visual
commit retains that committed skin; no rollback, second load or runtime restart.
Older callers cannot overwrite a newer selector or persist a late choice. The
consumer does not own or dispose the shared ThemeController.

## Validation and provenance

New production helpers are authored for the retained Phaser3.90.0 public Canvas
renderer and TweenManager ports; no legacy rendering is copied. Existing
EngineLease, MapError/abortable, ThemeController, frozen C03/C06 and actual runtime
ownership/resource boundaries are retained. Source tests borrow existing theme
and world-runtime regression fixtures read-only. Tests cover paint-phase pairing,
normal-mode method identity, captured-scale restoration, caller/runtime abort,
supersession, ignored abort, timeout, reentrant successor isolation, actual
WorldRuntime stop/privacy forwarding, committed-skin cancellation, new selection
protection and disposed candidate cleanup.

`source-01/report.json` pins ten exact source postimages, commands and exits.
172 tests/6 files, scoped lint and planning exit0. Full types initially exit2 for
three declarations: optional getter undefined under exactOptionalPropertyTypes,
a test ThemeId narrowing and required isolated cache-env narrowing. The small
original failure log is preserved; source-02/report.json records corrected full
types and scoped lint exit0 on exact ten-path WIP43524a9b. That finite follow-up
measured1,380,352 bytes/7.03seconds and released own link/temp/child immediately. No assertion or timeout threshold was weakened. Source job used a
new named21d allocation, measured1,716,224 bytes and12.25seconds; its own dependency
link, private temporary/cache tree and child were removed before release.

The combined native plan is ONE actual SettingsPage/ME/Auth-privacy/adopter +
incumbent Phaser + existing ThemeController source-cohort workflow, coordinated
directly with53d and Root. The adopter standalone instrumented-effects capture
stays NOT_RUN to avoid duplicate proof. Root source-only integration precedes
this composed job so all imports come from one pinned checkout. Controlled HTTP/
WebSocket/asset fixtures are labeled; they do not certify a live backend or
production art admission. Actual source/contract pins, commands, original capture
times, cleanup, limitations and published ancestry will be recorded with the
result; no native pass is claimed before execution.
