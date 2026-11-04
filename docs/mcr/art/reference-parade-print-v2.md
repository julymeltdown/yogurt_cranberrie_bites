# Shared parade print — ART-PARADE-PRINT-02

Status: **INTEGRATED — refined source and bounded native art inspection PASS;
screen reference differences remain; final publication/readback pending**.
This original decorative print is for the shared lower band in Meet, Giveaway
and Profile. Source validation and direct source-preview inspection pass. The
functioning consumer and retained native comparison are now recorded below;
art refinement, component-owner flow disposition and final publication remain
required. A rendered source asset is not an application screenshot.

## Ownership and base

The previous Wall/Me background assignment is complete and its leases were
returned. The new persistent goal was created only after inspecting the actual
goal state, which had no unfinished goal. Isolated source checkout:
`/Users/lhs/.codex/worktrees/12a1/mcr`; new task branch
`codex/mcr-art-parade-print-v2`, based on published master
`654974128a27c948a449fa9e58f466b8e612361d`. The prior source branch and commits
are preserved.

This author writes only:

- `frontend/public/reference-parade-print-v2/**`
- `docs/mcr/art/reference-parade-print-v2.md`
- `docs/mcr/evidence/art-parade-print-v2/**`

Consumer `01a1a213-61d6-7a20-a8e7-476649312f8c` owns the qualified
ParadeFooter/frame composition on `codex/mcr-fe-reference-chrome-v2` in its own
checkout. Root153 is the final binding/publication custodian. No per-screen
CSS, shared chrome source, character/member portraits, avatar derivatives,
World/Map/background atlases, central manifest, dependency or lockfile is edited
here. `docs/source/` remains untouched. There is no king/SL approval stage.

## Reference measurement and provenance

Both references were directly viewed and measured read-only. No source pixels
or contours were traced, imported or copied into the new artwork.

| Reference | Size | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Canonical `docs/mcr/design/reference-target-2026-11-04.png` | 1536×1024 | 2967126 | `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617` |
| Supplied `AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg` | 1280×853 | 482737 | `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23` |

The supplied file is retained at its original temporary attachment path under
`/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/`.
Its exact location, the six measured regions, row ink proxies, bounds and method
are recorded in [reference-measurements.json](../evidence/art-parade-print-v2/reference-measurements.json).
Bounds use exclusive right/bottom coordinates and approximately 3px manual edge
uncertainty. These are inspected lower-print regions, not semantic segmentation
or a claim of exact whole-screen layout equivalence.

| Canonical surface | Reference region `(left,top,right,bottom)` | Region height at illustrative 372px inner width |
| --- | --- | ---: |
| Meet | `(316,894,592,1011)` | 157.70px |
| Giveaway | `(620,927,906,1011)` | 109.26px |
| Profile | `(1233,925,1523,1011)` | 110.32px |

Meet includes an overlapping Create Party control; its dark-ink proxy cannot
be interpreted as isolated ornament mass. Canonical Giveaway/Profile dark-pixel
proxies are approximately 52.1%/44.6%, including paper grain. Source raster
coverage is a different measure: 52.88% alpha-weighted ink at 2×. These figures
guide composition density; they are not a pixel-match score.

The shared 360×96 source yields a 99.2px band at 372px width and allows the
reference's taller figures and substantial ground. Its 720×192 2× raster
therefore exceeds the default 720×140 ceiling for this measured reason.
Root153 and the consumer confirmed this source size; actual placed height,
CTA clearance and scroll behavior remain the consumer's validation duty.

## Original editable composition and asset interface

The following composition/table and original export observations describe
the preserved b68 source revision. The qualified successor and current public
postimages are recorded in the final section; old evidence remains unchanged.

The SVG is newly hand-authored vector source: nine independently drawn fictional
marchers, fifteen paths, varied leaning torsos, stride lengths, hats/hair,
split coats, a reed instrument, bass drum, trombone, side drum and a small
pennant. Each figure has its own contour; transforms position those distinct
figures and do not clone one person. A ragged ground and transparent print
nicks join the feet. It is static ornamental ink and does not depict actual
party members, public identities, selected-avatar movement or physical arrival.

| Public file | Dimensions | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `/reference-parade-print-v2/parade-print.svg` | 360×96; viewBox `0 0 360 96` | 5875 | `341c27a80e3988c9665b1ca8ea7579a750848bc304fd34d0f4ebf6ec039c9c9a` |
| `/reference-parade-print-v2/parade-print-preview.png` | 360×96 RGBA | 11240 | `1298e000cfadc385f184d13210d516d7c204ff941c5a0273a0f72f98e270b826` |
| `/reference-parade-print-v2/parade-print@2x.png` | 720×192 RGBA | 25153 | `99419c620afb19b8b511d310125c8e03e34f1d94427489c008d9199f1284f1c6` |

Root ID: `parade-print-v2`; self-contained ink group ID: `parade-print-ink`.
The consumer chose the standalone SVG as a native `img` with `alt=""` inside
the existing `aria-hidden` noninteractive footer. No symbol injection, external
SVG reference or document-global ID is required by that interface. Detailed
dimensions, asset hashes, alpha/color policy, references and example width
calculations are in the local asset manifest
[`placement.json`](../../../frontend/public/reference-parade-print-v2/placement.json).
This is not a central asset-contract or character manifest.

Ink is `#191817`: SVG root `color` and group `fill="currentColor"`. The paper
rectangle is absent, including behind the drum ring and ground nicks; both PNGs
have genuine alpha. The ground ends at y=96 and its irregular upper edge is
roughly y=72–84. Source edges intentionally crop the first/last silhouettes;
the primary integration should show the complete 360:96 composition in normal
flow without stretching or extra crop. Illustrative viewport/inner-width/
height calculations are 360/342/91.2, 390/372/99.2 and 430/412/109.87px. These
are predictions, not measured native rectangles or a CSS grant.

The source contains no font, image, script, CSS, filter, animation, external
URL, player asset or dependency. The footer must remain decorative and outside
the action tab order, with working actions, scrolling and normal error states
preserved. Asset failure does not authorize a legacy/character-art fallback.

## Source validation and direct inspection

Commands run in the assigned checkout:

These initial commands were source-authoring/export checks. They did **not**
run under a named Resource21d offer and do not supply an ACCEPT/deadline/FULL
release receipt. No browser, app server, application build or dependency install
was started. Root153 requested an explicit resource qualification receipt after
intake. That subsequent allocated revalidation is recorded separately below;
the initial evidence is not retroactively relabeled as allocated execution.

| Command | Exit | Meaning |
| --- | ---: | --- |
| `python3 docs/mcr/evidence/art-parade-print-v2/measure-reference.py` | 0 | Hash both references; measure six inspected regions without modifying originals |
| `python3 docs/mcr/evidence/art-parade-print-v2/render-assets.py` | 0 | Validate inert SVG, export PNGs and inspect alpha, dimensions, bounds, RGB and hashes |
| `git diff --check` | 0 | Source/document whitespace validation; not runtime evidence |

The source/export report is
[`source-validation.json`](../evidence/art-parade-print-v2/source-validation.json).
Renderer: PyMuPDF `1.27.2.3`, Python `3.14.3`. The source PNG converter does not
resolve inherited `currentColor`, so its first color assertion failed with
exit 1 and black output. The exporter now resolves that single token to the
SVG's declared `#191817` in memory, records the render-input SHA and leaves
the editable SVG intact. Native browser SVG load/color behavior is still
**NOT_RUN**; this conversion is not presented as browser acceptance.

The final 2× raster is 44.67% fully transparent and 50.29% opaque ink, with
52.88% alpha-weighted coverage. Nonzero alpha bounds are `(0,13,720,192)`;
all opaque pixels are RGB `(25,24,23)`. No white paper has been baked into the
public outputs. The transparent preview and source-only paper review image
were directly viewed. The latter is
[`source-review-paper.png`](../evidence/art-parade-print-v2/source-review-paper.png),
rendered over `#e9e1d4` solely for source inspection, not shipped as the product
or mislabeled as an app capture.

The first angular draft and its exact source/export report are retained
unchanged under `../evidence/art-parade-print-v2/draft-01/`. It was rejected by
the author as too schematic and is not qualified for integration. Earlier
published parade/footer art remains immutable in Git. Only the top-level
qualified public files in the interface table are the new integration source.

## Actual allocated source revalidation

Resource21d issued **ART-PARADE-SOURCE-B-202611041349Z**, isolated SOURCE slotB,
90 seconds total /60 work /30 independent cleanup. Actual ACCEPT was
`2026-11-04T13:51:18.037183Z`, with fresh free space30651777024 bytes, work
deadline13:52:18.037183Z and final deadline13:52:48.037183Z. The accepted floor
was16GiB and the live floor8GiB; no fresh16GiB requirement was added at each
child launch.

Frozen clean HEAD: `ae657fc3a6d7d417a19f593b8f2e8970839fa92d`, a successor of
`20ffdd85`/`f29f546f`; exact public source postimages remain those of
`4d297ce650a84d43b67203826c7a10dbfa21696d`. Full existing carry was17 files,
164980 logical bytes: all source, reports, own draft and requested guard/preflight
files. The3449863 bytes of read-only original reference inputs were counted
conservatively in WHOLE; no reference copy was made. No historical scoped file
was omitted. The14 original source/evidence files grew to17 solely through
the requested guard, preflight note and temporary-directory ignore file.

The parent deadline/resource guardian existed before either sequential output
child. It ran `measure-reference.py` then `render-assets.py`, with fresh exports
and reports in its unique temporary directory, followed by pinned-hash and
frozen-carry audits. It launched no install, app build, browser, server, new
worktree or native matrix. The exact argv, accepted limits, input pins, all
frozen carry hashes, command hashes, source observations and raw exits are in
the distinct immutable
[`allocated receipt`](../evidence/art-parade-print-v2/runs/ART-PARADE-SOURCE-B-202611041349Z.json)
and its sibling `.log`. Receipt SHA-256:
`4114e084ca6a7e51fddd2f312f555bec2b4ff364cf81c3fcd2a57d87d81df848`.

| Measurement | Actual / granted bound |
| --- | --- |
| Command results | Both children exit0; guardian exit0 /`PASS_SOURCE_ONLY` |
| Actual execution |13:51:38.131429Z–13:51:38.787869Z;0.629s execution,20.723s from ACCEPT |
| Cleanup |0.000735s; own temporary directory absent, zero live descendants, **FULL** release |
| Sampled WHOLE / temporary peak |3692062 /74566 bytes, within8388608 /1048576 |
| Sampled work-phase durable peak |167633 bytes; excludes the receipt written after cleanup |
| Final carry including raw receipt/log |176825 bytes /19 files, independently observed before the deadline; within2097152-byte durable cap |
| Sampled aggregate RSS / process count |94191616 bytes /3, within536870912 /4 |
| OS child maximum RSS |66027520 bytes, recorded separately from sampled aggregate |
| Minimum observed free space |30638804992 bytes |

The independent postwrite release observation occurred at13:52:40.279673Z:
both recorded child PIDs94748/94755 were absent, no temporary tree remained,
and all frozen inputs and both original report bytes were unchanged. Final
WHOLE at that observation was3626688 bytes including counted references.
The raw runner's disk peaks were sampled during work; its durable peak does
not silently include its own later receipt. That distinction is retained in
the [release observation](../evidence/art-parade-print-v2/runs/ART-PARADE-SOURCE-B-202611041349Z-release-observation.json).
FULL release was sent directly to21d; no subsequent output job uses that closed
lease. Commit/document authoring after release is not presented as part of the
timed job.

The actual command used `qualify-source.py` at the frozen HEAD, the named offer,
real ACCEPT/deadline/free-byte values and the granted90/60/30 clock,
8388608/1048576/2097152 disk bounds,536870912 RSS bytes,4 processes and65536
log bytes. The script traps cancellation/limits, stops only its owned child
session and auto-cleans its own directory. New temporary exports matched
every pinned PNG hash and the reference report; all initial public PNG/SVG,
metadata, draft and authoring evidence remained byte-identical. This qualifies
the **source/export handoff**. It does not establish browser SVG color/load,
actual footer placement, CTA clearance, character preservation in a new native
composition, live service or device acceptance.

## Required native handoff and completion limits

Root and consumer213 coordinate one bounded integrated capture after a **named
Resource21d offer** and frozen art/consumer source. No artist-owned duplicate
browser/native matrix is started. Required owner inspection covers actual
default-shell Meet, Giveaway and Profile at 360/390/430px against the canonical
reference, plus short-height/CTA clearance, relevant normal/forced-color states,
reduced motion, SVG load/color, and preservation of actual selected character
pixels, controls, text, scrolling and noninteractive footer semantics. The
consumer chooses sufficient combined functional checks; no live service,
physical device, global accessibility or full ten-screen parity claim follows
from source validation.

The preceding NOT_RUN statements describe the original source qualification.
The later native observation below advances those specific checks without
rewriting the historical source reports or frozen placement metadata.

## Retained NativeB comparison — material differences open

The art owner directly viewed the canonical reference and all18 retained
Meet/Giveaway/Profile captures: normal and480px-height body-end states at
360/390/430px. No duplicate browser, raster export or application build was
started. Exact image hashes, measured footer rectangles and dispositions are
in [native-01-assessment.json](../evidence/art-parade-print-v2/native-01-assessment.json).

The consumer packet is retained in its isolated checkout at
`/Users/lhs/Desktop/mcr-worktrees/fe-reference-frame-footer/docs/mcr/evidence/reference-chrome-v2/native-01/`.
Its `capture/report.json` SHA is
`07464529154845c17a5e6e573bfdec466ec4f43a063190fc19290259251ce725`.
It names source `c019e84d5a2102f8fd31f3498f37cb235795c55a`, qualified parade
`b68d40a906ac98f8dd3da86d4136fbbe425be8d9`, controls `65b0d5f6`, Feed overlay
`1a907343`, and job `REFERENCE-CHROME-NATIVE-BROWSER-202611041410Z`.
Its guardian reports syntax/native commands exit0, own descendants empty,
link/temp absent and cleanup before the original deadline. This is consumer
execution evidence, not a new artist-run resource lease.

**Technical source integration passes in this bounded fixture.** Actual print
boxes at viewport360/390/430 are330×88,360×96 and400×106.65625px, respectively,
at x15. These replace the earlier hypothetical width-minus18 estimates; the
360:96 ratio is preserved. All nine cases report native source canvas probes
with17151 opaque pixels,15248 transparent pixels and zero unexpected opaque
RGB values in normal and forced-colors. Source readback confirms this probe
checks RGB25/24/23. There are no retained forced-color screenshots, so this
is pixel-probe evidence rather than a manual forced-mode visual comparison.
The native context uses reduced motion; the source itself is static.

All short-height end images show the complete print inside the scrollbody,
clear of controls. The footer is decorative, aria-hidden and pointer-inert.
Normal Meet/Giveaway images put the band below the first viewport at every
width; normal Profile360 clips its lower part until scrolling. Those states
are recorded honestly rather than counted as an initially visible bottom band.
Selected fixture portraits remain distinct from the decorative silhouettes;
this does not certify source-pack pixel equivalence, live accounts or devices.

**Reference fidelity remains open.** The current smooth, comparatively upright
figures and similar head/arm rhythm differ materially from the reference's
hunched, irregular silhouettes. The shallow earth strip and regularly spaced
long nicks look cleaner than its heavy weathered ground. These are art-owner
fixes, not a reason to relabel the consumer's functional PASS as a visual PASS.
The qualified public source stays frozen while a scoped candidate is prepared
and qualified under a new named source offer.

Meet also separates Create Party from the band with Create party/People/Your
intro rows, while the reference joins the CTA to the parade. This is routed
directly to consumer213, who owns component/flow decisions and fixes. No king,
SL or independent QA approval is required. Native evidence uses production
components/default router/Phaser with explicit DEV synthetic HTTP/C05 fixtures;
it does not establish live service, physical device, Map or whole-screen parity.

The original goal remains unfinished through scoped fixes, updated native
evidence where source/flow changes invalidate prior observations, committed
handoff, named-owner publication and independent remote byte/ancestry readback.

### NativeB seal and actual resource receipts

The inspected packet was sealed by consumer commit
`c0c24f4abea2afa40f94d9349920ee297b3e6e76`. Git-blob readback matches the
retained report, actual pre-child ACCEPT snapshot and measured FULL release.
The sealed `native.mjs` SHA is
`c3d236f8516b068175fea9eb44716322d86e9f039c0157c435cc34b5da37601b`,
matching the ACCEPT pin; its image-probe and reduced-motion source were read.
`native-B-accept.json` SHA:
`9be33f9281063cf543a7793da21e8e595d69842fc433dd62227dc03148943dc2`;
`native-B-release.json` SHA:
`0dfd171e11af852157937253b4511618c1cbd1e8953856a010eb678b2a8e0ef2`.
Both sit beside `native-01/` in the consumer evidence directory. Actual ACCEPT
was14:12:09.605431UTC, work cutoff14:17:09.605431, cleanup14:18:09.605431;
guardian finished14:12:58.061450 and measured FULL release14:14:55.624134.
The later Root flat-paper patch was not in that run. Subsequent dirty consumer
files are not silently substituted for this sealed proof.

## Qualified source refinement — candidate02 successor

The native assessment led to a newly authored nine-figure composition rather
than a report-only handoff. Smaller heads, pitched shoulders, crooked arms,
unequal strides and broken hems replace the smoother figure rhythm. Fourteen
paths include a heavier earth edge at y67–75, fine scattered transparent nicks
and sparse uneven stalks. No reference pixels/contours or player art were used.
A static coordinate check caught cumulative ground-edge drift in the first
candidate text; explicit bounded coordinates fixed it before any render.

Resource21d offered **ART-PARADE-CANDIDATE02-SOURCE-A-202611041425Z** and
clarified accounting before ACCEPT. Existing own worktree Git administration
and its `.git` pointer count in WHOLE through a conservative2MiB reserve;
shared common Git/foreign checkouts are excluded read-only baseline. That
existing private Git baseline does not count against the2MiB source/output
durable cap. The guardian verifies actual private-admin bytes fit the reserve
and charges the reserve in every WHOLE check. Prior receipts are not relabeled.

Frozen clean HEAD: `65b393ff7d1fc7ebe6e6ebc182fdb5b7ac298f5a`.
Actual ACCEPT14:31:33.281969UTC, free27095855104 bytes, work deadline14:32:33.281969,
original cleanup14:33:03.281969. All24 owned files/219636 bytes plus3449863
read-only reference bytes and actual private Git1648286 bytes were accounted;
the full2MiB Git reserve was used in WHOLE. Accepted limits were90/60/30seconds,
8MiB WHOLE/1MiB temporary/2MiB durable,512MiB RSS,4 processes,64KiB log and
64KiB receipt reserve, one sequential output child,16GiB ACCEPT/8GiB live floor.

The guardian existed before children and sequentially measured references and
rendered only the candidate into its own temporary directory. Both children
and guardian exited0; frozen b68 public files and all24 inputs were unchanged.
Fresh candidate exports were retained only under `candidate-02/qualified-01/`.
No browser/server/build/install/dependency/public replacement was run in this
source allocation. Raw receipt SHA:
`1e2c8dd384ee562c495cd71faa88e5e515d8d11c6066cb4ca10acbc5e0f85d70`.

| Candidate source measurement | Actual |
| --- | --- |
| Work start/finish |14:31:47.187514–14:31:47.837636UTC |
| Execution / ACCEPT-to-finish / cleanup |0.622 /14.528 /0.000684 seconds |
| Sampled WHOLE including Git reserve / temp |5887864 /64428 bytes |
| Sampled work-phase durable |276421 bytes; raw receipt written subsequently |
| Postwrite final carry including receipt/log |30 files/289014 bytes, observed14:32:10.902054UTC |
| Postwrite WHOLE including refs/reserve |5836029 bytes |
| Sampled RSS / processes |91226112 bytes /3 |
| Child OS max RSS / minimum free |65536000 /27076902912 bytes |
| Release |**FULL**; temporary absent, recorded child PIDs gone, frozen inputs unchanged |

The immutable raw receipt/log and independent postwrite observation are under
[`runs/`](../evidence/art-parade-print-v2/runs/). The observation file and later
source/document commits are not misrepresented as timed measurements. FULL was
sent directly to21d before the original cutoff. No closed lease was reused.

The owner directly viewed the retained360px transparent preview and720px paper
source review. The revised smaller angular poses and heavier, irregular ground
address the source issues identified in NativeB. Alpha-weighted ink is50.145%
at2×, fully transparent47.459%, opaque47.624%, bounds `(0,26,720,192)`, opaque
RGB25/24/23. This is qualified source inspection, not new app-image acceptance.

After FULL release, exact retained source/PNG bytes were copied into the owned
public prefix and the local placement manifest updated. No export was rerun.
The immutable b68 pack remains in Git; old draft/source reports/native proof
were preserved. Current public postimages are:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `parade-print.svg`,360×96 |5894|`e7b401a8f431651c05b88a6c52d02b8aabc059f697bf960b41454ba59ae95a10`|
| `parade-print-preview.png`,360×96RGBA |9936|`48e3145b5675081376ab5cc1a5cf92b8abe85bc14ee47034cc58a36dbb6c2351`|
| `parade-print@2x.png`,720×192RGBA |21210|`27c087442ff29576901765f6dcfed3d0c2ee668044ac9593b733cd6d9b38fe6a`|
| `placement.json` |5617|`13c65b667b1e488d2ad125d69fa36ced60c7f736b7b505c0ef2cb3a72232ba2a`|

The URL, IDs, intrinsic dimensions, transparent ink and decorative semantics
are unchanged. Exact previous/current pins and promotion disposition are in
[`candidate-02/promotion.json`](../evidence/art-parade-print-v2/candidate-02/promotion.json).
The raw source report retains its actual temporary export paths; the guardian's
`candidateExportPins` binds those bytes to their durable qualified locations.
Consumer213 receives a committed qualified successor pin for a fresh selective
affected native round. Previous NativeB color/flow observations qualify b68
only. Refined print placement, current reference fidelity, Root's CTA/row fix,
final publication and independent remote ancestry/byte readback remain open.

## NativeC successor assessment

The art owner directly viewed all18 new retained Meet/Giveaway/Profile images
at360/390/430px: normal and480px-height scrollbody-end captures, against the
canonical reference. No artist browser/render/build was started. NativeC uses
consumer `db07437a2619b049d2030d8127b4910c1a44f6dd`, qualified print `ec40e531`,
controls `65b0d5f6`, Feed `1a907343` and Root flat-paper overlay `32fed3eb`.
Exact image hashes, rectangles, source/receipt pins and explicit limitations
are in [native-02-assessment.json](../evidence/art-parade-print-v2/native-02-assessment.json).
The four public ec40 files stay frozen; their placement metadata describes the
source-handoff snapshot and is not silently relabeled after this observation.

The actual retained report is
`/Users/lhs/Desktop/mcr-worktrees/fe-reference-frame-footer/docs/mcr/evidence/reference-chrome-v2/native-02/capture/report.json`,
SHA `6c140b707be8dd2d9a2dfa47267db858bb637177cf747e4fd41cd05ffe05df88`.
Consumer sealed this packet as `76afcfdd36ca956564fdc352616268f2cf7793bf`.
The artist independently read all18 inspected images plus report, ACCEPT,
FULL-release and guardian blobs from that immutable commit; every SHA matches
the retained-image assessment. Later Landing palette-only work adds no print
dependency and is not silently substituted for the inspected source.

**Bounded original-art implementation and native integration: PASS.** The new
nine-figure print is actually visible in all nine body-end states. Smaller,
angular, varied silhouettes, instruments and pennant replace the smoother b68
rhythm; heavier uneven earth and fine transparent nicks remain legible at all
three widths. The earlier source issues were addressed in owned art rather
than waived through the browser's functional PASS. Original figure contours
are not represented as traced reference pixels or copied selected characters.

Actual boxes remain330×88,360×96 and400×106.65625px at x15. All nine short-end
states show the complete band, preserving360:96 without extra cropping,
stretching or obstruction of controls. Intentional first/last source-edge
silhouette cropping remains. All nine normal/forced image canvas probes report
16152 opaque pixels,16094 transparent pixels and zero unexpected opaque RGB.
The source script at the pinned consumer commit matches the ACCEPT hash
`ba51160f2eef12fa84c2bcbb2ab66915689d04abf3becfd72babf7ffffc3e16a`;
its probe checks RGB25/24/23 and its browser context uses reduced motion.
No forced-color screenshot is retained, so forced-mode evidence is the pixel
probe, not a manual whole-screen comparison. Footer reports are aria-hidden,
pointer-inert and insideScroll, with no broken image, overflow or case error.

**Screen/reference parity remains open with component/per-screen owners.**
Replacing three Meet navigation rows with More actions reduces the earlier
separation, but the reported Create Party-to-print gap is still73.75px at all
widths; canonical Meet places the CTA against/within the band. Giveaway's
My gifts-to-print gap is50px at all widths. Both normal surfaces keep the print
below the first viewport. Normal Profile360 clips its lower band until scroll;
390/430 shows it. These are truthful observed states and remain routed to
consumer213/Root/per-screen owners. The art owner changes no component, CSS,
CTA state or selected character to conceal those differences. A disabled
fixture Create Party action is not rendered as a falsely enabled action.

NativeC ran under **CHROME02-SOURCE-SELECTIVE-NATIVE-202611041446Z**. Actual
ACCEPT14:47:07.808845UTC, work14:52:07.808845, cleanup14:53:07.808845;
independent FULL release14:48:26.503731 was before both cutoffs, with port5302
free, dependency link/temp absent and no recorded descendants. Source tests,
types, lint, syntax and native commands exited0;18/18 cases passed with27 new
PNGs. The owner-reported tests remain its validation; the artist did not rerun
them. Exact ACCEPT/release/guardian hashes are in the owned assessment.

This pass covers the original art, measured composition/ink/alpha and observed
native fit. It does not claim pixel-identical reference grain, full ten-screen
visual parity, live service, two accounts, physical device, software keyboard
or Map acceptance. The run uses explicit DEV synthetic HTTP/C05 fixtures with
real default components/router/Phaser and separate production-anonymous entry.
Remaining own duties are committed publishing handoff and independent final-master
ancestry/exact-byte readback. Screen differences
stay visible with their responsible implementation owners; no King/SL/QA
review or acknowledgment gate is introduced.
