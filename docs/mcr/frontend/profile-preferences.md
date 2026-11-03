# FE34 profile and preferences

Status: MOCK_READY controller/UI slice; root composition and real HTTP integration NOT_RUN.

Base: `1db1680e6a5041250b79ca0ffb1dfd965a279edc`. Sole feature ownership: `frontend/src/features/me/**`, `frontend/src/features/settings/**` and this document. Newly authored UI; no copied rendering. Contract sources: retained C03 `contracts/core/v1/core.schema.json`, `operations.json`, C06 avatar-config and shared avatar runtime at the base. No contract, lock, root router, backend or source archive changes.

Public entry points: `createMeController(auth)`, `MeHub`, `ProfileForm`, `SettingsPage`. Delivery owns shell routing and lifetime: create one controller from canonical shell Auth, explicitly call `load()` on entry, dispose on identity/event/lease change and final owner teardown. Feature React components only subscribe, so StrictMode does not dispose the shared controller. Reopening requires a new lease-bound controller after privacy invalidation.

GET/PATCH `/api/concert/events/{eventId}/me` use protected/core canonical `auth.request`, no-store/no automatic mutation retry and `privacy.run` on the captured scope. PATCH profile sends only expected_revision/display_name/bio; PATCH preferences sends only expected_revision and the four exact preferences. Avatar PUT is exclusively FE13. Strict own-user/event decoders reject leaked extra fields and mismatched identities. Drafts are in memory only and synchronously cleared by canonical privacy cleanup. Conflicts and unknown mutation outcomes keep drafts and block another save until explicit discard/reload. Successful saves require a strictly newer participant revision.

Preferences default to sound off. Saving does not apply runtime effects or start audio. `RuntimePreferencesPort` is an explicit existing-shell adapter; no supplied adapter means live controls unavailable. FE06 `ThemeController` can be injected separately for actual current-world theme selection; its local device setting and server preferred era are separate boundaries.

`ReviewedThumbnailSource` carries the original authorized bootstrap lease and retained manifest/pin/binding/resources; `AvatarThumbnail` calls the committed `createReviewedAvatarCompositor`, never a fixture or legacy fallback. Missing production reviewed binding is honestly unavailable. Thumbnails, canvases and compositor are released on unmount/privacy change. FE13 route is `/me/avatar`; Delivery must invalidate/reload profile after its verified `onCommitted` callback. No second Phaser/socket/identity.

Ticket section and visibility are specified IA requirements but absent from C03. The form explains unavailability and offers a separate `OptionalProfileFieldsPort` render seam for a future qualified owner implementation; it sends neither field. These requirements remain unaccepted. Social-handle configuration remains FE31; public profile contains no private social projection.

Activity links: `/meet` exposes the existing PARTIES/MY PLAN view, `/gifts/mine` own giving/receiving. `/world` is always reachable independently of optional profile completeness.

Initial validation: pinned Node24.22.2/pnpm11.7.0 offline frozen install, typecheck exit0, six focused controller/transport/privacy tests exit0. Native browser, remaining UI/lifecycle tests and final build/lint/boundary evidence follow in the validation commit. Backend BE07 reports preference persistence typed API only, no live HTTP route; canonical Profile owner and Delivery own display_name/bio HTTP composition. No real server, production artwork, multiuser, device or complete FE34 acceptance is claimed.
