# Production world-entry resolver and FE18 bridge

Status: **MOCK_READY for bounded integration review**. The production adapter has no fixture release, artwork fallback or manufactured admission. Its deterministic boundary tests pass. Current supplied C03/C06 fixtures are intentionally ineligible for product rendering, and the selected source pack has no prepared runtime atlas. Actual renderer/world availability, live services, root wiring, invitations and IN01 remain **NOT_RUN**.

Original assignment base `4d6fc85b710f69a73b6942374308941d8f7a7c38`, branch `codex/mcr-fe-world-entry`, isolated checkout `fe-world-entry`. Parent explicitly authorized cherry-picking only composition commit `17e5d43f3039f85393eff583c29071e9c32be57c`; its identical local cherry-pick is `26945ebd26c551f64a3ce19d436de66823c86a5b`, the adjusted implementation base. No duplicate prerequisites were selected. This patch changes only `frontend/src/app/world-entry/**` and this document; no composition implementation, app root/ports/router/main, backend, contracts, manifests, locks, generated artifacts or supplied source/art changes. Only the integrator publishes.

## Resolver

`createWorldEntryResolver({ auth, origin, localInput, assetFetch? })` returns the **committed public WorldReleaseResolver type**, not a parallel local DTO. Its request is the existing WorldReleaseRequest: ActiveScope, exact captured PrivacyScope, immutable ShellBootstrap with its document, and AbortSignal. Import from `app/world-entry/index.ts`.

The resolver captures primitive scope values and the supplied immutable document before awaits. It rejects missing/mutable/accessor/exotic document objects, mismatched actor/event/generation, stale policy, non-open gate and aborted scope. It revalidates the existing document locally with the canonical C03 decoder/hash/cross-manifest checks, then compares its complete shell projection to the supplied bootstrap. It never fetches bootstrap again or recaptures policy to bless older data. The returned `bootstrap` is the exact original immutable `ShellBootstrap.document` reference.

Within that same FE02 `privacy.run` lease, it requests protected/core `GET /api/concert/events/{eventId}/rooms` through the canonical SessionController. FE01 supplies `no-store`; FE18's existing `validateWorldRooms` applies the frozen C05 oracle and request/actor/event/status/default/uniqueness checks. No new fetch client, token store, refresh override, private cache or command is created. Canonical qualified refresh remains solely Auth's decision. Noncooperative late reads are fenced by the scope and abort boundary.

The selected theme is **the server-persisted `participant.preferences.theme` in the captured bootstrap**, validated against its theme manifests. No local default or preference refetch is invented. Room choice is `WorldRooms.default_room_id`, which must be a member of the validated catalog. Full default rooms do not trigger a guessed alternative or success state: tickets and replacement snapshots still decide admission. This adapter returns no admitted flag.

Resources must be PROVIDED with their exact paths and hashes. Map, selected theme and avatar provenance must be reviewed ORIGINAL_ART; synthetic C06 manifests/places, unreviewed art, missing resources and foreign avatar sources are refused. Avatar `provenance.source_id` must identify the user-selected `mcr_user_characters_20261103` source pack. A future reviewed derivative manifest must retain that source authority; this does not approve or generate any derivative. Its `source_revision` and reviewed provenance remain part of the canonical server-provided manifest/hash. No `NOT_PROVIDED` record is filled in by the browser.

The least-privilege allowlist contains exactly the map JSON, selected-theme PNG and selected-source avatar atlas PNG. It is derived from those authorized manifests and passed to existing `createAssetResources`, which enforces canonical same-origin `world/` paths, extensions, SHA-256 pins, duplicate refusal, bounded byte reads, content type, redirect refusal and hash verification. Other themes and arbitrary caller URLs are outside this release's allowlist. Resource descriptors are copied/frozen before async work so caller mutation cannot replace a pinned hash. Every read is again bound to the original privacy lease and both bootstrap/caller abort signals; logout/policy change or bootstrap abort prevents later reads or commits. No external asset URL, legacy image or test fixture is imported by production code.

`localInput` is required and has no fabricated defaults:

```ts
{
  revision: string,
  reviewedBy: string,
  movementPolicy: SimulationPolicy,
  zoom: number,
}
```

The integrator supplies actual reviewed application configuration. Nonempty revision/reviewer fields record its provenance; they are not an approval engine. The factory snapshots it, validates all FE15 policy bounds against the authorized map geometry and requires finite zoom in [1, 8]. These values govern local intent/path generation only. They do not establish authoritative C05 speed, collision, duration, prediction or server movement acceptance. Bootstrap preferences currently expose theme/power/motion/sound, not movement constants, so this adapter does not invent preference fields.

## Authorization and lifecycle bridge

`createWorldAccessBridge({ auth, replication, release })` accepts the existing structural read/subscribe/switchRoom port and one captured WorldReleaseRequest. It returns `{ world, controller, ready, withdraw, dispose }`. `world` is FE18's WorldAccessPort; `controller` is the existing createWorldAccessController instance. `ready` resolves true only after canonical local bootstrap validation and current-lease checks. Invalid binding resolves false and withdraws; malformed synchronous inputs may throw for the caller's safe unavailable presentation.

The bridge has no runtime, transport, ticket, reconnect or socket owner. It never disposes the injected replication. It exposes only the validated bootstrap's immutable capabilities and original PrivacyScope. A structural port's null, foreign actor/event/generation or foreign baseline is projected as null. Current authorized matching data can return only while that same lease survives. Reads and dispatches recheck authority after external port reads; late boolean dispatch completion cannot resurrect a withdrawn lease. A true dispatch result remains dispatch only; the existing FE18/FE16 admission checks are unchanged.

Bootstrap AbortSignal, FE02 policy/account/event/logout cleanup, `withdraw`, and `dispose` synchronously withdraw release, catalog, pending intent and nearby, unsubscribe observers and fence pending validation. Withdrawal is permanent for that bridge. Reauthorization constructs a **new bridge from a newly authorized bootstrap release**; do not restamp the previous roster or recapture current policy around it. Dispose is idempotent and releases only this feature/controller's resources. Partial allocation failure also unregisters its cleanup/subscriptions.

## Parent root composition

Reusable React helpers are exported from the same directory:

- `WorldAccessFeatures`: mount once inside the authorized shell, outside changing route panels. Supply canonical auth, the existing composition replication release, and the captured bootstrap release.
- `WorldRoomsPanel` and `WorldNearbyPanel`: compose the existing `/world/rooms` and `/world/nearby` panels centrally. Outside a valid feature provider they render safe unavailable status.

`WorldAccessFeatures` creates/disposes the bridge in an effect, handles StrictMode allocation/cleanup, and suppresses an old owner immediately when auth/replication/release identity changes. The supplied release object should be stable for the bootstrap lifetime, with its real teardown AbortSignal. The provider's cleanup also withdraws the bridge on bootstrap replacement/unmount. Do not use a short-lived HTTP read's signal as the shell teardown authority. The world lifecycle and resolver still belong to the single existing composition owner; these helpers add no root edits or second game/socket.

Parent wiring needs to construct the resolver with the current origin and reviewed local input configuration, inject it into the existing createWorldComposition, and supply that composition's stable replication port and the actual captured shell bootstrap release to these helpers. Renderer injection must wait for approved FE12/selected-source resources. Nothing here changes invitation, block authorization or capacity enforcement contracts.

## Provenance and checks

All new product code/UI binding is authored for this assignment. Read-only dependencies at the adjusted base: canonical SessionController/FE02, C03 decodeBootstrap/contract utilities, C05 FE18 room decoder, FE15 validatePolicy, existing hash-checked createAssetResources, selected source catalog, FE18 WorldRooms/WorldNearby/controller, and committed public composition types/prepareRelease. No legacy application rendering or art was copied; no dependency added. Source/contract/selected-pack hashes and exact changed-path hashes are sealed with the committed handoff receipt.

Tests only import frozen C03/C05 fixtures, canonical Auth test cookie/capability helpers and existing hash utilities. `__tests__/data.ts` constructs explicitly test-only metadata simulating reviewed provided shapes and small hash-reader bytes. Those are neither reviewed product assets nor a production release registry; they never enter product code and are never rendered as characters/maps. Positive resolver tests also call the actual composition `prepareRelease` to check the shared boundary. A structural replication fixture is appropriate for this authorization bridge; existing FE18 and actual composition regressions run alongside it. No duplicate native suite was needed for these deterministic seams, as assigned.

Pinned toolchain: Node 24.22.2 / pnpm 11.7.0. From assigned frontend:

| Check | Result |
|---|---|
| `pnpm install --offline --frozen-lockfile` | Exit 0; retained lock resolutions. |
| `pnpm exec vitest run src/app/world-entry/__tests__ src/features/world-rooms/__tests__ src/world/composition/__tests__/composition.test.ts --reporter=dot` | Exit 0, **99 tests / 5 files** (49 entry, 25 FE18, 25 composition); final machine-readable `/tmp/mcr-world-entry-final-tests-20261104.json` and `/tmp/mcr-world-entry-tests-final-20261104.log`. |
| `pnpm build` | Strict TypeScript + Vite PASS; `/tmp/mcr-world-entry-build-final-20261104.log`. Existing large-chunk warning remains. Root does not yet import this adapter, so bundling is not proof of production route composition. |
| `pnpm lint` | No diagnostics; `/tmp/mcr-world-entry-lint-final-20261104.log`. |
| Root `python3 scripts/mcr/check-plan.py` | Exit 0, provenance/planning PASS; `/tmp/mcr-world-entry-plan-20261104.log`. |
| Root `node scripts/mcr/check-ui-origin.mjs` | C08 static origin/import gate PASS. |
| Root `python3 scripts/mcr/check-boundaries.py --base 26945ebd26c551f64a3ce19d436de66823c86a5b --lane frontend --allow-doc docs/mcr/frontend/world-entry.md` | Exact lane boundary PASS; assignment/staged paths reviewed separately. |

New tests exercise same-lease/no-refetch/no-store/default/theme behavior; actual prepareRelease compatibility; immutable response/configuration/resource pins; missing/foreign/synthetic/unreviewed/duplicate/external-path resources; changed hashes/cross-binding; invalid intent policy/zoom; abort/logout/policy/identity/canonical token expiry during a noncooperative room read; asset read teardown; FE18 current/foreign/null release, full withdrawal and no stale policy restamp; late dispatch, reentrant revocation, partial allocation/unsubscribe-failure cleanup; provider fallback/DOM eviction/StrictMode/teardown. Existing room and composition tests retain their own deterministic attribution.

Initial checks caught local type annotations and three test setup issues (full canonical target room, cross-realm typed-array comparison and schema rejection occurring while constructing the invalid test input). Those were corrected without weakening product checks; final combined tests/build/lint pass. The patch intentionally does not claim native/device/browser art rendering, live backend admission/privacy, second-account flows, invitation completeness, root wiring, deployment or full FE18/IN01 acceptance. Persistent goal remains ACTIVE through the committed bounded handoff and parent review fixes.
