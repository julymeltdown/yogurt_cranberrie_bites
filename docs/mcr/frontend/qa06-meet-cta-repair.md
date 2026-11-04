# QA06-MEET-CTA bounded repair

The full Create party action and its truthful availability copy remain visible above the Parade footer when the Meet panel is short. Discovery content scrolls inside its available space; the three party rows retain their sizes and original appearance.

This is a bounded FE21/FE23/QA06 repair, qualified as **MOCK_READY** with desktop Chrome evidence. It does not close any whole parent task or certify physical devices, live services, or release acceptance.

## Source and ownership

- Base: `819d30c3cb32915862281d30cddbe5b7c9bd53cb`; branch `codex/mcr-fe-QA06-meet-cta`.
- Reporter candidate: `980fc4d85020f297b36fee8195e1ca92079ff33f`. Current base exactly matches its feature hashes: CSS `56adebbc5aa174b62c229e91377ed92065fd7c823078a281be176b64875949fd`, TSX `687d59c29d6155ead1df9152391af9f5422e6441b60b51b15df7084b0582f148`.
- Community explicitly extended the original CSS geometry lease to `.party-browse .meet-discovery` declarations `min-height` and `overflow-y`. Actual production edits are limited to this selector, the existing browse height/min-height, and footer flex shrink. No row-height, TSX, controller, root, shared shell, capability, contract, or artwork edits.
- New owned paths: feature `__tests__/qa06-meet-cta/`, evidence `docs/mcr/evidence/visual/qa06-meet-cta/`, and this report. No previous assignment rights were reused.
- Final CSS SHA256: `6e7c259074016d9f05d90cba8a3355403906a4330a384548302c8abe2b564e54`.
- Core/domain HTTP `0.1.0`, C05 wire `1.0.0`; no interface changes. Exact contract and fixture hashes are in [contract-input-pins.json](../evidence/visual/qa06-meet-cta/contract-input-pins.json).

## Reproduction and behavior

The owned preview adapts the QA06 owner's read-only `frontend/e2e/visual/qa06/preview.tsx` fixture; its SHA256 is retained in the preview header. It imports deterministic existing fixture logic rather than copying product UI. It mounts the actual canonical Auth, StrictMode app router, default production world loader and selected whole-character renderer. Synthetic HTTP/C05 and explicitly test-only map admission remain test inputs, not product authority. All sparse PNG/font responses are allowlisted tracked Git bytes; hashes are in the native report. No assets were restored or altered.

Input is `/meet`, full panel, REVENGE, clock `2026-11-07T07:00:00.000Z`, initial body scroll zero, and the same three schema-validated parties: Area A 12/20, After Show 8/12, Photo Walk 5/8. Public member portraits remain unavailable; no synthetic roster is added. Creation remains disabled under the actual production presentation binding.

At 390×637 before the fix, the scroll body's bottom is `563.703125px`, the action bottom is `574px`, and the copy bottom is `592.75px`. The action is partially clipped and the copy is below the visible boundary. After the fix, the action spans `487.953125–533.953125px`, and the copy spans `538.953125–552.703125px`. Both are fully visible and hit-testable. The three row rectangles are unchanged: 110px high, starting at y111, y226 and y341.

The feature now has a bounded 100% height and zero minimum height. Its discovery section may shrink and scroll, while the action footer cannot shrink. This reserves real layout space for the action without overlays or shared-shell changes. At reduced heights, later rows and filters are reached through the discovery scroller. Filters need scrolling at 390×637; all three rows and the full action remain visible initially.

The same-input screenshots are [before](../evidence/visual/qa06-meet-cta/before/meet-390x637.png) and [after](../evidence/visual/qa06-meet-cta/after/meet-390x637.png). [The baseline negative check](../evidence/visual/qa06-meet-cta/baseline-negative-check.json) retains the expected failing full-visibility assertion on unchanged source. [Before metrics](../evidence/visual/qa06-meet-cta/before/report.json) and [final qualification](../evidence/visual/qa06-meet-cta/after/report.json) pin their source and harness hashes. The before capture predates the later touch-scroll and expanded-target assertions; the product input is unchanged.

## Validation

Node `v24.22.2`, installed Chrome `156.0.8078.80`, touch-enabled mobile viewports, reduced motion, retained dependencies only. Each native/build invocation rechecks at least 8 GiB free and uses a unique reserved port and temporary Vite cache. Owned browser, server and cache resources close in `finally`.

| Check | Result / exit |
| --- | --- |
| `node src/features/party-discovery/__tests__/qa06-meet-cta/native.mjs before` on baseline CSS | Expected clipping reproduced; exit 0 |
| Same runner `after` against baseline CSS | Full-action visibility assertion correctly fails; exit 1 |
| Same runner `after` against final CSS | 7/7 PASS; exit 0 |
| `node node_modules/vitest/vitest.mjs run src/features/party-discovery/__tests__/reference-presentation.test.tsx src/features/party-discovery/__tests__/ui.test.tsx src/features/party-discovery/__tests__/controller.test.ts --cache=false` | 46/46 tests, 3/3 files; exit 0 |
| `node node_modules/typescript/bin/tsc --project tsconfig.json --pretty false` | PASS; exit 0 |
| `node node_modules/oxlint/bin/oxlint src/features/party-discovery/__tests__/qa06-meet-cta/preview.tsx src/features/party-discovery/__tests__/qa06-meet-cta/native.mjs` | PASS; exit 0 |
| Vite production `build({configLoader:'runner',publicDir:false,cacheDir:<unique /tmp>,build:{outDir:<unique /tmp>,emptyOutDir:true}})` | PASS; exit 0; existing >500 kB chunk warning; temporary outputs removed |

The final native matrix is 390×637, 360/390/430×844, and 360/390/430×420. Every case asserts full initial action/copy visibility, 44px feature controls (including expanded form controls), no document horizontal overflow, one canvas and at most one world socket. Native touch scroll advances the discovery scroller at 390×637 and all three reduced-height widths while preserving visible action/copy. Touch navigation exercises filter expansion/input/submit, People and browser Back, MY PLAN and Parties, final JOIN and browser Back. Unavailable creation stays disabled. Existing component tests additionally cover the explicitly supplied enabled Create party link.

The production world can suspend networking while a blocking panel is open; these runs correctly observe zero sockets, not a fabricated active connection. The harness's initial exactly-one-socket assumption was corrected to at-most-one after observing this legitimate state. An ambiguous People locator was narrowed to its feature ID. Neither required product changes.

Physical iOS Safari, Android Chrome, actual software keyboard, nonzero hardware safe areas, real backend authorization/multiuser journeys and other QA06 screens remain **NOT_RUN** here. The independent QA06 owner owns its recapture and parent qualification. No Meet/Gifts row-height change or Gifts Reserve product change is included.

## Delivery

Only the designated Root153 publisher integrates this exclusive handoff into `julymeltdown/mcr/master`, serially and without force-push. The handoff names the local commit, base, complete path list and validation; the publication receipt supplies its delivered mapping. The worker verifies fresh remote ancestry and exact payload hashes before completing its goal. This report's source hashes remain stable through documentation-only publication metadata.
