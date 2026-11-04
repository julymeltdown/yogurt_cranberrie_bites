# Targeted focus visibility evidence

The background-only title treatment and the minimal Meet summary inset were
captured together using actual default App routes at 360, 390 and 430 pixels,
with representative forced-color states at 390. The neutral fixture uses
controlled QA06 HTTP/C05 and local assets; this is not live/device acceptance.

Candidate `cadf860dd928d58df7f1e8f26a0b4e8dddd03911` contains title source
`c7f5f654` integrated as `2ea7fd4e` and Meet source `09ef7bd8` integrated as
`91058462`. Baseline restores only the two affected stylesheets to `1722a817`;
all other source, fixtures and selected character assets stay at the candidate.
The appended Meet rule preserves the current producer's prior typography edits.

The immutable packet is
`docs/mcr/evidence/cozy-backgrounds-v1/focus-04/`, sealed by `4591f927`.
It retains 18 contexts, 43 original PNGs, 688 verified Git source/asset pins,
53 artifact hashes, exact commands, logs and whole-workflow cleanup evidence.
The allocated workflow finished in 63.916 seconds and released browser, server,
process group, temporary tree and owned dependency link. Peak owned storage was
43,757,568 bytes including 7,413,760 bytes of carried art.

| Actual measurement | Result |
| --- | --- |
| Three affected source files | 15 tests pass; full TypeScript, scoped lint and syntax exit 0 |
| Map/Me normal title outline, all widths | Four sampled strokes visible; minimum 5.7567:1 |
| Normal title glyphs, focused and unfocused | Minimum 15.1140:1; dark unfocused header restored |
| Existing Me profile-link focus | Minimum 4.8097:1 across the three widths |
| Meet normal summary focus, all widths | Four strokes visible; minimum 6.8467:1; target/layout/type preserved |
| Meet forced-color focus at 390 | Four strokes visible; minimum 10.6776:1 |

The raw native run exits **1 / OPEN_FAILURES**. Its two failures are Map/Me
forced-color assertions at the left inset title stroke. Removing the outline
reveals a black title glyph at that sample; the system outline composites to
`[4,0,58]`, producing 1.0663:1 against the glyph. This same value is present in
the baseline. The other three samples use white backing and measure 11.3098:1;
all four stroke samples match the actual composited outline color. Original
screenshots and outline-suppressed backing probes are retained. The responsible
title owner is qualifying the sample interpretation and complete affected
evidence; the raw failure is not rewritten as a passing run. No additional
foreground, outline, geometry or forced-color override has been introduced.

Direct selected-character canvas RGBA/geometry and CSS sprite source/UV/size
records are retained, including explicit zero counts. Ordinary controls,
text, layout, game/socket counts and affected source properties are compared
before/after. Final owner dispositions remain separate from the immutable raw
packet; no whole-product accessibility or visual acceptance follows from these
bounded measurements.
