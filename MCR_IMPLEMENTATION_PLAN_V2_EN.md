# MCR Concert World — Implementation Plan v2

**Language:** English throughout. **Planning date:** 2026-11-03, Asia/Seoul. **Delivery repository:** `julymeltdown/mcr`. **Delivery branch:** `master`.

This document supersedes the previous MCR IA and development plan where they conflict. It is an implementation specification, not a statement that the application has already been implemented, tested, or deployed. All implementation tasks start as `NOT_STARTED` until supported by execution evidence.

> Repository adaptation (2026-11-03): the user selected `julymeltdown/mcr` on `master` as the delivery destination. Bigyeting and port-rust remain read-only source repositories. The unmodified upstream specification is preserved in `docs/source/MCR_IMPLEMENTATION_PLAN_V2_EN.md`; this adaptation changes delivery location only, not product scope or task acceptance.

## 1. Binding decisions

1. Build one mobile-first application whose authenticated home is a persistent, multiplayer 2D world. Do not build five different applications from the five design concepts.
2. Reuse the **non-visual frontend logic and tooling from bigyeting**. All pages, components that render UI, layouts, styles, design tokens, illustrations, navigation, and visual snapshots must be newly authored for MCR. Reskinning the old UI is not acceptable.
3. Use **port-rust as the backend foundation**, imported into this repository. Extract the relevant introduction, age-policy, access-request, and privacy logic from bigyeting; do not run two competing identity systems.
4. The existing bigyeting directory arrangement, application screens, route compatibility, and product architecture are not constraints. Replace them where the MCR design requires it. Preserve Git history and source provenance; do not interpret this permission as authorization to force-push, destroy live databases, or overwrite unrelated concurrent changes.
5. All resulting frontend, backend, contracts, assets, tests, and documentation must be committed and delivered to **`julymeltdown/mcr` on `master`**. Do not deliver backend work only to port-rust or leave completed work exclusively on another branch.
6. Freeze HTTP, realtime, and asset interfaces before feature implementation. Frontend tasks run against mocks and must not wait for the corresponding backend implementation.
7. P0 includes the real multiplayer world, recognizable avatars, three themes, entry parties, real-world meeting support, giveaways, timeline, introductions, consent-based social-handle sharing, a basic collaborative mural, and operational safety. A fake single-player scene is not an acceptable substitute.

The documented event is My Chemical Romance Live in Korea, **2026-11-07 at 19:00 KST, Paradise City Culture Park**, according to Ticketlink's postponement notice. Recheck the latest operational information before publishing venue guidance. The approved artwork establishes visual direction only; it does not verify real gates, fences, stages, or walking routes. [S07]

## 2. Source audit and reuse boundary

### 2.1 Inspected baseline

The inspected bigyeting `master` revision was `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`. Record the actual imported port-rust revision during C01/BE01; the preceding plan inspected `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`. Source inspections are not proof of passing builds, tests, or real-provider authentication.

| Source | Observed implementation | Decision |
|---|---|---|
| `frontend/package.json` | React, Vite, TypeScript, React Router, TanStack Query, Zustand, ky, Zod, React Hook Form, MSW, Vitest, Playwright, and Oxlint are declared. | Retain the selected stack and the existing lockfile resolution; no unrelated package-upgrade campaign. [S01] |
| `frontend/src/lib/api/real/runtime-client.ts` | ky transport, included credentials, CSRF cache hooks, error normalization, cancellation, and presigned upload progress. | Extract headless behavior, then adapt authentication and error contracts explicitly. Do not copy the old endpoint assumptions unchanged. [S02] |
| `frontend/src/lib/api/real/canonical-auth-client.ts` | Email signup stages, login, provider discovery, OAuth redirects, Naver verification, and X-account operations. | Reuse validation and orchestration concepts; rewrite adapters for the selected backend. Multiple X accounts and billing are outside P0. [S03] |
| `frontend/src/lib/query/query-keys.ts` | Numerous caches are scoped to `activeXAccountId`. | Replace account scoping with canonical `user_id + event_id`; preserve invalidation discipline, not X-account requirements. [S04] |
| `frontend/src/lib/api/client.ts` | Central exports include introductions, access requests, reveals, blocks, notifications, deletion, and unrelated commercial features. | Select only required use cases. Do not import this entire barrel into the new application. [S05] |
| `backend/services/bigyeting-api/src/adapters/out/repository/current_age.rs` | Verified-age arithmetic uses the Seoul date, next birthday, expiry, status, and canonical-claim selection. | Port the complete validity semantics and boundary tests, not a naive birth-year subtraction. [S06] |
| port-rust Gateway auth routes | Route module exposes paths such as `/auth/email/signup`, `/auth/email/verify`, `/auth/login`, and password-reset operations. | Map the new browser client to the actual exported Gateway contract. These differ from bigyeting's `/v1/auth/email/...` calls. [S10] |

### 2.2 What can and cannot be reused

**Allowed:** pure functions, validation schemas, non-rendering hooks, request orchestration, cancellation, privacy-aware cache invalidation, upload/compression logic, state machines, test fixtures, and non-visual test assertions. Any copied module needs its source path, source revision, dependency audit, destination path, and regression test recorded.

**Not allowed:** old JSX/TSX that renders screens or controls, old pages and widgets, old CSS, theme objects, layout components, animation choreography, brand assets, onboarding presentation, and old visual snapshots. A mixed module must be split: extract logic into a headless module and write new rendering code. Renaming or recoloring an existing component does not qualify as a new UI.

Use new semantic HTML and newly authored CSS/Tailwind components. Do not carry Mantine components into the MCR rendering tree. Do not preserve Three.js, GSAP, FFmpeg, or old 3D/video features merely because they appear in the current manifest. Dependency removal happens after an import audit; source code must not silently import removed dependencies.

## 3. Predefined technology stack

### 3.1 Frontend: fixed choices

Versions below are **the inspected manifest declarations**, not a claim that their exact resolved versions have been independently installed or tested. Preserve the existing pnpm lockfile's selected resolutions for retained packages. C02 records the actual resolution and runtime version in a reproducibility manifest.

| Concern | Selected technology | Inspected declaration / policy |
|---|---|---|
| Application | React + React DOM | `^19.2.7` |
| Language | TypeScript, strict mode | `~6.0.2` |
| Build | Vite + React plugin | Vite `^8.1.1`, plugin `^6.0.3` |
| Package manager | pnpm | `11.7.0` |
| Routing | React Router | `^7.18.1`; nested authenticated shell |
| Server state | TanStack Query | `^5.101.2` |
| Local UI state | Zustand | `^5.0.14`; no server-truth duplication |
| HTTP | ky | `^2.0.2`; adapted shared transport |
| Validation | Zod | `^4.4.3` |
| Forms | React Hook Form + Zod resolver | `^7.80.0`, resolver `^5.4.0` |
| Styling | Tailwind CSS + new CSS variables | `^4.3.2`; every visual token is new |
| 2D world | Phaser | **Add exact `3.90.0`**; deliberately fixed, not claimed to be latest. [S08] |
| World transport | Native browser WebSocket | Versioned JSON protocol; no Socket.IO or Colyseus |
| HTTP mocks | MSW | `^2.14.6`; shared contract fixtures. [S09] |
| World mocks | `MockWorldTransport` | Same interface as `WebSocketWorldTransport` |
| Unit/component testing | Vitest + Testing Library | `^4.1.9`, React Testing Library `^16.3.2` |
| Browser testing | Playwright | `^1.61.1`; mock, real-server, and mobile suites |
| Lint | Oxlint | `^1.71.0` |
| Image handling | browser-image-compression | `^2.0.2`; image-only P0 pipeline |
| Asset workflow | Tiled JSON + PNG sprite atlases | Versioned manifests; nearest-neighbor rendering |

The initial CI runtime is **Node.js 24 LTS** as a project selection. C02 must pin an exact available patch and container digest, verify package engines and lockfile installation, and record them. This is a runtime qualification task, not permission to switch the selected frameworks.

New UI motion should use CSS transitions. Avoid adding a second animation framework. Use an original pixel-icon set rather than importing the old branded icon treatment. All Phaser dependencies are dynamically loaded for authenticated world use; public legal and authentication pages do not need the world bundle.

### 3.2 Backend: fixed choices

Import the port-rust Rust workspace and its lockfile into `backend/`. Retain its Axum/Tokio HTTP runtime, Tonic/Prost internal contracts, SQLx/PostgreSQL persistence, Redis integration, and established authentication foundation. Add **one `concert-service`**, not one service per concert feature. Keep the inherited Auth, Profile, Friend, Post, Notification, and Gateway ownership boundaries unless a documented implementation ADR deliberately changes them.

The public browser uses same-origin HTTP and WebSocket endpoints. The edge routes `/api/*` to the Gateway and `/world-ws` to the Concert realtime listener. The listener is not a public unauthenticated alternate API: it requires a one-use ticket issued through authenticated HTTP. Internal service traffic remains authenticated; production secrets must not be replaced with demo credentials.

The new concert domain owns its own database. It must not directly join or mutate another service's tables. Existing Bigyeting data and port-rust data are not automatically interchangeable. P0 creates an isolated MCR data environment; any live-data migration requires a separate reviewed mapping and rollback plan.

## 4. Target repository structure and cutover

```text
mcr/
  frontend/
    src/
      app/                  # new router, providers, shell composition
      shared/
        api/                # extracted/adapted headless transport
        query/              # identity-neutral query keys
        validation/         # selected pure validation
        media/              # image-only processing
        ui/                 # entirely new MCR primitives
        styles/             # entirely new tokens and CSS
      entities/             # user/avatar/party/giveaway/etc. models
      features/             # new domain UI + headless controllers
      world/                # Phaser runtime, scenes, network, rendering
    public/world/           # maps, sprites, themes, manifests
    e2e/                    # rewritten selectors and new visual baselines
    package.json
    pnpm-lock.yaml
  backend/
    crates/                 # imported foundations + concert-domain/application
    services/
      api-gateway/
      auth-service/
      profile-service/
      friend-service/
      post-service/
      notification-service/
      concert-service/
    proto/                  # actual imported contract location may be preserved
    docs/api/               # inherited HTTP contract generation inputs
    scripts/
    Cargo.toml
    Cargo.lock
  contracts/
    realtime/
    assets/
    fixtures/
    generated/              # generated outputs, not independently edited truth
  scripts/mcr/
  infra/mcr/
  docs/mcr/
  MCR_IMPLEMENTATION_PLAN_V2_EN.md
```

The tree is the **target**, not a description of today's repository. Paths in task cards refer to this target. Preserve the actual imported proto layout when it is cheaper than relocating it; C01 records that mapping. Do not leave both the old and new frontend entrypoints active. Do not leave two executable backends competing for the same host port or authentication namespace.

Before replacing files, record the baseline commit and the source-reuse inventory. The original implementation remains retrievable from Git history. Remove obsolete imports, scripts, and build entrypoints in explicit cutover commits; do not keep dead UI under a renamed active directory. Existing production databases and volumes are outside this code-layout rewrite.

## 5. Product, interaction, and art specification

### 5.1 World-first experience

The default post-login destination is `/world`. A first-time user receives a valid preset avatar and can enter immediately; avatar customization is optional, not a mandatory gate. Necessary account eligibility and consent checks remain enforced before participation.

The persistent shell contains a compact header, `REVENGE / PARADE / DANGER`, the world viewport, a contextual panel outlet, and `WORLD / FEED / MEET / GIFTS / WALL / ME`. Opening a panel must not create a second Phaser game or a second socket. Direct links initialize the shell and then open the requested panel. Invalid or unauthorized targets return safely to the world without exposing private metadata.

Mobile panels support peek, half, and full states, with one primary panel at a time. Browser back closes detail before list and list before the world. Preserve camera, virtual position, room, list scroll, and safe drafts. Opening a blocking panel pauses movement input. Backgrounding must not promise persistent networking: reconnect and resynchronize on return.

Design at 390 by 844 CSS pixels and test at widths 360, 390, and 430. The default world should occupy roughly 70 percent of usable height as a design target. Primary touch targets are at least 44 by 44 CSS pixels. Six bottom tabs remain labeled and usable at 360 pixels. Respect safe areas, keyboard appearance, reduced motion, and readable contrast. Provide DOM alternatives for nearby people and places; essential tasks must not require canvas-only interaction.

### 5.2 Three themes, one world

`REVENGE`, `PARADE`, and `DANGER` are skin choices over identical geometry, collisions, place identifiers, room membership, and interactions. Theme changes do not teleport the user, recolor another person's outfit, or create a different multiplayer population.

The approved 2008-inspired visual direction uses compact navigation, text-forward community surfaces, strong borders, original stickers and pixel art, and restrained decoration. It is not a generic modern card dashboard. Main navigation, headings, and buttons are English. User-authored content is not restricted to English. Any necessary local-language legal or safety explanation is supplemental, not the primary interface.

### 5.3 Avatar contract

Use original emo pixel artwork with a large head, short limbs, approximately two-head-tall proportions, and large readable eyes. The proportion reference does not authorize copying another show's characters or drawing style.

Initial asset target: six presets; six hairstyles; four eye options; six outfits; four accessories; selectable skin tones. Use 64 by 64 sprite cells, foot anchor `(32,60)`, four directions, one idle frame and four walk frames per direction. AR03 validates the actual atlas before freezing it. The default on-screen avatar should read at approximately 56–72 CSS pixels tall. Use the same composition in the world, profile, feed, giveaway, and party surfaces.

Store `AvatarConfig` item IDs and manifest version, not arbitrary client-provided asset URLs. Validate compatibility on the server. Assets must align at the same anchor and remain distinguishable against all three backgrounds.

### 5.4 Virtual location is not physical location

`WorldPosition` is an in-game coordinate. `MeetBeacon` is a deliberately shared real-world meeting description. Never infer the latter from the former.

A physical beacon contains a verified place ID, description, optional authorized photo, audience, last confirmation, expiry, and revision. Initial TTL is 15 minutes, configurable by the operator. Moving the beacon creates a revision and invalidates the earlier guidance. Show stale and expired states explicitly. Do not fabricate GPS distance, walking duration, or official route safety. No continuous GPS collection is included in P0.

Use `YOU IN WORLD` and `ONLINE IN THIS ROOM` for virtual presence. Real arrival and pickup are explicit actions. Entry parties coordinate meeting and queuing; they do not change ticket validity, standing order, seat assignments, or venue entry rules.

## 6. Complete information architecture

Each row is a screen, panel, modal, or reusable global state, not necessarily a separate application page. All rows are P0 except M10, R02, and X02. Every screen inherits loading, empty, failure, access-denied, and retry handling where applicable. Specific states below are additional acceptance requirements.

### 6.1 Public and authentication

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| A01 | `/` | ENTER THE WORLD | New landing; signed-in redirect; no old homepage. | FE08, FE09 |
| A02 | `/login` | SIGN IN | Email/provider entry; invalid credentials, throttling, provider unavailable. | FE09, BE03 |
| A03 | `/signup` | CREATE ACCOUNT | Eligibility/consent, signup; duplicate and throttled outcomes without unsafe enumeration. | FE10, BE03 |
| A04 | `/verify-email` | VERIFY EMAIL | Verify and resend; expiry and resend cooldown. | FE10, BE03 |
| A05 | `/reset-password` | RESET PASSWORD | Request and complete reset; expired link and neutral account-existence response. | FE11, BE03 |
| A06 | `/auth/callback` | CONNECTING | Callback completion, denial, invalid state, safe return target. | FE11, BE03 |
| A07 | `/legal/:document` | TERMS / PRIVACY / SAFETY | Versioned notices, operator contact, independent fan-service disclosure. | FE08, BE38 |
| A08 | `/account/deleted` | ACCOUNT DELETED | Completion or pending deletion; clear private client state. | FE36, BE42 |

### 6.2 World

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| W01 | `/world` | WORLD | Movement, camera, interaction; loading, live, reconnecting, low power, full room. | FE07, FE12, FE14, FE15, FE16, FE17, BE10, BE11, BE12, BE13, BE14 |
| W02 | `/world/map` | MAP | Overview, camera and place legend; incompatible map version. | FE19, AR02, BE08 |
| W03 | `/world/places/:placeId` | PLACE | Photo and instructions; verified, concept-only, closed, removed. | FE19, BE08 |
| W04 | `/world/rooms` | ROOMS | Capacity, room switch, party invitation; full and denied. | FE18, BE11 |
| W05 | `/world/nearby` | NEARBY IN WORLD | Accessible nearby-user list; block-filtered presence. | FE18, BE13, BE37 |
| W06 | `/world/theme` | CHOOSE AN ERA | Skin selection; loading, fallback, persisted selection. | FE06, BE07, AR04 |
| W07 | `/world/check-in` | REAL-WORLD CHECK-IN | Explicit place/audience/expiry selection; active, expired, revoked. | FE20, BE09 |
| W08 | `/world/help` | HOW TO PLAY | Controls, virtual/physical distinction, meeting safety. | FE15, FE19 |

### 6.3 Timeline

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| F01 | `/feed` | FEED | Latest-first event feed, filters, cursor loading; stale and read-only. | FE27, BE31 |
| F02 | `/feed/new` | NEW POST | Text and image draft; upload failure, retry, published receipt. | FE28, FE04, BE24, BE31 |
| F03 | `/feed/:postId` | THREAD | Post, comments, reaction, report, linked entity; deleted/unavailable. | FE28, BE32 |
| F04 | `/feed/:postId/edit` | EDIT POST | Owner editing and deletion; version conflict. | FE28, BE31 |
| F05 | `/notices/:noticeId` | NOTICE | Source and confirmation time; superseded or withdrawn. | FE27, BE38 |

### 6.4 Parties and people

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| M01 | `/meet` | MEET | PARTIES / PEOPLE / MY PLAN; filters and restricted states. | FE21, FE29 |
| M02 | `/meet/parties/new` | CREATE PARTY | Capacity, ticket section, time, place, requirements; validation. | FE22, BE16 |
| M03 | `/meet/parties/:partyId` | PARTY | Apply/cancel; requested, accepted, full, cancelled, closed. | FE21, BE17 |
| M04 | `/meet/parties/:partyId/manage` | MANAGE PARTY | Review applications, remove member, edit meeting, close/cancel. | FE23, BE18 |
| M05 | `/meet/parties/:partyId/rally` | RENDEZVOUS | Approved-members-only gathering state; moved, expired, cancelled. | FE23, BE19 |
| M06 | `/meet/beacons/:beaconId` | FIND EACH OTHER | Physical meeting description, identification, status; stale/revoked. | FE20, BE09 |
| M07 | `/meet/people` | PEOPLE | Introduction discovery; no results and verification-required. | FE29, BE27 |
| M08 | `/meet/intro/edit` | YOUR INTRO | Intro draft, private eligibility conditions, publish/close. | FE30, BE27 |
| M09 | `/people/:userId` | PROFILE | Large avatar, public profile, request sharing, report/block; private/unavailable. | FE30, BE04, BE30 |
| M10 | `/meet/parties/:partyId/chat` | PARTY CHAT | P1: party-scoped messages; removed-member and reconnect states. | EX01 |

### 6.5 Giveaways

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| G01 | `/gifts` | GIFTS | Discover by time/place; remaining count and no-stock state. | FE24, BE20 |
| G02 | `/gifts/new` | CREATE GIVEAWAY | Item, photo, quantity, pickup window, place; invalid/upload-failed. | FE25, BE20, BE24 |
| G03 | `/gifts/:giveawayId` | GIVEAWAY | Owner and public details; open, paused, sold-out, completed, cancelled. | FE24, BE20 |
| G04 | `/gifts/:giveawayId/manage` | MANAGE GIVEAWAY | Owner reservations and stock, pause, confirm pickup; version conflict. | FE25, BE20, BE23 |
| G05 | `/gifts/:giveawayId/reservation` | RESERVE | Atomic reservation confirmation; held, exhausted, duplicate, expired. | FE26, BE21 |
| G06 | `/gifts/pickups/:reservationId` | MY PICKUP | Authorized place, time, confirmation code, status; claimed/expired/cancelled. | FE26, BE22, BE23 |
| G07 | `/gifts/mine` | MY GIFTS | GIVING / RECEIVING and history. | FE24, BE20, BE21 |

### 6.6 Mural

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| R01 | `/wall` | WALL | Pan/zoom, pixel preview, palette, explicit commit; cooldown/paused/frozen. | FE32, FE33, BE34, BE35 |
| R02 | `/wall/history` | WALL HISTORY | P1: snapshots and time-lapse. | EX02 |

### 6.7 Personal area

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| U01 | `/me` | ME | Avatar, profile completion, party and giveaway activity. | FE34, BE06 |
| U02 | `/me/avatar` | DRESS UP | Layer selections, presets, preview and save; invalid/version-conflict. | FE12, FE13, BE07 |
| U03 | `/me/profile` | EDIT PROFILE | Nickname, bio, optional ticket section, visibility; validation. | FE34, BE04 |
| U04 | `/me/social` | SOCIAL LINKS | Private-by-default handles; configure, replace, remove. | FE31, BE28 |
| U05 | `/me/requests` | REQUESTS | Sharing requests; pending, approved, rejected, revoked, expired. | FE31, BE29, BE30 |
| U06 | `/me/age` | AGE & PRIVACY | Unknown, verified, expired, unsupported, provider unavailable. | FE30, BE26 |
| U07 | `/me/settings` | SETTINGS | Theme, reduced motion, low power, sound off by default. | FE34, FE06 |
| U08 | `/me/blocks` | BLOCKED USERS | List and unblock; prior sharing grants do not return automatically. | FE36, BE37 |
| U09 | `/me/account` | ACCOUNT | Logout, deletion request and status, failure recovery. | FE36, BE42 |
| U10 | `/notifications` | INBOX | Read state and authorized deep links; stale target. | FE35, BE33 |

### 6.8 Archive, common states, and administration

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| X01 | `/archive` | AFTER THE SHOW | Read-only permitted posts and final wall; no private locations or handles. | FE40, BE41 |
| X02 | `/archive/souvenir` | OUR PARADE | P1: opt-in avatar souvenir export. | EX02 |
| S01 | `/report?targetType=:type&targetId=:id` | REPORT | Reason, evidence, receipt; reporter identity hidden from target. | FE36, BE36 |
| S02 | `route-error-boundary` | UNAVAILABLE | Safe 401/403/404/410/429/503 states; return to world. | FE07, FE41 |
| S03 | `connection-and-maintenance-overlay` | RECONNECTING / MAINTENANCE | Offline/stale/version mismatch; no silent offline reservation replay. | FE17, FE41 |
| O01 | `/admin` | CONTROL ROOM | Operational summary and kill switches; degraded state. | FE38, BE38 |
| O02 | `/admin/event` | EVENT SETTINGS | Event phase, feature flags, notices; publish confirmation. | FE38, BE38 |
| O03 | `/admin/places` | VENUE & PLACES | Source review, verification, closure, accessibility notes, map version. | FE38, BE08, AR01 |
| O04 | `/admin/reports` | REPORT QUEUE | Review, action, status; moderator access only. | FE37, BE36, BE39 |
| O05 | `/admin/users/:userId` | MEMBER REVIEW | Minimal necessary data, suspension, connection termination. | FE37, BE39 |
| O06 | `/admin/content` | CONTENT REVIEW | Hide/restore posts, parties, giveaways, and comments. | FE37, BE39 |
| O07 | `/admin/mural` | WALL CONTROL | Pause, region lock, restore, freeze; revision reset. | FE39, BE40 |
| O08 | `/admin/audit` | AUDIT LOG | Search actions and outcomes; redact sensitive contents. | FE38, BE39 |
| O09 | `/admin/archive` | CLOSE THE EVENT | Freeze, export, expire grants, cleanup; partial-failure recovery. | FE39, BE41 |

## 7. Contracts that unblock parallel development

### 7.1 Source of truth

Preserve the imported port-rust OpenAPI generation pipeline under its mapped backend paths. Add the concert operations to its inputs and produce one browser-facing bundle. Generate TypeScript types/clients from that bundle. Runtime Zod schemas must be generated or checked against it; an independently maintained type definition is not another source of truth.

Realtime messages use JSON Schema under `contracts/realtime/`. Map and avatar manifests use schemas under `contracts/assets/`. `contracts/fixtures/` contains deterministic normal, empty, forbidden, conflict, expired, throttled, unavailable, and reconnect examples. Mocks and server contract tests consume the same fixtures.

Every contract change includes a version/diff, examples, producer and consumer tests, and updated generated artifacts. One contract owner serializes edits to generated inputs and shared lockfiles.

### 7.2 Browser authentication and identity

Choose the imported port-rust access-token/refresh foundation: short-lived access token held **in memory**, refresh credential in a Secure HttpOnly cookie, and an explicit refresh/bootstrap adapter. Never persist bearer tokens in localStorage. Apply the backend's origin/CSRF protections to cookie-backed mutation and refresh paths. Do not retain the bigyeting assumption that every authenticated request is authorized solely by its old cookie session.

Extract the useful transport/cancellation/error/CSRF machinery from bigyeting, but change its endpoint map and token injection in FE01/FE09 and BE03. Refresh is single-flight; concurrent failures cannot create refresh storms. Failed or revoked authentication clears private queries, sockets, drafts containing sensitive data, and account context. Do not blindly retry non-idempotent mutations after refreshing.

One canonical UUID `user_id` comes from Auth. A concert participant is unique on `(event_id, user_id)`. X or Instagram is an optional social handle, not the identity required to use the world. Use `(user_id, event_id)` in private query keys. A late response from the previous identity must be discarded after logout or account change.

### 7.3 HTTP interface register

The following are **target interfaces**, not assertions that these endpoints already exist. Prefix `/api` is the proposed same-origin edge prefix. C03 maps inherited auth/profile/post operations to their verified generated contract; no obsolete `/v1` compatibility layer is required.

Let `E = /api/concert/events/{eventId}`.

| Interface group | Target operations | Owning data / guarantees |
|---|---|---|
| Auth | `/api/auth/email/signup`, `/api/auth/email/verify`, `/api/auth/login`, reset/refresh/logout/provider operations from the mapped Gateway contract | Auth; browser adapter and credential rules tested explicitly |
| Bootstrap | `GET E/bootstrap` | Event phase, own participant, safe profile, avatar, capabilities, manifests, clock |
| Participant | `GET/PATCH E/me`; `GET E/people/{userId}` | Public/private DTO separation; no social secret in public profile |
| Avatar | `PUT E/me/avatar` | Validated asset IDs and optimistic version |
| Places | `GET E/places`; `GET E/places/{placeId}` | Verification and closure status; no invented route precision |
| Beacons | `POST E/beacons`; `GET/PATCH/DELETE E/beacons/{id}` | Audience, TTL, revision, explicit revocation |
| World | `GET E/rooms`; `POST E/world-tickets` | Admission and short-lived, one-use credential |
| Parties | `GET/POST E/parties`; `GET/PATCH E/parties/{id}`; `POST E/parties/{id}/cancel` | Owner policy, lifecycle, capacity invariant |
| Applications | `POST E/parties/{id}/applications`; `POST E/party-applications/{id}/decision`; `DELETE E/party-applications/{id}` | Idempotency, uniqueness, atomic approval |
| Membership/rally | `DELETE E/parties/{id}/members/{userId}`; `GET E/parties/{id}/rally`; `PUT E/parties/{id}/rally/me` | Approved members only; explicit physical arrival |
| Giveaways | `GET/POST E/giveaways`; `GET/PATCH E/giveaways/{id}` | Stock, pickup window, pause/cancel; public DTO excludes recipients |
| Reservations | `POST E/giveaways/{id}/reservations`; `GET/DELETE E/reservations/{id}`; `POST E/reservations/{id}/claim` | Atomic stock, expiry, authorization, one-time pickup |
| Introductions | `GET E/intros`; `GET/PUT E/me/intro`; `POST E/me/intro/publish`; `POST E/me/intro/close` | Server eligibility, owner-only policy details |
| Age | `GET E/me/age-status`; verification start/callback mapped to the authenticated canonical identity | Unknown is not verified; missing provider disables only gated features |
| Social | `PUT/DELETE E/me/social-handles/{platform}`; `POST E/social-requests`; decision/revoke/reveal operations on `E/social-requests/{id}` | Explicit mutual consent, no-store reveal, block and expiry recheck |
| Feed | Event-scoped adapter over inherited Post CRUD/comments/reactions; `GET E/feed` | Chronological cursor feed; no private activity auto-publication |
| Media | `POST E/media/uploads`; `POST E/media/{id}/finalize`; authorized media reads | Validate bytes and ownership, strip image metadata |
| Wall | `GET E/mural/snapshot`; `POST E/mural/pixels` | Server time, quota/cooldown, revision, durable writes |
| Notices/inbox | Event notices plus inherited Notification inbox/read operations | Durable notifications, reauthorize the target on navigation |
| Safety | Report operations, inherited block operations with Concert policy adapters | Same policy for HTTP, WS, caches, and projections |
| Administration | `E/admin/...` for settings, places, reports, content, wall, audit, closure | Server role checks and append-only action receipts |

All proposed mutation methods and response schemas are frozen in C03/C04 before implementation, including exact inherited operation IDs. The register is not a replacement for the OpenAPI schemas that those tasks produce.

### 7.4 DTO, error, and retry rules

Use UUIDs for new identities; RFC3339 timestamps in UTC; KST for display; integer revisions; cursor pagination ordered by `(created_at, id)`; explicit nullable fields; and maximum text/image/page sizes.

`Bootstrap` includes `event`, `participant`, `avatar`, `capabilities`, `map_manifest`, `avatar_manifest`, `server_time`, and `contract_version`. `PublicProfile` excludes exact verified age, provider claims, email, social handles, and private meeting state. `MeetBeacon` never uses player coordinates as physical evidence.

Keep the inherited Gateway error envelope and add machine-readable reasons in `details.reason`. Required reasons include `PARTY_FULL`, `GIVEAWAY_SOLD_OUT`, `RESERVATION_EXPIRED`, `VERSION_CONFLICT`, `CONSENT_REQUIRED`, `RESOURCE_UNAVAILABLE`, `MURAL_COOLDOWN`, `MAP_VERSION_MISMATCH`, and `EVENT_READ_ONLY`. Avoid detailed age-policy failure reasons in public responses. UI behavior must not depend on comparing an English error sentence.

Require an idempotency key for reservation, approval, pickup, social decision, and other duplicate-sensitive commands. Store actor, operation, request-body hash, result, and expiry. Reusing the same key with a different payload returns a conflict. After an uncertain timeout, retrieve operation/resource status or retry the identical idempotent request; never assume the action failed.

### 7.5 Realtime protocol

```text
HTTP: POST world-tickets -> ticket bound to user/session/event/room, 30-second expiry
WS: connect same-origin /world-ws -> authenticate first message within 3 seconds
Server: consume ticket once -> authorize room -> snapshot
Client: input(seq, axisX, axisY, buttons) -> server validates and simulates
Server: delta(streamId, streamSeq, roomEpoch, serverTick, changes)
Gap/epoch change: request resync -> fresh snapshot, not blind replay
```

Never place bearer credentials in the WebSocket URL. Unauthenticated sockets have a strict timeout and cannot subscribe or receive world state. Validate Origin, message size, message rate, schema, and ticket reuse. Browser clients send input intent, not authoritative coordinates. Server owns speed, walkability, interaction distance, and membership.

Use an initial 20 Hz simulation and at most 10 Hz state updates as **tuning targets**, not measured capacity. Coalesce motion under backpressure; bound outgoing queues. Critical domain transitions remain durable HTTP/database operations and can be recovered through reads. Do not write every movement tick to PostgreSQL.

`streamSeq` is **per delivered subscription stream**. A room-wide sequence filtered differently for each viewer would create false gaps; use a separate room revision/epoch for global resets. Snapshot and following deltas must share a consistent baseline. Blocked users are removed from both directions of presence and interaction. Revocation, removal, suspension, and session expiry remove subscriptions promptly and fail closed when authorization cannot be established.

Use one room-owning realtime process for P0, with an initial configurable cap of 50 users per room. Test 200 total connections across four rooms before choosing the actual cap. Do not add replicas that independently own the same room. Horizontal room ownership is a later explicit design task.

## 8. Data ownership and state invariants

### 8.1 Minimum persistent model

| Owner | Tables or aggregates | Important keys/invariants |
|---|---|---|
| Auth/Profile/Friend/Post/Notification | Imported identity, profile, relationship, content, notification data | Retain owning-service boundaries and explicit adapters |
| Concert participant | `events`, `participants`, `avatar_configs`, `event_settings` | Unique event/user; avatar whitelist/version |
| Venue | `places`, `map_releases`, `beacons`, `beacon_audiences` | Verified source, closure, TTL, owner and authorized audience |
| Party | `parties`, `party_applications`, `party_members`, `rally_states` | Unique membership; approved count never exceeds capacity |
| Giveaway | `giveaways`, `reservations`, `pickup_receipts` | Nonnegative stock; no duplicate active reservation; single terminal pickup |
| Social | `intros`, `age_claims`, `social_handles`, `social_requests`, `social_grants` | Claims bound to canonical user; encrypted handles; per-platform consent |
| Mural | `mural_state`, `mural_operations`, `mural_snapshots`, `mural_limits` | Server cooldown; ordered durable revision; immutable recovery snapshot |
| Operations | `reports`, `moderation_actions`, `audit_entries`, `command_receipts`, `outbox`, `cleanup_jobs` | Least-privilege reads, deduplication, resumable work |

Do not expose all tables directly as CRUD. Application use cases enforce actor permissions and transitions. Each migration includes constraints/indexes, test setup, and a forward-recovery strategy. New database names and volumes prevent accidental writes to an old application environment.

### 8.2 Required state machines

| Aggregate | States / rule |
|---|---|
| Event | DRAFT -> OPEN -> LIVE -> READ_ONLY -> ARCHIVED; operator-driven, auditable |
| Party | OPEN -> CLOSED or CANCELLED -> COMPLETED; terminal rules explicit |
| Application | PENDING -> APPROVED or REJECTED or CANCELLED; removed membership is separate |
| Rally | ON_MY_WAY -> HERE -> MET, plus NEED_HELP and LEFT; self-reported, not GPS-verified |
| Giveaway | DRAFT -> OPEN -> PAUSED or CLOSED or CANCELLED; sold-out can be a derived stock state |
| Reservation | HELD -> READY -> CLAIMED, or HELD/READY -> CANCELLED/EXPIRED; CLAIMED is terminal |
| Beacon | ACTIVE -> EXPIRED or REVOKED; a location change increments revision |
| Social request | PENDING -> APPROVED/REJECTED/CANCELLED; approval creates explicit grants |
| Social grant | ACTIVE -> REVOKED/EXPIRED; block and handle replacement invalidate access |
| Mural | EDITABLE -> PAUSED -> EDITABLE, or FROZEN; restore creates a new revision, never rewinds sequence numbers |

For each giveaway, `available = quantity_total - reserved_unclaimed - quantity_claimed`. Reservation and release update the reservation plus counters in one transaction. Lock the giveaway row or use a guarded atomic update; do not use an in-memory counter as truth. Cancellation and expiry race through conditional state transitions so stock is released once. Quantity reduction cannot go below already committed units.

Reservation expiry follows the pickup window and configured grace period, not an arbitrary ten-minute lifetime starting days before the show. Claim requires the authenticated authorized actor and a reservation-bound confirmation mechanism; storing only a hashed short-lived code and throttling guesses is preferable to exposing ticket QR images. Duplicate confirmation returns the same receipt without consuming more stock.

Party approval similarly locks/guards capacity. Treat duplicate requests and competing approvals deterministically. Owner departure requires cancellation or explicit ownership transfer; it cannot leave an ownerless active party. Membership removal immediately revokes party-specific beacons and subscriptions.

### 8.3 Privacy and age policy

Retain half-open age conditions: `minimum_age <= age < maximum_age_exclusive`, with nullable bounds where supported. The inspected policy supports claims within 14–100 and uses expiring verification; this is an implementation boundary, **not** a statement of concert admission rules or a legal signup-age determination. Keep service eligibility, event admission, self-declared age, and verified policy claims separate.

A claim includes its verifier, canonical user binding, verification time, validity deadline, next-birthday semantics, status, and provenance. Unknown, expired, unsupported, or unavailable verification fails closed for restricted discovery and sharing. It must not block unrelated basic world use when service eligibility is otherwise satisfied. Port the original boundary fixtures, including birthdays at Seoul midnight and canonical-claim replacement.

A private Instagram handle is never sent in public profiles, lists, initial HTML, analytics, notifications, or unauthorized WebSocket payloads. The requester explicitly consents to sharing their own selected handle with the recipient; recipient approval authorizes the reciprocal exchange. The reveal endpoint rechecks both grants, platform, expiry, block, suspension, and membership conditions. Returning an account name is not automatic following on the external platform.

Revocation removes future access and invalidates application caches. It cannot erase an account name another person has already seen or copied; state this accurately. Handle replacement does not silently expose the new handle under an old grant. Retention and event-end deletion are configurable, reviewed, and auditable; keep precise locations and private handles out of public archives.

### 8.4 Timeline, mural, and operational rules

Feed entries may reference public party and giveaway announcements. Never automatically publish recipient identities, private applications, exact meeting updates, or social approvals. Reauthorize linked entities at read time; cached projections are not authority.

The basic mural is 128 by 128 pixels with 16 colors, one explicit pixel commit at a time, and a server-enforced configurable cooldown. Preview is not a committed write. Author/moderation metadata remains private. Snapshots plus ordered operations support recovery; a restore creates a new authoritative revision and causes clients to resynchronize.

Outbox records are committed with domain changes. Delivery consumers deduplicate by event ID. Durable notifications do not depend solely on Redis Pub/Sub. A content or relationship privacy change invalidates related projections and subscriptions. End-of-event processing freezes writes, snapshots permitted content, expires meeting/sharing access, and runs resumable cleanup. Account deletion also revokes tickets, sockets, grants, and private caches.

## 9. Phases and execution model

Phases organize outcomes; **they are not global sequential barriers**. A task can start when its listed dependencies and contracts are available. Frontend dependencies intentionally do not point to matching backend feature tasks.

| Phase | Outcome | Primary lanes |
|---|---|---|
| P0 | Repository/source mapping, fixed stack, contracts, fixtures, art specifications | LEAD, ART |
| P1 | Login -> persistent world -> two users moving together | BE-CORE, BE-CONCERT, FE-APP, FE-WORLD |
| P2 | Entry parties, actual meetings, giveaways, pickup correctness | Backend and frontend domain lanes |
| P3 | Feed, introductions, private social sharing, collaborative mural | Backend and frontend domain lanes |
| P4 | Moderation, lifecycle, real-device testing, recovery, release qualification | All lanes, QA |
| P5 | Optional enhancements after the required gates | Available implementation lanes |

Roles describe ownership, not a required team size. One person may own both backend lanes. Do not give multiple workers concurrent ownership of the same lockfile, contract root, router composition, or manifest registry.

### Task-card conventions

All rows below are implementation tasks. Each row specifies the dependency graph, the proposed owned files, concrete behavior, and acceptance evidence. `-` means no task dependency. Paths are relative to the target repository. Abbreviations: `CD=backend/crates/concert-domain/src`, `CA=backend/crates/concert-application/src`, `CS=backend/services/concert-service/src`, `GW=backend/services/api-gateway/src`, `FS=frontend/src`, `CT=contracts`, `TEST=backend/services/concert-service/tests`.

A row's acceptance column describes **tests to implement and run**, not tests already passed. Every code task also requires formatting, type/build checks for its scope, no unexpected mock use, a task-linked commit, and a verified remote commit on master. Unit-test success alone does not grant integration acceptance.

## 10. Foundation and asset tasks

| Task | Phase | Lane | Dependencies | Title | Owned paths | Implementation | Acceptance evidence |
|---|---|---|---|---|---|---|---|
| C01 | P0 | LEAD | - | Source and rewrite inventory | `docs/mcr/source-map.md` | Record source commits, copied logic, old entrypoints to retire, actual proto paths, and data-environment separation. | Every reuse entry names source/destination/test; no production-data deletion in cutover plan. |
| C02 | P0 | LEAD | C01 | Reproducible workspace and toolchain | `frontend/package.json`; `frontend/pnpm-lock.yaml`; `infra/mcr/`; `scripts/mcr/check.sh` | Freeze retained package resolutions, add Phaser 3.90.0, pin Node patch/image, map backend lockfile, and create scoped checks. | Clean frozen-lockfile install and minimal frontend/backend build; record versions and commands. |
| C03 | P0 | LEAD | C01 | Core HTTP and identity contract | `backend/docs/api/`; `CT/fixtures/core/` | Map actual auth operation IDs and tokens; define Bootstrap, public/private profile, avatar, places, errors, and capabilities. | Schema-valid success/failure examples; exact frontend/backend auth mapping reviewed. |
| C04 | P0 | LEAD | C03 | Domain HTTP and transition contracts | `backend/docs/api/`; `CT/fixtures/domain/` | Define party, reservation, social, age, feed, mural, admin commands; actor permissions, idempotency, and versions. | Every command has actor, precondition, resulting state, error reason, and retry rule. |
| C05 | P0 | LEAD | C03 | Realtime wire protocol | `CT/realtime/`; `CT/fixtures/realtime/` | Define authenticate, snapshot, input, delta, resync, revoke, room switch, and stream sequence rules. | Reject unknown/oversized messages; filtered streams have no false sequence gaps. |
| C06 | P0 | LEAD | C01 | Map and avatar interfaces | `CT/assets/`; `docs/mcr/assets.md` | Define atlas frames/anchor, layer IDs, map version, collisions, interactions, and place-source metadata. | Small golden asset fixtures validate and align across all themes. |
| C07 | P0 | LEAD | C03, C04, C05, C06 | Generated clients and fixture harness | `scripts/mcr/contracts/`; `CT/generated/`; `CT/fixtures/` | Generate TS API types, validate Zod compatibility, deterministic clock/seed, mock scenarios, and drift checks. | Generated output is reproducible; a deliberate schema mismatch fails CI. |
| C08 | P0 | LEAD | C01 | New-UI and source-boundary gate | `scripts/mcr/check-ui-origin.mjs`; `docs/mcr/reuse-allowlist.json` | Inventory legacy rendering files and hashes; allow only reviewed headless extracts; prohibit legacy styles and visual imports. | Gate detects an imported or copied legacy control; manual visual review remains required. |
| AR01 | P0 | ART | C06 | Venue source and landmark pack | `frontend/public/world/places/`; `docs/mcr/venue-provenance.md` | Verify relative buildings/paths from authorized references; distinguish temporary unverified gates and public meeting places. | Every physical place has source, review state, photo permission, and closure handling. |
| AR02 | P1 | ART | AR01, C06 | Walkable venue tilemap | `frontend/public/world/maps/` | Build terrain, collision, overdraw, interaction, spawn, and landmark layers from the reviewed source pack. | All spawn points are walkable; interaction IDs resolve; blocked routes cannot be traversed. |
| AR03 | P1 | ART | C06 | Original avatar atlases | `frontend/public/world/avatars/` | Deliver six presets and agreed layered assets using the fixed anchor and direction/frame layout. | No clipping, missing frames, or theme-dependent identity changes at actual mobile scale. |
| AR04 | P1 | ART | C06 | Three world skin packs | `frontend/public/world/themes/` | Produce Revenge/Parade/Danger decorations and palettes without changing geometry or collision IDs. | Pixel readability and contrast pass for all skins; manifest hashes and licenses recorded. |

## 11. Backend implementation tasks

| Task | Phase | Lane | Dependencies | Title | Owned paths | Implementation | Acceptance evidence |
|---|---|---|---|---|---|---|---|
| BE01 | P1 | BE-CORE | C01, C02 | Import port-rust foundations | `backend/Cargo.toml`; `backend/Cargo.lock`; `backend/crates/`; `backend/services/` | Import the selected workspace with provenance; retire old competing executable entrypoints; preserve required service boundaries. | Workspace resolves and builds; one documented process/port map; no accidental old DB target. |
| BE02 | P1 | BE-CORE | BE01 | Relocate build and migration tooling | `backend/scripts/`; `infra/mcr/backend/` | Repair relative paths, protobuf generation, SQLx migrations, health checks, image contexts, and isolated DB credentials. | Fresh isolated database migrates; services restart and persist a smoke record. |
| BE03 | P1 | BE-CORE | BE02, C03 | Browser auth contract adapter | `GW/adapters/in/http/`; `backend/services/auth-service/` | Wire signup/verify/login/reset/refresh/logout/provider discovery to the mapped contract; enforce safe redirects and cookie protections. | Real isolated signup/login/refresh/logout; expired/reused credentials and invalid OAuth state rejected. |
| BE04 | P1 | BE-CORE | BE03, C03 | Canonical participant identity adapter | `CA/identity/`; `CS/adapters/out/identity/` | Resolve Auth UUID, basic Profile projection, suspension state, and explicit provider-to-user binding; remove X-account prerequisite. | Email-only user participates; foreign identity cannot attach claims; private profile fields never leak. |
| BE05 | P1 | BE-CONCERT | BE01, C03 | Concert service and core schema | `backend/crates/concert-domain/`; `backend/crates/concert-application/`; `backend/services/concert-service/` | Add hexagonal modules, typed errors, migrations for events/participants/avatar settings, clock and policy ports. | Migration uniqueness checks and dependency-boundary tests; service health and isolated startup. |
| BE06 | P1 | BE-CONCERT | BE04, BE05 | Bootstrap and participant defaults | `CA/bootstrap/`; `CS/adapters/in/grpc/bootstrap.rs`; `TEST/bootstrap.rs` | Implement idempotent participant creation, preset avatar, event capabilities, manifests, and server clock aggregation. | Repeated bootstrap creates one participant; degraded optional dependencies do not fabricate data. |
| BE07 | P1 | BE-CONCERT | BE05, C06 | Avatar and preference persistence | `CD/avatar/`; `CA/avatar/`; `CS/adapters/out/repository/avatar.rs` | Validate asset IDs/layers/version; update avatar and theme with optimistic concurrency and bounded payloads. | Invalid URL/item rejected; stale version conflicts; other users' outfits remain unchanged. |
| BE08 | P1 | BE-CONCERT | BE05, C06 | Places and map-release reads | `CD/place/`; `CA/place/`; `CS/adapters/out/repository/place.rs` | Store reviewed landmarks and immutable map manifests; expose closure and verification; separate concept-only objects. | Unknown/closed place cannot be selected for a new physical beacon; manifest mismatch has a typed error. |
| BE09 | P2 | BE-CONCERT | BE08, BE37, C04 | Physical beacon lifecycle | `CD/beacon/`; `CA/beacon/`; `CS/adapters/out/repository/beacon.rs` | Create/read/update/revoke scoped beacons; DB-time TTL; revision changes; audience authorization through policy ports. | Expired, revoked, blocked, and unrelated viewers receive no private meeting details. |
| BE10 | P1 | BE-CORE | BE04, BE05, C05 | One-use world admission tickets | `CA/world_ticket/`; `GW/adapters/in/http/concert_world.rs` | Issue user/session/event/room-bound 30-second tickets; atomic consumption; no credential in URLs/logs. | Reuse, wrong room, expired ticket, wrong origin, and unauthenticated frames rejected. |
| BE11 | P1 | BE-CONCERT | BE10 | Room registry and membership leases | `CS/world/rooms.rs`; `CS/world/admission.rs` | One owner process per room; bounded capacity, session membership, join/switch/leave, and disconnect grace lease. | Concurrent joins never exceed cap; switching cannot duplicate membership; ghost leases expire. |
| BE12 | P1 | BE-CONCERT | BE11, C06 | Authoritative movement loop | `CS/world/simulation.rs`; `CD/world/` | Fixed-step movement from normalized input; speed limits, collision grid, valid spawn, and interaction range. | Teleport, excessive speed, invalid axis, sequence replay, and wall penetration tests. |
| BE13 | P1 | BE-CONCERT | BE12, BE37 | Recipient-specific snapshots and deltas | `CS/world/replication.rs`; `CS/world/interest.rs` | Project authorized players, stable snapshot baseline, per-stream sequence, avatar versions, bounded/coalesced queues. | Two viewers with different block filters resynchronize correctly; slow receiver cannot grow memory unboundedly. |
| BE14 | P1 | BE-CONCERT | BE13 | Reconnect and authorization revocation | `CS/world/session.rs`; `CS/world/recovery.rs` | Handle epoch changes, expired session, suspension, close codes, resync, and app foregrounding without motion replay. | New snapshot after server restart; removed/expired session cannot recover its old subscriptions. |
| BE15 | P2 | BE-CONCERT | BE05 | Domain outbox and command receipts | `CA/commands/`; `CS/adapters/out/repository/outbox.rs`; `CS/workers/outbox.rs` | Transactional event enqueue; idempotency-body hashes; bounded retries and delivery deduplication. | Crash after commit does not lose event; duplicate event consumed once; changed payload/key conflicts. |
| BE16 | P2 | BE-CONCERT | BE05, BE37, C04 | Party aggregate and CRUD | `CD/party/`; `CA/party/create.rs`; `CA/party/update.rs` | Event/ticket section, owner, capacity, rendezvous, lifecycle and version constraints; public projection excludes applicants. | Non-owner edits fail; invalid capacity/window rejected; cancelled party is not joinable. |
| BE17 | P2 | BE-CONCERT | BE16, BE15 | Applications and atomic approval | `CA/party/apply.rs`; `CA/party/decide.rs`; `TEST/party_capacity.rs` | Unique application, owner decision, transaction-guarded member count, idempotent response and outbox notification. | Two approvals for last slot produce one membership; duplicate apply does not create extra rows. |
| BE18 | P2 | BE-CONCERT | BE17 | Leave, remove, and cancel party | `CA/party/membership.rs`; `TEST/party_removal.rs` | Enforce actor permissions, leader cancellation/transfer rule, capacity release and immediate audience revocation. | Removed member loses rally/beacon access; owner cannot orphan an active party. |
| BE19 | P2 | BE-CONCERT | BE18, BE09 | Rally and real-arrival commands | `CD/rally/`; `CA/rally/`; `TEST/rally.rs` | Member-only state and meeting revision; explicit ON_MY_WAY/HERE/MET/NEED_HELP; no virtual-position inference. | Nonmember rejected; venue change invalidates stale rally instructions; self-state actor checked. |
| BE20 | P2 | BE-CONCERT | BE05, BE37, C04 | Giveaway aggregate and catalog | `CD/giveaway/`; `CA/giveaway/`; `TEST/giveaway_catalog.rs` | Owner CRUD, public filters, quantity and pickup-window validation, pause/cancel and optimistic stock edits. | Quantity cannot drop below committed units; recipients absent from public lists and details. |
| BE21 | P2 | BE-CONCERT | BE20, BE15 | Atomic reservation command | `CA/reservation/reserve.rs`; `CS/adapters/out/repository/reservation.rs` | Guarded stock decrement/hold, unique active user reservation, request receipt, and pickup-window expiry. | One unit with five simultaneous buyers yields exactly one hold; retry returns original receipt. |
| BE22 | P2 | BE-CONCERT | BE21 | Cancellation and expiry worker | `CA/reservation/release.rs`; `CS/workers/reservation_expiry.rs` | Conditional terminal transition and stock release in one transaction; DB clock, bounded batches, restart-safe scans. | Cancellation/expiry/claim races do not double-release stock; restart resumes pending expiry. |
| BE23 | P2 | BE-CONCERT | BE22 | Pickup confirmation and receipts | `CA/reservation/claim.rs`; `TEST/pickup.rs` | Bind confirmation to reservation/authorized actors; hashed expiring code, guess limits, terminal claim receipt. | Duplicate claim consumes no extra stock; wrong owner/code/expired hold is rejected. |
| BE24 | P2 | BE-CORE | BE05, BE37, C04 | Authorized image upload pipeline | `CA/media/`; `CS/adapters/out/media/`; `TEST/media.rs` | Initialize/finalize upload; byte/MIME/dimension limits, decode/reencode, metadata removal, ownership and private read control. | Forged MIME, oversized image, foreign upload ID, and unauthorized private media read fail. |
| BE25 | P3 | BE-CORE | C01, C04 | Extract introduction and age policy | `CD/intro_policy/`; `CD/age_policy/` | Port pure rule semantics and fixtures; replace source identity types and database-bound helpers with explicit ports. | Half-open range, null bounds, unknown claims, supported limits, and no public policy leakage. |
| BE26 | P3 | BE-CORE | BE25, BE04, BE05 | Verified-age claim persistence | `CA/age_verification/`; `CS/adapters/out/age_provider/`; `TEST/age_claims.rs` | Port claim expiry, next-birthday and canonical replacement; verified callback-to-user binding; provider availability capability. | Seoul birthday boundary, expired/replaced claims, callback replay, and missing provider fail closed. |
| BE27 | P3 | BE-CONCERT | BE26, BE37 | Introduction authoring and discovery | `CA/intro/`; `CS/adapters/out/repository/intro.rs` | Owner draft/publish/close, eligibility-aware discovery/detail, approved public profile projection, pagination. | Direct ID and list paths enforce the same eligibility; unknown verification cannot bypass a restriction. |
| BE28 | P3 | BE-CONCERT | BE05, BE37, C04 | Private social-handle storage | `CD/social_handle/`; `CA/social_handle/`; `CS/adapters/out/crypto/` | Normalize platform handle, encrypt at rest with managed secret, version changes, removal, and no plaintext logging. | Public DTO serialization rejects handles; replacement invalidates old-version grant bindings. |
| BE29 | P3 | BE-CONCERT | BE28, BE15 | Mutual social-sharing decisions | `CA/social_request/`; `TEST/social_consent.rs` | Requester's explicit consent, recipient approval/rejection, per-platform reciprocal grants, expiry and revocation. | No grant before both relevant consents; duplicate decision is idempotent; blocked pair rejected. |
| BE30 | P3 | BE-CONCERT | BE29 | Authorized reveal endpoint | `CA/social_reveal/`; `GW/adapters/in/http/social_reveal.rs` | Recheck grant versions, platform, expiry, block, suspension and actor; no-store response and privacy invalidation event. | Approved, unrelated, expired, revoked, changed-handle and blocked scenarios; no leaked error metadata. |
| BE31 | P3 | BE-CORE | BE04, BE15, BE37, C04 | Event feed and post adapter | `CA/feed/`; `GW/adapters/in/http/concert_feed.rs`; `backend/services/post-service/` | Add event scoping, latest cursor feed, public party/giveaway references and ordinary post CRUD through the owner service. | Cross-event/private references excluded; stable cursor order; linked entity is reauthorized. |
| BE32 | P3 | BE-CORE | BE31 | Comments, reactions, and content controls | `GW/adapters/in/http/concert_comments.rs`; `backend/services/post-service/` | Map comments/reactions, owner edit/delete, visibility filtering and moderation receipts to inherited contracts. | Blocked reply paths fail; deleted/hidden post cannot be recovered through a comment route. |
| BE33 | P3 | BE-CORE | BE15, BE04 | Concert notification adapter | `CS/adapters/out/notification/`; `backend/services/notification-service/` | Deduplicate domain event delivery, safe summary text, own inbox/read state and authorized target links. | No handle/location secret in payload; repeated delivery creates one notification; stale target handled. |
| BE34 | P3 | BE-CONCERT | BE05, BE37, C04 | Mural writes and cooldown | `CD/mural/`; `CA/mural/place_pixel.rs`; `TEST/mural_concurrency.rs` | Grid/palette bounds, server-time per-user cooldown, atomic operation/revision/outbox update, write freeze check. | Parallel requests cannot bypass cooldown; invalid coordinates/colors rejected; DB failure produces no false commit. |
| BE35 | P3 | BE-CONCERT | BE34, BE15 | Mural snapshot and realtime changes | `CA/mural/snapshot.rs`; `CS/world/mural_subscription.rs` | Produce consistent snapshot revision and ordered deltas; resume or full-resync on missed retention window. | New subscriber and reconnecting subscriber converge to the same pixels and revision. |
| BE36 | P4 | BE-CORE | BE05, BE37, C04 | Reports and evidence access | `CA/report/`; `CS/adapters/out/repository/report.rs` | Typed targets, reason limits, duplicate/spam handling, reporter privacy and role-scoped evidence retrieval. | Target never sees reporter identity; ordinary member cannot inspect moderator evidence. |
| BE37 | P1 | BE-CORE | BE04, BE05, C04 | Shared relationship policy port | `CA/ports/relationship_policy.rs`; `CS/adapters/out/friend_policy.rs` | Adapt inherited blocks/suspensions to canonical users; common authorization and invalidation hook for HTTP/WS/projections. | Bidirectional block and unblocking semantics; unavailable policy cannot reveal private data. |
| BE38 | P4 | BE-CORE | BE08, BE05, C04 | Event settings, notices, and flags | `CA/admin/event.rs`; `CA/admin/places.rs`; `CA/admin/notices.rs` | Operator roles, reviewed publication, source timestamps, place closure, feature availability and read-only gates. | Client-hidden feature still rejected server-side; nonoperator writes forbidden. |
| BE39 | P4 | BE-CORE | BE36, BE38 | Moderation commands and audit trail | `CA/admin/moderation.rs`; `CS/adapters/out/repository/audit.rs` | Suspend/hide/restore with reason and immutable receipt; invoke owner services and terminate affected sessions. | Action/result tied to actor; failed downstream action is not recorded as successfully applied. |
| BE40 | P4 | BE-CONCERT | BE35, BE38 | Mural pause, lock, and restore | `CA/admin/mural.rs`; `TEST/mural_restore.rs` | Region locks, global pause/freeze, restore from authorized snapshot into a new revision; force client resync. | Writes during freeze rejected; restore never reuses old sequence numbers; audit receipt exists. |
| BE41 | P4 | BE-CONCERT | BE15, BE30, BE35, BE38 | Event closure and archive jobs | `CA/event_lifecycle/`; `CS/workers/cleanup.rs` | Idempotent read-only transition, permitted archive projection, grant/beacon expiry and resumable retention jobs. | Partial failure resumes; public archive excludes private locations, recipients, and handles. |
| BE42 | P4 | BE-CORE | BE14, BE30, BE41 | Account-deletion integration | `CA/account_deletion/`; `backend/services/auth-service/` | Extend deletion orchestration to Concert; revoke sessions/tickets/grants, scrub retained personal fields, retry durable steps. | Repeated deletion safe; removed user cannot reconnect or reveal; failed step remains visible for retry. |
| BE43 | P4 | BE-CORE | BE03, BE10, BE24, BE30 | Boundary and abuse hardening | `GW/`; `CS/adapters/in/`; `infra/mcr/security/` | Scope-specific rate limits, content bounds, header/origin rules, redacted logs, no-store private responses, dependency audit. | Negative tests for auth/CSRF/origin/replay/upload/oversized WS; no secrets in captured logs. |
| BE44 | P4 | BE-CORE | BE02, BE38, BE41, BE43 | Reproducible real-stack startup | `infra/mcr/compose.yml`; `scripts/mcr/backend-smoke.sh` | Wire all inherited services and Concert, persistent volumes, real/mock provider separation, health and shutdown ordering. | Fresh startup and restart smoke; disabled provider is honestly unavailable, not simulated success. |

## 12. Frontend implementation tasks

All rendering in these tasks is new. Retain only explicitly reviewed headless source logic. FE tasks use the contract fixtures; corresponding backend implementation is deliberately not a prerequisite.

| Task | Phase | Lane | Dependencies | Title | Owned paths | Implementation | Acceptance evidence |
|---|---|---|---|---|---|---|---|
| FE01 | P1 | FE-APP | C02, C03 | Extract and adapt HTTP transport | `FS/shared/api/http.ts`; `FS/shared/api/errors.ts` | Port cancellation, error normalization and safe upload helpers; replace endpoint/auth assumptions with typed client ports. | Mock 401/409/429/network abort; no old `/v1` calls or token persistence. |
| FE02 | P1 | FE-APP | FE01, C03 | Identity-neutral queries and invalidation | `FS/shared/query/keys.ts`; `FS/shared/query/privacy.ts` | Replace activeXAccountId with user/event scope; cancellation and private-cache eviction on auth/policy changes. | Account A's delayed response cannot populate account B; no retained reveal after revoke. |
| FE03 | P1 | FE-APP | C04, C08 | Extract pure form and policy helpers | `FS/shared/validation/`; `FS/entities/intro/model/` | Port required Zod/pure logic only; separate mixed TSX logic from rendering; adapt canonical IDs and error enums. | Existing semantic fixtures pass; no import from old pages/components/styles. |
| FE04 | P2 | FE-APP | FE01, C04 | Headless image upload controller | `FS/shared/media/`; `FS/features/media/model/` | Extract image compression, preview cleanup, progress, abort, finalize and retry state; exclude video/FFmpeg. | Abort frees resources; failed finalize is not shown as uploaded; no credential on presigned PUT. |
| FE05 | P1 | FE-APP | C02, C08 | New accessible UI primitives | `FS/shared/ui/`; `FS/shared/styles/base.css` | Author buttons, fields, dialogs, sheet, tabs, status, toast, skeleton and error panels from new markup/styles. | Keyboard/focus/dialog dismissal tests; 44px targets; no legacy visual imports. |
| FE06 | P1 | FE-APP | FE05, C06 | Three-theme token system | `FS/shared/styles/themes/`; `FS/features/theme/` | New Revenge/Parade/Danger tokens, selection persistence, asset-loading fallback, reduced-motion transitions. | Theme switch preserves geometry, room and avatar identity; failed asset load restores previous skin. |
| FE07 | P1 | FE-APP | FE05, C03 | Persistent world shell and routes | `FS/app/router.tsx`; `FS/app/WorldShell.tsx` | Build header, six tabs, nested panels, history/back semantics, safe return paths, lazy public/admin route boundaries. | Panel navigation creates no duplicate world/socket; direct gift link initializes shell correctly. |
| FE08 | P1 | FE-APP | FE05, FE07 | New public landing and legal views | `FS/features/public/` | New world-first landing, entry CTA, versioned legal/safety/contact pages; signed-in redirect. | Anonymous pages load without Phaser; signed-in home resolves to world; no old homepage markup. |
| FE09 | P1 | FE-APP | FE01, FE02, FE07, C03 | Session controller and sign-in | `FS/features/auth/model/`; `FS/features/auth/ui/SignIn.tsx` | In-memory token, single-flight refresh, safe bootstrap, login/logout, provider availability, complete private-state cleanup. | Concurrent 401s perform one refresh; failed login and unavailable provider have truthful states. |
| FE10 | P1 | FE-APP | FE09 | Signup and verification views | `FS/features/auth/ui/SignUp.tsx`; `FS/features/auth/ui/VerifyEmail.tsx` | New forms using mapped server flow, validation, consent, challenge expiry and server resend countdown. | Expired verification cannot complete; double submit does not create extra requests. |
| FE11 | P1 | FE-APP | FE09 | Reset-password and callback views | `FS/features/auth/ui/ResetPassword.tsx`; `FS/features/auth/model/callback.ts` | New request/confirm reset views and OAuth callback error/return-target handling; sanitize local redirects. | Unknown email uses neutral response; hostile return URL rejected; denial returns safely. |
| FE12 | P1 | FE-WORLD | C06, C02 | Shared avatar compositor | `FS/world/avatar/`; `FS/entities/avatar/` | Deterministic layer composition, frame/anchor mapping, texture cache, and shared profile-thumbnail export. | Golden-config render aligns across world/profile; missing asset uses documented fallback. |
| FE13 | P1 | FE-WORLD | FE12, FE05, C03 | New avatar editor | `FS/features/avatar/` | New large preview, presets and item selectors; draft/save/cancel; optimistic-version conflict without losing draft. | Save changes world and profile consistently; cancelling retains persisted outfit. |
| FE14 | P1 | FE-WORLD | C06, C02 | Phaser runtime and map loader | `FS/world/runtime.ts`; `FS/world/scenes/`; `FS/world/map/` | Single lifecycle-managed game, manifest version/hash, layers, nearest-neighbor textures and resize/cleanup. | Repeated React mount cleanup leaks no canvas/listeners; invalid map fails visibly. |
| FE15 | P1 | FE-WORLD | FE14 | Mobile movement, camera, and interactions | `FS/world/input/`; `FS/world/camera/`; `FS/world/interaction/` | Pointer-safe joystick/tap movement, keyboard control, collision prediction, recenter, selection and blocking-panel pause. | Pointercancel/blur stops movement; keyboard opening and panel input never move the avatar accidentally. |
| FE16 | P1 | FE-WORLD | C05, FE01 | Realtime transport implementations | `FS/world/network/WorldTransport.ts`; `FS/world/network/mock.ts`; `FS/world/network/websocket.ts` | Same interface for mock and real transport; ticket/authenticate, typed messages, sequence and close handling. | Identical scenario suite runs with both transports; mock code excluded from production mode. |
| FE17 | P1 | FE-WORLD | FE16, FE14, FE12 | Replication and reconnect controller | `FS/world/network/replication.ts`; `FS/world/entities/` | Snapshot baseline, interpolation, bounded prediction/reconciliation, epoch reset and visibility-driven reconnect. | Missing delta requests resync; restart removes ghosts; no replay of old pickup/reservation commands. |
| FE18 | P1 | FE-WORLD | FE17, FE05 | Rooms and accessible nearby list | `FS/features/world-rooms/`; `FS/features/world-nearby/` | New channel chooser, switch feedback, room cap states and DOM list of authorized nearby avatars. | Full room handled without losing current membership; blocked character disappears from both representations. |
| FE19 | P2 | FE-APP | FE07, FE14, C03 | Map, place details, and help | `FS/features/places/`; `FS/features/world-help/` | New overview and place sheet, source/closed states, physical/virtual legend, accessible actions and controls help. | Concept-only place never offered as verified physical destination; map mismatch produces recovery UI. |
| FE20 | P2 | FE-APP | FE19, FE02, C04 | Physical check-in and meeting beacon | `FS/features/beacon/` | New audience/TTL form, last-confirmed time, photo/identification, moved/stale/revoked views and explicit arrival actions. | Virtual avatar motion does not change physical check-in; expired beacon cannot masquerade as current. |
| FE21 | P2 | FE-APP | FE07, FE02, C04 | Party discovery and application | `FS/features/party-discovery/` | New filters/list/detail, apply/cancel mutation state and capability-driven actions, MY PLAN link. | Empty/full/private/cancelled states; uncertain request reconciles by receipt instead of duplicate application. |
| FE22 | P2 | FE-APP | FE05, FE03, C04 | Party authoring form | `FS/features/party-authoring/` | New create/edit form for capacity, ticket section, time, place, and intro; RHF validation and safe draft restore. | Invalid time/capacity rejected locally and server errors mapped; version conflict keeps draft. |
| FE23 | P2 | FE-APP | FE21, FE20 | Party management and rally | `FS/features/party-management/`; `FS/features/rally/` | New application decisions/member list and explicit ON_MY_WAY/HERE/MET/NEED_HELP controls; remove/cancel confirmations. | Last-slot conflict updates UI; removed member loses panel access; moved meeting visibly supersedes old card. |
| FE24 | P2 | FE-APP | FE07, FE02, C04 | Giveaway discovery and own activity | `FS/features/gift-discovery/`; `FS/features/my-gifts/` | New list/detail and GIVING/RECEIVING views, remaining-stock/time filters and current action eligibility. | No recipient identities in public view; paused/sold-out/ended states prevent invalid actions. |
| FE25 | P2 | FE-APP | FE24, FE04, FE03 | Giveaway authoring and owner tools | `FS/features/gift-authoring/`; `FS/features/gift-management/` | New item/photo/quantity/window/place form and owner reservation inventory table; pause/cancel/edit flows. | Stock-edit conflict preserves form; unauthorized owner-tools route is unavailable. |
| FE26 | P2 | FE-APP | FE24, FE20 | Reservation and pickup journey | `FS/features/pickup/` | Idempotency-key lifecycle, hold receipt, expiry, meeting detail, confirmation code, owner claim and cancellation. | Timeout/reload reconciles with server; expired/sold-out/repeated pickup never shows fake success. |
| FE27 | P3 | FE-APP | FE07, FE02, C04 | Event timeline and notices | `FS/features/feed/` | New compact chronological feed, filters, cursor loading, linked party/gift references and source-stamped notices. | Stable ordering; revoked linked target safely disappears; no private activity projection. |
| FE28 | P3 | FE-APP | FE27, FE04 | Thread, composer, and comments | `FS/features/thread/`; `FS/features/post-authoring/` | New post/comment/reaction views, edit/delete, image upload, draft handling and report entry. | Deleted/hidden/conflicting content handled; double submission and failed upload recover cleanly. |
| FE29 | P3 | FE-APP | FE07, FE03, C04 | People and introduction discovery | `FS/features/people/` | New people list and filters, avatar-focused items, opaque eligibility/unavailable states and profile entry. | User without X account can browse allowed content; exact age and secret handles absent. |
| FE30 | P3 | FE-APP | FE29, FE12 | Introduction editor, profile, age status | `FS/features/intro/`; `FS/features/profile/`; `FS/features/age-status/` | New intro/profile rendering, private condition form and verified/unknown/expired/provider-unavailable flow. | Half-open boundary fixtures; policy detail visible only to owner; no self-declared verified badge. |
| FE31 | P3 | FE-APP | FE02, FE05, C04 | Private social-sharing UI | `FS/features/social-sharing/` | New handle settings, explicit request consent, decisions, reveal, revoke and expiry views; no-store sensitive fetch. | Network payload before approval contains no handle; revocation clears UI/cache; no auto-follow claim. |
| FE32 | P3 | FE-WORLD | FE05, C04 | Mural editing surface | `FS/features/mural/editor/` | New grid/palette, pan/zoom and pixel preview; confirm action; touch disambiguation and DOM controls. | Dragging does not place pixels; bounds/color/cooldown states correct on 360px screen. |
| FE33 | P3 | FE-WORLD | FE32, FE16 | Mural synchronization and world object | `FS/features/mural/model/`; `FS/world/interaction/mural.ts` | Snapshot/delta reconciliation, commit receipts, pause/freeze/restore reset and world-wall texture update. | Two clients converge; rejected local preview rolls back; restore discards obsolete deltas. |
| FE34 | P3 | FE-APP | FE07, FE12, FE02 | Personal profile and preferences | `FS/features/me/`; `FS/features/settings/` | New ME hub, profile form and low-power/motion/sound/theme preferences; links to party and gift activity. | Optional profile fields do not block world entry; private settings remain identity-scoped. |
| FE35 | P3 | FE-APP | FE02, FE07, C04 | Notification inbox | `FS/features/inbox/` | New unread count/list, mark-read/all-read and safe deep-link routing with stale-target handling. | Duplicate notification event does not duplicate UI; inaccessible target never exposes cached details. |
| FE36 | P4 | FE-APP | FE02, FE05, C04 | Report, block, and deletion controls | `FS/features/safety/`; `FS/features/account/` | New report dialog, block list/unblock, logout/delete confirmation and progress views; clear private state. | Blocking closes affected panels; unblock does not restore old consent; failed deletion is not displayed as complete. |
| FE37 | P4 | FE-APP | FE07, FE05, C04 | Moderator review screens | `FS/features/admin/reports/`; `FS/features/admin/members/`; `FS/features/admin/content/` | New plain tables/forms for report evidence, suspension, hide/restore and action reasons; server-role-aware entry. | Nonmoderator direct route blocked; failed action remains pending/failed rather than successful. |
| FE38 | P4 | FE-APP | FE07, FE05, C04 | Event operations screens | `FS/features/admin/event/`; `FS/features/admin/places/`; `FS/features/admin/audit/` | New settings, place review/closure, notice publication, capability flags and audit views. | Confirm risky changes; stale versions conflict; source and review state remain visible. |
| FE39 | P4 | FE-APP | FE38, FE33 | Wall control and event closure UI | `FS/features/admin/mural/`; `FS/features/admin/closure/` | New pause/region lock/restore controls and closure step-progress/retry presentation. | Restore confirmation names snapshot; partial closure failure is visible and recoverable. |
| FE40 | P4 | FE-APP | FE27, FE33, C04 | Read-only archive views | `FS/features/archive/` | New end-of-event page and permitted final mural/feed views; remove all write controls and private activity. | Archive does not contain handles or physical beacons; deleted content stays inaccessible. |
| FE41 | P4 | FE-APP | FE07, FE17 | Global degraded and lifecycle states | `FS/app/errors/`; `FS/features/connection-status/` | New offline/reconnect/maintenance/version/404 states; stale-data labels, foreground resync and render throttling. | No endless spinner; offline pickup is not queued silently; keyboard and reconnect preserve usable navigation. |

## 13. Verification and integration tasks

| Task | Phase | Lane | Dependencies | Title | Owned paths | Implementation | Acceptance evidence |
|---|---|---|---|---|---|---|---|
| QA01 | P4 | QA | C07 | Contract and fixture coverage | `scripts/mcr/contracts/`; `docs/mcr/evidence/contracts/` | Validate every fixture and generated response shape; run drift and missing-operation probes. | Broken schema and unknown enum cause failure; no empty-test success. |
| QA02 | P4 | QA | C08, FE05, FE07 | Zero legacy-UI audit | `docs/mcr/evidence/ui-origin/` | Static import/hash check plus manual comparison of all routes against old rendering and approved MCR direction. | No reused old control/layout/theme; new visual snapshots are reviewed, not blindly accepted. |
| QA03 | P4 | QA | BE17, BE22, BE23, BE34 | Database race suite | `TEST/concurrency/` | Run parallel capacity, reservation, cancellation, expiry, claim, idempotency and mural-cooldown scenarios against PostgreSQL. | Invariants hold with forced transaction interleavings and worker restarts. |
| QA04 | P4 | QA | BE30, BE37, BE39, FE31, FE36 | Privacy and authorization matrix | `TEST/authorization/`; `frontend/e2e/privacy/` | Test owner/member/stranger/blocked/suspended/expired/admin across list/detail/mutation/WS/cache. | No unauthorized handle, claim, evidence, beacon or recipient data in responses or cached UI. |
| QA05 | P4 | QA | BE14, BE35, FE17, FE33 | Realtime failure suite | `frontend/e2e/realtime/`; `TEST/realtime/` | Packet delay/gap, slow client, background/foreground, ticket replay, server restart and room switch. | Bounded queues, no ghost player or duplicated room membership, eventual state convergence. |
| QA06 | P4 | QA | AR02, AR03, AR04, FE13, FE23, FE26, FE30, FE33 | Mobile visual acceptance | `frontend/e2e/visual-mcr/`; `docs/mcr/evidence/mobile/` | Test widths 360/390/430, actual iOS Safari and Android Chrome, keyboard, safe area and all three skins. | Avatars remain legible, controls reachable, no panel clipping or old UI; record device/browser versions. |
| QA07 | P4 | QA | FE18, FE19, FE32, FE36 | Accessibility and input suite | `frontend/e2e/accessibility/` | Keyboard/focus/screen-reader labels, DOM alternatives, reduced motion and pointer cancellation. | Essential social/pickup tasks work without precise canvas interaction; focus returns on sheet close. |
| QA08 | P4 | QA | BE44, FE41 | Capacity and performance qualification | `scripts/mcr/load/`; `docs/mcr/evidence/performance/` | Initial 200 sockets/four rooms scenario, mobile rendering/bundle/memory measurements, slow-network and reconnect storms. | Publish measured ceilings and chosen caps; no claim of 200-user support without results. |
| QA09 | P4 | QA | BE44, BE42, FE39 | Recovery and lifecycle rehearsal | `scripts/mcr/recovery/`; `docs/mcr/evidence/recovery/` | Database restart/backup restoration, outbox replay, pending pickup, deletion and partial event closure. | Restored state is consistent; no lost committed pickup; shutdown and cleanup are resumable. |
| QA10 | P4 | QA | BE43, QA01, QA02 | Candidate provenance and release checks | `scripts/mcr/release-check.sh`; `docs/mcr/evidence/release/` | Verify same-candidate versions, genuine tests, no production mocks/secrets and task-to-commit traceability. | Missing evidence is NOT_RUN/BLOCKED, not PASS; no stale revision substituted for current candidate. |
| IN01 | P1 | INTEGRATOR | BE06, BE07, BE14, FE09, FE13, FE17, FE18, AR02, AR03, AR04 | Two-user world gate | `frontend/e2e/real/world.spec.ts` | Two independent accounts enter the same live backend world, move, customize, change skin, navigate panels and reconnect. | Both observe authorized movement; no duplicate game/socket; actual art passes visual check. |
| IN02 | P2 | INTEGRATOR | IN01, BE19, BE23, BE24, FE23, FE25, FE26 | Real meetup and pickup gate | `frontend/e2e/real/meetup-pickup.spec.ts` | Party request/approval, physical beacon/rally, giveaway reservation and confirmed pickup on two clients. | Full journey against real services; virtual walking never changes physical arrival. |
| IN03 | P3 | INTEGRATOR | BE27, BE30, BE32, BE33, BE37, FE28, FE30, FE31, FE35, FE36 | Social and privacy gate | `frontend/e2e/real/social.spec.ts` | Intro discovery, feed/comment, consent/reveal/revoke, notifications and block propagation. | Restricted age and revoked/blocked cases fail closed on HTTP, WS and client cache. |
| IN04 | P3 | INTEGRATOR | BE40, FE33, FE39 | Shared mural and moderation gate | `frontend/e2e/real/mural.spec.ts` | Multiple clients paint, hit cooldown, experience pause, restore and final freeze. | All clients converge to authoritative revision; unauthorized moderation fails. |
| IN05 | P4 | INTEGRATOR | IN02, IN03, IN04, FE40, QA03, QA04, QA05, QA06, QA07, QA08, QA09, QA10 | Release decision | `docs/mcr/release-decision.md` | Review same-candidate evidence, scope cuts, operator settings and known limitations; publish pass/block decision. | All required gates executed; master contains tested code and evidence; no false implementation claims. |

## 14. Optional tasks

All tasks in this section are P1 enhancements, not launch prerequisites.

| Task | Phase | Lane | Dependencies | Title | Owned paths | Implementation | Acceptance evidence |
|---|---|---|---|---|---|---|---|
| EX01 | P5 | FULLSTACK | IN05 | Party-only free-text chat | `CA/party_chat/`; `FS/features/party-chat/` | Add authorized history/send/reconnect, spam limits, report support, and member-removal revocation. | Removed or blocked user cannot read/send; persisted ordering survives reconnect. |
| EX02 | P5 | FULLSTACK | IN05 | Mural history and souvenir export | `CA/archive_export/`; `FS/features/souvenir/` | Snapshot history/time-lapse and opt-in avatar keepsake; rights and consent-aware export. | Private handles/locations excluded; opted-out/deleted avatars handled correctly. |
| EX03 | P5 | FE-APP | IN05 | PWA and offline read mode | `frontend/service-worker/`; `FS/features/offline/` | Cache public static assets and permitted read-only material; never cache private reveal responses. | Offline mode labels staleness and cannot silently queue stock or consent mutations. |
| EX04 | P5 | BE-CORE | IN05 | Additional identity providers | `backend/services/auth-service/`; `FS/features/auth/` | Add only configured/verified providers and explicit account-linking conflict handling. | Real provider acceptance and takeover-prevention tests; unavailable provider not advertised as working. |
| EX05 | P5 | ART | IN05 | Additional cosmetics and scenery | `frontend/public/world/` | More outfits, decorations and optional day/night variants on unchanged map geometry. | New assets obey atlas contract and performance budget; identity remains readable. |
| EX06 | P5 | FULLSTACK | IN05 | Advanced discovery and saved items | `CA/discovery/`; `FS/features/discovery/` | Add bookmarks and deeper filtering after event-specific demand is demonstrated. | Visibility changes remove cached/private saved-item previews; no unintended recommendation exposure. |

## 15. Parallel assignment and integration discipline

### 15.1 Initial independent queues

After C03/C04/C05/C06 are frozen, start the lanes independently. C07 supplies reusable fixtures; each feature may use its already frozen subset before the entire fixture catalog is complete. The final contract gate still requires the full catalog.

| Lane | First queue | Next queue |
|---|---|---|
| FE-APP | FE01, FE02, FE03, FE05, FE06, FE07 | Auth/public views, then party/giveaway forms and flows |
| FE-WORLD | FE12, FE14, FE15, FE16 | FE17, FE18, avatar editor, then mural |
| BE-CORE | BE01, BE02, BE03, BE04 | BE37, BE10, age-policy/claim extraction, then privacy/admin adapters |
| BE-CONCERT | BE05, BE06, BE07, BE08 | BE11–BE14 and BE15–BE23 in separate domain modules |
| ART | AR01, AR03, AR04 | AR02 and mobile-scale revisions |
| INTEGRATOR/QA | C07/C08 harnesses and test scaffolding | Integrate completed vertical flows; do not wait for all features |

BE05 depends on the imported workspace, but its domain/schema work can be prepared against C03 before runtime integration. FE21–FE26 do not depend on BE16–BE23. A frontend developer can finish all failure states against deterministic mocks before a backend endpoint is ready.

### 15.2 Shared-file ownership

The contract owner owns OpenAPI roots, proto registries, WS/asset schemas, and generated-client policy. The integration owner serializes `Cargo.toml`, `Cargo.lock`, `package.json`, `pnpm-lock.yaml`, top-level router composition, environment manifests, and CI entrypoints. Feature workers own separate module directories. Use a small registration interface so most work does not edit central files.

Development can use local worktrees/branches for isolation. That does not change the delivery requirement: **the tested result must reach `origin/master`**. One integrator serializes master updates. No force push, no resetting remote history, and no mass `git add .` over unrelated work.

```bash
# In a clean integration worktree; adapt task paths to the actual completed unit.
git fetch origin
git switch master
git pull --ff-only origin master
# Integrate the completed worker commit or apply the reviewed task changes.
# Run the affected checks and integration tests required by the task.
git add -- <explicit-task-paths>
git commit -m "feat(mcr/BE21): add atomic giveaway reservation"
git push origin HEAD:master
git fetch origin
# Verify the delivered commit is contained in origin/master.
```

A non-fast-forward rejection means fetch, inspect, integrate, and rerun relevant checks; it does not justify force. Use the repository's configured author identity. If branch protection prevents direct delivery, report the blocker rather than silently changing protection or claiming a successful push.

### 15.3 Task completion record

Every task completion must record: task ID; source baseline; files changed; implemented behavior; contract version; migrations; commands run with exit codes; test evidence paths; known limitations; local commit SHA; remote commit SHA or verified ancestry; and result status.

States are `NOT_STARTED -> IN_PROGRESS -> MOCK_READY` for frontend or `CONTRACT_READY` for backend, then `INTEGRATED -> ACCEPTED`. Use `BLOCKED` or `NOT_RUN` accurately. `MOCK_READY` is not production completion. `CONTRACT_READY` is not full UI acceptance. Each completed implementation unit must be committed and pushed; only the designated integrator publishes concurrent worker changes to master.

## 16. Release gates, scope reduction, and limits

The critical delivery path is: source/contracts -> identity/bootstrap + persistent world -> IN01 -> atomic party/giveaway/physical meetup -> IN02 -> privacy/mural/operations evidence -> IN05. Social and mural work run in parallel with the meetup track; they are not postponed until it ends.

Reduce asset counts, decorative animations, map extent, optional filters, additional login providers, free-text chat, time-lapse and souvenir export before cutting correctness. Keep the real multiuser world, recognizable avatar, real/virtual location separation, atomic inventory/capacity, server-side social privacy, report/block controls, reconnect recovery and operator kill switches.

The initial 200-connection scenario and 50-person room cap are qualification targets, not demonstrated production capacity. The target frame rate is 30 FPS on selected mid-range mobile devices; capture frame-time, memory, transferred bytes, startup and reconnect behavior rather than asserting universal performance. Keep full-world rendering paused/throttled when obscured and respect reduced-motion settings.

Do not promise that this entire backlog fits before the event merely because tasks are parallelized. Time pressure does not permit fake multiplayer, unguarded reservations, public private-account identifiers, or fabricated venue directions. If a required feature cannot pass its gate, disable it explicitly and document the reduced launch scope; do not present it as complete.

No general DM, voice/video rooms, ticket resale, payments, continuous GPS tracking, automatic external following, level/experience system, or five independent UI implementations are included.

## 17. Developer/agent execution brief

**Frontend brief:** Work only on assigned FE IDs and their new target modules. Read the fixed stack, relevant IA rows, contracts and fixtures. Extract only reviewed headless bigyeting logic. Build all UI from new markup and styles. Use MSW/MockWorldTransport without changing production component logic. Implement loading, empty, denied, conflict, expiry, offline and retry states, not just happy-path screenshots. Run source-origin, unit and route tests. Report MOCK_READY until the real backend journey is executed. Commit the task and deliver through the master integrator.

**Backend brief:** Work only on assigned BE IDs. Import from the recorded port-rust baseline and preserve source attribution. Use canonical Auth identity, service-owned data and explicit privacy ports. Implement domain types, application command/query, repository migration, HTTP/gRPC adapter, event/receipt handling and negative/concurrency tests for the specific unit. Do not redesign unrelated infrastructure. Do not access live databases without a separate migration authorization. Commit and deliver the tested unit to mcr/master through the integrator.

**Integration brief:** Verify the contract hash and source revisions, run the real services, disable mock mode, exercise at least two identities, and inspect both UI and network results. Record failures honestly. Verify that the tested candidate is the same commit delivered to master. Recheck changes introduced during concurrent integration rather than reusing an earlier green report.

## 18. Sources and evidence limits

These sources establish existing implementation or external reference facts. All architecture decisions, new routes, data models, sizing targets and task assignments above are proposals for this MCR implementation.

- **S01:** bigyeting frontend manifest, inspected revision: https://github.com/julymeltdown/bigyeting/blob/5eecdcaffd1a9d505b9ab2a974f8d5623217e90a/frontend/package.json
- **S02:** headless transport implementation: https://github.com/julymeltdown/bigyeting/blob/5eecdcaffd1a9d505b9ab2a974f8d5623217e90a/frontend/src/lib/api/real/runtime-client.ts
- **S03:** authentication client: https://github.com/julymeltdown/bigyeting/blob/5eecdcaffd1a9d505b9ab2a974f8d5623217e90a/frontend/src/lib/api/real/canonical-auth-client.ts
- **S04:** existing query-key scoping: https://github.com/julymeltdown/bigyeting/blob/5eecdcaffd1a9d505b9ab2a974f8d5623217e90a/frontend/src/lib/query/query-keys.ts
- **S05:** existing use-case export inventory: https://github.com/julymeltdown/bigyeting/blob/5eecdcaffd1a9d505b9ab2a974f8d5623217e90a/frontend/src/lib/api/client.ts
- **S06:** verified-age helper and test: https://github.com/julymeltdown/bigyeting/blob/5eecdcaffd1a9d505b9ab2a974f8d5623217e90a/backend/services/bigyeting-api/src/adapters/out/repository/current_age.rs
- **S07:** official revised event schedule: https://www.ticketlink.co.kr/global/en/help/notice/63744
- **S08:** selected Phaser 3.90 release: https://phaser.io/news/2025/05/phaser-v390-released
- **S09:** MSW documentation: https://mswjs.io/docs/
- **S10:** port-rust Gateway auth route implementation, master inspected during this revision: https://github.com/julymeltdown/port-rust/blob/master/boilerplate-grpc-rust/services/api-gateway/src/adapters/in/http/auth_routes.rs

The repository was inspected through the connected GitHub interface. This planning revision does not execute or certify the existing application build, the proposed rewrite, mobile device compatibility, capacity, real-provider login, database migrations, or production deployment. Those obligations are represented by explicit tasks and gates, not implied PASS statuses.
