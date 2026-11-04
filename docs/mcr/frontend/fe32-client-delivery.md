# FE32 original client completion

Original **FE32 pure-client criteria: SATISFIED**. Qualification is **MOCK_READY**
with current production MuralEditor/MuralSyncPanel, canonical HTTP and frozen
synthetic C04 fixtures. No identified original client behavior remains unfinished.
Live service, FE33 producer activation, multiuser/device/IN04 and whole-reference
visual acceptance remain separate; they do not withhold this client completion.

Assignment base `1c6b8ebaf9ac054ce7708e01fd60345b6fced376`; branch
`codex/mcr-fe-FE32-client`; isolated retained `fe-ref-feed` checkout. Final native
source and full harness: `21ceef6080c302ca8d7a92ef6045db88c7d5bf70`. Later sealing
changes documentation/evidence only. Prior Party/Gift branches/artifacts remain
intact; no prior lease continues. All three new output prefixes were absent
before writes. Same new unbudgeted goal remains active through actual Root153
publication, independent fresh remote ancestry/payload verification and clean STOP.

## Changes and original criteria

The starting source already implemented the newly authored editor and reference
layout. Current FE33/root composition was inspected read-only; earlier owner
notes describing uncomposed routes are historical, not fresh runtime findings.
Two real defects were reproduced by new regressions against unchanged source:

- A third held touch was forgotten. After the first two fingers lifted, a fourth
  contact could become a preview while the third was still down. Input now tracks
  every contact until release. A clean subsequent single touch still previews.
- A delayed placement acknowledgement kept a newer snapshot's revision but
  shortened its cooldown from 20s to 5s. The result now preserves the later
  monotonic deadline, so another confirmation stays blocked. Server cooldown
  and authorization remain authoritative.

Only `editor/input.ts` and `editor/controller.ts` change product logic. UI/CSS,
public APIs, command payloads and contracts remain unchanged. No source change
is forced where existing behavior is demonstrably complete.

| Original FE32 criterion | Disposition | Current evidence |
| --- | --- | --- |
| Newly authored grid | SATISFIED | Current Canvas renderer retains128×128 confirmed raster, square cells, bounded zoom grid and no per-pixel DOM. Native checks all 16,384 cell centers against the decoded snapshot. |
| Palette | SATISFIED | All 16 actual server colors; pressed state/check/text independent of color alone. Native scroll/keyboard reaches the final color and returns to selected color2; no command sent. Controller rejects invalid/fractional indexes. |
| Pan/zoom | SATISFIED | Native mouse/touch pan, Pan-mode keyboard view control, zoom2× transformed center(64,64), Fit1× and bounded short-height view. Input tests cover clamping1×–8× and transformed bounds. |
| Pixel preview | SATISFIED | Native touch/keyboard/coordinate selection remains local; correct selected/confirmed color and coordinate displayed. Cancel and Escape discard preview. Raster/revision remain confirmed only. |
| Explicit confirm action | SATISFIED | Canvas/form Enter only previews. Actual focused Confirm activation sends exactly1POST with current captured intent; concurrent confirmation disabled. Successful response alone updates the confirmed pixel. |
| Touch disambiguation | SATISFIED | New failing/passing regression plus real Chromium three-plus-contact sequence, clean subsequent tap, touchCancel and existing long-press/lost-capture/page lifecycle tests. Ambiguous contacts never select or write. |
| Accessible DOM controls | SATISFIED | Current labeled coordinate fields, native details/form, named palette/tools/status, keyboard preview, Pan and explicit Confirm. Native reaches controls without precise canvas input, measures every visible button/input/summary at least44px and tests short-height scrolling. No screen-reader hardware claim. |
| Dragging never places pixels | SATISFIED | Native actual mouse drag and actual touch drag out-and-back leave no preview and zero POSTs; unit regression preserves threshold memory after returning to origin. Pan tool never selects. |
| Bounds/color correct at360px | SATISFIED | Native fit corner(127,127), zoom center(64,64), coordinate127/0 and rejected128 input, selected color2 confirmed at index127; all 16 colors usable, no horizontal page overflow, uniform raster mapping. Controller rejects negative/128/fraction/NaN/infinity and invalid colors. |
| Cooldown correct at360px | SATISFIED | Native synthetic server60s deadline disables Confirm, visible cooldown, monotonic test-clock expiry enables it; new delayed-ack regression preserves a newer20s deadline. Existing tests cover server-relative timing and MURAL_COOLDOWN rejection. |
| Preserve safe command/privacy states | SATISFIED | Current affected tests plus native lost response → snapshot refresh → original status/result GET, exactly1POST/no replay;202 pending remains blocked then resolves; terminal rejection removes preview without painting; paused/frozen/read-only keep viewing but deny mutation; logout removes canvas/palette/preview. |

## Visual comparison

The supplied JPEG bottom-row fourth WALL target was actually viewed before work;
its SHA256/path are pinned in the evidence manifest. All three fresh loaded
captures plus390px preview/unknown and360px cooldown were individually inspected
against that target. This is an owner comparison, not a separate SL review or a
claim that capture count proves visual equivalence.

Current source preserves compact black/cream distressed frame and type, a
portrait10:11 canvas before auxiliary text, upper/lower right tool stacks,
framed single-row palette, red Confirm and small truthful state. No decorative
left accents or unnecessary cards appear. Measured current geometry:

| Screen width | Panel height | Canvas | Canvas top | Canvas share | Palette height |
| --- | ---: | --- | ---: | ---: | ---: |
|360|635.72px|342×376.19px|71px|59.18%|62px|
|390|668.72px|372×409.19px|71px|61.19%|62px|
|430|712.72px|412×453.19px|71px|63.59%|62px|

The original target's portrait proportion is approximately1.11; current is1.10.
Both raster axes retain the same scale (about2.67/2.91/3.22 CSS pixels per mural
cell at Fit). Its compact dense composition is preserved with accessible44px
controls and expanded safety states when relevant. No further FE32 presentation
patch was required. This is **not full reference-art/pixel equivalence**: the
reference's central illustration, album title, counters and unsupported tools
are absent from the authoritative DTO/commands. The synthetic original geometric
raster is clearly fixture content; no server activity, stock, author/private data,
new character or production mural art is fabricated. All 16 colors remain
available by scroll instead of compressing targets to match an illustrative
swatch count. Whole application/reference fidelity remains separately open.

## Provenance and validation

The owned fixture/driver adapts the retained editor
`__tests__/reference-target/preview.tsx`, `preview.css`, `preview.html` and
`browser.mjs` at the assignment base. It reuses read-only sync/controller/HTTP,
canonical memory session/privacy, deterministic C04 HTTP server and original
geometric test pixels. There is no new socket, Phaser game, backend listener,
package or art asset. Source/contract input revisions and SHA256s are in
[source pins](../evidence/fe32-client/source-contract-pins.json); the final
[native report](../evidence/fe32-client/native/report.json) pins21 source inputs.
All21 were rechecked against final current source with zero mismatch.

C04 mural/operations bytes and generated bundle remain unchanged. The schema
retains its0.1.0 title with delivered own-result additions, so exact input hashes
are authoritative rather than inferring version changes. Retained lockfile,
source originals and selected character source pack are untouched. No Bigyeting
rendering/headless code or historical evidence was copied into product code.

Node 24.22.2, actual Chrome 156.0.8078.80; touch/mobile/reduced-motion at
360/390/430×844. Commands use the pinned Node binary and retained dependencies
through an owner-only temporary symlink; no install or shared target mutation.

| Command/check | Result / exit |
| --- | --- |
| New `client-completion.test.ts` against unchanged base |2 expected FAIL, exit1; initial regressions log retained |
| `node node_modules/vitest/vitest.mjs run src/features/mural/editor/__tests__ src/features/mural/sync/__tests__ src/app/mural-entry/__tests__/routes.test.tsx --cache=false --maxWorkers=1` |136 tests /11 files PASS, exit0 |
| Application `tsc --project tsconfig.json --pretty false --incremental false` |PASS, exit0 |
| New fixture `tsc --project e2e/community-client/fe32/tsconfig.json --pretty false` |PASS, exit0 |
| Scoped Oxlint on two changed logic files/new regression/new fixture |PASS, exit0 |
| Native `MCR_NATIVE_SLOT=FE32-CLIENT-NATIVE-202611040544Z node e2e/community-client/fe32/native.mjs` |3/3 widths PASS,15 PNGs, no page errors; exit0 |
| Serial Vite production build (`publicDir:false`, unique temporary output/cache, finally cleanup) |PASS, exit0; existing >500kB chunk advisory |
| `python3 scripts/mcr/check-plan.py` |PASS, exit0; planning/provenance only |
| `node scripts/mcr/check-ui-origin.mjs` |PASS, exit0; static-origin only |
| Ownership/import/whitespace/source-pin checks |Only exact granted paths; no backend imports or outside source changes; PASS |

Lossless gzip logs preserve commands/results, including the initial two failures.
The queue's offered slot name differed from the initial runner prefix guard;
commit `21ceef60` admits the actual coordinator name without scenario/product change.
A ready-pin message briefly mistyped an uncreated SHA and immediately corrected
it to actual e04f61bd before allocation. One ancillary cleanup glob then matched a
retained `.log` as a cache; corrected directory-only inventory passed. The browser
was already closed and build cleanup completed. No failed native case is hidden;
this native attempt passed all widths on the sealed source.

## Capacity, limits and delivery

One explicit accepted slot `FE32-CLIENT-NATIVE-202611040544Z` followed FE18 release.
Fresh start 32.788GiB exceeded 16GiB, STOP floor8GiB, no other validation browser;
interactive MCP/user Chrome preserved. Own browser/server/cache closed before the
serial build. Own build output/cache and dependency link removed before FULL
RELEASE directly to coordinator21d+Community. Final free 32.7895GiB; evidence
729,455bytes before final log/docs sealing, below 0.01GiB durable and 0.2GiB whole
new-output ceiling. No resource reservation remains. Historical artifacts and
failures are preserved unchanged outside the new evidence prefix.

This is component+canonicalHTTP with synthetic C04 evidence. Actual root entry,
real cookies/Origin/service authorization, durable SQL cooldown races,
FE33 live ordered streams/world texture, real restore/convergence/moderation,
physical iOS Safari/Android Chrome, screen-reader hardware and IN04 are not
claimed. All 112 tasks/61 IA remain required; no scope cut or producer prerequisite
is invented for original FE32 client completion.

Publication remains pending in the same ACTIVE goal until designated Root153's
normal serial non-force publication and owner fresh fetch/independent ls-remote,
all delivered ancestry/latest payload checks. Then clean STOP and exact lease
return complete the goal; no extra receipt commit or King/SL review gate.
