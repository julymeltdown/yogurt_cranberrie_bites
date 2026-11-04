# UI background integration coordination

Date: 2026-11-04 (Asia/Seoul). Status: **IN_PROGRESS**. Background adoption,
new native comparisons and publication remain outstanding. This record assigns
coordination; implementation owners perform all technical and visual validation.

## Human direction and preserved boundaries

The latest human instruction requires noticeably cuter backgrounds than both
the current app and the supplied ten-screen collage. Use warm, soft, playful
scenery and surfaces. Dominant surfaces and large shapes must read as cuter at
a glance in an actual 390px comparison; tiny bows/hearts on an unchanged dark,
distressed field do not satisfy the direction. Keep readable quiet centers.
Preserve characters exactly, including selected source
bytes, pixels, atlases, crops, framing and renderers. Earlier portrait proposals
are withdrawn. The ten-screen layout, typography, controls, routes, truthful data
and authorization remain constraints. No decorative left accent lines.

Current human collage: attachment `120DC5EC-1683-42AB-AF36-0DA8EB43CDE0/1-Photo-1.jpg`,
SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
The newer direction changes background treatment; it does not authorize a
character redesign, fake content, physical venue guidance or geometry changes.

## Coordination and delivery ownership

| Responsibility | Owner / chat | Boundary |
| --- | --- | --- |
| UI adoption coordination | UI SL `01a1a533-0644-7ef0-9ee8-9f1755817fb9` | Only this document; branch `codex/mcr-sl-ui-background-integration`, isolated worktree `2a57/mcr`. No implementation inspection, tests or approval gate. |
| FE/art integration and master publication | Root153 `01a1a153-1227-7e72-801f-a2a6b91981e1` | Exact shared hooks, imports, source grants, combined developer validation and publication. |
| World/Map art coordination | `01a1a531-55d5-7572-8c50-58626b12b509` | Source/consumer handoffs; [lane record](art-world-background.md). |
| Screen art coordination | `01a1a531-d994-7100-909b-112e1215d8cb` | Asset producers and background stylesheet adopters; [lane record](art-screen-background.md). |
| World/Map functional context | `01a1a1fe-c276-7eb3-a36e-2f383c0e81b8` | Existing World, Map, FE18, FE32/FE33 ownership and exact seam allocation. |
| Identity/social functional context | `01a1a1fe-ca12-7cc3-9a1d-62bbb2689498` | Entry, Customize, Profile and Feed functional boundaries. |
| Community functional context | `01a1a1fe-c5f1-7e60-87c2-43785b07f941` | Meet/Gifts and community dependency handoffs; functional owners retain WIP. |
| Browser/native/output resource serialization | 21d `01a1a21d-39ec-7912-84cb-910058e7a110` | One named job at a time; leads launch no duplicate capture or export job. |

The UI coordination goal was actually created ACTIVE and unbudgeted in this chat.
Worker goal states below are owner reports, not an inference from dispatch.
All assignments retain implementation, relevant validation, documentation,
committed handoff, publication readback and required fixes as completion criteria.
Preserve the user's model/reasoning settings and communicate in English.

## Source producers and proposed consumption roster

No proposed stylesheet or existing consumer seam becomes writable implicitly.
Root and the relevant functional owner record exact paths, selectors, properties,
branch/base and previous-writer release before edits. Source asset grants below
are separate from application source rights.

| Task / screens | Implementation owner | Exclusive source scope / adoption state |
| --- | --- | --- |
| ART-BG-MAP-01, W02 Map and W01 shared source | Artist231 `01a1a231-0dc4-7b50-92bd-c0f0f56bd970` | Existing `frontend/public/world/reference-scene-v2/**` excluding `runtime/**`, plus existing `docs/mcr/art/reference-scene-v2.md`. Exact new ten-file `map-cozy-v1` source/doc grant and media allocation confirmed. Owner reports generation starting from matching `7c25078f` baseline with 40.6 GiB free. Earlier BLOCKED goal history is preserved, not falsely completed. |
| ART-BG-MAP-02 / ART-BG-WORLD-01, W02/W01 adoption | Root153 | Granted new closed asset adapter/compiler/source/pins plus exact `spatial-v5/model.ts` descriptor-tuple wrapper in World record `9153d3e5`. Original loader/overview/Phaser edits were withdrawn unwritten. Same atlas IDs, 34 World rows, 51 Map rectangles, labels, frames, geometry and characters. Actual ACTIVE goal reported by Root; new image handoff pending. |
| Entry asset production, A01/A02/U02 | `01a1a534-4fcf-7291-abe3-a9af7b895a45`, worktree `ac70` | Source-ready `9a962ea6`, same full `bfa01939` base, exactly ten new asset/doc paths. Five editable deterministic SVGs total 13,817 bytes; manifest pins URLs/hashes/bytes/viewBoxes/alpha. Owner reports source/contrast/plan/diff exit 0; actual render/publication NOT_RUN. Existing application CSS read-only; consumed by committed ID01 CSS. |
| Community asset production, F01/M01/G01 | `01a1a534-4fcf-7291-abe3-a97ba2fcafcd`, worktree `bf97` | Committed source handoff `2a5c67e5`, branch `codex/mcr-art-cute-community-v1`, base `bfa0193939c3eebd6c80ade85083c6530c6875e1`. Exactly nine new asset/doc paths. Manifest pins broad 390×640 SVG backdrops, paper tile and optional corners. Owner reports source/contrast/plan/diff checks exit 0; browser decode/visual NOT_RUN. Existing application CSS read-only; consumed by committed COM01 CSS after Root URL grant. |
| Wall/Me asset production, R01/U01 | `01a1a534-4fd0-7ba1-8fe8-4cc95674080f`, worktree `12a1` | Source-ready `8ff3a58f556bf2811ad1415cd31c5ba1953ccd58`, only new `frontend/public/backgrounds/cute-wall-me-v1/**` and `docs/mcr/art/cute-wall-me-backgrounds.md`. Three SVGs; owner handed exact URLs/dimensions/hashes/provenance to Root/ID01/COM01. Shared decode, truthful Wall guard and Me captures remain pending. Existing application CSS read-only. |
| ART-SCREEN-ID01 background adoption, A01/A02/U02/U01 | `01a1a533-50a8-7fb2-b3ed-96b941b96bbf`, worktree `2251` | Committed `e44b5d1c`, branch `codex/mcr-art-screen-id01`, base `7c25078f118da53dd9a524d0a58e707cca190f0f`, clean; ACTIVE goal. Exactly five new CSS/doc/evidence paths, all four routes, 18 granted selectors and seven pinned Entry/Me asset references. Developer reports scope/pin/inert-SVG/diff checks exit 0; render NOT_RUN. U02 and decorative paint grants received. Root owns import/publication. |
| ART-SCREEN-COM01 background adoption, F01/M01/G01/R01 | `01a1a533-50a9-73a1-8d4f-a16bf0b69aee`, worktree `06af` | Committed `8824401d`, branch `codex/mcr-art-screen-com01`, same full `7c25078f` base; ACTIVE goal. New cozy community CSS plus task documentation/evidence adopts pinned Community and Wall sources. Developer reports scope/asset-pin/plan/whitespace checks pass; render/publication pending. Root owns import and Wall semantic hook. |
| Withdrawn additional World source assignment | `01a1a534-4fd0-7ba1-8fe8-4c89834e0c9a`, worktree `ddbc` | Latest King/Root/World art reconciliation: STOPPED before source/export/browser writes. No separate `cute-backdrop-v1` pack or import. Preserve its unfinished goal without claiming completion. Artist231 is the sole cozy Map and World source author because both consume the same scenery tuple. |

The King confirmed complementary staffing: the three newer screen chats produce
assets and the two earlier chats adopt those assets through the two new scoped
sheets. Both developers accepted the exact grants and committed their source
handoffs; U02's later exact grant now covers all four identity screens. The newer additional
World source assignment was subsequently withdrawn before writes because one
atlas/ground tuple serves both World and Map. Earlier existing-CSS
candidate rights for asset authors are withdrawn. Do not create another
writer roster. Each asset output needs an actual named consumer; avoid parallel
unused artwork and independent CSS work that never adopts the source handoff.
All three producer-to-consumer connections and committed source handoffs are ready.
Source delivery is not in-app adoption or visual acceptance.

Social SL reports no current product-source claim to A01/A02/U01/F01 or either
new cozy stylesheet. Community also confirms no incumbent or reservation for
the new cozy community sheet. Earlier alternate community stylesheet proposals
are superseded. Former source leases have returned. World has now given explicit
U02/FE13 functional no-claim for outer window body/footer background color/image
in Motion/Whole/Avatar editor wrappers (record `9153d3e5`). Root subsequently
granted the exact U02 hooks and decorative paint replacement; ID01 reports them
implemented in its committed four-screen handoff. ID01 and COM01
have preserved their read-only preparation clean at `bfa01939`, with ACTIVE
goals and no source edits or workflows before the grants. Root's subsequent
grant permits only `background-color`, `background-image`, `background-position`,
`background-size`, `background-repeat` and `background-blend-mode`. Existing
decorative web/cemetery replacement required a separate exact grant, which
ID01 now reports received. Original SVG containers, character paint, text,
controls and layout remain frozen under that paint-only assignment.

## Ten-screen adoption and evidence checklist

| Screen | Asset handoff | Required consuming hook / current missing dependency |
| --- | --- | --- |
| Landing A01 | Entry producer to ID01 | Committed ID01 adoption; Root reports app imports wired, actual comparison pending. |
| Sign in A02 | Entry producer to ID01 | Committed ID01 paper/decorative paint adoption; Root reports app imports wired, actual auth-state comparison pending. |
| World W01 | Artist231 to Root | Exact shared descriptor-tuple grant; packed source ready for commit and ground export; actual adoption/comparison pending. |
| Map W02 | Artist231 to Root | Same shared atlas/ground tuple as World; source grant resolved, actual adoption/comparison pending. |
| Customize U02 | Entry producer to ID01 | World functional no-claim and Root exact grant received; ID01 reports outer paper adopted in `e44b5d1c`. Actual public wrapper rendered capture remains pending. Character preview remains frozen. |
| Feed F01 | Community producer to COM01 | Committed COM01 backdrop adoption; Root reports app imports wired; portraits/media excluded. |
| Meet M01 | Community producer to COM01 | Committed COM01 backdrop adoption; separate 204 typography delivery grants no background rights. |
| Gifts G01 | Community producer to COM01 | Committed COM01 backdrop adoption; actual stock/actions/guard preservation comparison pending. |
| Wall R01 | Wall/Me producer to COM01 | Root granted `.shell-panel[data-mcr-panel='wall'] > .shell-panel-body` background styling and owns the new semantic attribute on the existing main element. No new element/layout. Mural canvas/pixels/palette behavior excluded. |
| Me/Profile U01 | Wall/Me producer to ID01 | Committed ID01 backdrop adoption; actual full-character invariant evidence pending. |

Developer204 `01a1a204-c583-7670-965a-4891b8b1289e` remains ACTIVE through Meet
typography publication/readback, with handoff commits `e991` + `0d0b` as reported
by Community SL. That assignment grants no background edits. Community reports
FE32's bounded delivery complete with leases returned; remaining scope is not
removed. Existing FE18/FE32/Map owners must not be reassigned implicitly.

## First comparison, then complete coverage

Immutable first actual Map BEFORE: candidate
`6ca52f8851473090479f1c13a33e879b6cd099d0`, captured 2026-11-04 15:05:14 KST,
390px, synthetic fixture, at
`/Users/lhs/Desktop/mcr-worktrees/fe-reference-root/docs/mcr/evidence/reference-scene-v5/map-service-v1/native-01/map-parade-390.png`.
It is historical before evidence, not a cozy-background result.

First World/Map AFTER: **NOT_RUN**. First actual Landing AFTER exists at 390px;
the other seven panel pairs are running in the same finite job. 360/430px
background comparisons remain **NOT_RUN**.
No background visual acceptance or all-ten adoption is claimed.

First actual pair, job `PANEL-COZY-202611040632Z`, candidate
`c8a731e5c7f03c3fbcbd0581c6e872429806fa22`, actual public Landing under the QA06
synthetic fixture at 390×844:

- Before: `/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-01/baseline-landing-390.png`
- After: `/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-01/candidate-landing-390.png`

Root reports exact text, visible image source/rectangle/RGBA and control
rectangle/type/color/state comparisons passed, with no character-source path
changes. Its reported capture shows the rose field and large cloud/moon forms.
The Entry producer received the actual pair, and Screen SL has routed it to the
CSS owner for developer-owned composition/contrast comparison and fixes. This
first pair is neither all-eight coverage nor visual acceptance by this SL.

Current owner activity: Root reports both screen sheets imported and the Wall
semantic hook wired, with all 18 identity selectors and seven asset pins passing
source checks. Root's adopted publisher source `a6633eef` passed owner-reported
74 tests in six files, full TypeScript and scoped lint; source processes released.
Capture-driver candidate `c8a731e5c7f03c3fbcbd0581c6e872429806fa22` is READY in
21d's queue and now running: 16 captures, eight actual routes × baseline
and candidate at 390×844, with the landing pair first. The baseline restores only
four paint-integration files to exact `7c25078f` bytes; other candidate/fixture/
Meet typography is identical for both halves. Planned output is
`fe-delivery-publish/docs/mcr/evidence/cozy-backgrounds-v1/native-01` (Landing pair
exists; full report pending). Driver includes character/source/RGBA/rectangle and text/control
comparison. Remote reconciliation remains pending. Artist231
reports packing complete, a 1.20 MB atlas, unchanged 16 frame rectangles and
baselines, and exact RGBA bytes preserved for the two original baked-text frames.
The private media output job is released. Source commit, pinned ground-SVG export
and native World/Map result remain outstanding. These are developer reports,
not an SL implementation or visual review.

Shared shell/header background paint owner213
`01a1a213-61d6-7a20-a8e7-476649312f8c` reports committed `b4bade63`: three exact
allocated CSS background declarations, warm berry headers and peach shell,
reported header contrast 7.31:1. Its perimeter selector remains unchanged because
it was outside the grant; the foreground footer artwork replacement is withdrawn.
Root must integrate these and the screen/source handoffs before a combined
application result is claimed. Owner validation and actual captures remain open.

Latest scheduling correction from King and 21d supersedes the earlier first
World/Map cue requirement. Once Root adopts a pinned ready panel/entry cohort
and owner source checks pass, request one finite actual 390px in-app comparison
promptly, adding other ready screens in the same bounded job if cheap. Do not
wait for the World/Map sheet or all ten screens. World/Map uses a later single
slot after its actual tuple is integrated. Remaining affected 360/390/430px and
necessary era coverage stays required. All workflows remain serialized under
21d's named resource leases. Individual art developers consume each immutable
Root packet and fix their own sources; they do not reserve separate browser jobs.
Record candidate SHA, changed
paths, asset hashes, exact viewport, route, era, fixture/live mode, timestamp,
commands/exit codes, findings and fixes. Preserve originals and before captures.
Use consistent scenario/viewports for meaningful comparisons; record differences.
Background source preparation and ready panel captures proceed independently
of World/Map source integration, without a full-wave barrier. Current exclusive resource offer is
`ART-COZY-SOURCE-202611040621Z` for Artist231's one environment sheet and bounded
packing, with no browser or characters. Root's later READY request names the
committed app/assets, cases/commands and measured resource estimate after actual
source-job release; there is no concurrent browser during that job. 21d proposed
total 0.2 GiB, temporary 100 MiB, durable 30 MiB,
start at least 16 GiB free and stop at 8 GiB; a larger job needs a concrete bound.

Owners validate real FEED/WALL/MEET/GIFTS navigation and back behavior, one
persistent Phaser instance/connection, background loading/failure/cleanup,
contrast, 44px targets, keyboard/focus and responsive clipping. Character
invariance needs owner evidence; source claims alone are insufficient. Existing
text remains readable by adjusting backgrounds within scope. Mocks establish
only their bounded result; device, real-service and multiuser gates remain open.

## Handoff and publication

Each owner hands Root the exact base and committed SHA, changed paths, source and
contract revisions, provenance/hashes, validation commands/exits, evidence and
remaining limits. Root publishes the validated FE/art scope to
`julymeltdown/mcr` `master` with explicit staging, reconciled remote history and
verified ancestry. No force push and no wait for UI/art/functional SL approval.
Leads track reported behavior, commit progress, current goals and actual activity;
they route fixes to implementation owners without repeating their checks.

This document's delivery does not complete the active integration goal. All ten
background adoptions, required comparisons, publication evidence and fixes must
be finished before the goal can be completed.
