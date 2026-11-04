# Meet first-view composition — FE-REFERENCE-FIRSTVIEW-03

Status: `IN_PROGRESS` — developer validation complete; canonical publication pending. Source/native results: `PASS` for this bounded composition scope.

## Scope and behavior

Base: published `7d5d91bd1121b1a95b2302ca02e9224019a5deaa`; branch `codex/mcr-fe-reference-firstview-v3`. The previous Chrome task remains completed. This task changes only the shared flow rules in `frontend/src/app/reference-chrome-v2/chrome.css`; its new helpers and evidence live under `first-view/` and `docs/mcr/evidence/reference-first-view/`. The Meet feature stylesheet remains owned by its implementation owner and unchanged at SHA-256 `b6e0523eea6105e700d01d905b39027026feae6e5e21d4ad945bf9afa45d7f29`.

The existing shell body is a bounded flex child with vertical scrolling. Its reference flow formerly had only a minimum height, and its content used `flex:1 0 auto`. Consequently Meet's existing `height:100%` and inner discovery scroller received no definite remaining-height constraint. The natural content pushed the complete creation action and print below the reference first view.

The candidate gives the flow a definite body height, makes its content a shrinkable column, preserves the More actions summary's intrinsic height, and assigns the remaining content slot to Meet. These four rules apply only when `.party-browse .meet-discovery` exists and More actions is closed. Meet's existing discovery region owns inner scrolling. Expanded More actions and My Plan retain their natural outer scrolling; filters remain inside discovery. No rows, content, typography, selected artwork, authority or feature CSS are changed. The disabled creation action and its truthful unavailable explanation remain visible. No decorative accent rails are added.

## Provenance and dependencies

- Canonical ten-screen target: `docs/mcr/design/reference-target-2026-11-04.png`, SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`; original unchanged.
- Matched baseline: `docs/mcr/evidence/reference-chrome-v2/native-05/capture/report.json`, SHA-256 `92feccc7be2622283838f934e6d7cf83e2ed73a520f54f9f00e5530df264d662`. Meet reference viewports are 360×588, 390×637 and 430×703, derived from the retained target rectangle (295×482), not 844px height.
- New `first-view/native.mjs` uses the existing default App/router/Phaser/Meet through the labeled DEV `e2e/visual/qa06/preview.html` fixture. The unchanged preview source SHA-256 is `1c59c73e787566dbedea0e876bbf282b897bb1bf161ad41e6040ca20b492a2bd`. It supplies synthetic HTTP/domain/C05 data, not live service evidence. No capability injection or source/feature/art overlay is used. Public assets are frozen tracked HEAD blobs.
- New `first-view/round.py` adapts the published base's `reference-chrome-v2/native-round.py` (SHA-256 `33e4e3c844c2093f1175d929e282a04e09216762b84b6e3b9ba108263ba4448b`) into one focused source/native sequence. Original helper/config files are read-only. The new guardian retains original work/cleanup deadlines, private-cache handling, all-descendant accounting and cleanup, and counts both historical Chrome and new evidence.
- Reuse the retained read-only dependency tree only with byte-identical package/lockfiles. Pinned Node24.22.2 SHA-256 `b6fb419f0f43bb2fbca273700294dd2f3b685a64b60e8417c295f47646a70e4a`; existing Chrome channel. No install, build, database, new checkout or browser download.
- Approved Parade print from `ec40e53174695f03257fd7f772d91fffb12f4cf9` and controls from `65b0d5f6c5014aab6fc218d32dcf97e4c73be203` remain unchanged. `docs/source/` and selected character originals remain byte-for-byte preserved.

## Focused validation plan

Run only after a named usable Resource21d offer and explicit actual acceptance. Request one bounded 300-second work / 60-second cleanup round: one strict-port5302 Vite server, one existing Chrome with serial contexts, no installs/builds. Caps: 2GiB whole retained four-worktree/private-Git/TMP envelope; 200MiB private TMP; 16MiB complete old/new evidence; 2MiB complete logs; 93 historical/new PNGs (78 retained plus15 new), 12MiB combined PNG bytes and4MiB new; 24 descendant processes/4GiB RSS; 16GiB free at start/8GiB live floor. No automatic extension or unchanged matrix replay.

Sequence: new JS syntax; ten focused existing Vitest cases in `chrome.test.tsx` and `party-discovery/__tests__/reference-presentation.test.tsx`, single worker; new helper lint; nine changed-candidate Meet contexts (360/390/430px × matched reference,844px normal,480px short). Full TypeScript checking is not needed for a production CSS-only change without TypeScript/declaration changes.

Each native context checks complete initial CTA/copy and print, all three retained rows, actual inner scrolling at reference/short height, keyboard reachability/focus and44px controls, complete filter expansion/field reachability, all three More actions links and natural outer scrolling, reachable complete print, My Plan fallback, restored discovery, unchanged truthful authority, no overlap/horizontal overflow/broken image/page error, and one persistent Phaser canvas/world socket. Nine initial PNGs plus three short filter and three short More PNGs provide15 changed-candidate images. Compare reference captures with the retained matched baseline; do not regenerate the old15-case matrix.

## Delivery and limitations

Implementation owner performs validation and fixes and publishes a committed scoped handoff to Root153. Root's verified canonical remote receipt for the same candidate suffices; no king/SL/QA approval, separate owner readback, proof-only commit or ACK gate. Status remains active until those duties are complete.

Synthetic fixture results do not certify a live multiuser service, physical device/software keyboard, every screen's exact visual acceptance or release readiness.

## Executed evidence and developer disposition

Named offer `FE-REFERENCE-FIRSTVIEW-03-BROWSER2-202611041552Z`; actual acceptance `2026-11-04T15:52:04.941326Z`; original work/cleanup deadlines `15:57:04.941159Z` / `15:58:04.941159Z`. FULL release `15:55:31.206080Z`. Evidence: `docs/mcr/evidence/reference-first-view/round-01/` (acceptance, every failed attempt, final cohort and release). The offer's250ms guardian sampling was applied in `3b1d51ec58e6e4b4c83678cef727942711d24feb` atop production candidate `e8fbdcd6e71ff237de45424e094c89b6d83a53b7`; production CSS stayed SHA-256 `7abaf7ebc23945b75a97bef8e384a7f6dcf81b4bc0edca177ac2871109e0ad7e` throughout.

The saved guardian reports contain exact commands, timestamps and exit codes. Initial command: `python3 frontend/src/app/reference-chrome-v2/first-view/round.py --job FE-REFERENCE-FIRSTVIEW-03-BROWSER2-202611041552Z --deadline-epoch 1793807824.941159 --port 5302 --output <round-01/run>`. New JS syntax exit0; two source files/ten tests exit0; helper lint exit0. Bounded harness repairs then used the same original deadline and caps; `--native-only` retains the unchanged source-test results and runs syntax/lint/native. Final `round-01/cohort/guardian.json`: all commands exit0, native9/9 `PASS`, Chrome156.0.8078.80. No production fix or unchanged historical15-case replay occurred.

Failures remain visible: `run` stopped at the date input's native shadow-focus segment; `diagnose` added the exact target receipt; `native-fixed` completed layout/keyboard/More/My Plan and exposed a mistaken one-socket assertion while the full mobile panel deliberately suspended its hidden world. The final harness records both date host `:focus-visible`/`:focus-within` distinctions, keeps strict outline checks for ordinary controls, and reveals the world with the existing Reduce panel control before requiring one active/peak socket and the same canvas. These were harness corrections, not added product authority or route/data mocks.

The first360px reference image is reused explicitly from the retained initial attempt, with matching production CSS SHA and image hash in the final report. Fourteen additional PNGs complete the15-image changed cohort; no failed evidence was overwritten. All15 images and all three matched historical baseline images were visually inspected by the developer. No further native run is required by this scoped handoff.

| Width | Reference height | Historical action bottom / print top | New complete CTA+copy bottom | New print bottom / body bottom | Discovery client / scroll height |
| --- | --- | --- | --- | --- | --- |
| 360 | 588 | 575 / 598.75 | 487 | 575 / 581 | 252 / 388 |
| 390 | 637 | 624 / 647.75 | 528 | 624 / 630 | 285 / 415 |
| 430 | 703 | 666 / 689.75 | 583.34 | 690 / 696 | 341 / 451 |

At480px height the discovery viewport is144/128/118px for360/390/430 widths; complete print ends at467px inside a473px body bottom in all three. At844px all three original rows fit, with discovery client/scroll heights388/415/451px. Titles remain18px; row heights remain109/118/130px. Three rows are retained and reached through real Tab focus in every context. Short/reference first views intentionally show a scrollable subset instead of shrinking or removing rows.

All nine contexts passed: no horizontal overflow/broken images/page errors, no discovery/CTA/print overlap, full initial CTA+truthful disabled explanation and print, all row links/joins and More links at least44px with visible focus, all five named filter fields reached through native keyboard navigation, Show parties visible, More expansion restoring natural outer scrolling, full print reachable at the end, My Plan fallback and restoration preserving rows/authority. The native checkbox retains24px ink inside its44px label. The full panel has zero world sockets while suspended; revealing it yields one active socket/peak1 and the same single Phaser canvas.

Visual disposition: the scoped first-view clipping problem is resolved. The approved print is complete, intact and separated from CTA/copy. At reference height the third row is partially outside the bounded list viewport and remains fully keyboard reachable; at480px the list can show less than one complete tall row, with every44px target independently scrollable into view. Expanded filters keep the CTA/print stable; expanded More actions uses outer flow and can put them below the fold while keeping them reachable. Existing feature backgrounds, row density, unavailable member-portrait copy and broader header/target differences remain unchanged. This is not a claim of exact ten-screen visual acceptance.

Release peaks:11 all-descendant processes;2,512,016KiB RSS;1,478,221,824B whole retained envelope;29,661,475B private TMP;9,096,665B old/new evidence. Historical/new PNG count93, bytes7,666,873. All guards remained below their caps. Own descendants0, link absent, private TMP/cache absent, no listener and successful fresh bind on5302; free17,598,971,904B. No install/build/DB/download/shared-cache write.

Final validated helper SHA-256: native `19a3f9190b84ae818aa4d0f5333df3e36827ae4b5579e3534bbe28908b13f32d`; guardian `7af927f902e100eebc56355eaf0345c84000df4946629bfdf2c5735286d8d43d`. Feature stylesheet remains `b6e0523eea6105e700d01d905b39027026feae6e5e21d4ad945bf9afa45d7f29`. Source/public originals and historical evidence are preserved; no peer/shared-manifest/router/provider files changed.
