# QA06 mobile visual qualification

QA06 is **BLOCKED**, not accepted. The original scope remains unfinished. Desktop evidence is available; actual iOS Safari, actual Android Chrome, software-keyboard/hardware-safe-area checks, qualified Wall visuals, and product art/release admission remain open. This report preserves the original task rather than closing a desktop fixture contribution as the whole task.

## Authority, scope and provenance

- Original acceptance: `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, QA06 and section5.1. Required widths360/390/430; actual iOS Safari and Android Chrome; keyboard, safe area and three skins; legible avatars, reachable controls, no clipped panels or old UI; actual browser/device versions.
- Base: `980fc4d85020f297b36fee8195e1ca92079ff33f`; isolated branch `codex/mcr-qa-QA06-mobile-visual` in the preserved frontend reference worktree.
- Sole writes: `frontend/e2e/visual/qa06/**`, `docs/mcr/evidence/visual/qa06/**`, and this document. No application, existing helper, art, source archive, contract, manifest, lockfile or CI changes. Only Root153 publishes to master.
- The original reference image remains unchanged. Its exact copied bytes are `../evidence/visual/qa06/reference-target.png`, SHA256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`. Ten crop coordinates are reused read-only from `frontend/e2e/reference-integrated/cases.mjs`.
- New fixture boundary construction is adapted from `frontend/src/app/__tests__/fe07-shell-journey/preview.tsx` and `frontend/e2e/reference-integrated/preview.tsx` at the base above. Data helpers are imported read-only. Actual Auth, application browser router, panels, production world loader, Phaser and whole-character renderer are used. No rendering/panel replacement is copied.
- Synthetic HTTP/C05 data, synthetic local input, test-only service availability and legacy-map/theme/place admission remain explicitly fixture boundaries. Social/domain fixture identity/avatar projections use current selected whole-character configurations, never legacy character images. No live server, server invariant, multiuser, movement authority or physical-arrival proof is implied.
- Versioned world/map captures run the existing `reference-scene-v4` root fixture read-only. This uses an explicit source resolver/default Phaser with selected characters. Original provenance stays **UNREVIEWED**, map7004 is synthetic, places remain virtual concepts, and only the supplied era is available per fixture. The separate actual default production resolver rejects the source. Backend revision5 reservation `ed34331cac41d1997335b5f6dc93f33429005a91` does not admit it.
- V4 source geometry: `sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f`; canonical Tiled: `sha256:c589b9b01c3e9f7162ccf7443782363bc9c132dd807b5fe15557686df584cf6f`. Inputs stay on quality-v8; no silent v9 substitution.
- Sparse checkout omits some tracked image files. The harness serves only allowlisted tracked public image/font bytes read from HEAD Git objects, recording their hashes. It does not restore or modify the source files.

## Original acceptance disposition

| Criterion | Evidence/disposition | Remaining input and owner |
| --- | --- | --- |
| Widths360/390/430, target390×844 | Actual product DOM/canvas desktop Chrome touch viewports; full matrix report linked below | Physical browser/device confirmation remains required |
| All three skins | Current selected skin is asserted against each requested era; versioned source independently supplied in each era | Genuine qualified product releases and optional-era switching remain art/producer/integrator work |
| Ten supplied reference screens | Each crop reconciled below; screenshot proportions preserved | Wall cannot pass while its qualified current-world source is unavailable |
| Avatars legible / selected art only | Current selected whole-character manifest/configs; six real sprites in versioned source fixture; actual DOM portraits/previews | Device-scale and qualified product-art signoff remain open |
| Reachable controls / no horizontal overflow / no clipped panels | Control target sizes include associated checkbox/radio labels; actual focus, scroll and hit tests; height844 and selected height420 captures | Desktop evidence does not certify mobile browser chrome or software keyboard |
| Keyboard | Actual DOM focus/Tab and reduced-height layout, no claimed software keyboard | User-connected iOS Safari and Android Chrome required |
| Safe area | `viewport-fit=cover`, existing env-inset CSS and desktop zero-inset behavior recorded | Actual notch/home-indicator/browser UI required; no fabricated inset is physical proof |
| Actual iOS Safari / device and browser versions | **NOT_RUN**. devicectl observed a physical iPhone17 unavailable; iPhone Mirroring harness returned `not-running` once | User must connect/lock the iPhone through Mirroring. No Connect/Continue clicks or connection polling |
| Actual Android Chrome / device and browser versions | **NOT_RUN**. `adb` absent on PATH, then found in the existing standard macOS SDK. One sanitized `adb devices -l` check returned exit0 and zero devices; hardware availability remains pending, not inferred absent | User identifies/connects an available Android device; no SDK install or simulator substitution |
| Old UI excluded | Current product routes render the newly authored UI; target decoration does not create levels/points/general chat/provider functionality | No legacy image fallback permitted |
| Publication / review fixes | Committed handoff and actual remote/payload verification recorded below when delivered | Root153 designated publisher; no manager technical-review gate |

## All ten reference screens

| Supplied screen | Actual route and visual reconciliation | Qualification limit |
| --- | --- | --- |
| Landing | `/`: authored poster/entry, primary action and crowd/world introduction. Reference crop is preserved for side-by-side comparison | Decorative reference counters are not new product requirements |
| Login | `/login`: actual canonical email/password sign-in, accessible labels and clear primary action | Synthetic login endpoint; no fabricated social-provider support |
| World | `/world`: actual persistent world, six selected characters, era controls, movement/DOM alternatives and six navigation destinations | Legacy-map test admission for ten-screen baseline; v4 source separately labeled |
| Map | `/world/map`: actual map panel, independently zoomable/pannable overview and DOM Places information | Source fixture remains unadmitted; virtual coordinates never prove physical arrival |
| Customize | `/me/avatar`: whole-character preset selection, visible large preview, three-card carousel and real controls | Selected source pack is whole-character art; do not invent editable layers from reference imagery. Save capability is not fabricated |
| Feed | `/feed`: actual authored rows, filters, selected-character author portrait and composer entry | Synthetic contract feed; write/publication flows are not certified by screenshots |
| Meet | `/meet`: actual party rows, host portraits, capacity labels and detail/join-review entry; full label sizes measured | Synthetic read data; no server capacity/membership acceptance |
| Gifts | `/gifts`: actual giveaway discovery rows and review-entry controls | No inventory/reservation/pickup claim from synthetic data |
| Wall | `/wall`: actual unavailable-state UI is captured honestly | **NOT ACCEPTED**: default world fixture lacks a qualified current-world mural anchor/source. No fake canvas, panel override or old mural image is inserted |
| Me | `/me`: actual selected-character profile, activity tabs and explicit profile/account destinations | Synthetic identity; unavailable activity is preserved rather than fabricated |

The current UI uses symmetric frames and separators. No decorative left accents/rails are introduced by this QA patch. The reference has different phone aspect ratios; the comparison keeps both source crops and current844px captures proportional rather than stretching them to create false similarity. Visual reconciliation is a human inspection record, not a pixel-similarity or product approval claim.

## Commands and evidence

Run with the retained Node24.22.2 and retained frontend dependencies; no install is required:

```sh
QA06_OUTPUT="$PWD/docs/mcr/evidence/visual/qa06/new-matrix" \
  node \
  frontend/e2e/visual/qa06/native.mjs
node \
  frontend/e2e/visual/qa06/comparison.mjs \
  "$PWD/docs/mcr/evidence/visual/qa06/new-matrix"
```

The runner requires a **new** owned output directory and ≥8GiB free disk. It never overwrites the historical harness evidence. `report.json` pins the source Git trees, harness hashes, exact served public image/font bytes, selected-character manifest and browser/Node versions. The qualification is always desktop-only.

- `node .../typescript/bin/tsc -p frontend/e2e/visual/qa06/tsconfig.json --pretty false`: exit0. New fixture and imported type boundaries checked.
- `node --check frontend/e2e/visual/qa06/native.mjs`: exit0.
- `python3 scripts/mcr/check-plan.py`: exit0/PASS, planning/source provenance only; separate application/device acceptance remains required.
- Initial smoke attempts are diagnostic, not product failures: the first incorrectly required live replication while panels deliberately suspend input; a later sparse-Git read hit the default1MiB child-process buffer; another failed to open the real Places dialog before finding its searchbox. These harness issues were corrected without production edits. Final evidence supersedes those diagnostic runs.

Corrected `matrix-v3` completed **exit0**,78 screen cases plus9 source cases and135 exact-size screenshots. Saved-artifact verification (`review.mjs`) passed: zero recorded page errors, horizontal overflow, broken DOM images, undersized enabled baseline controls or unreachable active baseline controls. `modal-v2` independently verifies active modal scope. `fidelity-v3` completed **exit0**, ten equal-aspect screens and three v4 source variants; its ten paired comparison panels all rendered. These are bounded desktop fixture checks, not ten-screen fidelity acceptance. QA06 remains active until the original criteria are fulfilled.

## Developer comparison and concrete repairs

I directly inspected every equal-width/equal-aspect target/current pair, rather than inferring fidelity from capture counts or geometry checks. `../evidence/visual/qa06/fidelity-findings.json` records all ten observations, exact crop/capture hashes, source-versus-fixture classification and next owners. `fidelity-v3/comparison.html` and its ten `compare-*-390.png` panels provide the corrected current comparison; historical `fidelity-v1` remains preserved; the image source is unchanged and neither side is stretched.

The first impression that Meet/Gifts rows were much taller was **retracted** after the equal-aspect comparison: their row heights are close to the supplied target. The actionable Meet difference is the initially clipped Create Party area at390×637. The earlier disabled Reserve appearance was an **owned QA fixture defect**: recursively projecting every public owner's ID to the actor made the giveaway self-owned. The corrected fixture preserves foreign author/owner identities, binds private/queried subjects explicitly, and actual `fidelity-v3/compare-gifts-390.png` independently shows eligible red Reserve controls. No production Reserve fix or invented product photos are requested.

`../evidence/visual/qa06/repair-packets.json` is the concrete developer repair handoff:

- **Feed / Me:** actual writer202 owns FE27/FE34 feature fixes and the separately transferred **PanelScreen conditional-only** hunk for real `/feed` and `/me` feature children. Feed's real compose button appears immediately after short content; a generic root link duplicates it. Me similarly repeats existing destinations via generic root links. Exact source: `features/event-feed/EventFeed.tsx`, `feed.css`, `features/me/ui/MePages.tsx`, and `app/WorldShell.tsx` PanelScreen. `routes.ts` is read-only context. Cold unavailable profile activity and optional media are preserved; no fake joined/reserved events, levels or points are a repair.
- **Meet:** Community assigned sole repair owner204 a new bounded lease for PartyDiscovery action/container CSS and the Create Party block only if needed. Shared PanelFrame, ParadeFooter, root route links and capability projection stay read-only unless separately assigned. Source/pins,390×637 reproduction, original FE21/FE23/QA06 IDs and post-fix3width/420px checks are in the packet. This is a source-owner repair, not an SL visual-review gate.
- **World / Map:** Root153 owns the active v10 runtime/overview/IllustratedMap work. The default-loader baseline is the old sparse studio scene. The separately labeled v8/spatial-v4 source is nearer the festival reference, but its stage is cropped left at the self camera and initial MAP cover zoom cuts labels. At390×844, actual image x−18..407.47 exceeds viewport17..373; pressing the real Fit map contains the image356×600.75 in a356×718 viewport. Corrected diagonal **v10 SOURCE**, current **v8 runtime**, and **unadopted MAP planting9ff741f** stay distinct. Publishing an artist source or starting the new goal does not resolve fidelity; actual delivered consumer adoption must be independently captured and compared.
- **Customize:** the current1.1 whole-preset path differs from the reference's right category rail/layer/palette controls. Motion83 received the exact comparison. I independently verified and viewed its static candidate JPG (`1bb81f96a96f9f6795202f5f05283f99000d38df3ebf423bbb99845268ed9421`,87,364bytes) at candidate `df0522fa` + `d6185d26`. It has genuine additional controls but still horizontal categories. The owner fixture includes World/test navigation above the editor, so its whole-page height is not called production clipping. This is **static candidate inspection**, not fresh native/public-consumer adoption; no legacy or replacement character art is authorized.
- **Wall:** ordinary production correctly supplies no qualified mural factory and stays closed. Three fresh captures of the existing actual MapScene/Phaser caller independently confirm the synthetic qualified consumer with one game/socket, one mural object and zero pixel POSTs. Its geometric raster, synthetic anchor/eligibility/Auth/HTTP are not the reference artwork or default product Wall. Genuine qualification and an explicitly assigned activation writer remain required.

Landing/Login preserve the supplied broad composition without a newly identified layout defect in this comparison. Their newly authored contours, selected-character poses, legal/status text and unsupported-provider/Remember me differences are recorded rather than masked by fabricated capabilities. Other content differences—missing optional photos/member portraits and cold current-feed activity—remain explicit fixture/input limits. No numerical fidelity threshold or general ten-screen PASS is invented.

## Supplementary ready consumers and resource receipt

Original task mapping is preserved: FE26 is GiftPickup, FE30 is introduction/public profile/age status, FE33 is Wall. FE31 sharing is not substituted for FE30. The read-only FE26 route-adapter fixture, actual FE33 caller assembly, and QA06 actual-root FE30 routes were independently captured at360/390/430 with current source hashes. These are labeled synthetic consumers, not live/private-authority/provider/default-Wall or device qualification.

The nine `modal-v2` cases passed active-dialog44px target size, actual focus/scroll/hit tests, background-focus denial, reverse Tab inside the modal, and close-to-Places focus restoration. The generic first probe saw correctly blocked native-modal background controls; those are not filed as unreachable product controls. Actual software keyboard remains NOT_RUN.

`resource-status.json` preserves the historical7.53GiB hold and owned258.2MiB temporary-cache cleanup. The single resource coordinator subsequently issued a capacity release; my fresh preflight measured10.543GiB, zero active QA06 runners and a conservative100MiB job budget. Corrected `matrix-v3` resumed under the unchanged8GiB guard. Current harnesses reserve an explicit unique ephemeral port and remove their own temporary Vite caches in `finally`; they do not share-purge targets or terminate peers. Exact first-matrix harness bytes are retained as `.txt` snapshots under `frontend/e2e/visual/qa06/snapshots/baseline-v1/` for historical reproduction.


## Bounded repair review and current hold

`candidate-review/review.json` and byte-preserved copied owner images record direct independent static review, not a fresh runtime PASS. Feed/Me source `576f9848` and Meet source `52f7b2f9` visibly address the requested single-bottom-compose, compact existing account navigation, and complete Create Party/copy. Their exact production diffs were inspected. Root153 publishes and supplies actual publication pins before independent post-fix execution; the owner's own passing commands remain developer-reported evidence.

The newer Motion component source `228e1cd4`, published as `6518d793`, provides the actual five-item right category rail and compact preview/pose. I independently viewed its original390px component image and verified SHA256 `931b53da629dea709d3bf53c743bed9c3be61021f4818a01e4bb8f620f66dd7c` (56,020bytes). The new owned `motion-candidate.mjs` provides read-only candidate capture with all source bytes checked against228e, canonical test Auth/actual Phaser, no writes and no public motion activation. It has passed syntax/lint but has **NOT_RUN** runtime under the current capacity hold. Do not infer public C03/C05/Root admission from this component.

Root153's newer v10 actual-root captures show the complete stage at390×554 and horizontal FOOD containment at390×754. Direct inspection found an additional concrete problem: the absolute map status caption obscures the lower GIVEAWAY BOOTH painted label. This was routed directly to Root153 with the exact image/source and a requested painted-label-versus-caption/zoom visibility check. Whole-image containment alone does not establish label legibility. v10 publication/final parity and QA06 independent native checks remain open.

After completed matrix-v3/fidelity-v3 and saved-artifact rendering, `consumers-v5` refused at the8GiB guard **before creating a directory/server/browser**, command exit1. Fresh recorded free space was7.756GiB. No app capture remained active; the single disk coordinator received the stop receipt. Existing `consumers-v3` remains historical labeled15-case evidence before the foreign-owner fixture correction. It is not falsely marked as a corrected current rerun. Motion and published repair captures wait for capacity release/fresh preflight. No shared cleanup or peer-process termination was performed.

Historical valid `matrix-v1`, `fidelity-v1/v2`, and `modal-v2` evidence is preserved alongside current results. Preliminary smoke/modal/consumer diagnostic attempts remain local and are excluded from the publication payload. The initial comparison of all ten screens remains available and is not replaced by a geometry-only pass.

## Delivery checkpoint

Source harness commit: `59f88f9c1d8d3b1aa4568581b8c496981208e371`, parent production baseline `980fc4d85020f297b36fee8195e1ca92079ff33f`. This checkpoint changes only the three assigned QA06 prefixes. Whole QA06 is **IN_PROGRESS** with the persistent original goal active. Physical iOS Safari/Android Chrome, real keyboard/safe area, qualified product Wall/art/admission, independent published repair captures and publication verification remain required. Root153 alone integrates/pushes; no whole-task closure or lease return is authorized by this checkpoint.


## Verified remote checkpoint and focused queue

Fresh `git fetch origin master` and `git ls-remote origin refs/heads/master` agreed on `54e1325fce20e3abc98da50c68de0bd72dfd1a33`. Source59f88 mapped to published `c2df465abf7811c9e24dfec84ca0c30787eaef23`; b9cbd mapped to `dded7fe19dca366a699b243a638832a5158a4d04`; bb9e mapped to54e1325f. I independently checked published ancestry, every449 payload Git blob, and all420 indexed artifacts' SHA256/size (75,666,991bytes). `publication-receipt.json` records the exact changed paths and mapping. This verifies delivery of the bounded evidence checkpoint only.

The installed Android SDK exists outside PATH. One read-only enumeration returned zero connected devices and exit0. `android-availability.json` contains sanitized counts only; no serial or personal device name is recorded. The original physical-device question remains pending and is not duplicated. iPhone Mirroring is not polled.

Capture priority is Root153's existing v10 Map/World consumer validation first, then QA06 focused fresh post-fix reference qualification after that validated input is published. A single browser/native workflow may run, with at most0.2GiB new output. BackendSL is the sole capacity coordinator; an explicit release plus fresh own≥8GiB/process preflight is required before claiming the QA06 slot. The estimate sent directly to BackendSL and WorldSL uses actual existing evidence sizes:36 Feed/Me/Meet PNGs=2,373,548bytes and24 v10 World/Map PNGs=9,589,741bytes. The staged scope expects about15MiB evidence, reserves20MiB output and100MiB temporary cache. No unrelated successful87-case matrix or consumer run is queued by default. Required checks remain recorded, with staged/pending status honest.

`published.mjs` is an owned focused driver for frozen published Git blobs, with no new checkout, install, peer source write, or app renderer replacement. It serves the corrected own fixture boundary over actual published product sources. `QA06_PUBLISHED_SCOPE=repairs` covers Feed/Me/Meet360/390/430×three skins, reduced420, same3-row390×637 Meet and account-control navigation. `QA06_PUBLISHED_SCOPE=scene` covers actual published v10 source fixture at three widths/skins and390×554/754 reference aspects, image fit/reset, all six painted-label-versus-caption/zoom intersections, real six-character Phaser scene, and unchanged synthetic/UNREVIEWED admission status. Both remain **NOT_RUN** pending the queue inputs, despite passing syntax/lint checks. This preparation does not certify the new Map source fix.


## Current blocked audit

The same external prerequisite condition persisted across three consecutive goal turns. The current fresh fetch still points at `54e1325fce20e3abc98da50c68de0bd72dfd1a33`; direct published-source inspection confirms neither v5 initial fit nor the new own-flow caption is present. Fresh free disk8.491GiB passes the numeric floor, but no explicit coordinator release or validated published Map input has arrived. QA06 remains second after Root153's first consumer run. No new browser/native workflow was launched. `blocked-audit.json` records the current evidence, turn classification, exact input owners and resume sequence.

Source preparation is committed as `a85aaf4f` and `1da9d7df`, with syntax/lint passing and focused runtime **NOT_RUN**. Original whole-scope implementation acceptance is unchanged: physical Safari/Chrome, real keyboard/safe area, qualified Wall/productart/admission, independent published repair captures, fixes and final delivery audit remain required. The goal's blocked disposition is not COMPLETE, a scope cut, clean STOP, lease return, or IN02 activation. Original baseline evidence and diagnostic artifacts are preserved. The temporary task-created dependency link is removed while quiescent; retained dependencies are unchanged.

The iPhone connection instruction comes from the [phone-harness skill](/Users/lhs/.codex/skills/phone-harness/SKILL.md): “Retry once *after they confirm they've done it*, not before.” The original connection question remains pending; no repeated question, Connect/Continue tap, or connection polling is performed.


## Authorized fresh post-fix qualification — published12a5

BackendSL explicitly released `QA06-SECOND-20261104T0359Z` after Root153's first workflow ended and validated v10 input was published. Fresh own preflight measured9.599GiB and no active QA/reference native runner. Fetch and ls-remote agreed on `12a5d394eeabc89e1c7b0012fb51038f737c9a7f`; the supplied capture packet SHA256 `93770ac73a785f4b9da190da8d2929e130bbc7d54fa81c13b90278c9ca24eb96`, root native SHA256 `27a9dad326f60f8da806dcea4b1e44b36abe9ae2764ca56e7468cf069acd9cb8` and renderer native SHA256 `e0249c9176b2800475ac7eff95f04bd558f5b68cecfbba176f4c4465d084f050` were independently verified from actual published blobs. Initial fit and own-flow caption were present in actual published product source. The original aligned BLOCKED goal/objective was preserved, as the coordinator explicitly instructed; authorized ready work resumed without a replacement goal.

| Fresh scope | Actual evidence | Disposition |
| --- | --- | --- |
| World/Map3widths×3skins | `postfix-12a5-scene/report.json`: command exit0;9pairs;24PNG; real six sprites/one game; source7005 UNREVIEWED; default admission not inferred; zero socket/canvas/cache/blobURL cleanup | Bounded source consumer PASS. Initial fit/reset and all six labels clear caption/zoom; full stage/sign at390×554 and Map at390×754 directly viewed |
| Feed/Me/Meet3widths×3skins | `postfix-12a5-repairs` first nine width360 PASS cases plus `postfix-12a5-repairs-tail`18cases, unique27 coverage; same published candidate, exact shared source hashes and fixture hash | Bounded repair PASS. Single bottom compose; compact Me/account options; same3-row Meet CreateParty/copy fullyvisible at390×637 and reduced420 |
| Actual reference aspect | Added Feed390×631, Meet390×637, Me390×595 in all3skins | Actual viewport resizing, no stretched/cropped replacement for current layout |
| Character component3widths | `postfix-motion-04ea/report.json`: command exit0;3componentcaptures; all current text source bytes match clean `04ea146e65a7e6730b9ce5846380ece57bc638c2`; four critical files identical to original repair228e; actual Phaser2sprites/1game/0socket and no writes | Candidate component only. Rightfivecategoryrail and actual selectedHair6 verified; publicRoot/realservice/device acceptance remains open |
| Six target comparisons | `postfix-12a5-review/comparison.html`, six `compare-*.png`; saved evidence verifier command exit0 and every local image loaded | Developer directly inspected World/Map/Feed/Meet/Me/Customize against unchanged original crops. No invented fidelity score or whole acceptance |

The first repair command exited1 after12records because the new driver sampled immediately during637→420 viewport transition. It now waits for the shell's actual viewport CSS variable and two animation frames. At unchanged product/fixture source, the remaining18cases passed. The original nine successful width360 cases are retained; width390 was rerun to add required actual reference-aspect captures. This is an owned harness correction, not a product defect or a silent exit0 relabel. Both reports remain preserved.

The original Motion whole-source guard correctly refused old228e when the owner checkout had legitimately advanced. Read-only inspection established clean04ea and exact editor/style/fixture/producer-input bytes matching228e. The guard then excluded only sparse evidence image artifacts from its source-file loop; no original/asset/evidence image was restored or altered in the peer checkout. Both preflight failures occurred before a browser or output directory. The successful independent run uses the exact new source pin. The latest actual reviewed-application build record at04ea explicitly states positive production Root/runtime admission is NOT_RUN; a build is not promoted to that proof.

Direct target review closes the requested bounded Feed duplicate/footer, Me duplicate/compact, Meet action clip, Map caption/initialfit/oversized-marker, World full-stage clip and candidate right-category-rail findings. It does **not** close full fidelity. World entity scale/spacing and available canvas budget still differ from the supplied reference; landscape/building art differs in Map. Reference-only levels/points/chat and missing optional Feed photos, member portraits or cold activity are not fabricated. Customize's candidate retains selected source artwork, five real layers,192pxpreview, three-card choices and explicit pose/mode; the reference has a larger portrait and six icons/palette/random. Public Root qualification remains separate. `fidelity-findings.json` records these current observations alongside all ten historical comparisons.

The single allocation ended with18,720,495bytes (17.85MiB) of capture/comparison evidence, below20MiB reserved and the0.2GiB limit. All browsers, servers, temporary caches and the own dependency link were closed/removed; no shared cleanup, peer termination, install, checkout or application build was performed. The final release receipt measured9.419GiB free; an immediately earlier sample was9.705GiB. `resumption-preflight.json` and `resumption-release.json` preserve the precise samples. BackendSL and WorldSL received the explicit RELEASE. No new native workflow is active.

`postfix-delivery-index.json` pins all new artifact hashes/sizes, source candidate,39distinct selected checks, command exits and diagnostics. Original task remains unfinished: physical iOS Safari/Android Chrome, real software keyboard/hardware safe area, qualified public Wall/art/admission, publicRoot Customize evidence, remaining direct reference/art resolution, committed delivery/readback. Existing physical-device question is pending; no duplicate request or Mirroring polling. Scope, lease and original goal are preserved; no IN02 activation or whole-task completion is claimed.


## Existing-evidence implementation routing — World budget and Customize portrait

The [World presentation packet](../evidence/visual/qa06/world-presentation-budget-packet.md) and its SHA-pinned JSON separate the actual closed-World row/canvas problem from deterministic synthetic roster positions. At390×554 the saved raster has about281px of scene; camera projection estimates are explicitly source-derived, not newly captured DOM measurements. WorldSL dispatched the bounded WorldShell/closed-world CSS/composition-summary presentation scope to26e (`e6312ce4`, acceptance pending at dispatch). Camera, geometry, feet, approved art and Root's exact MAP eight-file lease are excluded. Root/WorldSL/26e receive the same technical packet; Artist receives the independent art distinction.

The separate Customize JSON routes fixed192px component portrait hierarchy to Motion83/WorldSL without inventing a new lease, sixth layer, palette, random action or public Root proof. All ten screens are triaged in the packet; Feed/Me/Meet fixes remain bounded verified results, optional/fixture content is not promoted to defects, and Wall's absent qualified runtime remains open. No new browser, build, native workflow, fixture change or matrix rerun was used. Original QA06 BLOCKED goal metadata, required fidelity/device/public acceptance and released capture-slot status remain unchanged.
