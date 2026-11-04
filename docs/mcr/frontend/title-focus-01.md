# TITLE-FOCUS-01 — title focus backing feasibility

Status: **IN_PROGRESS**. The original static lease is proven **NO_PATCH**.
Root subsequently granted a state-dependent background-only continuation.
Implementation, actual validation and publication remain unfinished. This is
implementation-owner evidence, not a king/SL review gate.

## Assignment and ownership

The prior ID01 assignment was delivered and closed. Startup for this task found
no current goal (`get_goal` returned null); a new unbudgeted goal is **ACTIVE**.
It covers constraint feasibility, a compliant correction if possible, meaningful
owned validation/fixes, documentation, committed handoff, Root publication and
allocated actual evidence. It was not narrowed to make this calculation count
as an implemented fix.

Exclusive checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-title-focus-01`.
Branch: `codex/mcr-title-focus-01`. Fresh published base:
`9e7952fcd89360f7b4fe43c69f22060b9aa55c74`. The worker's unclaimed provisional
`codex/mcr-fe-title-focus-01` branch was renamed locally to the required name;
no foreign branch/checkout was overwritten. The old 2251 ID01 checkout remains
untouched. Startup was clean.

Root confirmed the former writer and shared-CSS incumbents stopped before
transferring sole write ownership. The application grant is only the existing
`.mcr-chrome-panel-header` rule's `background-color`, `background-image` and
`background-size` in `frontend/src/app/shell-chrome/shell-chrome.css`.
Background position/repeat, new selectors, focus/outline/foreground/text,
TSX, controls, layout, geometry, character/scenery and producer paint changes
are excluded. Decorative left bars are excluded. Exact additional owned paths:
this document, `docs/mcr/evidence/title-focus-01/**`, and new meaningful source
fixtures under `frontend/src/app/__tests__/title-focus-01/**` if needed.

The unchanged CSS is 5,451 bytes, SHA-256
`08614a9b3263d7cbb27af4d59aaabc0ee4db07943812faa0e04bdba7f720291d`.
Its bytes also match production `8972b0b6` used for the retained focused-link
evidence. Root remains the sole publisher/capture custodian; Resource21d alone
allocates output checks. No independent browser, server, build, media export,
dependency install or packet copying/restoration is authorized or attempted.

## Uniform backing is impossible

The frozen ring `#a2232b` has relative luminance **0.0905786376**. Its greatest
contrast against any darker backing occurs at black and is only **2.8115728:1**.
Therefore achieving 3:1 requires a lighter backing with luminance at least
**0.3717359127**.

The frozen cream text `#f2eadf` has relative luminance **0.8305059579**. To retain
4.5:1 text contrast, its backing luminance must be at most **0.1456679906**.
These intervals are disjoint. The current dark `#251017` yields **2.4130529:1**
for the ring and **15.1140137:1** for the cream text. A uniform recolor cannot
meet both requirements.

## Spatial paint also conflicts with legitimate unfocused glyphs

Immutable native03 packet commit
`4600542ba8f5d16210075c93847fa60960809540` retains nine actual candidate Map
screenshots: three eras at 360/390/430px. The title ring occupies rows 21–23
and 41–43, plus the three-pixel vertical side strips. The left strip is x20–22
at every width. Actual visible MAP glyphs, including antialias pixels, are
x23–60/y24–40; the focus outline covers the preceding glyph columns.

A snapshot-only symmetric paper frame could seemingly separate those *currently
visible* glyphs from the ring. Paper `#e9e1d4` would give ring contrast 5.7567:1,
with dark backing under the remaining visible glyphs. This model is rejected:
it ignores glyphs that become visible when title focus moves elsewhere.

The actual immutable native02 capture
`docs/mcr/evidence/cozy-backgrounds-v1/native-02/candidate-me-focus-0-390.png`,
commit `5186f11017185a040e4ccde1c59589e140563ef0`, focuses the Edit profile link.
The shared Me title is therefore unfocused. Its full cream glyph cores occur
at x21 and x22, including coordinate **(21,24)** and rows 24–40. The shared
`PanelHeader` uses the same h1, font, unpadded title box and unchanged CSS.

At exactly **(21,24)** the retained focused Map PNG is `#a2232b`, while the
legitimate unfocused Me title PNG is `#f2eadf`:

| Required state | Actual foreground at (21,24) | Required backing luminance |
| --- | --- | --- |
| Focused Map title | Ring `#a2232b` | At least 0.3717359127 |
| Unfocused shared Me title | Glyph core `#f2eadf` | At most 0.1456679906 |

The three allowed background properties supply the same static spatial paint
for that shared header coordinate; they contain no focus-dependent branch.
The same backing pixel cannot lie in both intervals. One actual valid 390px
counterexample disproves a robust all-state/all-width correction of this shared
rule. Top/bottom stripes also fail the vertical ring requirement. A fixed paper
frame would degrade existing text and is not a compliant patch.

The evidence does not invent an unfocused Map capture or unfocused360/430
captures. The counterexample is the actual shared Me title state affected by
the proposed global header rule, not a font approximation. No new left rail,
focus border, glyph mask or product image is proposed.

## Historical source calculation and explicit scope decision

The source reader/calculator is
`docs/mcr/evidence/title-focus-01/feasibility.py`. It reads immutable Git blobs
in memory and uses Pillow for pixel inspection. It installs nothing, launches
no browser and writes no PNG or application source. Its allocated execution
records exact nine-case/report pins, the retained unfocused-title pin,
calculated luminance intervals and unchanged-source assertions in
`feasibility.json`. Output-check allocation and results will be recorded here
before committed handoff. Resource21d offered
`TITLE-FOCUS-FEASIBILITY-202611040825Z`; the owner explicitly accepted the
10-second / 32-KiB allocation. The exact command
`python3 docs/mcr/evidence/title-focus-01/feasibility.py` exited **0** in
**0.278733917 seconds**. Script plus document carry was 13,831 bytes; report
was 7,114 bytes; total was 20,945 bytes. Minimum observed free space was
33,479,512,064 bytes. No temporary files or live owned process group remained;
the allocation was fully released. This is source feasibility, not browser,
accessibility or product acceptance. The script intentionally asserts the
historical unchanged CSS and must be reproduced at this historical commit.

The implementation owner has sent the concrete contradiction directly to
Root153, World SL and the UI lead. Root then explicitly authorized adding only
`.mcr-chrome-panel-header:has(> #panel-title:focus-visible)` with background-color,
background-image, background-size, background-position and background-repeat.
The existing unfocused header remains dark. All foreground, outline, geometry,
layout, TSX, forced-color-adjust and other selectors remain frozen. The former
writer and incumbent CSS writers are confirmed stopped/returned. The same
original goal stays ACTIVE. Root will capture the finite affected focus/unfocus
Map and Me profile-link states at 360/390/430 plus representative forced colors,
under a fresh Resource21d allocation. The historical static result neither fixes
the issue nor certifies the conditional continuation.
