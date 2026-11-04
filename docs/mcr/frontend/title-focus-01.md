# TITLE-FOCUS-01 — title focus backing feasibility

Status: **DELIVERED / OWNER-QUALIFIED** for the pinned TITLE-FOCUS-01 cohort.
Implementation, actual validation and remote publication were independently
verified at `4d501d92bc2a00bdce29d33988203a957f790175`; the final audit below
is authoritative. Earlier sections retain the intermediate planning, blocked
and execution states as historical evidence. The raw native exit 1 and its two
forced-color point failures remain preserved with the owner's scoped
qualification. This is implementation-owner evidence, not a king/SL review gate.

## Assignment and ownership

The prior ID01 assignment was delivered and closed. Startup for this task found
no current goal (`get_goal` returned null); a new unbudgeted goal is **ACTIVE**.
It covers constraint feasibility, a compliant correction if possible, meaningful
owned validation/fixes, documentation, committed handoff, Root publication and
allocated actual evidence. It was not narrowed to make this calculation count
as an implemented fix.

Exclusive checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-title-focus-01`.
Branch: `codex/mcr-title-focus-01`. Fresh published base:
`9e7952fcd89360f7b4fe43c69f22060b9aa55c74`. The worker's unclaimed provisional
`codex/mcr-fe-title-focus-01` branch was renamed locally to the required name;
no foreign branch/checkout was overwritten. The old 2251 ID01 checkout remains
untouched. Startup was clean.

Root confirmed the former writer and shared-CSS incumbents stopped before
transferring sole write ownership. The initial application grant was only the existing
`.mcr-chrome-panel-header` rule's `background-color`, `background-image` and
`background-size` in `frontend/src/app/shell-chrome/shell-chrome.css`.
Background position/repeat, new selectors, focus/outline/foreground/text,
TSX, controls, layout, geometry, character/scenery and producer paint changes
were excluded under that initial lease. The later exact selector expansion is
recorded below. Decorative left bars remain excluded. Exact additional owned paths:
this document, `docs/mcr/evidence/title-focus-01/**`, and new meaningful source
fixtures under `frontend/src/app/__tests__/title-focus-01/**` if needed.

The unchanged CSS is 5,451 bytes, SHA-256
`08614a9b3263d7cbb27af4d59aaabc0ee4db07943812faa0e04bdba7f720291d`.
Its bytes also match production `8972b0b6` used for the retained focused-link
evidence. Root remains the sole publisher/capture custodian; Resource21d alone
allocates output checks. No independent browser, server, build, media export,
dependency install or packet copying/restoration is authorized or attempted.

## Uniform backing is impossible

The frozen ring `#a2232b` has relative luminance **0.0905786376**. Its greatest
contrast against any darker backing occurs at black and is only **2.8115728:1**.
Therefore achieving 3:1 requires a lighter backing with luminance at least
**0.3717359127**.

The frozen cream text `#f2eadf` has relative luminance **0.8305059579**. To retain
4.5:1 text contrast, its backing luminance must be at most **0.1456679906**.
These intervals are disjoint. The current dark `#251017` yields **2.4130529:1**
for the ring and **15.1140137:1** for the cream text. A uniform recolor cannot
meet both requirements.

## Spatial paint also conflicts with legitimate unfocused glyphs

Immutable native03 packet commit
`4600542ba8f5d16210075c93847fa60960809540` retains nine actual candidate Map
screenshots: three eras at 360/390/430px. The title ring occupies rows 21–23
and 41–43, plus the three-pixel vertical side strips. The left strip is x20–22
at every width. Actual visible MAP glyphs, including antialias pixels, are
x23–60/y24–40; the focus outline covers the preceding glyph columns.

A snapshot-only symmetric paper frame could seemingly separate those *currently
visible* glyphs from the ring. Paper `#e9e1d4` would give ring contrast 5.7567:1,
with dark backing under the remaining visible glyphs. This model is rejected:
it ignores glyphs that become visible when title focus moves elsewhere.

The actual immutable native02 capture
`docs/mcr/evidence/cozy-backgrounds-v1/native-02/candidate-me-focus-0-390.png`,
commit `5186f11017185a040e4ccde1c59589e140563ef0`, focuses the Edit profile link.
The shared Me title is therefore unfocused. Its full cream glyph cores occur
at x21 and x22, including coordinate **(21,24)** and rows 24–40. The shared
`PanelHeader` uses the same h1, font, unpadded title box and unchanged CSS.

At exactly **(21,24)** the retained focused Map PNG is `#a2232b`, while the
legitimate unfocused Me title PNG is `#f2eadf`:

| Required state | Actual foreground at (21,24) | Required backing luminance |
| --- | --- | --- |
| Focused Map title | Ring `#a2232b` | At least 0.3717359127 |
| Unfocused shared Me title | Glyph core `#f2eadf` | At most 0.1456679906 |

The three allowed background properties supply the same static spatial paint
for that shared header coordinate; they contain no focus-dependent branch.
The same backing pixel cannot lie in both intervals. One actual valid 390px
counterexample disproves a robust all-state/all-width correction of this shared
rule. Top/bottom stripes also fail the vertical ring requirement. A fixed paper
frame would degrade existing text and is not a compliant patch.

The evidence does not invent an unfocused Map capture or unfocused360/430
captures. The counterexample is the actual shared Me title state affected by
the proposed global header rule, not a font approximation. No new left rail,
focus border, glyph mask or product image is proposed.

## Historical source calculation and explicit scope decision

The source reader/calculator is
`docs/mcr/evidence/title-focus-01/feasibility.py`. It reads immutable Git blobs
in memory and uses Pillow for pixel inspection. It installs nothing, launches
no browser and writes no PNG or application source. Its allocated execution
records exact nine-case/report pins, the retained unfocused-title pin,
calculated luminance intervals and unchanged-source assertions in
`feasibility.json`. Output-check allocation and results will be recorded here
before committed handoff. Resource21d offered
`TITLE-FOCUS-FEASIBILITY-202611040825Z`; the owner explicitly accepted the
10-second / 32-KiB allocation. The exact command
`python3 docs/mcr/evidence/title-focus-01/feasibility.py` exited **0** in
**0.278733917 seconds**. Script plus document carry was 13,831 bytes; report
was 7,114 bytes; total was 20,945 bytes. Minimum observed free space was
33,479,512,064 bytes. No temporary files or live owned process group remained;
the allocation was fully released. This is source feasibility, not browser,
accessibility or product acceptance. The script intentionally asserts the
historical unchanged CSS and must be reproduced at this historical commit.

The implementation owner has sent the concrete contradiction directly to
Root153, World SL and the UI lead. Root then explicitly authorized adding only
`.mcr-chrome-panel-header:has(> #panel-title:focus-visible)` with background-color,
background-image, background-size, background-position and background-repeat.
The existing unfocused header remains dark. All foreground, outline, geometry,
layout, TSX, forced-color-adjust and other selectors remain frozen. The former
writer and incumbent CSS writers are confirmed stopped/returned. The same
original goal stays ACTIVE. Root will capture the finite affected focus/unfocus
Map and Me profile-link states at 360/390/430 plus representative forced colors,
under a fresh Resource21d allocation. The historical static result neither fixes
the issue nor certifies the conditional continuation.

## Conditional paint candidate

The continuation adds only the authorized direct-child focus-visible selector.
It declares `background-image`, `background-size` and `background-repeat`.
All other CSS bytes, including the original unfocused header and focus rules,
remain unchanged. Candidate CSS SHA-256:
`5c9e194e62eaf438f8b98436b4e8d7315702ead3307e60684fe4f61800d067c4`.
The earlier `c2f4a488` proposal was
superseded before execution/capture to expose the backing outside the ring.

The three hard-stop gradient layers cover the control zone in dark, paint the
top/bottom ring backing, and paint the two side-ring backings. They use the
existing 8px left padding, three 44px controls and 2px right padding. The native
padding-origin coordinates are x12/y10; the focused ring's retained raster rows
are 21–23 and 41–43. The image size is the header's full padding box. This is
background paint behind the existing symmetric focus outline, not a new border,
outline or decorative rail. It extends one pixel outward from each ring edge:
otherwise opaque red would conceal the entire backing and leave visible
contrast unchanged. The right exposed pixel occupies blank transparent control
padding; control icons/text and the remaining interior retain dark backing.
When the heading loses focus-visible, the selector
stops matching and every backing pixel returns to the original dark color.

No URL images or `forced-color-adjust` opt-out are introduced. The
[CSS Color Adjustment specification](https://www.w3.org/TR/css-color-adjust-1/#forced-colors-properties)
forces non-URL background images to none in forced-colors mode. Thus the paint
defers to the system palette; actual browser forced-color legibility/focus is
still required and is not inferred from that specification.

`check-paint.py` independently parses the authored gradient stops and applies CSS
stop-position fixup. It checks every retained red ring and visible glyph pixel,
all four exposed outer paper edges, the control interiors and unfocused padding
box, the actual unfocused
Me cream-core counterexample, and byte equality outside the exact selector.
It writes only `paint-check.json`; it does not render or synthesize a candidate
image. Formal source-model execution **PASS** is recorded below. Actual
candidate Map focus/unfocus, Me profile-link focus at
360/390/430, forced colors and publication remain **NOT_RUN**.

The frozen checker SHA-256 is
`9aecba785bad64663bdc0d122ab574879523d708394cdcac6bc2db29ae924f77`.
Resource21d confirmed the exact 10-second / 64-KiB total task allocation request
is queued next after the active IN01 browser job's actual FULL RELEASE; that
message was queue placement, not an offer or acceptance.

Resource21d subsequently offered `TITLE-FOCUS-PAINT-SOURCE-202611040841Z`;
the owner explicitly **ACCEPTED**, referencing the preflight timestamp
2026-11-04T08:43:27.858611Z with the exact CSS/checker/input pins, three WIP
paths, 39,204-byte carry and 31,960,649,728
bytes free. The exact command
`python3 docs/mcr/evidence/title-focus-01/check-paint.py` exited **0** in
**0.376160125 seconds**, without timeout. Result:
`SOURCE_MODEL_PASS_ACTUAL_RENDER_NOT_RUN`. Output was **4,626 bytes**; all
assigned durable source/scripts/docs/proofs/report totaled **43,830 bytes**
within 64 KiB. Observed free floor: **31,821,676,544 bytes**. The owned process
group was absent, with no temporary files or app processes. The job completed
at 2026-11-04T08:43:55.558450Z; its result and FULL RELEASE were reported in
the next coordination call.

All nine retained Map cases passed the authored-gradient model, including every
retained red ring/visible glyph pixel, four exposed outer paper edges, dark
control interiors, full unfocused dark padding box and the actual Me cream-core
counterexample. All source outside the exact leased selector is byte-exact.
There is no API/contract, cross-lane import, dependency, artwork or geometry
change. The original source comes from the published base above; all added CSS
and checker logic are newly authored for this assignment.

This result does **not** prove actual CSS parsing/rasterization, selector/focus
transitions, native/browser geometry, forced colors, live/multiuser behavior or
design/accessibility acceptance. Those targeted runtime checks and Root
publication remain open; the persistent goal remains **ACTIVE**.

## Delivery and actual-evidence continuation

Owned committed sequence: historical proof
`f0d587c1430b9ee7b051c6f736d8ae8fd3496842`, then conditional implementation
`c7f5f654d1da859ee9cbca7a6107eb5e3c2b882d`. Root integrated them respectively
as `df0769e5` and `2ea7fd4e07aebb8b4d58938e8cba9d378c5fb855`.
The implementation owner read back the integrated CSS SHA-256 and confirmed
the expected `5c9e194e` bytes; the proof integration is an ancestor of the
implementation integration (`git merge-base --is-ancestor`, exit 0).
These are integration facts, not remote publication or runtime acceptance.

Root authored combined focus driver `f8de2622`, currently **NOT_RUN**, and
requested a fresh 180-second Resource21d allocation after the archive job.
The bounded cohort covers before/after Map, Me and the separately owned Meet
fix at 360/390/430 plus representative forced colors, focused/unfocused glyphs
and the existing Me profile first-link focus. Root owns that capture checkout;
this developer owns interpretation, title-paint fixes and final documentation.
No independent duplicate browser/native cohort is launched here.

## Immutable focus-04 packet and resumed owner qualification

The earlier resource dependency persisted across three no-progress goal turns;
the original goal was truthfully marked **BLOCKED**, with Root/UI notified and
all implementation requirements intact. On the actual packet handoff this same
context resumed. `get_goal` still returned **BLOCKED**; its objective, identity
and metadata were preserved, with no replacement goal or false completion.

Custodian packet commit: `4591f927023d8fb5f5afa3e8d2f11a501543cae4`.
Canonical files remain in Root's checkout under
`docs/mcr/evidence/cozy-backgrounds-v1/focus-04/`; nothing was copied or restored.
Candidate: `cadf860dd928d58df7f1e8f26a0b4e8dddd03911`.
Executed driver SHA-256:
`f3d7db08efcce14a3f507113b6c6f3f556b0eb35385b2525515e78244b2bcc2d`.
Raw report SHA-256:
`795dadfdaecfafccea1a50242cbb2f14fe290d5b9e7d117224731b1dbf8aeb21`.
Capture packet SHA-256:
`c489b856143bc728bb02ed48ef59e2f16fc64d33ed030662bce8daa8ceed768d`.

The immutable run retains native **exit 1**, raw `OPEN_FAILURES`, and exactly two
candidate Map/Me390 forced-color assertions. At (21,32), a system stroke over a
black title glyph changes black to (4,0,58), giving 1.0662601:1; this sample is
also present in the baseline. The other three measured points are against white
Canvas. No raw artifact, return code or failure is relabeled by owner analysis.
The four prior source-check commands exited 0, including 15 tests in three files,
TypeScript, lint and syntax. The native job ended at 18:04:00.512510 KST after
63.915931 seconds; its guard records temporary/link cleanup. These custodian
records still require the independent pin/readback check below.

Original candidate Map forced390, Me focused390 and Me profile-link focused390
PNGs were visually inspected in place. The forced title is black on Canvas with
a visible blue outline. Normal title focus shows the symmetric exposed paper
surround; moving to Edit profile restores the original dark title backing. A
single low same-pixel glyph-overlap measurement does not establish invisible
focus, and does not justify changing frozen foreground or geometry.

The adapted `check-native.py` preserves those exact raw failures and independently
checks all declared source/asset/artifact/PNG hashes, normal title states, Me link
focus, frozen geometry/characters, forced header RGBA identity before/after,
all four outer edges against adjacent Canvas, and qualifying stroke area. Its
area calculation excludes pixels whose contrast against the suppressed-stroke
probe is below 3:1. [W3C Focus Appearance guidance](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html)
defines a rectangular two-pixel perimeter area as `4w + 4h`; the checker also
requires exceeding the larger complete exterior-band area. The probe retains
focus while suppressing only the stroke; it is not an actual forced-unfocused
capture, and this analysis makes no full AAA/WCAG conformance claim.
[W3C Non-text Contrast guidance](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast)
separately discusses adjacent-color contrast and whether the remaining indicator
still communicates its state. Actual measured adjacent-edge and area results
will be recorded after the allocated check, not inferred from the raw four points.

Fresh Resource21d owner-readback allocation is queued after C07's pinned runtime
job. The command will read immutable Git blobs in memory and write only
`native-check.json`; no new rendering, dependency, temporary packet or recapture
is requested. Formal owner execution is recorded below. Exact checker
pin and all carry were frozen at explicit acceptance. Publication and own remote
readback remain required; no main/King/SL technical review or ACK is a gate.

## Allocated actual readback and owner decision

Resource21d offered `TITLE-FOCUS-PACKET-READBACK-202611040916Z`; the owner
explicitly **ACCEPTED** with preflight at **18:17:44.249184 KST**. Frozen
checker SHA-256:
`3fceab75adbfc80c4e1008807f776a023c6e9baa09edc79c147f86aafc8ddfe1`.
The own branch was `codex/mcr-title-focus-01`; only the owned reader and this
document were modified. CSS remained exactly `5c9e194e`. All carry was
**63,032 bytes** and initial free space **19,496,456,192 bytes**.

Exact command:

```sh
python3 docs/mcr/evidence/title-focus-01/check-native.py 4591f927023d8fb5f5afa3e8d2f11a501543cae4 docs/mcr/evidence/cozy-backgrounds-v1/focus-04
```

The owner readback exited **0** in **1.369341292 seconds**; it completed at
**18:17:45.871142 KST**. The 25-second child / 30-second total guard did not
fire. Continuous observed free floor: **19,493,642,240 bytes**. The only new
output was `native-check.json`, **5,011 bytes**; all assigned durable content
totaled **68,043 bytes** within the 96-KiB allocation. The owned process group
was absent. FULL RELEASE was sent in the next coordination call; no browser,
server, temporary packet, cache, dependency, render or copy was created.

Independent pin checks passed for **53 artifacts**, **688 declared source/asset
pins** representing **346 unique immutable blobs**, and **43 original PNGs**.
The original raw native report remains `OPEN_FAILURES`, exit **1**, with exactly
the two forced-color point failures. This owner result is separately labeled
`OWNER_QUALIFIED_WITH_RAW_NATIVE_EXIT1_PRESERVED`.

| Assigned actual behavior | Owner evidence/result |
| --- | --- |
| Map and Me focused title, each 360/390/430 | All six rings 5.7567446:1 against backing; exposed paper present on all four outer edges |
| Visible title glyphs, focused and unfocused | All six cases 15.1140137:1; cream foreground unchanged |
| Title focus leaves | All six shared headers restore original dark backing; actual Me (21,24) cream core preserved |
| Me Edit profile link, each 360/390/430 | All four strokes visible; minimum 4.8096892:1 at360 and 5.6373754:1 at390/430 |
| Outline, layout, controls and characters | Frozen title foreground/outline geometry exact; normal geometry, text/control states and character records match baseline |
| Forced Map and Me390 | Black heading, Canvas backing, all three non-URL gradient layers suppressed; header RGBA pixel-identical to baseline |

For both forced titles, **all four outer edges** against immediately adjacent
white Canvas have minimum **11.3097746:1**. Each has **1,446 changed stroke
pixels**. Excluding every pixel below 3:1 against the suppressed-stroke probe
leaves **1,384 pixels for Map** and **1,391 for Me**. Both exceed the **988px²**
two-pixel perimeter reference and even the larger **1,004px²** complete exterior
band. The low black-glyph sample remains **1.0662601:1** and is not counted as
qualifying area. Its baseline/candidate sample values and full header pixels
match exactly.

**Owner decision:** the assigned normal focus/backing/glyph/focus-transition
behavior is qualified. The forced-color point failures are an inherited
glyph-overlap limitation of the four-point minimum assertion, not evidence of
a missing or newly degraded visible indicator. Actual adjacent-edge contrast,
remaining qualifying area, black-on-Canvas title and before/after pixel identity
support the assigned visible-focus requirement. No product CSS repair or new
capture is justified. The immutable failure is preserved, not converted to a
green native run. The area probe is not an actual forced-unfocused capture;
this is neither full AAA/WCAG certification nor broad design/live/device
acceptance. Meet remains separately owned.

The three original390 images identified above were visually inspected in place.
When additional selected originals were requested, their former physical Root
path was no longer materialized; the immutable Git blobs remained present and
all 43 PNGs were independently verified in the allocated readback. No packet
was restored/copied to work around that change. Publication and exact own remote
proof are the remaining delivery boundary.

## Final publication audit

Root published `master` at **`4d501d92bc2a00bdce29d33988203a957f790175`**.
The implementation owner independently ran fresh `git ls-remote origin
refs/heads/master` and `git fetch origin master`; both exited **0**, and the
advertised remote SHA matched that delivery exactly. The own checkout was clean.

`git diff --exit-code` between own validated handoff
`81b11df97bfda92f3d48b8c18f76ee96a710d762` and that remote delivery exited **0**
for all **eight owned postimages**: `shell-chrome.css`, this document and the six
reader/result files under `docs/mcr/evidence/title-focus-01/`. The entire
`docs/mcr/evidence/cozy-backgrounds-v1/focus-04/` subtree also compared byte-exact
against original seal `4591f927023d8fb5f5afa3e8d2f11a501543cae4` with exit **0**.
Thus the independently verified 53 artifact, 688 declared source/asset and 43 PNG
pins, including the raw native failure and resource record, are retained on the
remote. No packet was materialized to perform these ordinary Git checks.

Every `git merge-base --is-ancestor` check below exited **0** against the
published delivery:

| Evidence | Remote ancestor |
| --- | --- |
| Historical static-lease proof | `df0769e5` |
| Validated conditional background implementation | `2ea7fd4e07aebb8b4d58938e8cba9d378c5fb855` |
| Immutable actual capture seal | `4591f927023d8fb5f5afa3e8d2f11a501543cae4` |
| Integration/provenance documentation (`53d5e6f3`) | `34da4a29` |
| Initial immutable packet reader (`e1db0e3e`) | `01ed0a23` |
| Executed owner qualification and result (`81b11df9`) | `9a32a98a` |

The completion audit covers the original bounded requirements: exclusive
ownership and explicit expanded lease; static feasibility proof; the compliant
conditional paint with frozen foreground/outline/layout and unchanged unfocused
state; meaningful source and actual 360/390/430 validation; honest forced-color
qualification and failure retention; scoped committed documentation/handoffs;
and exact remote postimage/ancestry proof. No assigned implementation or
validation fix remains. No broad design, device, live/multiuser or full WCAG
conformance is inferred from this cohort.

Application CSS ownership was already STOP/RETURN to Root for subsequent work.
This audit is the sole documentation follow-up to the verified delivery; its
publication and final document readback require no application test/capture
rerun. The owner returns the remaining task documentation/evidence leases after
that readback and records actual goal completion, without creating a new goal
or treating publisher notification as technical approval.
