# Architecture v2 source candidate

Status: **SOURCE_CANDIDATE_CORE_CUTOUTS_VERIFIED_PACKING_NOT_RUN**.

`venue-sheet.png` is an original built-in imagegen output, copied byte for byte
from its provider output. The final prompt is `prompt.md`; generation mode,
reference roles, hashes, dimensions, proposed source crops and findings are in
`source-metadata.json`. Existing source images and selected character artwork
remain unchanged. This candidate replaces no current runtime resource.

The published12a5d394 QA06 target pairs show a remaining architectural mismatch:
low, broad, roof-heavy venue forms differ from the target's denser urban MAP.
The new sheet supplies two-storey façades for the first three existing frame
identities and a three-storey `west_hall`. The three columns retain the existing
REVENGE/PARADE/DANGER color roles. No new canonical place, footprint, interaction,
physical eligibility, character or geometry revision follows from this artwork.

Reproduce source analysis with:

```sh
python3 -B frontend/public/world/reference-scene-v2/architecture-v2/inspect-source.py
```

The inspection checks exact source/reference bytes, RGBA transparency,12 substantial
4-connected alpha cores, outer-edge clearance and one core per proposed crop.
It does not qualify native output, browser rendering or reference fidelity.
Raw alpha below32 has small residue touching the sheet edges. The immutable
source retains it. Proposed source crops include a2px margin around each
alpha>=32 core; subsequent packing must retain the exact RGBA pixels inside
those rectangles, using uniform nearest scaling and transparent letterboxing.
No alpha thresholding or source repaint is authorized by this record.

Some buildings are narrower/taller than the requested1.35–1.5 silhouette ratio.
The metadata records their actual sizes and prospective letterboxing rather
than treating the prompt as proof. Their taller wall masses are visible in the
original sheet, but actual frame occupancy, map massing, label occlusion and
360/390/430 target comparisons remain open. Whole-scene density also remains
open; a building sheet alone cannot resolve roads, planting or World viewport.

ART-ARCH-SOURCE-01 authorizes one cloud generation plus the returned PNG, owned
copy and small metadata only. It grants no browser, native, Tesseract, atlas,
runtime, root, geometry or character changes. A separately budgeted allocation
is required for any subsequent output job. Goal metadata remains BLOCKED while
this explicitly resumed source work proceeds; the original unfinished goal
is neither replaced nor completed. No new critic score or release acceptance.
