# CSS-PARITY-MY-PROFILE

Status: MOCK_READY CSS contribution; full-reference parity remains OPEN. Owner: `01a1a5d9-6955-7820-bcd4-8e365e51f589`.
Goal metadata: BLOCKED, unbudgeted; the original objective continued under the
named offer without reset or a completion claim. Publication belongs to Root153
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

The sections below record commands, exits, evidence, source hashes, resource
release and committed handoff.

## Preparation checkpoint before allocation (historical)

The first increment increases the identity name/era/bio typography, distributes
the four action targets across the header, removes spacing between the activity
navigation controls, and gives own-profile activity a 52px visual and 16px action
label. The activity-scope notice and metadata remain visible; public-profile
activity rules retain their original sizes. Forced-colors focus uses the system
Highlight outline. Normal focus and all avatar styling remain unchanged.

Before the named offer, validation was NOT_RUN. The allocator
reported a 16 GiB start-threshold failure during queueing. No browser, Vite,
dependency link, test, compiler or image export was started before allocation.


## Executed round and iterative comparison

Accepted `CSS-PARITY-PROFILE-202611041001Z` at 10:00:53 UTC on November 4;
FULLRELEASE at 10:05:13 UTC (19:00:53–19:05:13 KST), 260 seconds within 300.
Initial clean source was `bf078e2038d03522f542519978338b568ba8878a`.
Pinned Node 24.22.2, retained Vite 8.1.3, React 19.2.7, Playwright 1.61.1 and
Vitest 4.1.9 were verified; retained and candidate pnpm locks were byte identical.
No installation or shared dependency/cache change was made.

The first attempt failed because it asserted the world's canvas count immediately
when the independently loaded profile portrait became ready. Its captured state
had `published:false`, zero sockets and zero world canvases. The driver now waits
for the actual root `published` state. The first failed screenshot, exact driver
and report remain under `round-01/`; this was a driver timing error, not a reason
to weaken the root invariant.

`round-02/` passed the original increment at all three widths. Comparing its
390px root and partial-activity captures to the extracted reference panel led to
a second CSS increment: 26px action glyphs and 18px tab labels. `round-03/` passed
that final CSS. `round-04/` additionally checked every actual-root action's
normal/forced-colors focus at each width. All six before/after cases pass in the
final round. The browser is Chrome 156.0.8078.80; original reports retain the
emitted capture status, with the subsequent visual disposition recorded separately.

The foreground now has stronger identity typography, distributed 48×44 action
buttons, contiguous 44px-high tabs, 52px activity visuals and 16px activity labels.
The 390px profile header grew from 208.39px to 221.11px without moving or scaling
the selected portrait. The synthetic long display name wraps at 360/390 and fits
one line at 430, preserving the real string. The partial-feed fixture displays
actual post excerpts and its scope notice rather than inventing reservations.
Normal red focus and forced-colors system focus remain plainly visible.

### Commands and outcomes

All native commands used the absolute Node path and pinned child PATH. The driver
is `docs/mcr/evidence/css-parity-my-profile/native.mjs`, with `MCR_PROFILE_JOB`,
absolute `MCR_PROFILE_OUTPUT` and private `MCR_PROFILE_TEMP` set per attempt.
Each attempt's exact driver is preserved alongside its report.

| Command / evidence | Result |
| --- | --- |
| Native driver, `round-01/report.json` | Exit 1; preserved early-canvas driver failure |
| Corrected driver, `round-02/report.json` | Exit 0; before/after 360/390/430 |
| Refined final CSS, `round-03/report.json` | Exit 0; before/after 360/390/430 |
| Actual-root normal/forced focus, `round-04/report.json` | Exit 0; same matrix, all four action links |
| `node node_modules/vitest/vitest.mjs run --config src/features/me/__tests__/reference-target/runner.ts --configLoader runner --cache=false --maxWorkers=1` | Exit 0; 31 tests / 6 files; `tests.log` |
| `python3 scripts/mcr/check-plan.py` | Exit 0; planning provenance only; `plan.log` |
| `git diff --check` and exact-path/background/avatar-rule comparison | Exit 0; `source-checks.json` |

No product TSX/JS, backend, routes, assets, shared styles, manifest, lock or
`docs/source/` file changed. Source hashes in `source-checks.json` match the
candidate inputs in the final browser report. Full build, real-service flows,
physical devices and whole-reference acceptance remain NOT_RUN/OPEN.

### Preserved inherited portrait limitation

At 390 and 430 the selected portrait has a 128×128 backing and DOM rectangle.
At 360 its inherited DOM rectangle is **117.59375×128px**, while its backing
remains 128×128. The baseline and final RGBA, DOM position and dimensions are
identical. This is a pre-existing aspect mismatch, not a new square-portrait
acceptance claim. The explicit no-character-framing/scale lease prevents fixing
it here. Root153 receives the exact dimensions and source selector for the
responsible sizing decision: `.me-hub--reference .me-avatar canvas` and the
inline dimensions supplied by `WholeCharacterView`.

### Resource release

`release.json` records a free port 5294, zero owned handles, removed own dependency
symlink, removed private temporary/cache/profile directories and guard, and zero
temporary bytes. Observed native peak was 57,678,383 whole bytes, 51,924,124 temp
bytes and 5,754,259 durable bytes; minimum free was 19,531,862,016 bytes. Durable
bytes at release including source/carry were 5,772,376, well within all caps.
Only source documentation, source snapshots and commits were written afterward;
no output job restarted after FULLRELEASE.

## Direct publisher handoff

Final native evidence: `docs/mcr/evidence/css-parity-my-profile/round-04/`.
Owner disposition: `docs/mcr/evidence/css-parity-my-profile/owner-disposition.json`.
CSS is validated as a bounded contribution, not exact full-photo acceptance.
Root153 must retain the named content/shared-title/portrait constraints above.
No KING, SL, QA or main-chat review/acknowledgment is required for integration.
Published delivery: `6d1b5b36fe014ff0c1efb1e0f6e1648dd462eaa9`.
The implementation owner independently fetched `origin/master`, verified delivered
ancestry (exit 0) and compared all 106 owned postimages byte for byte with the
committed qualified handoff. All match. `publication.json` records each SHA-256
and the three local-to-published commit mappings. No native job reran. Full-photo
parity and the original goal remain unfinished.


### Root shared-copy follow-up

Root153 independently authored `d860ae6d4e50e335d655fc011698d8895432ad66`:
`/me` heading becomes `My profile` and `/me/avatar` becomes `Customize`.
At its handoff this was not intaken, published or native-qualified. The frozen
profile captures deliberately retain the old `ME` title; no extra capture or
shared-source refresh is included. This is a concrete Root-owned follow-up,
not a reason to add CSS replacement text. Root retained the inherited portrait
geometry restriction and acknowledged the exact 360px limit without transferring
avatar-sizing rights.
