# Original FE06 completion

Status: IN_PROGRESS. Developer-owned qualification and normal Root153 publication remain required. Base: `fcac0bed033b3f0bca336df6417d3ff617ffb637`; exclusive branch: `codex/mcr-fe-FE06-theme-completion`.

The original FE06 row requires Revenge, Parade and Danger tokens, selection persistence, asset-loading fallback, reduced-motion transitions, and a switch that preserves geometry, room and avatar identity. Failed artwork must retain the previous skin. This assignment covers that whole row; the change below fills its missing default-app palette binding.

## Implementation and ownership

The canonical source already had semantic palettes, the identity/event-scoped local preference adapter, the transactional theme controller, runtime compatibility checks and the actual era controls. Its default application never imported the palette sheet or projected the controller's committed selection onto its shell. Consequently, switching the world skin did not switch the application's semantic tokens.

`features/theme/original-fe06/useShellTheme.ts` now imports that existing sheet and borrows the one existing `[data-mcr-shell]` element. `shellProjection.ts` observes the existing controller's committed `selected` value. Loading or a failed asset cannot project a pending selection. Presentation abort, controller disposal, replacement and unmount restore the exact original attribute; old emissions and cleanup cannot replace a successor or erase a later external value. The binding creates no wrapper, controller, preference store, identity, renderer, engine or connection.

Root153 explicitly transferred only the hook import and invocation in `app/world-panels/index.tsx`, using its existing `current?.theme` and `current?.request.signal`. Its other composition logic is unchanged. Theme/token ownership was separately transferred; no Me/Profile, Auth, ports, router, World, compatibility, Map, character, manifest, lockfile, backend or infrastructure ownership was acquired.

The existing `world-shell` paper override must not win over the selected palette. The three palette selectors therefore also identify an existing themed shell. Existing illustrated panel bodies and World tools retain their original warm semantic colors and light native-control scheme. This preserves legibility of the selected artwork and its reference frame, without repainting artwork, changing geometry or adding decorative borders. The existing `data-mcr-reduced-motion` owner and motion stylesheet remain unchanged.

## Original criteria

| Criterion | Implementation and evidence |
| --- | --- |
| Three semantic token palettes | Existing `shared/styles/themes/themes.css`; this change loads and binds them in the actual default app rather than only isolated fixtures. New qualification is required below. |
| Selection persistence | Existing `features/theme/model/preferences.ts` and controller: encoded user/event local key, write after successful skin commit, scoped restore, truthful storage failure. `world/composition/index.ts` owns that same controller and invokes restore. Retained theme integration and QA06 W06 results are reused. Local device persistence does not imply server preference synchronization. |
| Failed asset restores previous skin | Existing controller and runtime retain the committed lease until a compatible candidate commits; failed/late candidates are disposed. The projection also retains that committed palette. Retained theme integration and runtime-preferences native-03 are reused; the new default-app check exercises the changed binding during a pending and failed texture. |
| Reduced motion | Existing theme scope rules plus production Settings runtime adapter's user/OS OR, borrowed shell attribute, and `motion.css` remain intact. No new animation or transition is introduced. Retained production native-03 covers preference application and restoration. The new projection must leave that attribute untouched. |
| Geometry, room and avatar identity survive switch | Existing runtime compatibility checks and one-controller/one-World composition are unchanged. Retained theme integration/native-03 supply the engine/camera/geometry/character evidence. The new binding must preserve the same shell, host, canvas and socket and cause no new domain mutation or ticket. |

## Retained evidence and limits

- `docs/mcr/evidence/theme-integration/index.json`: retained 262 tests and native 360/390/430 results for theme transactions, scoped restore, resource cleanup and identity preservation. Its synthetic-texture and sentinel limits remain historical limits, not current live or selected-art certification.
- `docs/mcr/evidence/world-runtime-preferences/native-03/report.json`, `qualification.json` and `capture-packet.json`: controlled actual Settings/Auth/Me/Phaser/theme composition, Parade success, unavailable Danger fallback, motion application/restoration, same game/camera/connection/geometry/six sprites. Source pins include `9e7952fc` and motion composition `7d4c5234`; publication `1bb911c3`. No live persistence, audible producer, device or battery claim is inferred.
- QA06 owner W06 handoff: source `ceeeb29abaa8351539ee864e02970df285e2db06`, final `7f48a3e12efd9e1b6302a12710963607fd16216d`, `docs/mcr/evidence/visual/qa06/software-remainder/nearby-era-theme/round-03/`: actual default-app three-era controls and account/event-local restoration at 360/390/430. This is retained software-width evidence, not a fresh functional rollback, selected-art identity or live-server proof.
- Avatar1eed's reviewed admission and persistence/moving-peer packets remain independently scoped; no old TEST_ONLY avatar manifest is relabeled as current reviewed admission. This patch does not modify character art, registry keys, admission, config, renderer or runtime.

## New qualification

NOT_RUN pending a fresh finite offer from resourcelead21d. Prepared scope: focused projection/StrictMode/lifetime/controller-failure tests, full TypeScript, scoped lint, planning check, browser syntax and one 390px actual-main cohort for the changed default binding. No full historical matrix replay, install, build, database, live service, device or QA08 job is authorized here.

The new native HTML borrows the existing `app/social-composition/__tests__/sharing-safety-default/native.tsx` network fixture read-only. That fixture performs actual UI sign-in and strict canonical enrollment admission before bootstrap, supplies controlled HTTP/C05, and imports actual `main.tsx`. Its immutable selected-source `wholeData` assets remain unchanged. It supplies no harness Auth, controller, readiness factory, renderer or replacement World. This is controlled producer data, not a live backend or full reference acceptance.

Finite supervision in `docs/mcr/evidence/original-fe06/{admission,lease,combined}.py` adapts the same files from `docs/mcr/evidence/sharing-safety-default-app/` at the base revision. It preserves one acceptance baseline/deadline, full checkout/shared-Git/dependency/browser/Node carry accounting, positive growth across red/fix/green, process/RSS/free-space guards and full resource cleanup. All new source is authored for MCR; no historical source or `docs/source/` bytes are changed.

## Delivery

Root153 is the publishing developer. Source/evidence commits and the canonical receipt will be recorded after qualification; there is no king/SL review or acknowledgment gate. Whole live integration, real backend persistence and full ten-screen visual/device acceptance remain outside these client receipts and are not silently marked accepted.
