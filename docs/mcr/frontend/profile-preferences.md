# FE34 profile and preferences

Status: MOCK_READY bounded FE34 implementation and native-fixture qualification. Root composition, real HTTP integration and whole-task acceptance remain NOT_RUN.

Base: `1db1680e6a5041250b79ca0ffb1dfd965a279edc`. Sole feature ownership: `frontend/src/features/me/**`, `frontend/src/features/settings/**` and this document. Newly authored UI; no copied rendering. Contract sources: retained C03 `contracts/core/v1/core.schema.json`, `operations.json`, C06 avatar-config and shared avatar runtime at the base. No contract, lock, root router, backend or source archive changes.

Public entry points: `createMeController(auth)`, `MeHub`, `ProfileForm`, `SettingsPage`. Delivery owns shell routing and lifetime: create one controller from canonical shell Auth, explicitly call `load()` on entry, dispose on identity/event/lease change and final owner teardown. Feature React components only subscribe, so StrictMode does not dispose the shared controller. Reopening requires a new lease-bound controller after privacy invalidation.

GET/PATCH `/api/concert/events/{eventId}/me` use protected/core canonical `auth.request`, no-store/no automatic mutation retry and `privacy.run` on the captured scope. PATCH profile sends only expected_revision/display_name/bio; PATCH preferences sends only expected_revision and the four exact preferences. Avatar PUT is exclusively FE13. Strict own-user/event decoders reject leaked extra fields and mismatched identities. Drafts are in memory only and synchronously cleared by canonical privacy cleanup. Conflicts and unknown mutation outcomes keep drafts and block another save until explicit discard/reload. Successful saves require a strictly newer participant revision.

Preferences default to sound off. Saving does not apply runtime effects or start audio. `RuntimePreferencesPort` is an explicit existing-shell adapter; no supplied adapter means live controls unavailable. FE06 `ThemeController` can be injected separately for actual current-world theme selection; its local device setting and server preferred era are separate boundaries.

`ReviewedThumbnailSource` carries the original authorized bootstrap lease and retained manifest/pin/binding/resources; `AvatarThumbnail` calls the committed `createReviewedAvatarCompositor`, never a fixture or legacy fallback. Missing production reviewed binding is honestly unavailable. Thumbnails, canvases and compositor are released on unmount/privacy change. FE13 route is `/me/avatar`; Delivery must call `controller.refreshAfterAvatar(scope, avatar.revision)` from its verified `onCommitted(avatar, scope)` callback (the refresh seam rejects stale privacy leases and non-increasing avatar revisions): clean state reloads, dirty or pending state blocks new writes and requires explicit reload without silently rebasing drafts. No second Phaser/socket/identity.

Ticket section and visibility are specified IA requirements but absent from C03. The form explains unavailability and offers a separate `OptionalProfileFieldsPort` render seam for a future qualified owner implementation; it sends neither field. These requirements remain unaccepted. Social-handle configuration remains FE31; public profile contains no private social projection.

Activity links: `/meet` exposes the existing PARTIES/MY PLAN view, `/gifts/mine` own giving/receiving. `/world` is always reachable independently of optional profile completeness.

Validation uses pinned Node24.22.2/pnpm11.7.0 and retained lockfile resolutions. The first implementation checkpoint is `a90c6c4`; final qualification descends directly from that checkpoint. Backend BE07 reports preference persistence typed API only, no live HTTP route; canonical Profile owner and Delivery own display_name/bio HTTP composition. No real server, production artwork, multiuser, device or complete FE34 acceptance is claimed.


## Integration contract

Delivery mounts `MeHub` at `/me`, `ProfileForm` at `/me/profile` and `SettingsPage` at `/me/settings` using the existing persistent shell. The feature does not mount its own router, query client, Auth identity, game or connection. `MeHub` receives the same canonical `auth` and optional original-lease `thumbnailSource`; `SettingsPage` receives the same `auth`, optional `runtime` and existing FE06 `theme` controller. Do not remount the controller for ordinary panel navigation or silently discard drafts on avatar commits. Load is explicit and its button clearly discards drafts before reloading. Dispose after unmount and replace the controller when canonical privacy scope changes. Form subscriptions support React StrictMode.

The runtime adapter receives a lifetime signal that remains linked after its promise settles. It must start with audio disabled, require the explicit apply click, respect reduced motion/low power, and stop audio/release applied effects on abort. Missing or rejecting adapters never claim live success. Current-world theme selection uses the existing FE06 controller separately from server preferred era. No runtime implementation changes are included.

A server 403 clears displayed own data. A successful GET cannot reduce an already observed participant revision or a verified avatar revision floor. Successful PATCH must return a newer revision; unchanged/stale/invalid replies remain unknown outcomes. Another section's dirty draft survives a successful save. Optional owner UI is isolated through `OptionalProfileFieldsPort`; it owns its own qualified persistence and must honor canonical privacy cleanup. Do not expose private handles in the public profile form.

## Final validation evidence

All commands below exit 0 using the pinned Node/pnpm paths documented by C02. Run frontend commands from `frontend/` and Python checks from repository root.

| Check | Command | Result |
|---|---|---|
| Retained dependency install | `pnpm install --offline --frozen-lockfile` | PASS; no dependency or lock changes |
| Affected tests | `pnpm exec vitest run src/features/me/__tests__ src/features/auth/__tests__/session.test.ts src/shared/query/__tests__` | 84/84 PASS: 21 FE34 tests plus canonical Auth/privacy regression coverage |
| Typecheck and application build | `pnpm build` | PASS; existing application chunk-size warning, feature routes await Delivery composition |
| Static checks | `pnpm lint` | PASS |
| Native browser | `node src/features/me/__tests__/browser-smoke.mjs` | PASS: actual installed native Chrome, 390×844 and 1280×844, reduced-motion, React StrictMode, 20 native protected HTTP fixture requests, zero page errors |
| Planning provenance | `python3 scripts/mcr/check-plan.py` | PASS; docs/source preserved, planning baseline not acceptance |
| Lane boundary | `python3 scripts/mcr/check-boundaries.py --base 1db1680e6a5041250b79ca0ffb1dfd965a279edc --lane frontend --allow-doc docs/mcr/frontend/profile-preferences.md` | PASS; manual exact assignment and cross-directory imports reviewed |
| Patch formatting | `git diff --check` | PASS |

Browser probes cover route links, empty optional bio/world navigation, profile save/cancel, 409 draft retention/no replay, 503 unknown outcome/no replay/explicit reload, keyboard save, server preference subset, sound off until explicit runtime apply, immediate privacy cleanup including applied adapter signal, account-switch late-response fencing, zero local/session-storage writes, mobile overflow and 44px button targets. Hub/profile/settings screenshots were visually inspected for the mobile layout, restrained symmetric borders and lack of decorative left accents. Production art remains honestly unavailable with no canvas/default image. Lifecycle unit tests additionally cover shared-builder use, stale bootstrap rejection, thumbnail source removal, ignored-abort builder completion, runtime late-success/unmount invalidation, Unicode validation, own-user/event/extra-field rejection, section-specific payloads, avatar notification during saving and stale revisions. Builder mocks qualify lifecycle ownership only; they do not certify production art pixels.

Local review artifacts: `/tmp/mcr-fe34-evidence/result.json` and `hub-390.png`, `profile-390.png`, `settings-390.png` (and desktop equivalents). These are explicitly synthetic fixture evidence, not durable deployment or acceptance receipts.

Pinned input SHA-256:

- `contracts/core/v1/core.schema.json`: `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88`
- `contracts/core/v1/operations.json`: `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62`
- `contracts/assets/v1/avatar-config.schema.json`: `37b9be22cff7cbde882ab144aec3b75b5edc24f70653f97c9dcec14e87c652bd`

Unqualified gates: live authenticated GET/PATCH and canonical Profile composition; root route mounting; real runtime preference application; supplied-art reviewed binding/atlas; optional ticket-section/visibility contract and persistence; real devices and multiuser flows. None is silently removed from FE34 or IN05 acceptance. This handoff delivers the authorized bounded implementation with explicit ports and preserves those remaining integration/acceptance obligations.

## 2026-11-06 producer-backed closure (task 43)

**CLOSED for the current revised scope.** A real participant producer served
`GET /me` at revision 1, accepted `PATCH /me` with `expected_revision: 1` and
`preferences.low_power: true` at revision 2, and returned `409 / VERSION_CONFLICT`
for the repeated stale revision. The actual `createMeController`, `SettingsPage`
and `ProfileForm` rendered the saved state and the conflict state at 360/390/430,
keeping the draft and offering explicit reload. No automatic replay or silent
rebase occurred. Evidence: `.omo/evidence/mcr-finish-project/task-43-fe-closure.md`
and `frontend/e2e/fe-closure/`. This is development-stack evidence, not release,
device or QA08 qualification; runtime sound/motion/theme application remains a
separate effect boundary.
