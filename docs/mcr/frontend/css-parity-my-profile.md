# CSS-PARITY-MY-PROFILE

Status: IN_PROGRESS. Owner: `01a1a5d9-6955-7820-bcd4-8e365e51f589`.
Goal: ACTIVE, unbudgeted. Publication belongs to Root153
(`01a1a153-1227-7e72-801f-a2a6b91981e1`).

## Scope and provenance

Isolated checkout: `/Users/lhs/.codex/worktrees/66d3/mcr`.
Branch: `codex/mcr-fe-css-parity-my-profile`.
Fetched and fast-forwarded clean base: `54320d71c4ae0bff3524d1b500cea772e0ba42e0`.
Product ownership is limited to:

- `frontend/src/features/me/ui/me.css`
- `frontend/src/features/profile/reference-presentation/activity.css`

The new reference is the user-supplied `1-Photo-1.jpg`, bottom row, fifth panel,
My Profile. Original attachment path:
`/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg`.
It supersedes the earlier PNG for this assigned foreground comparison.
SHA-256: `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`
(Root153's supplied pin; native preflight independently verifies the bytes).
Preserve both originals. No upstream code is copied by this increment.

Root153 granted the exclusive two-file CSS lease before product writes; former
background writers stopped on these paths. Preserve the existing portrait square,
framing, sizing and 128px renderer exactly. No avatar CSS changes are included. Native browser,
Vite, tests and exports require a resource21d named offer and accepted preflight.
Documentation preparation does not consume an output slot.

## Foreground comparison and constraints

The photo has a large square portrait, strong identity name, compact era text,
short bio, four centered action icons, contiguous Activity/Parties/Gifts tabs,
and two compact activity rows. Existing production components provide those
regions; foreground typography, action spacing and activity proportions can be changed in CSS.
The avatar square and selected-character framing/sizing remain unchanged.

Content and ownership differences that CSS cannot resolve:

- The reference level is absent by design: leveling is explicitly excluded by
  specification section 16. Do not fabricate it or reserve a fake value.
- The four production actions are Edit profile, Dress up, Manage private social
  links and Sharing requests. External X/Instagram/Discord actions in the photo
  need an explicitly authorized product destination and privacy model from the
  responsible implementation owner. Keep current routes and accessible labels.
- Profile activity currently accepts only an authorized bounded feed page and
  displays up to two own-author posts. Reservation/join history in the photo
  requires separately authorized inputs and producer/consumer work. Keep the
  partial-view notice, real post text, dates and counts truthful.
- Activity visuals currently use a generic event-feed glyph. Photo-specific gift
  and party thumbnails require the activity implementation owner to supply
  authorized item/media context; CSS must not replace them with fabricated art.
- The feature fixture has no admitted portrait binding and must show its honest
  unavailable state. The root selected-character fixture exercises the actual
  approved artwork. Neither fixture establishes a live-service or device gate.
- Background art, shared root/title/footer chrome, selected character imagery
  and rendering are outside this lease. Do not change them to mimic the photo.
- Shared route metadata at `frontend/src/app/routes.ts` supplies `/me` with the
  title `Me`; the reference says `MY PROFILE`. The feature's internal `My profile`
  heading is hidden by shared embedding. Root153 received the exact title-label
  request. Do not replace the title using CSS generated text or out-of-scope TSX.

## Validation plan

Use the actual `features/me/__tests__/reference-target/preview.html` production
component fixture for partial activity, loading, empty, long, error, expiry,
keyboard focus, real route actions, options disclosure and edit/cancel behavior.
Also use
`src/app/__tests__/cozy-backgrounds-v1/preview.html?entry=/me&era=PARADE`
for the default root with the admitted selected character and shared embedding.
The focused fixture separately exercises privacy scrub.
Capture 390px foreground before/after and 360/430px responsive states at 844px
height. Compare geometry and images, iterate on concrete differences, preserve
failed attempts. Do not certify full visual acceptance from passing assertions.

Commands, exit codes, evidence, source hashes, resource release and committed
handoff will be appended after the bounded owner-run round.

## Source increment awaiting native allocation

The first increment increases the identity name/era/bio typography, distributes
the four action targets across the header, removes spacing between the activity
navigation controls, and gives own-profile activity a 52px visual and 16px action
label. The activity-scope notice and metadata remain visible; public-profile
activity rules retain their original sizes. Forced-colors focus uses the system
Highlight outline. Normal focus and all avatar styling remain unchanged.

Validation status: NOT_RUN pending a named native-output offer. The allocator
reported a 16 GiB start-threshold failure during queueing. No browser, Vite,
dependency link, test, compiler or image export was started before allocation.
