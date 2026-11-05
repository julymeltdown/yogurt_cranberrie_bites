# PI08 Parade Ink World HUD

Status: implementation and bounded source/native qualification complete. Canonical integration is recorded separately by the standing publisher; this report does not claim a live service journey.

PI08 applies Photo 2's World frame, title, era row, context strip and six-action navigation to the existing default shell. Original scope: FE06/FE07/FE14/FE15/FE18/AR04/QA06; IA W01/W06. It introduces no new original task or functional acceptance claim for unrelated features.

## Ownership and behavior

The feature owns `frontend/src/features/parade-ink/world-hud/**`, this document and its evidence directory. The only shared edits are WorldShell imports/class/render wrappers and shell-chrome EraControls/BottomNavigation imports/return JSX. Existing children, key, authoritative props, state derivation, subscriptions, selection/persistence controller, navigation links and routes remain with their incumbent owners. No engine, socket, map, geometry, camera, avatar, auth, input authority, API or schema change is intended.

`WorldHudLayout` holds only caption visibility and a reference to its visible menu opener. The original identity child remains mounted with its original props. The World header's controls minimize/show the context caption, toggle the existing World destinations menu, and navigate to Feed. Inbox and Settings remain reachable from that menu. Escape closes it and focuses its visible opener. These are literal functional adaptations of the reference's window glyphs.

`WorldContextStrip` subscribes to the existing WorldAccess/replication readers. It checks the current session/privacy lease and aborted release before using `projectNearby`. A live authorized roster including self is labeled “in view.” That is neither room occupancy nor physical attendance. Missing/withdrawn authority and room switching show unavailable/changing state. The action opens the existing Nearby route when live, otherwise Rooms. The reference's sample “Plaza,” eight-person count, JOIN action and level are not fabricated.

Era appearance is independent of era selection. REVENGE, PARADE and DANGER still use the existing controller, pending/error status and account/event-scoped local persistence. The Parade Ink appearance must not select PARADE as a side effect.

## Frozen inputs

- Source base: `aac0d96d152e47150f4fac0f57b69659ad20a956`.
- PI01 implementation: seventeen leaves from `056a8c48ff667134dee8e5e99710185294e4b898`, plus the sole `frontend/src/app/shell.css` correction from `b5b44e47531e3b75febc5ee5cdd6a11b1648779c`, contract `pi01-photo2-v1`. That correction restores the existing fullscreen panel-height override. Its supplied status is native-unqualified.
- The fixed eighteen-leaf mode/blob/revision/size/SHA tuple array is `frontend/src/features/parade-ink/world-hud/foundation-readset.json`, SHA-256 `a9b89a8bee524066009722f020cccc869ff506f65a79b35e5d7de289a6f24fd7`, 732,123 logical bytes. Every leaf was independently checked against its immutable Git postimage. The consumer does not depend on a changing producer evidence manifest.
- PI01 tokens SHA-256: `15b68ce29df755e81849d2bc359f2c2aaade9857d5e04c32692bf9aab4700556`.
- Reference source: `f025bdb6fb409f3b7706e475dfabb5f3d21fe168`; Photo 2 SHA-256 `9e7f17a666e1ecfc738a14b75bb3a166235a05c401f206aa24f26cf7463cbc27`.
- Original: 1280 × 853; World crop `[436,60,832,847]`; normalize width by `390/396`. Original image stays unchanged.

The validation overlay reads exact PI01 postimages with `git show revision:path` and verifies their hashes before/after use. Font URLs resolve to those immutable bytes in memory for the fixture; no producer working-tree stylesheet or font is an execution dependency. It neither copies token values nor writes the foundation checkout. PI01 dependency publication must be reflected in the final canonical receipt; this source checkpoint alone does not ship the combined appearance.

## Validation boundary

SOURCE01 stopped before Vitest ran because the harness requested an unexported Vitest package path. It reached FULL release at 2026-11-05T00:10:53.329229Z with both guards stopped, no owned children, and the dependency link/private scratch removed. Its failure receipts and log remain intact; no HUD test, typecheck or lint ran.

SOURCE02 passed all ten synthetic HUD cases, scoped feature/shared-caller lint and six harness syntax checks at source `df5445fb5ccaa68bf7e9bdd3895ea37221765a32`. Full frontend typechecking failed on two fixture type annotations and external generated-contract Zod resolution. FULL release was at epoch `1793837815.109966`, with both guards stopped and no retained link, scratch or children. The primary and independent guards recorded a maximum 163,235,249-byte cumulative NEW charge and 5,278,744,576-byte WHOLE charge; subsequent rounds retain those high-water marks.

SOURCE03 failed its frozen-input check before any validation child: a producer evidence manifest changed while queued, although the eighteen production postimages did not. FULL release was at epoch `1793838469.949436`; tests, types and lint were not run. The original failure remains preserved. The overlay now reads the consumer-owned immutable tuple array above.

SOURCE04 passed under its distinct finite offer. It confirmed byte-identical TypeScript-erased test JavaScript before retaining the ten SOURCE02 passes, then passed the full nonincremental frontend typecheck with zero diagnostics, changed-file lint and driver/overlay syntax checks. The two fixture annotations are type-only, and the compiler resolves Zod through the existing read-only dependency installation. Actual FULL release was at epoch `1793839351.539408`; both guards stopped, with no owned children, dependency link or private scratch. The nonshrinking NEW maximum increased to 172,512,689 bytes; WHOLE remained 5,278,744,576 bytes. No unit replay or native execution occurred. Browser work requires a separate fresh offer.

Native qualification used actual `main.tsx`, canonical Auth, existing providers/router and the current Map4/approved static C06 inputs with explicitly controlled HTTP/C05 authority. It covered 390, changed 360/430, keyboard/focus, short height, reduced motion and nonzero safe-area behavior; verified 44px targets, independent persisted eras and one incumbent game/camera/canvas/socket; and produced unmasked normalized-reference/actual-App comparisons with literal geometry, typography, spacing, controls, state and scenery deviations. A gallery, palette match, build or synthetic source test is not native acceptance or a live-service claim.

Native progress is recorded without converting failed rounds into full passes:

| Round | Qualified work | Stopping point |
| --- | --- | --- |
| Native01 | No app checks ran. | Own controller identity comparison rejected relative launch spelling; FULL epoch `1793839868.9184399`. |
| Native02 | Actual App at 390×775, 390×844, 360×800 and 430×932; loaded pinned fonts, type/ink, 44px targets, no horizontal overflow, settled incumbent camera, one game/canvas/socket; reversible caption, menu Escape focus, Nearby/back; REVENGE default and all three era choices. | Feed correctly opened a full panel, hiding accessible navigation; the harness used a visible-role locator for the subsequent state assertion. FULL epoch `1793840244.1515179`. |
| Native03 | All six destination routes and selected DOM states, incumbent continuity, DANGER retained across navigation. | Harness requested the nonexistent CDP `Emulation.setSafeAreaInsets`; the retained protocol specifies `Emulation.setSafeAreaInsetsOverride`. FULL epoch `1793840678.937908`. |
| Native04 | No app checks ran. | Initial census rejected a changed ordinary publisher script; the later parser resolves the current factual role instead of one historical basename. FULL epoch `1793841360.1899142`. |
| Native05 | No additional case is claimed from this interrupted round. | A bare ordinary-observation receipt was misparsed; overlapping stop signals interrupted cleanup. The raw fallback falsely said no workload started. It remains unchanged; `native05/FULL-correction.json` records verified cleanup at `1793841825.976037`. |
| Native06 | Additive safe-area padding 22px for 20px inset; 390×500/reduced motion; unmasked reference pair and overlay. | Canonical anonymous gate offered a Sign in link before the login form; the harness expected the form immediately. FULL epoch `1793842115.66416`. |
| Native07 | No additional app case is claimed. | Census rejected an accepted byte booking with a null controller; accepted unstarted execution reservations can also have no controller. Cleanup conservatively retained ACCEPT until fresh verification. Corrected FULL epoch `1793842547.73936`. |
| Native08 | PASS: actual new document and canonical Sign in link/form; DANGER restored despite bootstrap REVENGE; one incumbent canvas/socket; positive roster withdrawn to Presence unavailable/Rooms; all fourteen observed public assets match frozen Git blobs and SHA-256. Both guards passed twenty-six isolated census cases. | Completed with zero page errors; literal FULL epoch `1793842664.464797`, within the original deadline. Native02/03/06 passed blocks were retained, not replayed. |

The final harness retains the immutable Native02/03/06 reports and the exact measured initial 390×775 image. It does not repeat the passed width, era or navigation matrices. A fresh document uses the same explicit controlled scenario selectors before browser reload; those selectors choose a fixture and grant no production authority. Both owned guards enforce complete inode accounting and actual process birth/ancestry/global reservations. Each finite round is permanently closed at FULL, before diagnosis or documentation.

The measured reference-height App has a 48px title, 44px era controls with 18/22px display text, a 72px context strip, an 86×44px action and 58px navigation links with 14/18px labels. The chosen reference rectangle puts the context 16.5px higher and 2.65px narrower after normalization; this is an explicit geometry difference, not a masked comparison. The reference's scenery, selected PARADE era and illustrative occupancy/Join copy are separate from the current Map4, independent era selection and authorized replication context.

Resources return directly to the allocator and named successor before final documentation. Publication goes directly to the standing publisher; Root retains runtime/custody, with no added technical review or acknowledgment gate.


## Final evidence and limits

The compact machine-readable result is [final-result.json](../evidence/parade-ink-world-hud/final-result.json). Native08 source input is `34268c0fb`; frozen readset SHA-256 is `385413256171e13e141a2de4bdb1f8235fd5d756e20b8bfaee6d47bfd4c39e0a`. Chrome 156.0.8078.80 ran at DPR 1. The exact browser executable/plist, actual Python executable, Node binary, source and fixture hashes remain in each frozen readset. The final report links retained evidence by its SHA-256 rather than recasting failed whole rounds as passes.

The unmasked [reference/App pair](../evidence/parade-ink-world-hud/native06/output/photo2-comparison.png), [50 percent overlay](../evidence/parade-ink-world-hud/native06/output/photo2-overlay.png), [nonzero safe area](../evidence/parade-ink-world-hud/native06/output/world-safe-area.png), and [short/reduced-motion App](../evidence/parade-ink-world-hud/native06/output/world-short-reduced-motion.png) were visually inspected. Title/frame/tab proportions and black/white/crimson treatment are close to the reference. The tab gaps, heavier display font, context position, approved Map4 scenery/actors, live-reader wording and functional window-control semantics are literal differences. None is masked. The offline capture preserves the incumbent warning above the World; that warning pushes the context below its viewport crop. Withdrawal is asserted against the actual DOM, with no stale count and a Rooms destination, rather than inferred from that cropped image.

The fourteen served-asset records in `native08/output/native.json` include their URL, source path, Git blob, byte count and SHA-256. They cover the actual map/roof textures and layout, canonical Map4 JSON, the approved static avatar, label font and entry assets observed during this journey. The broader 1,416-file public inventory is a frozen lookup, not a claim that all assets were requested. Map4 remains `mcr_reference_scene_r3` revision 4, content hash `736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8`, geometry hash `899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f`. Approved C06 static v1 remains hash `0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b`, preset `photo01_front_01` revision 1.

The qualified app uses controlled HTTP/C05 fixtures, current-date 30-second tickets, normal Auth/providers/router and a controlled SOCIAL baseline. It does not prove production transport, live backend integration, physical attendance, unrelated feature availability or the full PI01 visual family. The source-only ten HUD cases and later full type/lint checks have their separate Source02/04 receipts.

At observed canonical master `3a125d952cc55afbfac028afcafd7748dffaa056`, fifteen of the eighteen PI01 dependency leaves match the native fixture exactly. Three newer published leaves differ: `WindowInk.tsx` adds the panel-toggle triangle, `chrome.css` corrects its rotations, and `base.css` layers the two legacy background imports. Integration must preserve those published fixes. The visible World header uses its own scoped inline glyphs; this is static compatibility evidence, not a rerun of the current combined master.

Native08 primary/guardian maxima were cumulative NEW 467,695,025 bytes, WHOLE 6,889,254,912 bytes, thirteen simultaneous owned processes and 3,135,881,216 bytes RSS. Minimum available disk was 23,324,712,960 bytes. The amended shared NEW ceiling was 469,762,048 bytes; the entire additional 16 MiB final-document allowance and 8 MiB guard reserve were already included. Native07's missing primary final receipt was conservatively carried with an extra 4 MiB rather than replaced by zero. No deletion credit, clock extension or closed-round restart was used.

Both guards and workload exited; owned children, dependency link and private scratch were absent at FULL. Release was sent directly to allocator, Root and the named successor before this documentation. The immutable publication manifest supplies exact own postimages and limits the two shared integrations to their authorized hunks; publisher preserves current unrelated code and foundation fixes. No Main/SL/Root technical review or screenshot acknowledgment is a publication gate.
