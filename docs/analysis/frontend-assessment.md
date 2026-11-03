# MCR frontend and world assessment

Assessment date: 2026-11-03. Scope: understand the supplied product, frontend structure, and implementation boundaries; no application implementation. Delivery destination is **`julymeltdown/mcr`, branch `master`**, as explicitly directed in the current assignment. References to `julymeltdown/bigyeting` as the delivery destination in the supplied narrative, ledger, and generated task cards are superseded. Bigyeting remains a proposed source of reviewed headless logic.

Source key: [Plan](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md), [IA ledger](../source/mcr_v2_en/IA_EN.md), [task export](../source/mcr_v2_en/generated/TASKS_EN.json), [developer cards](../source/mcr_v2_en/generated/DEVELOPER_TASKS_EN.md), [package README](../source/mcr_v2_en/MCR_DELIVERY_README_EN.md), and [approved reference image](../source/mcr_v2_en/reference/approved_mobile_world.png). Section numbers and task/IA IDs below refer to these local sources.

## 1. Assessment and evidence boundary

MCR is specified as a mobile fan community organized around a persistent multiplayer 2D concert world. Feed, meeting coordination, giveaways, profiles, and a collaborative mural extend that world through panels and explicit domain workflows. This is substantial frontend work: a new rendering system, shared avatar pipeline, multiplayer client, accessible panel navigation, privacy-aware state handling, and operational screens. The architecture is suitable for independent frontend queues after interface freezing, but the supplied package is a specification rather than an implemented application. [Plan §§1, 5, 9, 12, 15]

The inspected workspace initially contained the Git directory and documentation; no application source tree, frontend manifest, or runnable entrypoint was present in the enumerated files. I read the narrative and relevant ledgers/task cards, inspected the approved image, and parsed the supplied task JSON. That parse found **41 FE tasks: 32 FE-APP and nine FE-WORLD**, all declared `NOT_STARTED`, with no direct FE-to-BE task dependencies. These are properties of the planning artifact, not executed application results. Other analysts' concurrent documentation is outside this assessment.

The supplied `VALIDATION_EN.json` says `PASS` for planning structure only. Neither that result nor the README's exporter-test account proves authentication, rendering, mobile compatibility, networking, performance, or any real user journey. No application build, package installation, browser/device test, multiplayer test, or legacy-source implementation audit was performed for this report. Source revisions, existing-library behavior, and reuse candidates below are assertions in the supplied plan, pending the integrator's source/provenance work. [Plan §§2.1, 18; package README “Actual validation performed”; task export; generated/VALIDATION_EN.json]

## 2. Product experience and persistent shell

The intended authenticated entry is `/world`. An eligible first-time participant receives a usable preset avatar immediately; optional customization, profile completion, or possession of an X account must not become an extra entry gate. Necessary service eligibility and consent remain enforced. One compact shell holds the header, `REVENGE / PARADE / DANGER` selector, world viewport, contextual panel outlet, and six labeled tabs: `WORLD / FEED / MEET / GIFTS / WALL / ME`. [Plan §§5.1, 7.2; A01, W01, U02; FE07, FE09, FE13]

The key architectural boundary is **shell lifetime versus panel lifetime**. Opening `/gifts/:giveawayId`, `/feed/:postId`, or `/me/avatar` should initialize or reuse one authenticated shell and open the appropriate panel. Panel transitions must retain the same Phaser game and world connection. They must preserve virtual position, camera, room, list scroll, and safe drafts. Invalid or unauthorized links must fail safely without briefly displaying cached private metadata. Browser back closes detail before list and list before returning to the unobscured world. [Plan §5.1; FE07, FE35, FE41]

Mobile sheets have peek, half, and full states with one primary panel at a time. A blocking panel suspends movement input; a full sheet should allow rendering to pause or throttle. Backgrounding is a connection lifecycle event: return requires reconnect/resynchronization rather than a promise of an uninterrupted socket. Logout, revocation, and deletion also require coordinated world teardown and private-state cleanup. [Plan §§5.1, 7.2, 16; FE15, FE17, FE36, FE41]

**Recommended integration seam:** the integrator should define a small shell/world bridge before FE07 and FE14 meet. It should cover mount/dispose, current event/participant context, input suspension, visibility/low-power state, theme/avatar updates, and world-object-to-panel navigation. The bridge should expose typed commands/events rather than let feature panels reach into Phaser scene internals. This interface is a recommendation; the supplied plan does not yet specify its API. Define how ordinary dialogs stack over the single primary sheet and restore focus/history, since those details span FE05, FE07, and FE15.

## 3. Route and feature families

The IA contains 64 entries, of which 61 are launch-required and three are optional. An entry can be a sheet, modal, reusable state, or route; 64 entries do not imply 64 isolated page applications. Phase P1/P2/P3/P4 and launch priority P0 are different concepts: later-phase safety and administration remain required. [Plan §6; package README “Plan inventory”]

| Family | IA and principal routes | Frontend ownership and behavior |
|---|---|---|
| Public/authentication | A01–A08: `/`, `/login`, `/signup`, verification/reset/callback/legal/deletion | FE08–FE11, FE36; new forms, safe redirects, truthful provider availability, consent/expiry states; public routes do not load Phaser. |
| World | W01–W08: `/world`, map, places, rooms, nearby, theme, check-in, help | FE06–FE07, FE12–FE20; canvas world plus DOM alternatives, room lifecycle, map/version errors, explicit physical check-in. |
| Timeline | F01–F05: `/feed`, composer/thread/edit, `/notices/:noticeId` | FE27–FE28 and FE04; chronological cursor lists, image-only authoring, source-stamped notices, reauthorized entity links. |
| Meeting and people | M01–M09: `/meet`, parties/manage/rally, beacons, people/intro/profile | FE20–FE23, FE29–FE31; PARTIES/PEOPLE/MY PLAN, capacity and access states, private eligibility, consent-driven sharing. |
| Giveaways | G01–G07: `/gifts`, create/detail/manage/reservation, pickups, mine | FE24–FE26 and FE04; GIVING/RECEIVING, owner inventory, receipts, timeout reconciliation, expiry and single pickup. |
| Mural | R01: `/wall` | FE32–FE33; grid editor, explicit commit and authoritative revision synchronization; world-wall representation shares the same state. |
| Personal/inbox | U01–U10: `/me` and children, `/notifications` | FE13, FE30–FE31, FE34–FE36; avatar/profile/preferences, private handles and requests, blocks/account, safe notification targets. |
| Common/archive/admin | S01–S03, X01, O01–O09: report, global errors/connection state, archive, `/admin/...` | FE36–FE41; role-aware moderation, place review, flags, mural restore/event closure, read-only archive and recoverable failures. |

Source: Plan §§6.1–6.8 and §12; IA ledger. Administration should retain a lazy, role-checked boundary; the plan does not settle every detail of whether operational pages visibly retain the world viewport. The integrator should decide that layout explicitly while keeping ordinary participant panels in the persistent shell. Route workers should supply feature exports/registration data; the integrator owns central router composition. [Plan §§3.1, 15.2; FE07, FE37–FE39]

Optional routes are **M10 party chat (EX01), R02 wall history (EX02), and X02 souvenir (EX02)**. The reference's chat bar, floating chat controls, and level display do not authorize global messaging or a level/experience system. PWA/offline read mode is EX03 after required gates. [Plan §§6, 14, 16]

## 4. New UI and headless reuse boundary

All rendering is new: semantic markup, controls, pages, layouts, navigation, CSS, tokens, illustrations, animation treatment, and visual baselines. Existing screens cannot be renamed or recolored into compliance. Mantine rendering must not enter the new tree. Mixed rendering/logic modules need extraction before reuse. C08 provides an allowlist/origin gate, and QA02 still requires manual comparison; a static gate alone cannot establish visual originality. [Plan §§2.2, 10; C08, FE03, FE05, QA02]

The proposed stack is React/TypeScript/Vite, React Router, TanStack Query, Zustand, ky, Zod, React Hook Form, Tailwind/new CSS variables, dynamically loaded Phaser **3.90.0**, native WebSocket, MSW, Vitest/Testing Library, and Playwright. Retained resolutions and Node 24 runtime qualification belong to C02; manifest declarations in the plan are not evidence of a compatible installed tree. Three.js, GSAP, FFmpeg, and legacy video/3D behavior are not automatic carryovers. [Plan §3.1; C02]

| Candidate reuse described by source | Required adaptation and boundary |
|---|---|
| Transport cancellation, errors, CSRF hooks, upload progress | FE01/FE09 must map actual Gateway operations and token handling; old cookie-only and `/v1/auth/email/...` assumptions are incompatible. |
| Query-key and invalidation discipline | FE02 uses canonical `(user_id, event_id)`, discards late old-identity responses, and evicts revoked private state. No required `activeXAccountId`. |
| Pure validation, state machines, policy helpers | FE03 extracts reviewed non-rendering logic, adapts canonical IDs/errors, and preserves boundary fixtures. No broad import of the old API barrel. |
| Image compression/upload orchestration | FE04 retains abort/progress/finalize/retry and preview cleanup; no video stack or credential leakage to presigned uploads. |
| Non-visual assertions and fixtures | Preserve useful semantics and provenance; author new route selectors, markup expectations, and reviewed visual snapshots. |

Source: Plan §§2.1–2.2, 7.2, 12; FE01–FE04, FE09. Every copied module needs source revision/path, destination, dependency audit, and regression evidence in C01. This report has not inspected or approved those upstream modules.

Server data belongs in TanStack Query; local UI concerns belong in Zustand; movement prediction/rendering is a world-client concern. Avoid duplicating server-authoritative stock, consent, room membership, or mural revisions as independent local truth. Access tokens remain in memory, refresh credentials use the selected secure cookie arrangement, refresh is single-flight, and private cleanup spans query caches, sockets, and sensitive drafts. [Plan §§3.1, 7.2, 8; FE02, FE09, FE17]

## 5. Art, themes, mobile, and real-world meaning

I visually inspected the approved composite. It shows five densely framed mobile concepts with pixel characters, a venue-like world, prominent era tabs, compact community navigation, giveaway and pixel-wall landmarks, paper/sticker treatments, and a desktop-window motif in some variants. The consistent useful direction is a readable world surrounded by an early-web fan-community shell. The written plan selects **one** application with **three** themes; the image is not a requirement to build all five shells. It also does not establish asset ownership or verified venue geometry. [Approved reference image; Plan §§1, 5.2, 16]

### Asset contracts

| Contract | Required invariant | Owners and acceptance |
|---|---|---|
| Map and places | Versioned Tiled JSON, stable layer/place/interaction IDs, collision/spawn information, manifest version/hash, place source/review/closure metadata. Concept-only places cannot become verified destinations. | C06, AR01–AR02, FE14, FE19; walkable spawns, resolvable interactions, blocked-route checks, visible mismatch recovery. |
| Avatars | Original layered PNG atlases; 64×64 cells; foot anchor `(32,60)`; four directions with one idle and four walk frames per direction. Config stores approved item IDs plus manifest version. | C06, AR03, FE12–FE13; golden composition, alignment, missing-asset fallback, version conflicts, matching world/profile/feed/gift/party identity. |
| Three skins | REVENGE/PARADE/DANGER share geometry, collision/place IDs, room population, and interactions. Switching cannot move the player or recolor another person's outfit. | C06, AR04, FE06, FE14; failed-load rollback, unchanged identity/position, manifest hashes/licenses, contrast across skins. |

Source: Plan §§3.1, 5.2–5.3, 10, 12. Initial avatar scope is six presets, six hairstyles, four eyes, six outfits, four accessories, and selectable skin tones; AR03 must validate the atlas before freezing the final layout. The specified visible character target is about 56–72 CSS pixels tall. Contract fixtures can unblock implementation, but temporary graphics cannot establish final-art acceptance.

### Mobile and accessibility requirements

Design starts at **390×844 CSS pixels**, with acceptance at widths **360/390/430**. The unobscured world targets roughly 70% of usable height, while all six bottom labels remain usable at 360 pixels. Controls need at least **44×44 CSS pixel** touch targets, safe-area handling, keyboard accommodation, readable contrast, reduced motion, and sound off by default. Full panels and software keyboards make the nominal viewport budget state-dependent; verify each state rather than force the 70% target while editing. [Plan §§5.1, 6.7, 16; FE05, FE07, FE34; QA06]

Provide DOM alternatives for places/nearby users and essential social/pickup actions, keyboard and focus handling, screen-reader labels, recentering, and reliable pointer cancellation. Movement must stop on blur/pointer cancellation and while typing or interacting with blocking panels. Mural panning previews rather than writes: its 128×128 grid and 16-color palette require explicit pixel confirmation, touch disambiguation, and accessible controls. [Plan §§5.1, 8.4; FE15, FE18–FE19, FE32; QA07]

Actual iOS Safari and Android Chrome coverage belongs to QA06, with browser/device versions recorded. The 30 FPS target, 50-user initial room cap, 20 Hz simulation, at-most-10 Hz updates, and 200-connection/four-room scenario are qualification targets. None is measured capacity in this package. [Plan §§7.5, 16; QA05–QA08]

### Physical meetings require separate UI truth

`WorldPosition` is virtual. `MeetBeacon` is deliberately shared physical guidance with verified place, audience, confirmation time, expiry, and revision; initial configurable TTL is 15 minutes. Show `YOU IN WORLD` and `ONLINE IN THIS ROOM` for virtual presence, explicit ON_MY_WAY/HERE/MET actions for self-reported rally status, and clear moved/stale/expired/revoked meeting states. Never derive physical arrival, GPS distance, walking time, or venue safety from avatar motion or the drawing. Venue-related operational facts need the separate review required by AR01 before publication. [Plan §§1, 5.4, 8.2; FE19–FE20, FE23, FE26]

## 6. Deterministic mocks and independently schedulable frontend work

Freeze HTTP/identity (C03), domain transitions (C04), realtime (C05), and assets (C06) before their consumers implement features. C07 supplies generated clients, schema checking, a deterministic clock/seed, and shared fixtures. A feature can consume its frozen subset before the entire fixture catalog is complete. HTTP mocks use MSW; `MockWorldTransport` and `WebSocketWorldTransport` implement one interface. Production components must not branch into fake success behavior. [Plan §§7.1, 9, 15.1; C03–C07, FE16]

Recommended scenario coverage uses the same generated schemas and fixed clocks for normal, empty, forbidden, conflict, expired, throttled, unavailable, and reconnect cases. In particular, exercise delayed responses across an account switch, uncertain reservation outcomes, server-time expiry, moved beacons, revoked sharing, blocked presence, missing deltas, room epoch changes, and mural restore. Exact scenario IDs and scripts are implementation work, not existing capabilities established here. [Plan §§7.1–7.5, 8; FE02, FE17, FE20, FE26, FE31, FE33]

The parsed graph confirms no **direct FE-to-BE scheduling edges**, consistent with the package's intent. This does not make frontend tasks mutually independent: FE16 needs FE01; FE18 and FE13 need FE05; FE17 joins the runtime, avatar, and transport branches. Shared fixtures and interfaces must be available without waiting for a live backend. A mock-complete feature is `MOCK_READY`; only executed real-service journeys can support `INTEGRATED`/`ACCEPTED`. [Task export; Plan §15.3]

## 7. Recommended first disjoint queues

These are future assignments, not implementation started by this report. C01/C02/C08 and the relevant frozen contracts are prerequisites. Paths are proposed target paths under `frontend/src/`; most are not present in the inspected workspace. Phase numbers should not become global barriers. [Plan §§4, 9, 12, 15]

| Queue | First unit and exact prerequisites | Owned target modules | Reviewable evidence required |
|---|---|---|---|
| FE-APP transport | **FE01** after C02/C03; then **FE02** after FE01/C03 | `shared/api/http.ts`, `shared/api/errors.ts`; then `shared/query/keys.ts`, `shared/query/privacy.ts` | Abort, 401/409/429 and normalized reasons; correct auth mapping; old-identity response cannot populate new-identity cache. |
| FE-APP rendering foundation | **FE05** after C02/C08; then **FE06** after FE05/C06 | `shared/ui/`, `shared/styles/base.css`; then `shared/styles/themes/`, `features/theme/` | Newly authored markup/styles, focus/dismissal, touch sizing; skin loading/fallback and stable-world theme application. |
| FE-APP policy extraction | **FE03** after C04/C08 | `shared/validation/`, `entities/intro/model/` | Reviewed headless provenance, canonical IDs/enums, semantic fixtures, no legacy rendering imports. |
| FE-WORLD runtime | **FE14** after C06/C02; then **FE15** after FE14 | `world/runtime.ts`, `world/scenes/`, `world/map/`; then `world/input/`, `world/camera/`, movement interactions | Golden-map loading and errors; repeated mount/dispose; resize; collision/input/pointercancel behavior and panel input suspension. |
| FE-WORLD avatar | **FE12** after C06/C02 | `world/avatar/`, `entities/avatar/` | Deterministic layered composition and shared thumbnail output; anchor/frame/fallback evidence. No concurrent edits to map or shell. |
| FE-WORLD transport | **FE16** after C05/**FE01** | `world/network/WorldTransport.ts`, `world/network/mock.ts`, `world/network/websocket.ts` | Matching transport scenarios, one-use ticket handshake and sequence/close handling; production excludes mock mode. |

Source: Plan §12 task cards and §15.1. These are disjoint module assignments, not a recommendation to create more agents or one worker per row. Keep the runtime, avatar, and transport module boundaries even when one person executes the FE-WORLD queue serially.

**First composition checkpoint:** FE07 follows FE05/C03 and supplies the persistent shell, nested routes/history, direct-link behavior, and a single world mount. Although FE07 names `app/router.tsx` and `app/WorldShell.tsx`, §15.2 reserves central router composition for the integrator. An FE worker can prepare the shell/route contribution; the integrator serializes router edits. Package files, lockfiles, contract roots/generated policy, CI/environment manifests, and the asset manifest registry also remain centrally owned. [Plan §§9, 15.2; FE07]

**Next converging work:** FE17 after FE16/FE14/FE12; FE18 after FE17/FE05; FE13 after FE12/FE05/C03; FE09 after FE01/FE02/FE07/C03. FE08 can follow FE05/FE07. Then FE19–FE26 provide the meetup/giveaway track while FE27–FE33 can proceed against their own dependencies. FE15 owns movement interactions; FE33's later `world/interaction/mural.ts` needs an explicit subpath handoff to avoid a broad directory ownership collision. [Plan §12]

The first useful integrated product checkpoint is **IN01**: two actual identities in the same live world, visible authorized movement, avatar changes, theme changes, panel navigation, and reconnect, using actual reviewed art. A scripted second avatar or mock socket is useful development evidence but cannot satisfy that gate. [Plan §13; IN01]

## 8. Integration risks and unresolved decisions

| Risk | Why it matters | Concrete integration action |
|---|---|---|
| Stale destination metadata | Narrative, README, and generated task cards still direct delivery to bigyeting. | Integrator consistently applies `julymeltdown/mcr`/`master`; worker reports and future generation inputs must not silently restore the old destination. This analyst does not rewrite those files. |
| Shell/game/socket ownership | Nested route remounts, React cleanup, or independent panel controllers can create duplicate games/connections and lose state. | Freeze the shell/world bridge and lifecycle owner; test direct links, back navigation, repeated mount/dispose, auth changes, and full-sheet/background transitions. FE07/FE14/FE17/FE41. |
| Authentication and privacy drift | Old cookie/endpoint and X-account assumptions can survive extraction; delayed responses and realtime events can restore revoked data. | Review FE01/FE02/FE09 against C03 and clear/invalidate both HTTP and world state on policy/auth changes. No localStorage bearer tokens. QA04/IN03. |
| Theme/art coupled to geometry | Separate scene copies can split populations or shift collision/anchors; oversized atlases can exceed mobile budgets. | Freeze stable IDs/anchors, validate golden assets, switch visual packs with rollback, and measure at real mobile scale across all skins. C06/AR02–AR04/QA06/QA08. |
| Reference treated as feature or venue authority | The artwork includes chat/levels and illustrative physical landmarks. | Apply written exclusions and place review states; keep virtual presence distinct from self-reported physical arrival. AR01/FE19/FE20/FE23. |
| Realtime/mural sequence ambiguity | A room-global filtered sequence can create false gaps; stale epochs or mural restore can leave ghosts or obsolete pixels. | Use per-delivered-stream sequence and explicit epoch/revision reset/resync; bound queues and replay no old domain commands. C05/FE17/FE33/QA05. |
| Mutation timeout shown as failure/success | Reservation, last-slot approval, and pickup can commit while a response is lost. | Use server reasons, stable idempotency keys and receipt/status reconciliation; preserve drafts on version conflicts. No silently queued offline pickup. FE23/FE25/FE26/FE41. |
| Mobile layering and canvas-only controls | Sheets, keyboard, joystick and canvas focus can conflict; essential actions may become inaccessible. | Decide sheet/dialog focus rules and input suspension before composition; run 360px keyboard/safe-area scenarios and DOM alternative journeys. FE05/FE07/FE15/QA06/QA07. |
| Operations/privacy postponed behind attractive screens | Launch-required moderation, block propagation, closure, deletion and archive constraints sit late in the phase table. | Keep them on the release checklist even when absent from a narrow integration dependency list. FE36–FE41/QA04/IN05. |
| Planning or mock evidence overstated | The package already contains structural PASS artifacts and generated acceptance descriptions. | Record the exact candidate, commands/results and evidence; separate `MOCK_READY`, real integration, device acceptance and release decision. QA01–QA10/IN01–IN05. |

Source: Plan §§5–8, 12–16; package README “Release-policy clarification”. These risks are derived from the specified boundaries; they are not bugs observed in an application implementation.

## 9. Handoff

The first authorized implementation step, if later requested, should establish source boundaries, reproducibility, frozen interfaces, fixture subsets, and exclusive ownership; then execute the disjoint FE-APP and FE-WORLD queues above. Preserve the product's strongest invariants throughout: one persistent world, new UI, recognizable shared avatars, three visual skins on one geometry/population, explicit physical meeting state, private consent handling, recoverable mutations, and accessible mobile controls.

This assessment changes only `docs/analysis/frontend-assessment.md`. It does not import code, install packages, implement application features, commit, push, or certify a launch. The parent integrator owns source provenance, broader checks, final assessment, and delivery to `julymeltdown/mcr` on `master`.
