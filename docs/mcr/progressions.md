# Paradise City venue flows — 2026-11-05

This is the product-flow document for the user's ten new venue references. It describes required behavior and comparison inputs; it does not award implementation credit. The [scope amendment and extension register](venue-feature-amendment-2026-11-05.md) remains Main-owned. Preserve the original 112 required task IDs, 61 required IA entries and their historical evidence. Added venue work must remain explicit in that register rather than silently inflating old completions or combining denominators.

The new references replace conflicting Culture Park-only, old Map v3 and roof-only targets for the affected venue surface. They add opt-in real phone location, fixed-landmark meetup invitations and contextual MESSAGE/conversation, and place-anchored notes. Earlier no-DM or excluded-feature assumptions cannot veto these specifically requested flows. This amendment does not authorize unrelated unrestricted global or party messaging. Independent Profile/editor, private pages, operator pages and unrelated World work continue.

## Source packet and provisional roles

All ten original JPEGs are preserved byte for byte in [the reference manifest](design/venue-redesign-2026-11-05/reference-manifest.json). The [state map](design/venue-redesign-2026-11-05/reference-state-map.json) splits the composite sheets into 48 individual screen/component cases. Verified source dimensions are canvas dimensions, not observed phone viewports.

| Photo | Canvas | Role |
|---|---|---|
| [1](design/venue-redesign-2026-11-05/photo-01.jpg) | 1024 × 1280 | Fenced whole-site alternative |
| [2](design/venue-redesign-2026-11-05/photo-02.jpg) | 1280 × 853 | Provisional meeting/message/invite anatomy; fenced alternative map background |
| [3](design/venue-redesign-2026-11-05/photo-03.jpg) | 1280 × 853 | Alternate Notes states and components |
| [4](design/venue-redesign-2026-11-05/photo-04.jpg) | 1024 × 1280 | Distinct fenced whole-site alternative |
| [5](design/venue-redesign-2026-11-05/photo-05.jpg) | 1024 × 1280 | **Provisional primary base map**, explicit TOILETS and simplified standing/queue boundary |
| [6](design/venue-redesign-2026-11-05/photo-06.jpg) | 1280 × 853 | Alternate Notes states and components |
| [7](design/venue-redesign-2026-11-05/photo-07.jpg) | 1280 × 853 | Alternate Friends states and components |
| [8](design/venue-redesign-2026-11-05/photo-08.jpg) | 1280 × 853 | Fenced Friends alternative |
| [9](design/venue-redesign-2026-11-05/photo-09.jpg) | 1280 × 853 | **Provisional primary Find Friends** states and components |
| [10](design/venue-redesign-2026-11-05/photo-10.jpg) | 1280 × 853 | **Provisional primary Notes** states and components |

The frontend king owns the already-pending optional human primary-reference choice. Common typed flows proceed with the provisional roles above. Keep each alternative distinct; do not add a user-visible design variant selector or blend conflicting fences and geometry.

Photo 5's full-map body is the expected base-map region. Photo 2's meeting UI and individual components are expected anatomy regions. Their exact crop rectangles are **unmeasured**: `sourceROI: null` and `DEVELOPER_MEASUREMENT_REQUIRED` are intentional. Photo 2's old fenced background is an explicit primary-variant difference when its meeting UI overlays Photo 5. Do not claim whole-frame Photo 2 identity, stretch geometry or hide that difference in masks. Each developer measures the relevant map, handset or component region before using it for comparison. Poster headings, captions and surrounding montage space do not automatically become product UI.

## The venue surface within the existing application

The authenticated shell retains one canonical identity, one persistent Phaser game and one World connection. Its six global navigation destinations and era controls remain intact. The venue surface has three local tabs: **MAP / FRIENDS / NOTES**. Local tab changes preserve the selected place, useful camera state and safe drafts within their authorized lifetime. Browser back closes detail before its list and then returns to the existing shell behavior.

The local active Map/Friends/Notes tab uses burgundy/crimson with white text. Audience and expiry selectors use black with white text for selected choices. This local reference rule does not globally replace unrelated Parade Ink styling. Match the references' dark header, white paper, dense rows, compact controls and crimson primary actions in functioning components. Preserve approved characters from the [selected source pack](../../frontend/src/entities/avatar/character-source-pack.json) and [character guidance](character-art.md); no legacy, synthetic-test or newly generated character fallback. Do not use decorative left accent bars or rails.

An illustrated map texture can supply the site-plan artwork; controls, people, lists, invites, messages, notes and state changes must remain interactive. A screenshot of the composite reference is not a delivered application surface.

## MAP: orient, select and recover

1. Open MAP to see the complete Paradise City site plan, its supplied named landmarks and queue zones. The full-site extent remains discoverable through pan, zoom and recenter. Do not crop the product into the old roof-only or Culture Park-only scope.
2. Select a reviewed landmark to show its place card and available actions. VIEW ON MAP focuses that fixed landmark. A note's MEET HERE uses its anchored landmark. A friend card's MEET HERE requires a fixed reviewed selected place; a moving phone pin is not a meeting place.
3. Pan and zoom preserve stable image-space anchors. Recenter explicitly identifies its target: overview, selected landmark or the user's available authorized phone location. Phone recenter must not move the virtual player, teleport the World camera or emit virtual movement.
4. Missing, unsupported or invalid map revision/hash/bounds produces a truthful unavailable/error state. Retry must not display stale geometry as a newly verified map. Loading and disposal must respect the active surface lifetime.
5. If physical projection is not qualified, show the illustrated plan and usable place/list alternatives while keeping physical coordinate projection unavailable. An approximate illustration is not a verified GPS transform.

Venue inputs include immutable venue/map revision, asset SHA, canvas dimensions/full extent, stable reviewed landmark IDs and labels, kind, source anchors/bounds, provenance and availability. Artwork anchors, image pixels, physical coordinates and virtual World coordinates remain separate types and authorities. Do not infer a transformation from the drawing, old Map4/C05 coordinates or an example pin.

## FRIENDS: explicit physical-location sharing

Device location permission and service sharing consent are separate. Opening the tab, accepting a meetup, existing friendship, exchanging social handles or moving in the World does not grant phone-location sharing.

1. With sharing off, show the off state and an explicit opt-in action. Request platform permission only through that flow. Handle unsupported geolocation, denied permission, unavailable position, timeout and interrupted requests truthfully. Explain the active sharing audience before starting.
2. On permission and service authorization, render the user's actual sharing state and available authorized position. Pending requests and rejected service writes remain distinct from active sharing. Do not fill missing coordinates with a design example or virtual position.
3. FRIENDS shows authorized people as rows and, where physical projection is available, map markers. Distinguish **current phone location**, **check-in** and **last-known location**, with the actual source and timestamp/freshness. Example labels such as 40 seconds, 8 minutes or 12 minutes are fixtures; derive live labels from sample times and the server clock.
4. VIEW focuses an available authorized target. ASK LOCATION is a permission-scoped request, with pending, denied, expired, blocked and retry states from the frozen producer contract. It cannot silently enable a friend's sharing or substitute a social-handle grant.
5. Clusters include only authorized members and counts. Zoom/select may reveal the permitted members. Never expose hidden membership through a marker, cluster count, stale cache or an error label.
6. Pause/revoke updates the service sharing state, withdraws or expires visibility according to the producer contract and removes unavailable markers from authorized consumers. Preserve clear pending/error behavior if the change fails. Logout, deletion, block, friendship revocation, expiry and account/event changes invalidate relevant private location state through the responsible lifecycle owner.

Required inputs: authorized identity and approved portrait; source discriminator; actual sample/check-in time; server clock; freshness/expiry; sharing/permission state; visibility revision; physical projection or its absence; near-landmark ID if qualified; and per-action capabilities. No local hardcoded freshness cutoff or inferred friendship policy replaces the backend freeze. Lists remain useful when coordinate projection is unavailable.

## Fixed-landmark meetings and contextual MESSAGE

Photo 2 contains separate states: a selected friend on MAP; a fixed Main Gate meeting flag with moving friend/self markers; a contextual conversation and accepted meetup card; and individual place, freshness, fixed-versus-moving and pending-invitation components. These are independent cases, not one permanently visible montage.

1. From a selected authorized friend or anchored note, choose MEET HERE at a fixed reviewed real landmark. Show the place and intended participant before sending. The backend authorizes the request and supplies invitation identity, revision, status and expiry.
2. The recipient sees the pending invitation with DECLINE and ACCEPT when permitted. Pending actions disable duplicate submission and use server idempotency/revision behavior. Late, cancelled, declined, expired, blocked, unauthorized and conflicting replies receive explicit current-state handling.
3. On acceptance, retain the fixed meeting flag while authorized live friend/self markers may move. **Acceptance and “on my way” do not prove physical arrival.** Physical arrival/check-in, if supported, needs its own qualified producer evidence.
4. MESSAGE from an authorized selected friend opens the contextual conversation/composer without requiring invitation creation or acceptance. `invitationId` is optional. Opening the composer performs no implicit invitation or message write. Render actual participants, messages, timestamps, send/pending/failure state and withdrawal/expiry capabilities from the frozen contract. The server must authorize the selected-friend context; a bare user ID does not grant unrestricted global DM access.
5. CANCEL MEETUP uses the server-supported capability and current revision. Preserve the resulting terminal state and conversation access policy supplied by the producer. Do not assume cancellation deletes conversation history or leaves it readable forever.
6. Retry, reconnect, background return and concurrent actions reconcile with server state. Private messages/invites must not survive an account/event switch or authorization withdrawal in another user's cache.

Required inputs: selected-friend contextual conversation identity and authorization, optional invitation linkage, participants and message/read/send state; for invitation actions, fixed landmark ID/revision/provenance, invitation ID/revision/status/expiry and capabilities; and server-defined cancellation, revocation and retention semantics. Existing party/rally/beacon workflows may offer reusable headless behavior but are not automatically this two-person fixed-landmark invitation or contextual conversation.

## NOTES: a place, an audience and an expiry

Photos 3, 6 and 10 each contain a left Notes map/list state, middle composer and separate right-side marker/card/selector/reply components. Photo 10 is the provisional primary. Each variant stays separately traceable.

1. Open NOTES to show authorized single/group envelope markers and counts. Select a place or group to open its list; selecting a note shows its actual author, body, place, timestamp, audience/expiry and actions. Close returns to the preserved map/list state.
2. New note opens a composer with a fixed reviewed place. CHANGE PLACE selects another reviewed landmark without turning a free-moving friend or virtual position into a place ID. Preserve a safe unsent draft when appropriate; clear private data on identity/event invalidation.
3. Choose Friends or Everyone from server-supported audience values. Backend owners must freeze what Everyone means for this event, who can discover/read/reply, and how friendship changes or blocks affect existing content. Do not silently interpret it as an unrestricted public Internet audience.
4. Choose 1h or End of day. The server computes `expiresAt` using the event's authoritative timezone and policy; local browser midnight is not the authority. Render the returned expiry and handle a note expiring while selected or during reply submission.
5. POST NOTE validates the body and current place/audience/expiry capabilities, sends idempotently and reconciles the returned note/revision. Pending, validation, denied, blocked, rate-limited, stale-place and network failures keep truthful state and allow the appropriate retry.
6. REPLY uses the note's authorized conversation/reply semantics and expiry policy. Inline replies show real send state; counts and list updates must not leak hidden or expired replies. MEET HERE starts the fixed-landmark flow at the note's place; it never implies that the note's author is physically there.
7. Expiry, deletion, moderation, audience changes and revoked authorization remove or replace stale markers/cards according to the service contract. Expired notes must not reappear through the old Feed/Wall archive or a stale private cache.

Required inputs: note/reply IDs and revisions; fixed landmark ID/revision; authorized author and approved portrait; body/timestamps; audience; selected expiry choice plus server `expiresAt` and event timezone; group count/selection; status and allowed actions. Map/list/composer/reply error and empty states are part of the flow.

## Producer and writer boundaries

This document records component input requirements, not a new wire schema or a grant over shared contracts. Root/backend contract custodians freeze actual operation names, schemas, generated public types, privacy semantics and capabilities. Deterministic fixtures let frontend work proceed while real producers are implemented; fixture screenshots never certify a live multiuser/device flow.

| Owner | Current venue responsibility | Boundary |
|---|---|---|
| Art 231 | Photo 5 pack under `frontend/public/venue/paradise-city/v1/**` and feature-local `art-source/**` | Immutable art, source anchors and provenance; no inferred GPS authority |
| Spatial 26e | Feature-local `spatial/{contract.ts,model.ts,image-resource.ts,registration.ts,viewport.ts}` and assigned spatial tests/docs | Image-space types, bounds/revision/hash validation, load lifetime and pure camera; consume producer-verified physical projection only |
| UI 40fde | Feature-local UI/model/ports and its granted new application entry | Local tabs, map/friend/note surfaces and selected physical-friend cards; no duplicate spatial registry or shared World lifetime |
| Meeting 204 | Its exact granted invite/contextual-conversation leaf, new entry and two new routes | Resolve interface directly with UI and backend; preserve original admission/real-meeting invariants |
| Root 153 | Current Map mount/provider and exact shared route/title/header/composition seams | No second root writer, game or World socket |
| Backend Root 1fc and named producers | Location/privacy, invitations/conversation, notes and actual shared contracts | Named L/M/N custodians and schema freeze are still required; frontend design inputs are not producer authority |

These descriptive scopes do not broaden the Root's operative path/hunk grants or override fresh resource booking requirements. Prior crossed proposals do not void the accepted grants. Old affected PI11/Map v3/roof work is preserved under its historical evidence; it is not acceptance against this reference packet.

Existing PI12 private social handles are not phone consent. Age status is not location permission. Inbox remains render-only until new notification/deep-link producers and exact controller ownership are assigned. Operator place presentation cannot invent real-place IDs, physical fields or provenance. Existing virtual nearby popovers retain their truthful virtual scope; the new physical selected-friend card belongs to 40fde. Independent PI07 editor/profile/static frames and the PI06 qualified Profile handoff remain valid within their recorded scope.

## Continuous developer comparison and acceptance

The [state map's capture contract](design/venue-redesign-2026-11-05/reference-state-map.json) applies throughout implementation and fixes. Responsible developers measure each case's source ROI, capture the same actual state at native 360/390/430 CSS widths, record actual height and DPR, and inspect the reference/capture pair, overlay and diff. They repeat after material fixes until differences close. No king or SL inspects implementation pixels or supplies an approval acknowledgment.

Each pair records original photo SHA, case/variant/role, measured ROI, actual CSS viewport, DPR, explicit uniform scale/crop transform, implementation commit, original capture time with timezone, fixture versus real-service/device label, exact producer/contract/source pins, actual capture/reference crop/overlay/diff paths, declared variable-data masks, material discrepancies and fix disposition. Masks may cover declared variable data only; never missing controls, layout or containers. Do not treat a composite JPEG as a handset viewport, use nonuniform stretching or silently substitute a different reference variant.

Reference pairing covers loading, empty, permission denied, stale/unavailable location, pending/expired/cancelled invitation, conversation send failure, composer errors, note expiry and unauthorized/blocked states as well as the pictured happy paths. Original screenshot files and capture metadata stay intact. A visual comparison cannot establish service authorization, real GPS accuracy, actual arrival, device behavior or multiplayer correctness; developers retain the relevant real vertical-flow obligations. Visual acceptance stays open while material differences remain.

## Delivery record and known limits

The documentation owner has an explicit unbudgeted objective: preserve all originals; deliver these complete flows and machine-readable provenance/state map; run bounded hash/dimension/JSON/path/link/scope checks; commit only the two assigned paths; resolve publication fixes and obtain a canonical publisher receipt. The historical background-integration persistent goal is incompatible and still **BLOCKED**. It is preserved without reset, false completion or pause; this assignment's objective and criteria are recorded in the manifest.

The packet's initial checks cover documentation and source metadata only. Application tests/builds, browser capture, pixel comparison, live backend/device GPS, physical arrival, real conversations/notes and combined-master runtime are **NOT_RUN by this owner**. Developer comparisons and actual producer freezes remain implementation duties. No required task or IA entry is accepted by publishing these files.

The packet is authored from base `97b4dce908cf1dab8fa193bed12d7b7964ee3227` in the UI lead's exclusive checkout. Main returned these exact new paths before writes. The publisher receives the immutable scoped commit directly; publication does not wait for Main or frontend king approval. [Validation evidence](design/venue-redesign-2026-11-05/validation.json) records the bounded checks and any external link awaiting Main's separate delivery.
