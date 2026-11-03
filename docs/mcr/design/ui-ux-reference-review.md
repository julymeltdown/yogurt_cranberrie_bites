# UI/UX reference audit — 2026-11-04

## Decision

**PARTIALLY CONVEYED; VISUAL IMPLEMENTATION NOT ACCEPTED.** Navigation and several functional controls exist, but the delivered frontend does not yet reproduce the supplied reference's visual identity or compact screen composition. Functional qualification remains valid within its recorded limits; it is not evidence of visual fidelity. This audit does not change the 112-task acceptance ledger or certify a live product flow.

The user supplied a ten-screen mobile reference on 2026-11-04. Its unchanged copy is [user-reference.jpg](ui-reference-audit-2026-11-04/user-reference.jpg), SHA256 `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`. This is the current screen-level design handoff. The earlier [approved mobile world sheet](../../source/mcr_v2_en/reference/approved_mobile_world.png) contains five world compositions, not these ten screens. It remains an immutable historical source.

## Evidence and limits

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

UIV03 is actively assigned to `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` in an isolated
frontend worktree for feed, party discovery and gifts presentation. Its actual
unbudgeted goal is active; its handoff is not yet delivered. World/shell, map,
customize, wall and profile adoption remain open. Sending the reference to the
other leads is still distinct from their implementing it.

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

## Screen comparison

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

The audit/reference brief was sent to Delivery and the World, Community, Social and Operations leads. Sending is recorded as a handoff, not proof that all recipients have acted. The UIV01 worker (`01a1a26e-1de2-7b41-a294-c9a79cfded9b`) explicitly confirmed its isolated `codex/mcr-fe-UIV01` branch at audit commit `be85021c1e28f0b0f4a6182c4300a93b16179f5b`, completed previous assembly goal, and newly created **ACTIVE unbudgeted UIV01 goal**. It is inspecting the actual image and implementing only the new paths above. Existing application adoption and the other correction packages remain open.

## Visual completion evidence

For each changed surface, provide 360/390/430 CSS-pixel screenshots in comparable states, with the reference alongside the result and a short remaining-difference list. Include loaded, empty, unavailable/error and pending-action states where relevant. Demonstrate keyboard focus and the primary action. Fixture evidence must say fixture; final integrated world/character evidence must use the approved art and real composition. No screen becomes visually accepted merely because its build or controller tests pass.

At this audit checkpoint, visual corrections are **OPEN**. The report and delivered instructions establish a concrete handoff; they do not claim the application has already been restyled.
