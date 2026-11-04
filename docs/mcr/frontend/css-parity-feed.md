# Feed CSS foreground parity

Task: CSS-FEED, related F01/FE27 and shared F03/FE28 styling. Bounded CSS status:
MOCK_READY. Full foreground/photo parity: IN_PROGRESS with exact constraints below.
The intrinsic-media follow-up passed its bounded native/fixture validation;
its publication is independently verified. The separate reference-media
comparison and selected-tab correction also passed their bounded fixture
validation; publication and full target convergence remain open.
The published first pass and all immutable raw reports remain separately identified.
Owner: `01a1a5d9-332b-7822-a8f7-412e295fa211`. Persistent goal metadata: BLOCKED; the aligned objective remains unfinished.
Both later native rounds ran after capacity recovery. The full objective
remains open despite passing the bounded checks.
Delivery goes directly to Root153 (`01a1a153-1227-7e72-801f-a2a6b91981e1`),
who integrates and publishes. No king/SL technical review stage applies.

## Scope and provenance

The isolated checkout is `/Users/lhs/.codex/worktrees/d51f/mcr`, branch
`codex/mcr-fe-css-parity-feed`, fast-forwarded before source edits to
`54320d71c4ae0bff3524d1b500cea772e0ba42e0` from `origin/master`.
Root153 explicitly granted exclusive ownership after the former community
art writer stopped. Product changes are confined to
`frontend/src/features/event-feed/feed.css`. Documentation and evidence stay in
this file and `docs/mcr/evidence/css-parity-feed/`.

The new human reference is the supplied `1-Photo-1.jpg`, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
The Feed panel occupies rectangle `(0,446)-(255,853)` in that 1280×853 photo.
The evidence crop is normalized to 390×623 for inspection; it is an evidence
derivative, never application content. The original is untouched.

No external logic, dependency, contract, asset, or rendering code is imported.
Existing C04 Feed data, authorization and route callbacks remain unchanged.
There are no database migrations. Retained dependency lock hash matches the
read-only delivery tree: `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`.

## CSS intent

The reference has a compact four-tab strip, bold author/time hierarchy,
compact post text, narrow media strips aligned with the text, a single action
row and a centered compose control. The final pass adjusts tab proportions,
17px condensed bold post text, 16px card inset, media-column alignment, 2.85:1
media strip geometry, action text/alignment and a 216px maximum compose width.
Every new selector is scoped to `.feed-browse`; generic Thread/composer rules
and shared post-content selectors remain unchanged.

Selected-character CSS, 48px portrait boxes and canvas/image rendering remain
byte-identical. Decorative page/card/footer backgrounds, shared styles,
shell/chrome, product TSX/JS, assets, manifests and lockfiles are unchanged.
No generated text, fake controls, identities, posts, counts, or media are added.
No decorative left rails are introduced. Existing 44px targets and focus remain
required in the browser checks.

## Validation and iteration

The named `CSS-PARITY-FEED-202611040938Z` round was offered and explicitly accepted
after an own fresh 16.114 GiB free-space/port/runtime/lock preflight. Source
checkpoint `b16133bd1285dd7ba4fef812bee6c4517cf4fc24` preserved the first pass and
driver. The first driver attempt failed because dynamic import of Playwright's
CJS entry did not expose `chromium`; Chrome never launched. That report and exit
1 are preserved in `failed-before-1/`, `failed-before-1.log` and
`supervisor-attempt1.json`. The evidence-only driver now uses its CJS require
entry. Repairs retained the original resource deadline.

The evidence-only [driver](../evidence/css-parity-feed/native.mjs) reuses the existing EventFeed reference fixture
for two explicit synthetic media rows and its actual controller/privacy behavior.
It compares the saved baseline CSS at 390px with the changed CSS, then checks
360/390/430 geometry, tabs/keyboard, notice source, thread callbacks, compose,
options/media filtering, refresh, privacy revocation, empty, read-only,
unavailable, pending and error states. It also consumes the existing cozy
default-root fixture with canonical Auth, router, selected-character renderer
and Phaser over synthetic HTTP/C05. That fixture is not live admission evidence.
The script writes only owned evidence and private temporary caches.

The [comparison index](../evidence/css-parity-feed/comparison.html) shows the
human crop, fresh baseline, first and final iterations, and current default app.
The baseline and both candidate rounds passed their relevant assertions, exit 0.
First iteration copy used 16px Courier New. Actual 390px comparison showed
serifs/heavy glyphs and extra truncation; the second source iteration changed it
to 17px bold condensed sans-serif and reran the same browser checks at all three
widths. The final CSS SHA-256 is
`60a55f5e4f3493dfe2ffad49f26772e8703e5ea21d49c5f7036ace950edba178`;
the final browser report and [input pins](../evidence/css-parity-feed/final-input-pins.json)
bind the tested source and existing fixtures. Those reports describe the
published first pass; the later intrinsic-image validation and publication
are recorded separately below, followed by the unvalidated tab correction.

At 390px, the first iteration placed the first card at `(14,108)`, width 362px,
height 238.70px, with media at `(99,210.48)`, 260×91.22px. These align closely
with the normalized reference's first-card location and roughly 90px media strip.
The final typography revision retains the layout structure. Both candidate
rounds record all exact dimensions; the photo remains an image comparison, not a
claim that different text/media populations are pixel-identical.

| Check | Result |
| --- | --- |
| Native baseline at 390px | PASS, exit 0 |
| Iteration 1 and final iteration at 360/390/430px | PASS, exit 0 each |
| All visible Feed controls >=44px; document width equals viewport | PASS in every candidate case |
| Keyboard tab navigation, notices/source, all three post actions, compose, options/filter, refresh | PASS |
| Normal and forced-colors visible keyboard focus | PASS; final captures retained |
| Empty/read-only/unavailable/pending/error and privacy media/blob revocation | PASS |
| Actual default cozy root, selected-character canvas, compose route and textarea | PASS at 390px |
| Default root single Phaser canvas/socket and zero after disposal | PASS |
| Existing Feed/FeedThread controller/presentation/UI regressions | PASS, 37 tests in 4 files, exit 0 |
| `git diff --check` | PASS, exit 0 |
| `python3 scripts/mcr/check-plan.py` | PASS, exit 0; planning only |
| Background and portrait declaration comparison against saved baseline | PASS, exit 0 |

Browser: installed Chrome version recorded in each report. Node: absolute
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (24.22.2),
matching bin first in every child PATH. The exact single-worker Vitest command
and exits are in `supervisor.json`/`tests.log`. No install, build or type compiler
ran. Source changes are CSS only; no TypeScript contract or application code changed.

Full resource release occurred 243.91 seconds after the original supervisor
start. Last browser/test activity ended about 168.31 seconds after that start;
more than 60 seconds remained for whole owned cleanup. Measured peak whole
output was 22,851,772 bytes against 193,273,528; durable evidence at release was
3,240,390 bytes against 20 MiB. Lowest measured free space was 16,797,978,624
bytes, above the 8 GiB stop floor. Every browser/server/context and private
cache closed, port 5292 was freshly bindable, and the own temporary dependency
link and slot TMP/profile directory were removed. Resource21d received the full
release directly. [Release receipt](../evidence/css-parity-feed/release.json).

Broad build, live-service, device, real multiuser, full QA06 and IN03 gates are
NOT_RUN. No browser/resource reservation remains during documentation/publication.

## Exact remaining constraints and requests

- At normalized 390px, the photo portrait frame is about 60px; current approved
  selected-character slots are 48px. Root153 explicitly excluded character
  source/rendering/UV/crop/framing/scale changes. This discrepancy was sent
  directly to Root153; this worker cannot resize or reframe selected art.
- Reference identities, post wording and reaction/comment counts are supplied
  content. Existing fixtures explicitly identify fictional authors and venue
  illustrations. Those inputs remain separate from real event data. The later
  reference-thumbnail fixture authorization below permits only qualified image
  extraction; it does not authorize fabricated real posts or original attachments.
- The photo's second attachment is shallower than its first. The authorized
  `FeedImage` already exposes media width/height, so intrinsic sizing is possible
  in scoped CSS. The actual two qualified fixture images and their metadata are
  both 1152×896, unlike the reference's differently proportioned wide images.
  A complete image at the current 260px column width needs 202.22px height;
  forcing it to the reference's roughly 61px second strip crops about 70% of its
  vertical composition or reduces it to about 78px painted width with containment.
  Arbitrary second-row sizing would fabricate that distinction. The exact
  [follow-through and media-owner request](../evidence/css-parity-feed/media-geometry-follow-through.md)
  supersedes the earlier imprecise Post-only explanation. No additional CSS crop
  or fabricated media is added. This remains material; green checks and the
  close first-card dimensions do not close full parity.
- Shared titlebar, close control and full-shell composition are Root153's scope.
  Current cute background artwork has a separate owner and is preserved.

## Intrinsic-media follow-up

Further source analysis found one useful CSS correction independent of the
missing wide reference attachments: the fixed figure ratio also forces genuinely
wider images into the same-height box. The candidate replaces that forced height
with intrinsic image height capped at the existing thumbnail ceiling. A scoped
inline-size container makes the ceiling relative to the actual media column;
the image uses `height: auto` and `max-height: calc(100cqi / 2.85 - 2px)`, with the
two pixels accounting for the unchanged figure border. Existing 9:7 previews
retain the current cap, while a genuine 4.2:1 attachment may remain shallower
without cropping or stretching it to fill a taller box. No selected portraits,
background declarations, data, contracts or post-order special cases change.
The figure also allows overflow so an image failure/retry control's keyboard
outline can extend beyond its border. The actual image still stays constrained
to the existing width/maximum thumbnail height.

This candidate passed the later named intrinsic round recorded below. Its
supervisor checks the separate
20 MiB durable and 160 MiB temporary caps as well as the whole-footprint ceiling,
8 GiB continuous free-space floor and bounded active deadline. A fresh named
resource round is requested before
claiming it validated. The new evidence-only `native-intrinsic.mjs` preserves
the qualified original driver and raw reports, repeats actual current Feed/root
checks at 360/390/430, and adds separate neutral SVG geometry probes for wide,
9:7 and portrait proportions plus loading/failure controls. Those probes are
explicitly synthetic layout fixtures, never post data, photographs, approved
product assets or a substitute for the missing real wide attachments. The
current real-fixture second card remains an input mismatch even if that general
CSS correction passes.

## Updated reference-media fixture authorization

The subsequent Main/human direction, relayed directly by Root153, approves the
exact `docs/mcr/design/reference-target-2026-11-04.png` for **fixture-only** Feed
media extraction. The existing artist owns the new
`frontend/public/reference-feed-media-v1/` package: `source-provenance.json`,
`reference-card-1.png`, `reference-card-2.png`, and `extract.py`. Root153 and the
artist will route the qualified source revision, exact crop boxes, dimensions,
hashes and pixel-identity/native-inspection evidence. This CSS worker does not
become an asset writer or extract competing media crops.

Canonical source: 1536×1024 PNG, SHA-256
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`,
Git blob `feeeb7ea637db1ef94d1cbfad970d405e28e1c99`, source revision
`cdcd834f`. It is distinct from the earlier 1280×853 JPEG used in this worker's
whole-panel comparison. The qualified artist package is committed at
`8f6fe07f389f73f09d9fc61a6012b49846b24ce6` (asset postimages unchanged
from `ffab0618d937181c5390bf6adb0661320b183e19`). Card 1 is the native
`[80,704,277,771]` crop, 197×67, PNG SHA-256
`a28db3162d4fe822b17d3228ad82866d4eeb0e2524fb606d469ccd925fd7385d`.
Card 2 is `[80,885,277,931]`, 197×46, PNG SHA-256
`2c5863585bce002be0ea0b057f4927d67c6f99e97cc7f5edee6ba9c47589ffdd`.
The artist reports lossless reopen/native RGB identity and direct inspection
PASS. Their final documentation preserves a 5.17-second resource deadline
overrun; asset qualification does not erase that process exception.
Own normal read-only Git fetch/`ls-remote` now independently agree on master
`145a8e939fb7bede7e46931d6559adb2c51f62a7`, a descendant of the reported
asset publication `5879fc485f226f08ddb84d1b9517cb2cfece198c`. All five
postimages match the qualified artist commit byte for byte, including its
resource exception. This [publication proof](../evidence/css-parity-feed/reference-media-publication.json)
performs no extraction, image inspection, browser or output job and does not
requalify pixels or the still-missing fixture seam.

Each crop is the **complete fixture media input** for both card and detail.
Use true native dimensions, fit down only, and never upscale the display or
invent a higher-resolution original, scene, venue, photographer, date or real
post. Provenance labels belong in artifact/test/harness metadata only, outside
normal product navigation. Production media keeps its genuine authorized
metadata and policy. This approval does not change production TSX/API contracts,
server authorization, real posts, selected character art or original reference
bytes. It supersedes the earlier lack of approved fixture-thumbnail inputs;
it does not make screenshot crops original post attachments.

The draft `frontend/src/features/event-feed/__tests__/css-parity-feed/`
harness is source-prepared, **NOT_RUN**, and preserved uncommitted. Root153
revoked the earlier new-prefix grant to this CSS worker after the explicit
CSS-only ownership correction and took sole ownership of the entire prefix.
This worker has stopped all fixture wiring and will consume only the immutable
external input Root supplies. The draft reuses actual `EventFeed`, `FeedImage`
and `FeedThread`, decodes qualified PNG bytes for real width/height metadata,
and applies native/fit-down sizing solely inside its fixture frame. Both card
and detail consume the exact same complete crop. Synthetic row identity/counts
and qualification text remain explicit harness metadata. No production entry
imports this harness. Its driver must pin the artist commit and both PNG hashes
before synthetic HTTP serves them. The 197px native widths will remain smaller
than a whole reference panel normalized to 390px; that display-policy constraint
must be reported, not hidden by an upscale or passed off as exact parity.

The ready intrinsic-CSS round remains independent of that artist handoff. Once
Root supplies the qualified external media-only fixture, this worker can compare
actual Feed card geometry with those exact thumbnails at 390px and check 360/430px without
post-order styling or invented approved photographs. Older qualified reports,
drivers and input-pin files remain immutable.

The exact preserved draft files are `preview.tsx`, `preview.css`, and
`preview.html` under that transferred prefix. None is committed or validated by
this CSS worker; Root owns completion, integration and qualification.

Full target parity remains open until the browser comparisons and all material
owner/content differences are resolved. A passing fixture run alone does not
close that requirement or the original persistent goal.


## Imported external fixture input and next comparison

Root's immutable three-file source checkpoint
`008248295c120c75c08c8a4bb8fa2ca2c1c6026a` is now imported unchanged,
with the exact published five-file artist package. This is an authorized
read-only test-input import, not an ownership transfer back to this CSS worker.
The original draft's exact three postimages remain preserved in stash commit
`19c793feeaa9889ca3bd4bd093eac0592fd94594`; hashes and source/local commit
mapping are in [the import record](../evidence/css-parity-feed/external-fixture-source.json).
The CSS candidate hash is unchanged by these imports. Root's fixture is still
**NOT_RUN**; any fixture/interface failure goes directly to Root for its fix.

The separate evidence-only `native-reference-media.mjs` is source-prepared,
**NOT_RUN**. A separately declared named allocation must precede execution.
It pins all eight external postimages, compares the published first-pass CSS
with the intrinsic candidate on actual Feed at 360/390/430px, opens both actual
Thread details using the same complete native crops, records card/image geometry
and captures, checks native dimensions/no display upscale/no overflow, checks
unchanged Thread geometry, and exercises compose focus/callback and private
blob revocation. It never changes or patches the Root fixture. The existing
supervisor can select this driver with `--driver reference`; its default
intrinsic allocation and assertions remain independent. Passing either fixture
alone cannot establish full photo parity or a real multiuser flow.

## Handoff

Source checkpoint: `b16133bd1285dd7ba4fef812bee6c4517cf4fc24`.
Validated first-pass CSS/evidence handoff: `654bbb16a045c3ce6306066eb66a36f6c34f471e`.
Root153 published it through `24fdc624` and `333767f3`, contained in
`bb1345b5302eead08c65deab9423e52344fe58fe`. Own fresh fetch and `ls-remote` matched
that master SHA; all 49 delivered file contents and all 12 qualified input pins
match exactly at that publication. [Own remote proof](../evidence/css-parity-feed/publication.json).
Root153 subsequently published the intrinsic-media follow-up at
`416051c984c403a0bd4cede94bd410d027440c30`, parent
`c0ed4243862cd4ec110fa591921e070eabd79c38`, from validated source
`8d39ce8c45e9d2f60065f04ae7e4aeb0fb17c643`. Own fresh fetch and
`ls-remote` agree on that master commit; **all 83 owned postimages** match the
validated source byte for byte, including the unchanged raw Vitest log EOF.
[Intrinsic delivery proof](../evidence/css-parity-feed/publication-intrinsic.json).
Artist assets and Root fixture paths are excluded from this worker's delivery
scope. The later `6543ba4c` selected-tab correction is not part of that published
validation or this receipt. Root153 remains the named publisher; no separate
king/SL review or acknowledgment is required. The original goal remains
unfinished, with BLOCKED metadata preserved; the next reference comparison waits
for its separately declared named allocation.


## Latest allocation preflight

The actual `CSS-FEED-INTRINSIC-202611041043Z` offer was returned **without
ACCEPT**: fresh free space was 17,049,772,032 bytes (15.87884 GiB), below the
required 17,179,869,184-byte starting floor by 130,097,152 bytes. The clean
source was `7483b2fd7f55d1d2fcb483c1f1ad36405d5a65d6`; matching retained
lock/runtime, absent own dependency link, free port 5292 and 3,342,058-byte
carry were observed. No native workload, browser/server, temporary directory,
cache or dependency link was started. [Preflight receipt](../evidence/css-parity-feed/preflight-CSS-FEED-INTRINSIC-202611041043Z.json).

Both original intrinsic and separate reference-media rounds remain ready for
capacity scheduling. A named re-offer and fresh successful preflight are needed;
there is no technical approval, king review or acknowledgment dependency.
The aligned goal remains BLOCKED without changing its objective or asserting
completion; the available goal API has no active/resume status operation.


## Intrinsic validation delivered

The fresh `CSS-FEED-INTRINSIC-202611041121Z` offer was explicitly accepted at
epoch 1793791355.634 after a clean `146c950f` source freeze, 22,347,096,064
free bytes, exact retained lock and absolute Node 24.22.2 preflight. The full
[acceptance record](../evidence/css-parity-feed/accept-intrinsic.json) preserves
argv, cwd, carry and absolute deadlines. No reference-media driver ran in this
round. All 12 prior qualified input pins remain exact except intentional Feed
CSS changes; [new pins](../evidence/css-parity-feed/intrinsic-input-pins.json)
also bind the intrinsic driver, supervisor and shared base stylesheet.

`native-intrinsic.mjs` exited 0 at +9.67 seconds. Actual current 9:7 fixture
media retained previous geometry at all widths: 230×80.6875, 260×91.21875 and
300×105.25 at 360/390/430px respectively. The separate neutral 4.2:1 SVG probe
painted 228×54.28125, 258×61.421875 and 298×70.9375, keeping its natural ratio
without filling the taller ceiling. Existing landscape/portrait caps, contained
44px retry target and visible focus passed. These SVGs are layout probes, never
post media or approved photographs. Actual Feed controls/states, normal and
forced-color focus, privacy blob revocation and the canonical Auth/router/Phaser
fixture also passed. The latter maintained one canvas/socket and disposed both;
it is synthetic service evidence, not a real multiuser claim.

Focused Vitest exited 0: **11 tests in 2 files** (`presentation.test.tsx` and
Thread `ui.test.tsx`), using `--configLoader runner --no-cache --maxWorkers 1`.
Absolute Node syntax and exact background/portrait declaration preservation
passed. [Source boundary receipt](../evidence/css-parity-feed/intrinsic-source-boundary.json).
Active work ended +41.633357 seconds; FULLRELEASE was sent at +71.900519 seconds,
within the 110/180-second limits. Browser/Vite/processes, own link, private
TMP/cache/profile were closed, and port 5292 was bindable. Whole peak was
34,403,285 bytes, temporary peak 29,648,699, durable 4,813,598 before the release
receipt, and minimum measured free space 22,310,961,152. No retries or exceptions.
[Release](../evidence/css-parity-feed/release-intrinsic.json).

Owner read the saved 360/390/430 Feed captures, 390px neutral probes, forced-color
focus, default-root selected-character capture and composer. They confirm the
bounded behavior above and preserve the original synthetic content/background
limitations. Full target parity is still open. Inspection also found a remaining
selected-tab underline: shared `.mcr-tab[aria-selected="true"]` has higher
specificity than the earlier generic Feed text-decoration reset. This requires
a separate scoped CSS correction, to be exercised in the queued reference-media
round. The intrinsic validation above binds CSS SHA-256
`a1cd9c28e90cf05155ae4786a9e749a4655bd881077e950f62af9ddb3a07527e`.


## Selected-tab follow-up awaiting reference round

After preserving the validated intrinsic packet at `8d39ce8c`, the Feed-only
selected-tab selector now explicitly sets `text-decoration: none`. This beats
the shared selected-tab rule without changing shared CSS or focus indicators.
The new reference-media driver asserts the computed selected-tab decoration is
`none` at all three widths. This one-property correction and that new assertion
are source-prepared, **NOT_RUN**, until the separately queued named round.
The immutable intrinsic report/pins above continue to bind its earlier exact
stylesheet, not this later correction. Root received the validated intrinsic
handoff directly for normal publication; no correctness acknowledgment is needed.


## Current delivery mapping

| Scope | Validated source | Published master | Owner proof |
| --- | --- | --- | --- |
| First foreground pass | `654bbb16` | contained in `bb1345b5` | 49 exact postimages / 12 pins |
| Intrinsic media follow-up | `8d39ce8c` | `416051c9` | 83 exact owned postimages; fresh fetch equals ls-remote |
| Selected-tab underline correction | `6543ba4c` plus reference evidence below | unpublished | reference2 PASS; normal publication pending |

The latest remote proof establishes delivery of the tested intrinsic scope only.
It does not prove the pending correction, approved-thumbnail card/detail flow,
full photo parity, live services or real multiuser acceptance. No new runtime,
browser, extraction, test or output job was run for this Git publication proof.


## Qualified reference-media comparison

The independent `CSS-FEED-REFERENCE-202611041215Z` round was explicitly accepted
at epoch 1793794627.255 with clean source
`9d493b3a8c85077c1b832ae2868f3d16d79c258b`, 18,596,544,512 free bytes and
4,876,714 bytes of full carry. The accounting guard now includes owned CSS and
prose source growth in addition to evidence, imported files, private cache and
TMP. Root fixture `00824829` and all five artist files from `8f6fe07f` remained
byte-identical; no fixture, product TSX/API, asset or selected-character change
was made. [Acceptance](../evidence/css-parity-feed/accept-reference.json).

The baseline is the **published first foreground pass** `654bbb16`, not the
later intrinsic publication. Both baseline and current candidate rendered the
exact same eight pinned external inputs at 360/390/430px; both actual Thread
details were opened from their corresponding Feed rows. Every candidate image
has true 197×67 or 197×46 natural/declared dimensions and native rendered size,
with complete crop ratio, no display upscale and no overflow. Candidate figures
are 199×69 and 199×48 including border; baseline figures were both about
198.95×69.80 because of the old fixed ratio. Actual Thread geometry is identical
between baseline and candidate at every width. Selected-tab computed decoration
is `none`; compose callback and private blob revocation/disposal passed.

`reference1` exited 0 at +6.18 seconds, but owner inspection of its values found
an evidence weakness: checking the stored 3px outline width accepted
`outline-style: none` after pointer interaction. That run/log is preserved and
is not the final keyboard-focus proof. During the same live lease, only the own
evidence driver was repaired to navigate Tab/Shift+Tab, assert actual focus and
a non-`none` outline, and save a keyboard-focus capture. No product/fixture CSS
was changed. `reference2` exited 0 at +61.91 seconds: six baseline/candidate
cases, both details at each width, **solid 3px keyboard focus at all widths**,
no page errors. Its own driver hash and tracked driver-only changes are recorded
in the raw report and [input pins](../evidence/css-parity-feed/reference-input-pins.json).
No full build, extra UI test job, server or real multiuser gate ran here.

Work ended +91.298960 seconds and FULLRELEASE was sent at +119.175497, within
110/180 limits. Whole peak 38,140,126 bytes, temporary peak 29,284,281, durable
8,928,173 before receipt, minimum free 18,544,066,560. Zero own Chrome/Node/Vite
processes remained; port 5292 was bindable and own link/TMP/cache/profile absent.
No cap/deadline exception. [Release](../evidence/css-parity-feed/release-reference.json).

Owner read saved candidate Feed captures at 360/390/430, both actual 390px Thread
details and the genuine keyboard-focus image against the supplied reference.
This establishes bounded **MOCK_READY** behavior; full photo parity remains open:

- At 390px the candidate's first card is `(14,108)`, 362×217.390625; the second
  starts at y=330.390625 and is 196.390625 high. Native media at x=100 is 197×67
  and 197×46. The normalized reference visually has roughly 253px-wide media,
  so this fixture cannot fill that normalized footprint under the explicit
  no-upscale/native-only input policy. No higher-resolution original is known.
  The reference-size estimate is an inspection observation, not a pixel gate.
- The selected-character boxes remain 48px and unchanged. The isolated fixture
  has explicit synthetic neutral identities and initials; the prior actual-root
  capture exercises selected character rendering. This run does not certify a
  reference-matching character crop or approve initials as product art.
- Background artwork and full shell/chrome differ and remain separately owned.
  Root fixture Thread intentionally retains existing functional Thread layout;
  unchanged geometry is a regression result, not target styling acceptance.
- A small foreground content-column offset remains: current author/body x=99
  and media figure x=99 are visually left of the normalized reference's text
  around x=105 and media around x=102. That is a remaining CSS alignment slice,
  separate from the protected portrait framing and native-media policy.
- Fixture posts/counts are synthetic. No screenshot-derived identities, higher
  resolution media, live-post provenance, venue/date or real API authorization
  are inferred from this passing fixture.

The full [saved comparison index](../evidence/css-parity-feed/reference-comparison.html)
keeps the original target, first-pass baseline, candidate, both details and focus
separate. Validated CSS SHA-256 is
`e744a26807021b0671e967f1595e6909e47b959a6857864f1081991330777538`.
