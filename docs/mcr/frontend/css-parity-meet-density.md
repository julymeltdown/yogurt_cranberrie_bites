# Meet first-view density

Task: `CSS-MEET-DENSITY`. Status: `MOCK_READY`; developer validation passed all
nine native browser contexts on the existing deterministic fixture. Committed
handoff to Root153 is the publication route. This closes the scoped first-view
card-density defect; it does not close an original task or full visual parity.

The responsible developer owns implementation, validation, fixes and committed
handoff. Root153 publishes the scoped handoff. No king/SL review or acknowledgment
is required. The existing unfinished Meet goal remains recorded by the goal API
as BLOCKED from the earlier disk interruption; this assignment resumes its work
without falsely completing or replacing that objective.

## Candidate and ownership

- Isolated branch: `codex/mcr-fe-meet-density`.
- Published base: `3cbaeae5e5c33d4ef3327b6f0f3e0f27c9544254`.
- Validated source checkpoint: `0024191782bcdbe6eca34d5f247d1b45df6fb202`.
  Subsequent handoff changes contain documentation/evidence only.
- Root renewed sole-writer ownership of
  `frontend/src/features/party-discovery/ui/party-discovery.css`, this document,
  and `docs/mcr/evidence/css-parity-meet-density/**`.
- Shared composition dependency: developer213's qualified
  `7a87414a212f5d202360db4980909a5d8e123193`, published as
  `40a40c3caa90e4b78ae48f179ab0b9de0a87f752` and already in the base.
  `frontend/src/app/reference-chrome-v2/chrome.css` is unchanged, SHA-256
  `7abaf7ebc23945b75a97bef8e384a7f6dcf81b4bc0edca177ac2871109e0ad7e`.
- Feature CSS before: `b6e0523eea6105e700d01d905b39027026feae6e5e21d4ad945bf9afa45d7f29`.
  Density candidate: `4bcd821855f976b558c5fd979ce759a97cbb1d4032fdf445dfdf3f468833b6db`.

No TSX, shared/root composition, artwork, manifest, lockfile, World or backend
change belongs to this patch. Preserved source material is unchanged.

## Change and reference

The saved qualified 390px image shows two complete rows and a clipped third row,
while the complete disabled Create action, its explanation and Parade print are
visible. The supplied Meet target shows three rows. The current implementation
must additionally retain More actions, accessible controls and truthful authority
and unavailable-portrait explanations.

The candidate replaces the proportional 109–130px minimum card height with an 80px
minimum, changes vertical card padding to 5px, removes the 23px empty reservation
for missing member portraits and tightens metadata/unavailable-copy margins.
Rows still grow with their content. Title 18px, metadata 14px, unavailable copy 10px,
portrait dimensions/scale and 44px title/Join controls are unchanged. All three
rules retain their original colors, content, link targets and selected art.

References are preserved:

- Original supplied photo SHA-256:
  `2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
  Existing Meet crop: `docs/mcr/evidence/css-parity-meet/reference-meet.png`.
- Official target: `docs/mcr/design/reference-target-2026-11-04.png`, SHA-256
  `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
  Meet bounds `(307,539,295,482)` define reference heights 588/637/703 at
  widths 360/390/430; 844px and 480px heights exercise ordinary and short panels.
- Qualified baseline report:
  `docs/mcr/evidence/reference-first-view/round-01/cohort/capture/report.json`,
  SHA-256 `4062dc45d58b437ec15872355773887a002767244142a57d0d8b2197df28763a`.
  Its saved screenshots are reused for comparison; the unchanged baseline is
  not rerun.

## Validation provenance and scope

The new `native.mjs` and `round.py` under the assigned evidence directory adapt
developer213's published `frontend/src/app/reference-chrome-v2/first-view/`
helpers from 7a87414a. The borrowed native helper SHA-256 is
`19a3f9190b84ae818aa4d0f5333df3e36827ae4b5579e3534bbe28908b13f32d`.
Their imports resolve the retained frontend dependencies; no dependency is
installed or version changed. Pinned Node 24.22.2 and unchanged pnpm-lock SHA-256
`47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6` apply.

The single changed candidate executed the actual default App/router/Meet/
Phaser components through the existing labeled DEV synthetic HTTP/C05 fixture.
The driver records source and requested-asset hashes, exact geometry, retained
copy/font metrics, complete first-view row count, native Tab focus, filters,
More actions, My Plan and restoration of the same world canvas/one connection.
This is deterministic browser evidence, not a live-service or physical-device
acceptance claim. Production creation and member-portrait authority are not
injected by the fixture or CSS.

The supervisor was installed before child jobs and enforced the accepted time,
disk, process/RSS and output limits, then records cleanup of its own browser,
server, dependency link and temporary directory. No output-producing job starts
before a named offer, explicit acceptance and a fresh 16 GiB start sample.

## Result and commands

Resource offer: `CSS-MEET-DENSITY-BROWSER1-202611041623Z`, explicitly accepted
with full candidate/tool hashes and a fresh 16,858,148 KiB free-space sample.
The native run was 2026-11-05 01:23:08–01:23:54 KST; the evidence retains its
original UTC timestamps. Installed Chrome 156.0.8078.80 was used. No unchanged
baseline, install, application build, database or performance run was executed.

```sh
python3 docs/mcr/evidence/css-parity-meet-density/round.py \
  --job CSS-MEET-DENSITY-BROWSER1-202611041623Z \
  --cleanup-deadline-epoch 1793809687 \
  --port 5298 \
  --output docs/mcr/evidence/css-parity-meet-density/round-01 \
  --baseline docs/mcr/evidence/reference-first-view/round-01/cohort/capture/report.json
```

The absolute deadline above is historical; rerunning requires a new allocation
and deadline. The supervisor, `node --check` and native driver all exited 0.
No TypeScript source/declaration changed; a compiler or mirrored unit test was
not needed for this three-rule CSS change.

| Width × reference height | Before card height | After card height | Discovery viewport | Complete rows before → after |
|---|---:|---:|---:|---:|
|360 × 588|109px|80px|252.25px|2 → 3|
|390 × 637|118px|80px|285.25px|2 → 3|
|430 × 703|130px|80px|301px after natural shrink|2 → 3|

All three rows remain fully visible at 844px height. At 480px, one complete row
fits initially; native Tab reaches all three titles and Join actions through
the real inner scroller. The title and Join targets remain at least 44×44px,
with retained 18/14/10px text and complete copy inside each card. Filters remain
reachable even at the tight 360px reference height; they are not hidden when
below the initial discovery viewport. Native keyboard tests also passed for
the filter controls, the three More actions, My Plan and Parties restoration.
Reducing the full panel restored the same Phaser canvas and exactly one active
world socket; the full panel's existing suspended state was preserved.

CTA/explanation and Parade print y-coordinates match the qualified baseline in
all nine contexts. Eighteen requested asset hashes match the baseline, with no
new asset request or changed common hash. No unavailable copy, capability,
portrait or selected artwork was substituted.

Developer image assessment used the saved 360/390/430 reference captures and the
390px short/filter/More captures, plus 360/430px short filter captures. Each
reference capture shows all three cards with readable retained content and
complete CTA/copy/print. Short-panel screenshots show the expected scrolling
tradeoff and visible native keyboard focus. The supplied target's member
portraits, smaller/absent functional toolbar and different shared composition
remain separate boundaries; this result is not a pixel-identical screen claim.

Evidence under `docs/mcr/evidence/css-parity-meet-density/round-01/`:

- `capture/report.json`: 9/9 PASS, geometry, content metrics, keyboard records,
  source/asset hashes and 15 native screenshots. `report.partial.json` retains
  the incremental job record; it is not another run.
- `guardian.json`, `syntax.log`, `native.log`: exact commands/exit codes and
  bounded-supervisor observations.
- `qualification.json`: scoped `MOCK_READY` disposition and baseline comparison.
- `release.json`: actual `FULL` cleanup and successful 5298 reuse-bind.

Peak resource observations were 12 processes, 2,472,592 KiB RSS,
1,344,147,456 whole bytes, 29,966,228 temporary bytes and 1,318,555 output bytes.
Fifteen new PNGs total 968,463 bytes. All accepted caps and the original work/
cleanup deadlines passed. Own browser/server descendants, dependency link,
private temporary/cache directory and listener were gone before release;
port 5298 could be rebound. No further output job ran after release.

## Remaining boundaries

The feature cannot author absent member-portrait markup or a server-qualified
Create capability through CSS. Selected portraits and shared chrome/print stay
with their existing owners. The supplied target omits some retained functional
controls and truthful unavailable text. Full visual parity remains open wherever
those constraints or longer content prevent the target composition; any measured
first-view limit will be recorded rather than hiding information or reducing
readability or touch targets.
