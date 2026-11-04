# FE34 runtime preference application leaf

Status: IN_PROGRESS. This leaf does not certify original FE34, FE06, U07,
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
production adoption. Runtime producer delivery, exact root wiring and remote
ancestry/payload verification are still outstanding at this checkpoint.

The exact five-existing-file consumer proposal is preserved as text in
`docs/mcr/evidence/fe34-runtime-preferences/root-wiring-proposal.patch`, generated
against the full base above and **not applied** by this owner. It forwards the
incumbent `WorldRuntime.effects`, exports the existing presentation context,
binds the owned hook and narrows both theme consumer types. Root owns any
application/reconciliation of those existing-file hunks after the producer source
is available. No art, CSS, layout, text or character change is proposed.

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
