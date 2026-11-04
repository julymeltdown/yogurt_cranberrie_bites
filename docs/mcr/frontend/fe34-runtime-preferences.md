# FE34 runtime preference application leaf

Status: INTEGRATED for the bounded runtime-preference application leaf.
This leaf does not certify original FE34, FE06, U07,
real-server flows, supplied artwork, or release acceptance.

Implementation owner: `01a1a53d-36b8-7522-ae11-646f5e6f0b30`.
Exclusive checkout: `/Users/lhs/.codex/worktrees/d68a/mcr`.
Branch: `codex/mcr-fe-FE34-runtime-preferences`.
Full base: `7c25078f118da53dd9a524d0a58e707cca190f0f`.
Persistent unbudgeted goal is active through implementation, validation,
documentation, committed handoff, Root adoption/publication, remote verification
and required fixes. Root153 is the sole frontend/art master publisher.

Exclusive authored paths:

- `frontend/src/features/settings/runtime-adoption/**`
- `docs/mcr/frontend/fe34-runtime-preferences.md`
- `docs/mcr/evidence/fe34-runtime-preferences/**`

No existing product component, layout, text, styles, art, character source,
backend, contract, central manifest, lockfile or source archive is modified.

## Original criteria and source contracts

The original plan FE34 requires profile and low-power/motion/sound/theme
preferences with identity-scoped settings and optional fields never blocking
World entry. U07 specifies sound off by default. FE06 requires skin switching on
the same world without changing geometry, room or avatar identity, with fallback
on asset failure. Existing profile/save/conflict behavior stays owned by FE34's
retained ME controller. This leaf neither invents optional profile fields nor
amends those remaining original criteria.

At the full base above, these owning sources establish the boundaries:

| Source | Retained contract |
|---|---|
| `features/settings/SettingsPage.tsx` | `RuntimePreferencesPort.apply(saved,scope,signal)` only after explicit click; lifetime linked through abort |
| `features/me/model/contract.ts` | Strict C03 preferences `theme`, `low_power`, `reduced_motion`, `sound_enabled`; defaults sound off |
| `shared/query/privacy.ts` | Canonical user/event/session generation/policy generation/gate, synchronous registered cleanup |
| `features/theme/model/controller.ts` | Existing theme owner and truthful selected/phase state, existing skin replacement |
| `world/runtime.ts` | Sole incumbent engine, camera and resource owner; no motion/power/audio setters at base |
| `world/composition/index.ts` | Current `WorldPresentationRelease` with exact request privacy scope/signal and incumbent theme projection |
| `app/account-entry/panels.tsx` | Settings route currently omits runtime and theme injection at base |

No reused Bigyeting code or rendering is introduced. All new logic is authored
for MCR using existing public TypeScript contracts and retained dependencies.
Server persistence remains solely the existing canonical protected request path.
No new DTO, request, identity, Phaser instance, socket, QueryClient, storage key
or cache is created by the adapter.

## Adapter and producer contract

`createRuntimePreferencesAdapter` borrows the original presentation release
scope/signal, canonical privacy coordinator, incumbent theme controller, runtime
effects and optional existing audio capability. Construction is inert. It never
applies saved settings or starts audio implicitly. `apply` is the explicit
RuntimePreferencesPort entry, separate from save. It snapshots and strictly
validates the four saved preferences.

The separate World runtime implementation owner is
`01a1a26e-1de2-7b41-a294-c9a79cfded9b`. Its negotiated structural producer API is:

```ts
interface RuntimeEffectLease { dispose(): void }
interface RuntimeEffects {
  readonly motion: boolean
  readonly lowPower: boolean
  acquire(value: Readonly<{ reducedMotion: boolean; lowPower: boolean }>,
    signal: AbortSignal): RuntimeEffectLease | Promise<RuntimeEffectLease>
}
interface RuntimeSound {
  acquire(signal: AbortSignal): RuntimeEffectLease | Promise<RuntimeEffectLease>
}
// Existing owner, extended by the producer:
select(theme: ThemeId, signal?: AbortSignal): Promise<void>
```

Capabilities must perform real effects on the same engine. Signal abort must
synchronously release/reset each acquisition's own effects, even before an
asynchronous acquisition returns. Disposable leases are idempotent and isolated
from successors; late ignored-abort leases are released on return. No audio
source exists at the base: sound enable is unavailable unless the composition
explicitly supplies a real authorized existing source. Sound off needs no audio
capability. A true support flag alone is not an effect implementation.

Theme cancellation fences pending commits and obsolete persistence. A skin
already committed atomically remains the incumbent skin until another explicit
selection or shell teardown; no rollback, old-skin reload or world remount is
requested. The adapter checks actual selected/ready state because the retained
theme controller catches loading errors instead of rejecting them.

The adapter ORs OS reduced-motion with the saved user setting and updates effects
when OS preference changes. Such updates never start/restart audio or theme
selection. User/event/policy transfer, presentation withdrawal, caller abort and
disposal release effects and audio; lifetime remains linked after apply settles.
Reentrant/superseding applies and asynchronous completion are fenced by exact
operation ownership. Work is bounded to 15 seconds by default. Missing,
unsupported, failed or timed-out capabilities reject, so Settings cannot claim
successful application. An OS-update failure releases the active effects/audio
and records `failed` in the adapter diagnostic state.

## Root handoff mapping

Root retains all existing-file writes. It must borrow the SAME current
`WorldPresentationRelease`, construct the adapter using its original
`request.privacyScope` and `request.signal`, forward producer effects/audio and
`theme`, and pass `runtime` and the same theme controller into SettingsPage.
Dispose the borrower on release replacement/withdrawal, identity/policy transfer
and owner teardown. Do not relabel a release with freshly captured authority or
construct a second runtime/theme owner. No existing-file grant has been assumed.

The owned `useRuntimePreferences` hook provides that consumer lifecycle. Root
passes canonical auth, the account owner's original scope and the current
presentation; the hook refuses missing effects, expired/mismatched authority and
release withdrawal, and disposes each replaced borrower. It supports StrictMode
without constructing any world resource or automatically applying effects.

The base presentation exposes only `read/subscribe/select` while SettingsPage and
ThemeSelector currently type the full ThemeController. Root can narrow those
consumer parameter types to the actually used public projection or explicitly
provide the full incumbent controller; a cast or fabricated controller is not a
production adoption.

The exact five-existing-file consumer proposal is preserved as text in
`docs/mcr/evidence/fe34-runtime-preferences/root-wiring-proposal.patch`, generated
against the full base above and **not applied** by this owner. It forwards the
incumbent `WorldRuntime.effects`, exports the existing presentation context,
binds the owned hook and narrows both theme consumer types. Root owns any
application/reconciliation of those existing-file hunks after the producer source
is available. No art, CSS, layout, text or character change is proposed.

Root applied exactly those five existing-file hunks in
`dd379b4512405403033b198abf2a3727cfc7f592`. The producer source was adopted as
`53b67c37`, adapter source as `dbb2d08c` and this handoff's initial documentation
as `5ef20bb4`. The adopter independently compared its source/document payloads
against those publisher commits: identical. This is actual production source
composition; subsequent publication and composed checks are recorded below.
No duplicate existing-file edits or ownership transfer was
made by the adopter.

Validated adapter source checkpoint:
`3eea76f47f9f7825c98e5d72616fcb8b72849720`.
Its exact 20 new paths, unchanged source-archive proof and base contract hashes
are recorded in `docs/mcr/evidence/fe34-runtime-preferences/handoff.json`.

## Validation and limits

Owned tests cover adapter behavior, actual retained ThemeController success and
failure, exact SettingsPage save/apply separation, privacy scope changes,
sound-default/explicit enabling, OS overrides, lifetime release, ignored-abort
leases, timeout and reentrant disposal. The controlled fixture uses real
SettingsPage/ME/Auth/privacy/ThemeController and this adapter with instrumented
effects/audio. Those effect counters are fixture evidence, not actual Phaser,
audio playback, live HTTP, multiuser or device proof.

Source check and browser workflows are separately queued with ResourceSL21d.
Root authorized reversible borrowing of its retained immutable dependency tree
at `/Users/lhs/Desktop/mcr/frontend/node_modules` through this checkout's own
absent-only symlink, removed after checks. No install, manifest, lockfile or
dependency resolution change is authorized. Generated cache/output must stay in
the bounded owned temporary prefix. Exact executed commands, exit codes,
evidence and delivered commits will be recorded after execution.

Initial named source workflow `FE34-SOURCE-CHECKS-202611040636Z` was stopped by
its output monitor with Vitest exit 143 before results. Vitest's temporary
client/SSR fetched-module transport output reached 12,814,847 bytes between
300ms monitor samples, beyond the initial 8MiB allocation. No test/type/lint
success is claimed. Its dependency symlink and entire temporary prefix were
removed; the failure/source-hash receipt is
`docs/mcr/evidence/fe34-runtime-preferences/source-check-stopped.json`.
A corrected finite allocation is requested; persistent filesystem module cache
is already off by default, and does not disable this temporary module transport.

Corrected workflow `FE34-SOURCE2-202611040640Z` completed with a measured
13,005,648-byte peak, within its 64MiB limit. The five test files passed all
52 tests in 2.94 seconds (adapter/Settings/hook plus retained ME/theme controller
regressions). Initial typechecking exposed four test-only typing errors; the
fixture IDs and Testing Library role options were corrected, then typecheck and
scoped lint both exited 0. No behavior changed in those typing corrections.
Planning provenance and patch integrity also exited 0. The serial initial checks
took 10.46 seconds; the typing fix checks remained within the same 180-second
allocation. All child processes ended, both temporary dependency symlinks were
removed after their respective runs, and the temporary prefix was deleted.

Exact commands (pinned Node24.22.2, from `frontend/` unless stated):

```text
MCR_FE34_RUNTIME_TEMP=/private/tmp/mcr-fe34-runtime-source TMPDIR=/private/tmp/mcr-fe34-runtime-source
node node_modules/vitest/vitest.mjs run --config src/features/settings/runtime-adoption/__tests__/vitest.config.ts --configLoader runner src/features/settings/runtime-adoption/__tests__ src/features/me/__tests__/controller.test.ts src/features/theme/__tests__/controller.test.tsx
node node_modules/typescript/bin/tsc -p tsconfig.json --incremental false
node node_modules/oxlint/bin/oxlint src/features/settings/runtime-adoption
python3 scripts/mcr/check-plan.py # repository root
git diff --check # repository root
```

See `docs/mcr/evidence/fe34-runtime-preferences/source-checks.json` for exact
pre-test and final source hashes, commands, exit codes and limits, with adjacent
logs retaining the initial type errors and the successful fix checks. No
application build, browser, live server, multiuser or device result is implied.
The standalone instrumented native fixture is intentionally NOT_RUN: the
runtime producer and adopter agreed to one combined native workflow with actual
Phaser, SettingsPage, canonical controllers and this adapter after Root's
production binding. That execution remains outstanding, not a separate QA gate.

## DOM reduced-motion follow-up

Root identified that engine effects alone did not project an explicit saved
reduced-motion preference onto the existing shell CSS. The owned adapter now
publishes motion/lifetime state to subscribers. `projectRuntimeMotion` borrows
the single existing `[data-mcr-shell]` element, sets the existing
`data-mcr-reduced-motion` attribute before theme selection and updates the value
using the adapter's OS/user OR result. The hook refuses a missing or ambiguous
shell instead of changing another document surface. No new wrapper, CSS rule,
Root file, layout, control, artwork or storage is introduced.

Caller abort, apply failure, release/privacy invalidation, hook replacement and
teardown restore the exact prior attribute (including absence). Old cleanup
does not erase a later external writer's value. Subscriptions are removed on
teardown; observer errors cannot strand engine effects. A reentrant abort during
applied-state publication is checked before the apply promise can report success.

New tests cover explicit projection before theme selection, OS OR updates without
audio restart, baseline/absent attribute restoration, privacy/caller abort,
failure, external writer preservation and reentrant abort. Root hook tests add
the actual shell element and exercise StrictMode and missing-shell refusal.
Root's subsequent composed source workflow
`ROOT-COMPOSED-SOURCE3-202611040716Z`, pinned to
`8972b0b634035055eb023dbc2f786e4463b172e5`, passed 206 tests across 11 files,
full TypeScript and scoped lint, all exit 0. Its measured duration was 16.16
seconds and peak temporary output 21,012,480 bytes. The committed command/exit
receipt and adjacent compressed logs are under
`docs/mcr/evidence/runtime-preferences-root/source-03/`. Two earlier unsuccessful
composed attempts remain recorded there separately. This composed result
includes the DOM projection tests, actual account-root binding and presentation
release tests. It does not certify loaded production motion CSS or native Phaser.

The adopter independently fetched and read back master
`e237572719602da6a1a08b7fe0579b0e9c96da1f`, checked all mapped source commits
and Root wiring as ancestors, and compared the complete owned source and
documentation payloads with local checkpoint
`7e776c538fd83e1e545ea65dd67ece12ba034aff`: identical, exit 0.
`docs/source/` remained byte-identical to the full base. Root's published DOM
projection commit is `df68f206db7cabcc525f6322f8d719e5e8bb501f`.

Production `main.tsx` did not load the existing theme sheet's reduced-motion
rules. Consequently, attribute projection alone leaves DOM suppression OPEN.
After the separate finite panel capture, Root took explicit ownership of only
an additive motion-rule stylesheet and its hook import; this owner leaves both
paths untouched. Broad palette/forced-color theme stylesheet loading is not
part of this handoff. Root delivered only the existing suppression rules in a
six-line `motion.css` and its import in the owned hook through
`7d4c523461840ff5bfdbff3df06c86757b0c8139`. The producer owns one combined actual
Settings/Phaser native proof of that published source, including computed
transition suppression and restoration. Its subsequent result is recorded below.

## Post-freeze cancellation correction

Source commit `d6dccde98a98e2a8ec9abb00fc2e0e52274a31ce`, based on
`7e776c538fd83e1e545ea65dd67ece12ba034aff`, changes only `index.ts` and
`__tests__/adapter.test.ts` in the assigned prefix. Releasing an old applied
effect can synchronously abort the incoming caller before it acquires its own
operation. The cancellation guard now withdraws the old applied state only
when the same revision still owns the empty active slot and the adapter is not
disposed. A reentrant successor or disposed state remains authoritative.

The new regression starts with applied reduced motion and sound, aborts the
incoming caller from old effect cleanup, and checks rejection, no new
acquisitions, both old releases and an observed idle/baseline state. The existing
reentrant-successor regression remains in place. Patch integrity exited 0;
tests, TypeScript and lint for this correction subsequently passed in Root's
necessary final composed source workflow, recorded below. No standalone
adopter workflow was launched.

## Final composed source checkpoint

Root published the cancellation correction as
`44aa157c56107309e57383e8615d5b37305a5761` and the preceding documentation
checkpoint as `a4197421a9022c7f9693dfe0fa7addec54434acb`. The final source
workflow `ROOT-PREFERENCES-FINAL-SOURCE-202611040810Z` used exact source
`7d4c523461840ff5bfdbff3df06c86757b0c8139`. The adopter read the committed
`docs/mcr/evidence/runtime-preferences-root/source-04/report.json` and all four
adjacent compressed logs: all 35 affected tests in four files, full TypeScript,
scoped lint and native harness syntax checking passed, each exit 0. The test
command uses the owned adapter test prefix, a bounded temporary Vitest config,
runner config loading, no cache, no parallel files and one worker. TypeScript
uses the project configuration with incremental output disabled. The receipt
retains each exact command and source/driver pin.

Execution took 10.529 seconds, with 13,328,384 new output bytes measured and a
20,742,144-byte whole-tree peak including 7,413,760 carried art bytes. The job
ended at `2026-11-04T08:06:23.623310+00:00`; its own temporary prefix and
dependency link were absent. This result covers the new cancellation
regression and imported CSS source, without claiming browser cascade execution.

A fresh fetch and independent `git ls-remote origin refs/heads/master` both
resolved to `9e7952fcd89360f7b4fe43c69f22060b9aa55c74`. The correction,
documentation and motion-loading commits above are all ancestors (exit 0).
The published `index.ts` and adapter regression exactly match local
`d6dccde98a98e2a8ec9abb00fc2e0e52274a31ce` (exit 0). Across the entire owned
prefix, the only additional changes are Root's explicitly transferred motion
stylesheet and hook import. The source archive still matches the full base.

The separate live run encountered two `/me` HTTP 504 responses and did not
qualify customization or real settings persistence. The combined native runtime
workflow uses explicit deterministic `/me` fixtures and cannot close that
real-server limitation. Sound enable remains unavailable in production because
no authorized audio source is supplied. Whole FE34 acceptance remains open.

## Combined native runtime qualification

The producer's single combined workflow qualified on its third bounded attempt,
`WORLD-PREFERENCES-NATIVE3-202611040829Z`. The original receipt is
`docs/mcr/evidence/world-runtime-preferences/native-03/report.json`, SHA-256
`ec6ff9191ae5312930034d70f66fdcbcb4d36365cdb91d8653b243aec5409cc1`.
Its committed clean harness source is
`f9781febcd3784f7beeff862bc0939f243018803`, which includes frozen production
`9e7952fcd89360f7b4fe43c69f22060b9aa55c74` as an ancestor (exit 0).
The adopter read the original receipt and relevant harness assertions, viewed
all three original 390×844 captures, and independently compared 20 reported
adapter/Root/runtime/theme source hashes against that published production
revision: every pin matched. No receipt, screenshot or workflow was duplicated.

The actual Root Settings route uses the retained ME/Auth/privacy controllers,
production adapter and theme binding, incumbent Phaser game and real MapScene.
Settings remains mounted in the existing reduced panel while the World is
visible. Controlled HTTP, C05 transport and source-controlled asset fixtures
remain explicit boundaries. The receipt records these results:

- Saving low power leaves engine methods, tween scale and CSS transition
  unchanged until the explicit Apply action. A baseline interval measured
  48 pre/render/post paints and 48 updates over 803ms.
- Applying saved low power measured 16 pre/render/post paints and 72 updates
  over 1203ms, within the 15Hz paint ceiling. Tweens, scene updates and transport
  pong traffic continue; this is not a battery or authoritative-physics proof.
- OS reduced motion stops the real nonessential tween (`scale=0`, zero delta)
  while updates continue. The loaded production CSS changes the probe from
  `0.12s` to `0s`. Removing the OS override restores the tween and `0.12s`;
  explicit saved reduced motion still applies with OS preference disabled.
- Sound off succeeds and creates no audio element or sound source. Explicit
  sound enable reports application failure because no eligible source exists.
- The game, camera, connection, six rendered sprites and map/geometry identity
  remain the incumbent ones through application and normal navigation. Native
  theme selection uses the available incumbent PARADE fixture; unavailable
  DANGER leaves PARADE selected with an error and no pending selection. This
  does not prove a successful alternate-art switch.
- Navigating away from Settings releases its applied lifetime, restores normal
  engine methods/tween scale and the exact prior shell motion attribute/CSS.
  A separately borrowed public producer lease is also synchronously restored
  by caller abort after acquisition settles. Adapter caller-abort/race cases
  remain covered by the source tests.
- Privacy withdrawal removes the private World presentation and restores the
  applied effects/attribute. Teardown closes the connection, removes the canvas,
  disposes the source cache and revokes all created object URLs. No page or
  runtime errors were recorded.

The original captures are `settings-saved-only.png`,
`settings-applied-low-power.png` and `settings-privacy-withdrawn.png` in that same
receipt directory, captured at `08:30:20.984Z`, `08:30:22.302Z` and
`08:30:23.548Z` on 2026-11-04. These are functional 390px captures; no new
360/430px visual-target or whole-product acceptance is claimed.

After closing the browser, development server, profile and cache, the workflow
also passed the actual `frontend/index.html` / `src/main.tsx` Vite application
build with target `es2022`, temporary output and `copyPublicDir:false`. The
successful awaited command and its parameters are retained in the pinned native
driver. This checks the application module build, not a full copied release
asset bundle. The workflow lasted 7.563 seconds, measured 78,586,053 peak bytes,
and completed at `2026-11-04T08:30:24.409Z`. Its receipt records browser/server
closure and temporary removal; the producer reported full allocation release
with its borrowed dependency link removed.

The original failed receipts remain under `native-01/report.json` and
`native-02/report.json` in the same evidence prefix. Attempt 1 stopped before
opening a browser while hashing an archived PNG absent from the checkout.
Attempt 2 reached the actual World/Settings baseline probes but its exact era
label locator timed out. The producer corrected the pin list and used the
unique combobox role with the Preferred era prefix, retaining the original
timeout and all behavior assertions. Neither failure established a product
defect, and neither is represented as a pass.

The producer committed the original native evidence, failed attempts, captures
and ownership manifest in
`c16c0c875b446f494137b72bc1feee6f27fdbe14`; its harness correction commits are
`948232992914a306d0a267511b5266a67e1d8098` and
`f9781febcd3784f7beeff862bc0939f243018803`. Root153 remains the sole publisher.
The adopter's final delivery reports the fresh remote ancestry and exact
receipt/document payload readback after publication; this snapshot's independent
production-source verification is pinned to `9e7952fc` above.

Implementation, composed source validation, the controlled native criterion and
committed documentation are the bounded leaf delivery. Original FE34, live
preferences persistence (including the reported `/me` 504), physical devices,
production art admission and release acceptance remain outside this result.
