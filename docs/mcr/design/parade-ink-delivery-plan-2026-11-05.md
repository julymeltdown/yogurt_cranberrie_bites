# Parade Ink — design-first delivery amendment

Status: implementation plan, not visual acceptance. Decision date: 2026-11-05, Asia/Seoul.

The latest human choice is **PARADE INK, Photo 2**. It replaces the earlier After Dark choice. Use one app-wide appearance: light surfaces, black title bars, black selected tabs with white lettering, strong symmetric borders, and crimson primary actions. There is no appearance selector or new appearance-preference API. `REVENGE / PARADE / DANGER` remain independent World eras; the UI name must not select PARADE or alter a user's saved era.

This amendment follows joint planning by both kings and the existing UI, World, Community and resource leads. It changes design priority and records required presentation work. It does not certify any application screenshot, code, query or test. Implementation owners perform those checks and fixes.

## Reference authority

| Priority | Source | Use |
| --- | --- | --- |
| 1 | [Photo 2 — Parade Ink](parade-ink-2026-11-05/parade-ink.jpg) | Exact target for Feed, World frame, avatar popover, Wall controls, tabs, bottom toolbar and text-only post anatomy. |
| 2 | [Photo 1 — Silver Web](parade-ink-2026-11-05/silver-web.jpg) | Missing Giveaway listing/reservation/control-state anatomy, translated into Parade Ink; no silver appearance option. |
| 2 | [Photo 3 — After Dark](parade-ink-2026-11-05/after-dark.jpg) | Missing Profile/activity/avatar-editor/reply anatomy, translated into Parade Ink; no charcoal appearance option. |
| 2 | [Photo 4 — Bone + Crimson](parade-ink-2026-11-05/bone-crimson.jpg) | Missing Meet/member/inline-reply anatomy, translated into Parade Ink; no bone appearance option. |
| 3 | [Previous ten-screen target](reference-target-2026-11-04.png) | Unpictured screen structure and composition, adapted to Parade Ink. Earlier light/dark textures and palettes cannot override Photo 2. |
| Always | Selected character source pack and qualified derivatives; original specification and explicit later product decisions | Preserve character identity, original required behavior, server authority, Wall B, accessibility and actual World geometry. |

All four new originals are copied byte for byte. [The reference manifest](parade-ink-2026-11-05/reference-manifest.json) records attachment identity, SHA-256, dimensions and precedence. These are design references, not product screenshots or physical-venue provenance. Product UI must be functioning DOM/Phaser components, never a screenshot overlay. Published prior art and code remain available in Git; this is an incremental presentation change.

## What must match, and what may vary

For the repeatable visual fixture, match the fixed parts of the reference: component bounds, title-bar/tab/footer heights, insets and gaps, column alignment, typography and line heights, border weight, corner shape, icon silhouette, selected/unselected treatment, button proportions, image frames/crops, and visual density. Palette-only or frame-only adoption is insufficient.

Variable production fields are explicitly limited to authorized names and bios, post/reply text, timestamps, counts, stock/capacity, real status/permissions, selected approved avatar, user-authorized media, World actors/positions and mural pixel content. They do not make their containers, typography, padding, icons or controls variable. Different text length may cause the specified responsive wrapping/expansion; it must not justify arbitrary whitespace, clipping or a different card system.

Use deterministic, source-pinned fixture content for comparisons so text and image rendering can be compared directly. A fixture is visual evidence only, never proof of real authentication, inventory, social policy or multiuser behavior. Production uses real authorized data, without invented people, counts, levels or success states.

| Component | Fixed requirement | Dynamic or state-dependent portion |
| --- | --- | --- |
| Window frame | Black header, reference title/control alignment, strong symmetric outline; no decorative left rail | Title/active panel; collapse/close availability must correspond to actual behavior |
| Tabs/navigation | Equal-height aligned tabs; selected black/white; unselected light/black; six labeled World navigation actions | Selected tab, unread badges, capability-dependent availability |
| Feed/post | Separate text-only and media variants; compact avatar/author/time row, reference media frame, star/comment/share row, bottom toolbar | Authorized text/media/reaction counts; content height follows declared wrapping rules |
| Meet/Giveaway | Dense repeated rows; stable portrait/thumbnail, title/meta and right action columns | Membership/stock/capacity/time/place and current action state |
| Profile/avatar | Reference identity framing, activity tabs/rows and editor selection geometry; pixel-preserving approved characters | Profile fields/activity, actual catalog selection and save state |
| World/Map | Reference HUD/frame/popover/place strip/bottom navigation; legible approved art | Room actors, selected place and independent era artwork; geometry/IDs/physical review state remain authoritative |
| Wall | Reference grid/tool/palette/action anatomy translated to the real editor | Actual 128×128 content, all 16 colors, selection, cooldown, budgets, revision and permissions |

Derive the token and typography values from Photo 2 during implementation. Sheet pixels are not assumed to equal CSS pixels. The shared-component owner records reference crop bounds, the normalized 390px layout, font choice/metrics, palette, spacing, borders and responsive rules in one reusable specification consumed by other writers. The 12px/8px/1px and 24/28–16/22–12/16 proposals from planning are starting estimates only, not measurements or acceptance evidence. Do not independently invent a different token set in each screen.

Essential hit areas remain at least 44×44 CSS pixels and do not overlap; visible glyphs can retain the reference size. Preserve keyboard focus, short-height access, safe areas and keyboard-visible layouts. Any unavoidable reference conflict is named in the deviation record; it cannot be hidden by a broad screenshot mask.

## Explicit behavior and reference conflicts

The following dispositions prevent an image redesign from corrupting working behavior. An unresolved visible conflict is recorded openly; it is not silently accepted as exact parity.

| Reference element | Required disposition |
| --- | --- |
| `Lv. 12` | Sample data, not authorization for a leveling/XP system, which the original specification excludes. Never fabricate a level. Omit unsupported level data and record this reference exception; adding a genuine system requires a separate explicit product decision. |
| `Say hi` | Do not add general DM or reveal social handles. Reuse an existing authorized introduction/profile action only with a truthful label and recorded adaptation. If none fits, record the unsupported action rather than ship an inert or misleading button. |
| Guestbook in secondary Photo 3 | Not an existing feature merely because depicted. Preserve real Profile/activity behavior. A real guestbook requires a separately recorded functional delta with persistence, permissions, privacy, moderation, notifications and IA/API ownership; no implicit endpoint or pretend entries. It does not block the selected Photo 2 Feed/World milestone. |
| Reservation minus/quantity/plus | Reflect actual reservation semantics, stock and limits. Do not introduce a client-only multi-unit parameter. Retain unique reservation, expiry, idempotency and one-time pickup; unavailable quantity changes are disabled with a truthful explanation and recorded adaptation. |
| Hair / Eyes / Outfit | Already original FE13/avatar scope, not optional new features. Preserve the original catalog requirements and approved source pack. Missing required approved parts are an actual FE13/AR03/C06 gap to resolve; no synthetic fallback or silent scope cut. |
| Wall heart, small grid and eight visible swatches | Example content/anatomy, not a replacement for the real 128×128 grid, 16-color palette or chosen Wall B behavior. Preserve bounded fill/opaque erase/undo, explicit confirmation, cooldown, server budgets and DOM controls; make all required colors/tools reachable. |
| `SAVE`, `JOINED`, `CONFIRMED` | Real command/receipt state. A visual click must not manufacture persistence, membership, reservation or pickup success. Preserve cancel/conflict/retry semantics. |
| Minimize, dropdown, close, ellipsis, refresh | Name actual behavior before exposing a control: minimize to the existing panel state; close/back through current navigation; dropdown/ellipsis open a real relevant menu; refresh revalidates the current authorized query. No duplicate game/socket, automatic unsafe command replay or dead decorative controls. |
| Static example venue/people/media | No physical arrival from virtual movement, fabricated reviewed place or unauthorized media. Existing provenance, audience/revision/expiry and user/event cache boundaries remain. |

## Twelve required design families

These are child delivery groups under the existing QA06 design obligation, not 12 extra original tasks. **Initial status: 0/12 accepted.** Every family's final disposition must name reference revision, exact delivered component scope, owned paths/base, relevant states, remaining deviations and publication. The shared foundation is not complete just because colors changed.

| ID | Family and finish-to-delivery scope | Original task links | Primary required IA coverage | Reference |
| --- | --- | --- | --- | --- |
| PI01 | Shared tokens, typography, frame, fields, tabs, buttons, dialogs/sheets, statuses and common chrome; default app consumes them | FE05, FE06, FE07, FE41, QA06, QA07 | Shared dependency of all 61 entries; primary coverage counted below | Photo 2; supporting control states |
| PI02 | Feed, authors, media/text-only cards, linked content, notices, reaction row and toolbar | FE27, QA06 | F01, F05 | Photo 2 |
| PI03 | Thread, inline replies, post/comment composer, edit/delete and their state presentation | FE28, FE04, QA06 | F02, F03, F04 | Photo 2 plus Photos 3/4 anatomy |
| PI04 | Giveaway discovery/detail/own activity, authoring, reservation, pickup and owner views | FE24, FE25, FE26, QA06 | G01–G07 | Photo 1 anatomy in Parade Ink |
| PI05 | Meet discovery, applications, create/manage/rally, People and introductions | FE21, FE22, FE23, FE29, FE30, QA06 | M01–M08 | Photo 4 anatomy in Parade Ink |
| PI06 | Own/public Profile, profile edit, actual activity and preferences; explicit guestbook/level limits | FE30, FE34, QA06 | M09, U01, U03, U07 | Photo 3 anatomy and Photo 2 popover |
| PI07 | Shared portrait framing, authorized avatar popover and complete avatar-editor controls | FE12, FE13, FE18, QA06 | U02; supports W01/M09 | Photo 2 popover; Photo 3 editor anatomy |
| PI08 | World HUD, viewport/context strip, six-tab navigation and independent era controls | FE06, FE07, FE14, FE15, FE18, AR04, QA06 | W01, W06 | Photo 2 |
| PI09 | Working Wall B editor/palette/tools/confirmation, sync and degraded presentation | FE32, FE33, QA06 | R01 | Photo 2, original Wall B |
| PI10 | Landing, sign-in/signup/verification/reset/callback/legal and deleted-account views | FE08, FE09, FE10, FE11, FE36, QA06 | A01–A08 | Previous ten-screen structure with Parade Ink controls |
| PI11 | Map/place/rooms/nearby/check-in/help presentation, actual source/closed/mismatch states | FE18, FE19, FE20, AR02, QA06 | W02–W05, W07, W08 | Previous ten-screen structure plus Photo 2 World language |
| PI12 | Social/privacy/settings links, inbox, reports, unavailable/recovery, operations and archive | FE30, FE31, FE35, FE36, FE37, FE38, FE39, FE40, FE41, QA06 | U04–U06, U08–U10, X01, S01–S03, O01–O09 | Unpictured structure with shared Parade Ink components |

All 61 required IA entries have exactly one primary family; PI01 and cross-links express shared dependencies without multiplying counts. Optional original IA/tasks remain optional. No unpictured required screen is dropped.

## Incremental implementation without tangling current work

1. Preserve the current source base, valid queued patches, historical failures/qualifications and exact active ownership. The reference amendment alone does not authorize resetting/rebasing peer worktrees, replacing working controllers or discarding an older qualified patch. Publishers reconcile incoming patches at their actual base; only affected overlaps return to the responsible writer.
2. Root153 delegates **one** exact shared token/chrome/primitives writer after literal former-writer STOP. This is a scoped transfer, not ownership of every router, manifest or controller. Record isolated worktree/branch, paths/hunks, base, read-set and compatibility contract. Screen workers consume that contract and may author independent rendering in parallel; no whole-backend or whole-design-wave barrier.
3. Screen presentation writers own disjoint markup/component CSS. Existing functional owners retain API adapters, state machines, Auth, inventory, permissions, persistence and private cache behavior. Cross-boundary needs go directly to the owning developer with an explicit transfer when needed. Never mix FE and BE patches.
4. Character atlases, selected parts, anchor/frame rules and World geometry remain intact. Art workers receive exact derivative/UI-art paths and frozen input hashes. UI appearance is not permission to recolor all era backgrounds, move collision/place IDs, change cameras or regenerate characters.
5. Client rendering reuses versioned local/static assets and the shared avatar compositor/cache. Styling, opening a panel and rendering repeated portraits must not add per-component server fetches. Keep existing bootstrap/query ownership; cache keys include authorized config/manifest and user/event boundaries where required. Private data must still invalidate on account/revoke/delete changes. No new server appearance endpoint is required.
6. Developers finish implementation, relevant checks, visual comparisons, fixes, minimal evidence and committed direct publication as one coherent assignment. No king/SL technical review, screenshot approval or acknowledgment. The plan's publication starts the new design queue; later work does not wait on a ledger update.

Protected current assignments include 1eed FE38, 204 IN02, 50a8 QA01/C04, 53d Community clock/Admin, 230 Reports, 21e FE41 and Root runtime/composition. They are not spare presentation writers. Map40fde owns only its actual viewport-fit seam/loan;26e returned its consumer hunks. Lawn534 and façade/compositor231 keep disjoint art grants and input dependencies. Publisher412e is publication-only, not the former Feed CSS writer. Returned/archived titles do not confer a current lease; use actual grants. The former Social lead ca12 remains unreachable after the explicit resume attempt; FrontendKing385/allocator21d keep the existing fallback until a real handover.

## Design-first sequence and staffing

**First visible milestone:** PI01 plus a functioning PI02/PI03 Feed/Thread slice: real navigation, text/media variants, approved portraits, reply/composer, matching fixed chrome and developer comparisons at 360/390/430. A token file or isolated component gallery is not this milestone.

**Parallel next families:** PI04/PI05 Giveaway and Meet; PI06/PI07 Profile and avatar; PI08/PI09 World and Wall through their actual art/runtime counterparts. PI10–PI12 follow the same component contract and retain all existing IA. Shared dependencies unlock individual families as soon as stable; no waiting for all families or corresponding backend completion to begin source work.

FrontendKing385 and existing functional/UI leads allocate exact disjoint scopes to compatible existing chats after the published plan, including preserved completed contexts when useful. Art tasks supply the first family's missing approved resources before unrelated decorative variants. Assign a concrete unbudgeted finish-to-delivery goal; preserve an incompatible unfinished goal truthfully and record the new authorized objective rather than falsifying metadata. Preserve selected models/reasoning. Leads dispatch the next useful scope on completion or retire a finished context; no busywork to inflate active-chat counts.

Safe existing BE/runtime development continues in parallel. Source work may use frozen contracts; real IN02/IN03/IN04 gates require actual producers. Resource lead21d remains the single scheduler, using fresh measured reservations: current source3/global4/browser2/Rust1 ceilings are scheduling context, not blanket execution grants. Preserve accepted budgets and actual release/decline custody; account failure or timeout does not release a resource. No new target, cleanup or model override is authorized by this plan. New design validation gets the first fitting frontend slots; unrelated backend work does not need to stop.

The human's resume-first instruction was executed before this draft: 22 failed chats received continuations, three unfinished QA contexts were restored with custody restrictions, and active jobs were not restarted. Current activity is a staffing observation, not proof of source progress or recovered resource custody.

## Developer-owned exact comparison and completion

Each implementation owner performs this as part of its own delivery, not a separate managerial QA stage:

1. Pin application source, reference SHA/crops, approved asset/manifest versions, fixture data, browser/version, viewport/DPR and selected era. Crop/normalize the reference panels without repainting or altering the original. Record the transform; do not guess sheet pixels as CSS pixels.
2. Capture the functioning default application at 390px and changed layouts at 360/430px. Compare the relevant reference panel/component and actual output side by side and with a normalized overlay/diff. Check anchor positions, insets, border widths, text baselines/wrapping, image frames, icon/control alignment and row density. At the normalized baseline, fixed layout anchors should match within CSS-pixel rounding; intentional responsive changes must follow the recorded rule.
3. Keep the fixed-region mismatch list explicit. Tolerances may explain JPEG noise, antialiasing or fractional-pixel rasterization; they may not excuse missing controls, wrong typography, large spacing/layout differences, dark instead of light surfaces, generic rounded cards or mismatched approved artwork. Do not equate a favorable whole-image similarity score with acceptance.
4. Prefer deterministic data over masks. When masks are necessary, name each changing field/region and preserve container edges, padding, controls and clipping. Do not mask entire cards, the World viewport, avatar frames or all text. Also check long text, empty content and real permission/status changes so fixture alignment does not hide unusable production layouts.
5. Exercise applicable normal/pressed/focus/selected/pending/disabled/error/completed states, keyboard/short-height/safe-area access and supported original interactions. Check selected-era contrast/asset coherence where affected. Preserve actual iOS Safari/Android Chrome obligations and existing IN gates; a mock browser capture does not close those. Reuse genuinely unchanged qualified coverage; no automatic 61×3×3 capture campaign or QA08 load run.
6. Fix remaining fixed-region differences, publish the component patch and its concise developer disposition directly through412e. Record source/contract/reference revisions, evidence paths, limitations and canonical commit in the family record. A family is accepted only when its mapped required presentation/state work is delivered and no unapproved material fixed-region deviation remains. Unresolved semantic reference exceptions remain visible; they cannot be buried in a CSS completion claim.

## Progress accounting and unchanged obligations

The published pre-amendment baseline is **49/111 functional task outcomes**, **62 active tasks remaining**, **55/112 original criteria outcomes**, all **112 original IDs** and **61 required IA entries** retained. QA08 is the one user-deferred task, neither passed nor completed. Product readiness remains NOT_READY.

The new Parade Ink visual acceptance starts at **0/12 families accepted**. It is not added to the 112-task denominator and is not blended into a product percentage. Prior screenshots and theme/style adoption do not certify this new target. Presentation-only work does not blanket-reopen FE05/FE06/FE07/FE27/FE28 or accepted backend rows. If a developer identifies a changed functional requirement or failed original criterion, record the precise original ID/delta and preserve its earlier delivery history.

The machine register links every original task to relevant families and every required IA entry to its primary family. QA06 and IN05 retain the design completion obligation. Original scopes, original acceptance text, completion rules, historical evidence and original task states are preserved by this amendment. Authorization/privacy/inventory/persistence/recovery/accessibility/device and real integration requirements remain intact. Existing pending physical-place, Party audience, provider/claims and Profile product inputs are not answered by a style selection.
