# UI/UX reference audit — 2026-11-04

## Decision

**HANDOFF CONFIRMED; PARTIAL APPLICATION ADOPTION; VISUAL IMPLEMENTATION NOT ACCEPTED.** The exact reference has been read by the visual implementation owners. Default shell chrome, qualified Map/theme producers, Social route composition and visual outfit choices are integrated. Social services require explicit readiness; Wall still lacks qualified write inputs. Approved character/venue art, scene prominence and compact screen hierarchy remain material gaps. Functional qualification remains valid within its recorded limits; it is not evidence of complete visual fidelity. This audit does not change the 112-task acceptance ledger or certify a live product flow. The initial audit verdict was **PARTIALLY CONVEYED**; dated follow-through records what changed.

The user supplied a ten-screen mobile reference on 2026-11-04. Its unchanged copy is [user-reference.jpg](ui-reference-audit-2026-11-04/user-reference.jpg), SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`. This is the current screen-level design handoff. The earlier [approved mobile world sheet](../../source/mcr_v2_en/reference/approved_mobile_world.png) contains five world compositions, not these ten screens. It remains an immutable historical source.

## Evidence and limits

### Latest handoff recheck and compact layouts — `f790a98`

**The exact image reached the UI owners; faithful implementation is still
incomplete.** Main freshly compared the original attachment against the committed
copy: identical bytes, SHA-256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
The public/auth, community, Map, Customize, Wall, My Profile and shell handoffs
each record viewing this same reference. This is stronger evidence than merely
sending an instruction. The screen checklist and common design requirements
below remain the handoff; this audit adds no new review gate.

Main inspected the existing source-bound 390px World, Map, compact controls,
Customize, Feed, My Profile and Wall captures, and checked current root route
registration and shared presentation CSS. These are fresh inspections of
recorded native evidence, not newly executed live-service journeys. The frame,
condensed headings, black/ivory/red palette, era controls and icon navigation
are visible. No decorative left accent was found in the inspected captures or
the scoped presentation styles; symmetric borders and focus outlines remain.

| Surface | Current delivery | Remaining gap |
| --- | --- | --- |
| World | Root short-viewport fix `dd86095`; compact controls `22b45fb`, both pushed and remote-verified. Controls occupy 136px; normal scene space increases substantially. Secondary help uses an accessible disclosure. | Captures still use synthetic terrain and unavailable character art. FE07 owns the single combined short-height/expanded-help proof; separate author captures alone do not prove the combined layout. |
| Map | The default route mounts Places; `dd86095` removes the redundant inner title/frame while preserving the outer Map heading, Back, actions and region. | The geometry is a schema-derived schematic, not the illustrated venue in the reference. Physical routes remain unverified. |
| Customize | `f790a98` integrates author `11e2cc6`: one visual outfit chooser, compact category rail, disclosed ID alternatives, explicit Save/Cancel and usable short-panel scrolling. | The visual examples are labeled TEST ONLY geometry. Approved character layers, real product previews and actual root/save flow remain unqualified. |
| Feed / Meet / Gifts / My Profile | Reference-derived feature presentation is delivered; root registration exists. | Authorized imagery/avatars and rich populated identity/activity rows remain visibly incomplete. A registered route does not guarantee a qualified live producer. |
| Wall | Reference-derived editor and default route registration exist. | The default provider still has `release={null}`. It correctly remains unavailable until qualified scene geometry and write authority exist. Fixture canvas styling does not prove collaboration. |
| Landing / sign-in | Existing public-route visual adoption and viewed-reference provenance remain delivered. | Distinctive poster lettering, illustrated silhouettes and approved characters remain unfinished. No new capture or live entry pass is claimed here. |

The latest integration used the completed author/peer handoffs without a second
general review. Customize's 40 changed paths, four existing-file preimages and
48 source/evidence hashes match; all integrated bytes match the author commit.
Fresh combined strict TypeScript passed at `f790a98` (exit 0, 4.84s). Its author
recorded 26 focused tests and nine native scenarios passing. The previously
reported 16px nested editor body is corrected to 192px in the three tested
half-panel widths. The independent root and control authors' validation remains
bound to their own documented source revisions.

Main forwarded published `22b45fb` and the exact reference pin to FE07, which
started the combined check. The World lead received the outstanding approved-art
and visual-fidelity gaps within its existing assignment. Layout writers retain
disjoint ownership; this check neither assigns backend work to frontend authors
nor changes the 112-task/61-IA acceptance counts.

[Compact World capture](../../../frontend/src/world/__tests__/controls-layout/evidence/after-world-390.png),
[compact Customize capture](../../../frontend/src/features/avatar/__tests__/compact-layout/evidence/native/loaded-390x844.png),
[Customize handoff](../frontend/avatar-compact-layout.md),
[controls handoff](../frontend/world-controls-layout.md) and
[root handoff](../frontend/default-shell-integration.md) expose the actual result
and its limits. The dated sections below are historical snapshots; this section
supersedes their former compact-layout ownership and delivery status.

### Earlier visual comparison and delivery follow-through — `35bac11`

The reference again matches the user's original attachment byte for byte at the
SHA-256 above. Main inspected source-bound 390px default World and Map captures
from the qualified producer checkpoint, plus the delivered Customize option
capture. This is a fresh comparison of existing native evidence, not a newly
run live-service or physical-device test.

| Surface | Delivered evidence | Remaining reference gap and owner |
| --- | --- | --- |
| World shell | Compact header, three era controls and six icon/label destinations reach the default app. The new producer uses the existing Phaser/camera/socket. | Actual capture is dominated by movement controls, help and status text; terrain is synthetic and approved avatars are absent. A separate FE chat owns compact movement controls; FE07 retains root spacing/status composition. |
| Map and place detail | Default routes now mount the Places adapter with the same qualified map/camera. Map → Place → Back restores the Map origin; direct Place falls back to Map. | The schematic is not the reference's illustrated venue. Nested outer/inner Map headings need a clearer hierarchy; root presentation follow-up stays with FE07. |
| Customize | `bcb0428` integrates three bounded full-outfit option previews using the existing compositor, without implicit saves. | The inspected fixture is explicitly geometric TEST ONLY. Bulky categories and repeated preset grids push actions down the page. The completed avatar owner has a new compact-layout goal; approved character art remains separate. |
| Feed and social routes | `a557188` delivers the canonical Social package; `35bac11` connects its routes, providers and single navigation blocker to the actual default shell. | Explicit qualified service readiness is still required. A missing producer stays unavailable even with a C03 capability flag; mocked native routes do not certify a populated live Feed. |
| Wall | Default route now mounts the existing adapter. | No qualified mural anchor/write-policy input exists; the native default route correctly displays unavailable and emits zero writes. Editor fixture styling is not a live collaborative Wall. |
| Landing, sign-in, Meet, Gifts and My Profile | Their previously delivered visual checkpoints are retained below. | Approved illustrations/avatars and live flows retain their recorded limits. This follow-through does not claim a new visual or runtime pass for those surfaces. |

The reference is a screen hierarchy and visual direction, not permission to add
decorative levels, currency, unsupported controls or unverified venue guidance.
The inspected current captures use symmetric frames and ordinary focus styling;
no decorative left accent bars were found in these captures.

The two independent presentation follow-ups have isolated checkouts and disjoint
ownership: movement controls only (`controller.ts`, `controls.css`), and
Customize presentation only (`AvatarEditor.tsx`, `avatar-editor.css`,
`OptionStrip.tsx`). Root routes/composition remain exclusively FE07-owned. They
must preserve keyboard/DOM access, 44px targets, privacy/disposal, explicit Save,
and the existing single world owner. No new art or fallback image is authorized
by these UI assignments. Both assignments carry concrete unbudgeted goals with
implementation, focused/native validation, documentation and committed handoff.

Producer author `81866181d7ace1f3fda5650a0b8b8c9b0b11d738` is integrated as
`0b2d3a5`; its 65 assigned paths, 87 hashes and existing dependency preimages
matched. The narrow Place-parent test follows as `0d2ea12`, with an unsupported
Testing Library `exact` option corrected in `8c0e7d3` from author `21f8d60`.
Fresh combined strict TypeScript and all six Places route tests pass (exit 0).
The initial combined type failure is retained and is not counted as a pass.
The author's 119 focused tests, pre-parent-change 759-test run, actual Phaser
native checks at 360/390/430 (30 captures), and 12 fresh-document entries remain
bound to their documented candidate. Native transport/place reads are mocked;
no real two-user qualification is inferred.

Social author `504bfacb6d1e694da0c95f0e3edf3590a05c227c` is integrated as
`35bac119adc80385a5aae5146422f9bca8faa871`. All 31 assigned paths, 105 hashes
and existing input/preimage bytes match; the underlying Social/Inbox package is
unchanged. Fresh combined strict TypeScript passes (exit 0). The author's final
980 tests across 48 files, strict types, lint and production build pass; its
actual-default native Social fixture passes at 360/390/430 with nine captures,
and qualified world native checks were rerun. These use actual Phaser and
canonical Auth composition with deterministic HTTP/MockSocket fixtures. Original
privacy binding, one world owner, draft retention, Inbox preflight and Link/POP
blocking with exactly one uncertain POST are covered. Live readiness, approved
art and real multiuser acceptance remain open. FE07 continues its disjoint root
presentation follow-up; the two compact feature-layout goals are actually ACTIVE.

[World capture](../../../frontend/src/app/__tests__/default-shell/evidence/qualified-world-390.png),
[Map capture](../../../frontend/src/app/__tests__/default-shell/evidence/qualified-map-390.png),
[Customize capture](../../../frontend/src/features/avatar/__tests__/option-previews/evidence/native/loaded-390.png),
[producer handoff](../frontend/default-shell-integration.md),
[option handoff](../frontend/avatar-option-previews.md), and
[Social composition handoff](../frontend/social-composition.md) retain the
evidence and its limits. Older sections below are dated snapshots; the table
above supersedes their former statements that Map registration and visual
outfit choices were still missing.

### Latest default shell checkpoint — `a1dcf56`

The actual `WorldShell` now consumes the shared compact header, three era tabs,
icon-and-label navigation and panel header, within the original navigation and
single-world owner. Main verified all 18 assigned paths and 17 hashes from
author `a7e15fd`; fresh combined strict TypeScript passed. Three actual default
route tests and native 360/390/430 checks passed in the author's instrumented
world fixture. See [default shell evidence](../frontend/default-shell-integration.md).

Main inspected the 390px default-world capture. The black/ivory frame, typography
and six navigation controls are visible, with no decorative left accents.
The center plainly says it is an instrumented test host; era choices are
unavailable and the stage, terrain and character illustrations are absent.
This proves default layout adoption, not a finished match to the ten-screen
reference or an actual Phaser/multiuser journey. Approved character/venue art,
world illustration and complete default panel integration remain material gaps.

FE07 continues qualified Map/theme/Wall producers; the resumed social owner
continues Feed/default-provider composition, and FE13 continues bounded visual
option previews. Missing mural anchor/write authority must remain unavailable.
No duplicate renderer, invented authorization or legacy artwork is permitted.
The former Delivery reservation was released only after a terminal interruption;
[current ownership and recovery evidence](../ownership-recovery-2026-11-04.md)
supersedes historical ownership statements below. Earlier route tables remain
dated baselines until the corresponding producer handoffs are published.

### Follow-through after the initial audit

The initial comparison below is retained as a dated baseline. It is no longer a
description of every current screen. UIV01's shared concert-window components are
published in `4533121`; UIV02's actual landing and sign-in adoption is integrated
in `be644ec` from author commit `1a8fdadcaa06152fc91ea652ea25962f9c76bd34`.
See [the before/after evidence](../frontend/public-auth-visual.md). The author
confirmed viewing this exact reference and completed the bounded goal and clean
committed handoff. These two routes are **MOCK_READY**, not full visual or live
entry acceptance. Their remaining differences include approved characters,
illustrated silhouettes and poster lettering.

UIV03's actual feed, party discovery and gifts presentation is published in
`bea2e72` from clean author handoff `41adada6eff2de12a87f94a6ac65684c4597bb69`.
Its bounded goal completed and the writer stopped. The owner recorded 261 tests
and 96 native browser screenshots at 360/390/430, including the existing default
community composition. Integration checked the exact assigned paths and bytes,
121 delivery hashes and fresh combined strict types (exit 0). All 20 author
source pins match its tested commit; this audit's follow-through narrative is
the sole intentionally newer integrated pin. See [community evidence](../frontend/community-visual.md).
Qualified imagery and root shell chrome remain open; no live flow is inferred.

UIV05-MAP is published in `dda0439` from clean author handoff
`3225685477cfbebe39376453e559897242ce1b62`. Its 61 affected tests and native
360/390/430 checks passed, with three before and 54 after screenshots. Integration
verified 78 assigned paths, 77 delivery hashes, 23 source pins and fresh combined
strict types. The author goal completed and writer stopped. See
[map evidence](../frontend/map-visual.md). The published schematic is explicitly
virtual, not approved venue artwork; the default map route remains uncomposed.

UIV04-ME's actual MeHub/ProfileForm adoption is integrated in `970b272` from clean
author handoff `600bc50afdedf4db2713dd680e60f9ecd5428e26`. Its 132 affected tests
and 51 native state/width cases passed, including unchanged Settings geometry.
Fresh combined strict types also passed. [My Profile evidence](../frontend/me-visual.md)
discloses broader-suite failures separately; [environment follow-up](../qa/contract-validation-environment.md)
resolves the two missing Python format-checker failures without claiming to fix
the other three intermittent failures at that checkpoint. Subsequently,
[QA-WORLD-ASYNC](../qa/world-async-tests.md) corrected the socket/frame test
readiness races in `4b1f8c4`. [Back-readiness diagnosis](../qa/back-navigation-readiness.md),
published in `fe0c335`, traces the intermittent direct-beacon Back failure to an
assertion before the visible Meet route commits. The reserved original test is
still unchanged; its exact readiness correction remains with its owner.
Approved avatar rendering and live profile
composition remain unqualified. World/shell adoption remains open.
Sending the reference to other leads is still distinct from implementation.

UIV05-WALL is integrated in `45b6f1f` from clean author handoff
`63f95f6bad029f2f59581180d3e5ccbe5b5c2d9c`. Its 205 affected tests, 84 native
state/width cases and three keyboard/coordinate/confirmation/no-replay flows
passed. Integration verified 122 assigned paths, 105 image hashes, 23 source and
reference pins, exact source equality and fresh combined strict types. The
bounded goal completed and writer stopped. See [wall evidence](../frontend/wall-visual.md).
This qualifies the actual editor/sync presentation in deterministic fixtures,
not the default `/wall` route or live collaboration. Main inspected the pinned
390px map and loaded wall captures for this visual comparison; they visibly
adopt the compact black header, ivory frame and restrained palette while the
map's outer shell still retains its earlier presentation.

UIV04-AVATAR is integrated in `81991563d1d1b62f0238bd6de79f3197a1415fcd`
from author `395dd781237a29ac4b92f219874f54312c23f6b2`, whose exact parent is
`b3889feeed0fa7485a2326cf5bd00ffc4bb4f894`. The clean handoff has 94 assigned
paths: only AvatarEditor and its CSS change production behavior. Main verified
all seven source hashes and 85 artifact hashes, exact integrated bytes and fresh
whole-frontend strict TypeScript (exit 0). The author's 50 focused tests, lint,
build and 81 native captures remain the bounded evidence; no duplicate general
review was added. See [Customize evidence](../frontend/avatar-visual.md).

Main inspected the 390px loaded fixture and actual default account-route capture.
The frame, larger preview, category toolbar and Save/Cancel composition now adopt
the reference. The fixture preview is explicitly blank test content. The actual
account route still shows **Preview unavailable**, disabled Save and no canvas
because approved character resources are not admitted. Item choices retain ID
labels: the catalog supplies neither thumbnail URLs nor tone colors. The outer
header, theme link, repeated panel heading and text-only bottom navigation still
differ visibly from the supplied world screen. This is **MOCK_READY** presentation,
not a finished character screen or a real world/session/save journey.

The fresh reference-file check again matches the user's attachment byte for byte
at SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Freshly inspected sign-in and wall captures also show the compact black title bar,
ivory frame and deep-red actions. Their missing illustrations and fixture content
remain visible limitations, not full visual acceptance.

### Shell presentation follow-through at `0beafce`

FE07-CHROME is integrated from stopped, clean author
`d6d7d935aafbc5b03db72e1e66bea357e1b0a76f`. Its bounded goal was actually
completed and a subsequent goal read returned null. The reference was inspected
by the author and remains byte-identical to the supplied attachment.

The reusable header, three direct era controls, icon-plus-label bottom navigation
and panel header now implement the compact black/ivory/red direction. Main
inspected the actual 390px Parade, Me-panel and unavailable-era captures. Their
symmetric frames, typography and compact navigation address the earlier outer
chrome gap; no decorative left accent bars are introduced. The images explicitly
show synthetic terrain, unavailable avatar art and fixture panel content. They
are **module composition evidence**, not screenshots of a finished default app.

[Parade module capture](../../../frontend/src/app/shell-chrome/__tests__/evidence/world-parade-390.png),
[panel capture](../../../frontend/src/app/shell-chrome/__tests__/evidence/me-panel-390.png),
and [unavailable-era capture](../../../frontend/src/app/shell-chrome/__tests__/evidence/theme-unavailable-390.png)
show both adoption and the remaining visual limits. See
[shell handoff and exact root insertion](../frontend/shell-chrome.md).

The author recorded 81 affected tests, strict types, lint, production builds and
30 native Chrome captures at 360/390/430. The fixture uses actual Phaser and the
existing theme controller with synthetic qualified maps and mock transport;
confirmed theme selection, rollback, navigation, Back focus and disposal passed.
Main verified all 46 assigned new paths, 8 new source/document hashes, 51 unchanged
input hashes and 37 evidence hashes. Fresh combined whole-frontend strict types
passed after the unrelated Unicode read-path handoff was integrated serially;
[the integration receipt](../frontend/shell-chrome-integration.json) binds the
candidate and records this without repeating the author's general review.

**Default WorldShell adoption remains open.** Its root files are unchanged, and
its production ports do not yet expose the existing theme controller with a
qualified resource/lifetime producer. The module must be inserted into that
single existing shell; a second runtime or fabricated selected theme is not a
substitute. Default Feed, Map and Wall registration, approved character/venue
imagery, visual avatar item choices and live/device acceptance remain open.
The previous reachability table therefore still applies. This delivery advances
the shell implementation package without marking the screenshot reproduced or
changing task/IA acceptance counts.

### Default application reachability — root source recheck at `572473f`

The source check follows `main.tsx` into `createAppRouter(ports)`, then
`createAppRoutes(ports)` without a `composition.panels` override. The default
`featurePanels` registry in `app/router.tsx` determines the mounted features.
Declaring a URL in `app/routes.ts` alone does not mount its implementation:
`PanelScreen` in `app/WorldShell.tsx` displays **Still taking shape / is not
available yet** when it has no children. Auth/bootstrap gates can prevent entry
even before that fallback. This is static composition evidence, not a new live
browser or multiuser pass.

| Surface | Default application composition | Consequence for the reference audit |
| --- | --- | --- |
| Landing / sign in | Public boundary is registered; UIV02 actual routes are published. | Bounded route presentation is delivered; approved poster/character art and real entry remain separate. |
| Meet / gifts | Party discovery/detail and gift discovery/detail/mine are registered through `CommunityFeatures`. | These browse components reach the default composition; this does not certify every create/manage/reserve route or live workflow. |
| Me / profile / customize / settings | `AccountPanel` registers `/me`, `/me/profile`, `/me/avatar`, `/me/settings`. | Me/profile and Customize layout adoption are delivered; approved avatar art, visual item metadata and live save behavior retain their recorded gates. |
| Feed | `/feed` is declared but absent from `featurePanels`; no production `EventFeed` consumer was found in `app/`. | Updated feed component evidence is not default-route delivery. |
| Map / place detail | `/world/map` and `/world/places/:placeId` are declared but absent from `featurePanels`. Production owner/provider/route adapters now exist in `app/places-entry/`, delivered in `6c018d6`. | The adapters have bounded native composition evidence; the default map/place entry still needs qualified map/camera exposure and root registration. |
| Wall | `/wall` is declared but absent from `featurePanels`. Production sync/HTTP/scene owner and provider adapters now exist in `app/mural-entry/`, integrated from author `c91c7f0`. | The adapter preserves unresolved placement across navigation and release replacement, but needs a qualified scene/write release and default root registration. |

These are concrete root-composition dependencies, not missing acknowledgements
from leads. Delivery's reserved root ownership remains unchanged. Do not silently
edit its active checkout or treat visual fixtures as completion of those routes.
Source hashes and exact inspected paths are recorded in
[default-route-composition.json](ui-reference-audit-2026-11-04/default-route-composition.json).
All six root route-source pins remain unchanged at `572473f`. FE19-ENTRY is now
published in `6c018d6` from author `4809fad`: 150 focused tests, 18 native captures
at 360/390/430, actual FE14 loader/Phaser and FE15 camera, real document recovery
and one feature owner across navigation. Main verified the exact 38 new paths,
73 author hashes and fresh combined strict types. See
[Places entry evidence](../frontend/places-entry.md). The existing world owner
must still expose its qualified map and camera under one abortable privacy
release, and the root must mount/register these adapters. An undisclosed camera
disposal cannot be detected through its read-only public port; the producer must
withdraw the release before disposal. The detail header Back parent correction
also remains with the root owner. These are explicit integration requirements,
not acceptance of the default map route. The completed Places chat has moved to
the separate FE41 degraded-state scope.

FE33-ENTRY is integrated from the clean author handoff
`c91c7f017c6374a98225c9518374ff32c8863d55`. Its 148 affected tests, strict types,
lint, default and adapter production builds, and 21 native captures passed.
Main verified all 36 assigned paths and 35 new file hashes, all 23 author input
pins, and fresh combined strict types. The 22 executable/contract inputs remain
identical in the combined tree; the current audit prose is intentionally newer.
The native fixture uses one existing Phaser scene and the real route composition
seam with explicitly synthetic authority. It demonstrates an unknown placement
surviving navigation and release replacement with exactly one POST, plus
synchronous privacy raster erasure. See [Wall entry evidence](../frontend/mural-entry.md).
Default registration, qualified scene/anchor/write authority, approved art and
live two-user IN04 remain open. The author completed its bounded goal and stopped
before the next assignment; this handoff is not a completed default Wall screen.

FE41-STATES is integrated from clean author handoff
`c0de4c88cb1b81ef0cf838b5b9535ef9bbdd70e1`. Its offline, reconnect, resync,
maintenance, version and safe-route states use the same reference-derived
concert-window presentation. The author recorded 103 affected tests, strict
types, lint, production fixture build and 42 native captures at 360/390/430.
Integration verified all 64 new assigned paths, 63 source/evidence hashes and
44 author input pins, then passed fresh combined strict types. All 43 executable,
contract and unchanged-reference inputs still match; this audit is the sole
intentionally newer prose input. See [degraded-state evidence](../frontend/degraded-states.md).
The existing WorldHost/Phaser/FE17 fixture proves bounded recovery presentation
and no pickup POST replay. Global placement, root error routes and trusted
maintenance/version producers remain unconnected; this does not complete the
default shell or a live recovery journey. The author completed its bounded goal
and stopped before reassignment.

Integration verified exact assigned paths and source equality with the UIV02
handoff, all ten tested-source hashes and 51 final screenshot hashes, and fresh
combined whole-frontend strict types (exit 0). The author's 356 existing tests,
lint, production build and 360/390/430 browser checks remain its bounded evidence;
they were not repeated as another general review gate. The 390-pixel landing and
populated sign-in captures were inspected for this requested visual comparison.
No task or IA acceptance count is advanced by these visual checkpoints.

- Audited application commit: `6240ea72f078fb235c4fb947b50bd3264c4f7bfd`, published to `master` and independently verified against remote HEAD before this audit.
- Fresh browser captures: 390 × 844 CSS-pixel viewport, isolated browser context, dedicated Vite server, retained dependencies, Node 24.22.2. No install, application edit, backend change, or change to another worker's checkout.
- Production entry captures: `/` and `/login`, with no configured live event. Sign-in correctly reports that entry is unavailable; the missing live configuration is not called a visual defect.
- Populated feature captures use existing deterministic preview modules. Auth capabilities, feed GETs and profile GETs use existing synthetic contract fixtures. No live authentication, real user data, venue verification, stock reservation or multiuser claim is made.
- The feed preview was additionally loaded with the production `base.css`; its standalone harness otherwise defaults to browser typography. The profile harness explicitly supplies a dark background. Those harness-only differences are not attributed to production theme behavior.
- Both initial and expanded party/gift panels were captured. Expansion worked; the expanded gifts page had no document-level horizontal overflow at this width. A collapsed sheet alone is not counted as a defect.
- The world/map fixtures use synthetic terrain. They cannot establish whether unpublished art work is correct. Avatar controls were inspected in source; no approved production avatar composition was visually certified in this audit.
- Screenshots and byte hashes: [files.json](ui-reference-audit-2026-11-04/files.json). This is a visual audit, not a second review of already-reviewed feature logic.

## Was the direction handed off?

The implementation specification already requires compact navigation, text-forward community surfaces, strong borders, original stickers/pixel art, restrained decoration and a 2008-inspired direction (§6, line 153 at the audited commit). `docs/mcr/project-assessment.md` links the earlier five-composition reference and correctly distinguishes decorative concepts from feature requirements.

However, the inspected frontend working agreement and feature handoff documentation do not provide a shared, screen-by-screen reproduction checklist for this ten-screen reference. `frontend/AGENTS.md` specifies character provenance and prohibits left accents, but does not define title-bar chrome, typography, content density, icon navigation or screenshot comparison acceptance. Existing functional browser evidence therefore did not establish that the visual direction had been carried through.

The root style uses a system font and flat cream surfaces (`frontend/src/shared/styles/base.css:1–12`). Public/auth CSS adds spacious headings and rounded controls. The shell has plain title rows and text navigation (`frontend/src/app/WorldShell.tsx`, `shell.css`). This is a concrete implementation gap, not merely a missing asset file.

## Initial screen comparison at `6240ea7`

| Reference screen | Current evidence | Required correction / boundary |
| --- | --- | --- |
| Landing | [Actual root](ui-reference-audit-2026-11-04/landing.png): large text hero, long explanatory page, no concert-poster composition. | Black poster-like composition, distinctive lettering, original grunge ornament, supplied approved characters, prominent red entry action. Keep truthful unavailable-entry state. |
| Sign in | [Populated fixture](ui-reference-audit-2026-11-04/login-fixture.png), [unconfigured root](ui-reference-audit-2026-11-04/login-unconfigured.png): functional form/state, modern system heading, rounded fields, stacked provider buttons, black submit. | Cream paper within a symmetric dark frame, compact uppercase heading and red submit. Provider layout may become compact but must retain capability gating and accessible names. The illustrated Discord icon is not authorization to add unsupported OAuth. |
| World / shell | [Shell within party fixture](ui-reference-audit-2026-11-04/parties.png): six destinations exist; plain text footer and a single theme link replace the icon cells and three theme tabs. | Compact identity header, actual theme tab controls and icon-plus-label navigation around the persistent world. Preserve one Phaser instance and one world connection. Current synthetic terrain is not product art acceptance. |
| Map | [Map fixture](ui-reference-audit-2026-11-04/map.png): legend, numbered schematic and place lookup exist; introductory text dominates. | Make the illustrated map the primary surface, with labeled landmarks, concise legend and adjacent zoom controls. Keep verified-place status and virtual/physical distinction. Never copy invented physical paths from the reference. |
| Customize | `frontend/src/features/avatar/AvatarEditor.tsx` and `avatar-editor.css`: preview plus sequential native selects and textual presets; save/cancel. | Large central sprite, category toolbar, visual item thumbnails and tone swatches. Preserve accessible alternatives, explicit save and conflict handling. Random/reset are local draft actions only if added; do not relabel current cancel semantics. Approved derivatives are still required before claiming the finished character view. |
| Feed | [Populated isolated component with production base CSS](ui-reference-audit-2026-11-04/feed-base-css.png): filter select, separated notices, text author and a long thread/count button. | Compact filter tabs, avatar/name/time row, authorized image area, concise reaction/comment controls and red compose action. Image absence in this no-media fixture is not proof of a broken image loader. Source `EventFeed.tsx` has no author-avatar presentation in its row. |
| Meet | [Expanded party fixture](ui-reference-audit-2026-11-04/parties-expanded.png): tabs and capacity/state data exist, but the introductory header/filter consumes most of the first viewport. | Dense party rows with visual identity, place/time/capacity and a clear primary action; compact create action. Keep the real apply/approval flow; a visual JOIN label must not imply immediate membership or physical arrival. |
| Giveaway | [Expanded gift fixture](ui-reference-audit-2026-11-04/gifts-expanded.png): stock/time/state data and links exist, but rows are text-only and use generous blank space. | Compact browse/my-giving/my-pickup navigation, authorized item imagery, concise stock/time/place and a visible primary route to reservation. Do not bypass detail/confirmation or expose pickup credentials. |
| Wall | [Editor fixture](ui-reference-audit-2026-11-04/mural.png): canvas, zoom, palette, coordinate alternatives and explicit confirmation work in the fixture; controls extend through a long form. | Canvas-first composition with compact tool strip, swatches and status/action area. Keep keyboard/coordinate alternatives readily available without making every explanatory control dominate the canvas. Preserve explicit confirmation/cooldown/permission behavior. |
| My profile | [Profile fixture](ui-reference-audit-2026-11-04/profile.png): identity and activity links exist, avatar is explicitly unavailable; no reference-style identity card or activity tabs. | Large reviewed avatar, compact name/bio/social presentation and activity/party/gift navigation. Read history from authorized projections. Do not expose private handles or invent levels/currency from decorative reference details. Harness dark background alone is not a production theme defect. |

## Common design requirements

1. Use black, aged ivory and deep red as the shared composition baseline. Support REVENGE/PARADE/DANGER through intentional variants without changing map identity or permissions.
2. Establish compact uppercase display typography, readable body typography, black window title bars, accessible red close controls, strong symmetric borders and restrained original paper/grunge detail. Avoid generic large marketing headings and repetitive rounded cards.
3. Keep world navigation icon-plus-label, with an obvious selected state. Arrange screen content so the main useful object or first content row appears promptly; eliminate redundant introductions before lists.
4. Keep 44-pixel interaction targets, keyboard focus, contrast, reduced-motion support, safe areas and browser-back behavior. Reference pixels are not a reason to shrink touch targets or remove status/error messages.
5. Decorative left borders, colored left rails and pseudo-element left bars remain prohibited. The red rectangles around focused panel headings in these screenshots are symmetric keyboard focus outlines, not that prohibited pattern.
6. User-selected character originals and reviewed derivatives are the sole character source. Neither this collage nor old world references authorize replacing that pack or presenting synthetic metadata fixtures as product characters.
7. Do not copy the collage's levels, balances, unsupported providers, arbitrary account handles or unverified venue facts as implemented capabilities. It specifies visual/interaction direction; product and authority contracts still apply.

## Correction allocation

These are visual-correction work packages within existing product scope, not additions to the 112-task denominator. Each implementation assignment requires an isolated worktree, explicit paths and an actual unbudgeted goal. Reuse existing functional evidence and run checks affected by the change; do not add another general SL review gate.

| Package | Coordination owner | Writable scope / dependency |
| --- | --- | --- |
| UIV01 reusable window presentation | Main; assigned to `mcr(fe) UIV01 Concert window presentation` | NEW `frontend/src/shared/ui/concert-window/**` and `docs/mcr/frontend/concert-window.md` only. Build reference-derived frame, title/action controls, tabs, icon navigation and compact row examples in an isolated preview. No existing base CSS, root composition, contracts, manifests or other lane edits. |
| UIV02 public entry and shell adoption | World SL / integrator | Public/auth presentation and shell are separate exact-path assignments. Root/shell/global-token adoption remains explicitly integrator-owned. UIV01 alone is not application adoption. |
| UIV03 community screens | Community SL | Separate feature UI/CSS ownership for feed, parties and gifts. Preserve existing controllers, server invariants and each current writer's ownership. |
| UIV04 character/profile presentation | World + Social SLs | Avatar/profile UI paths and approved source-pack derivatives; coordinate existing art owner. Do not make unapproved cutouts or legacy fallbacks to manufacture a screenshot. |
| UIV05 map and wall presentation | World + Operations SLs | Place/map and mural editor presentation only, separately owned; preserve current lifecycle/archive functional assignments. |

An owner must not silently take over a file another active worker owns. If ownership is unavailable, record the exact path dependency and continue new, disjoint work. Delivery's stalled composer is not permission to edit its dirty checkout.

The audit/reference brief was sent to Delivery and the World, Community, Social and Operations leads. Sending is recorded as a handoff, not proof that all recipients have acted. The UIV01 worker (`01a1a26e-1de2-7b41-a294-c9a79cfded9b`) explicitly confirmed its isolated `codex/mcr-fe-UIV01` branch at audit commit `be85021c1e28f0b0f4a6182c4300a93b16179f5b`, created an actual unbudgeted goal, inspected the exact image and delivered the bounded package. That chat subsequently delivered UIV02, UIV04-ME, UIV05-WALL and UIV04-AVATAR in separate isolated scopes. The Avatar turn is completed and its writer stopped; a fresh goal read returned null, confirming no unfinished assignment remains. The current adoption records above supersede the initial assignment status; root composition and remaining visual/asset work stay open.

## Visual completion evidence

For each changed surface, provide 360/390/430 CSS-pixel screenshots in comparable states, with the reference alongside the result and a short remaining-difference list. Include loaded, empty, unavailable/error and pending-action states where relevant. Demonstrate keyboard focus and the primary action. Fixture evidence must say fixture; final integrated world/character evidence must use the approved art and real composition. No screen becomes visually accepted merely because its build or controller tests pass.

At this audit checkpoint, visual corrections are **OPEN**. The report and delivered instructions establish a concrete handoff; they do not claim the application has already been restyled.
