# Wall initial motif — After-show Bloom

Assignment: `WALL-INITIAL-MOTIF-ART-01`. **SOURCE-QUALIFIED / VISUAL PASS**;
canonical publication is pending. Server provisioning and live Wall acceptance
are NOT_RUN by this art author. No client overlay or live canvas mutation is
part of this contribution.

## Source and ownership

The existing exclusive checkout is
`/Users/lhs/Desktop/mcr-worktrees/art-cute-community-v1`. New branch
`codex/mcr-art-wall-initial-motif-01` starts from fresh master
`679adb7d9ca1b1e77aec51aa13c7282c5f4e4de9`. Historical community-art branch
`codex/mcr-art-cute-community-v1` and final `f297b8fa` remain preserved.
Only these new paths are authored:

- `frontend/public/wall/initial-motif-v1/**`
- `docs/mcr/art/wall-initial-motif-v1.md`
- `docs/mcr/art/evidence/wall-initial-motif-v1/**`

Backend202 froze the exact `mcr-mural-initial-seed-art/1` input at
`1607fbf8:docs/mcr/backend/mural-seed.md`. That document was read directly from
its Git object; it was not yet on this fresh base. Backend202 directly confirmed
the reserved pack root and pack-relative `qualification_evidence: README.md`.
No manager acknowledgment is a qualification or integration gate.

## Original design and reference fidelity

The reference's exact bytes were verified at its retained actual path:
`/Users/lhs/Desktop/mcr-worktrees/fe-delivery-publish/docs/mcr/design/reference-target-2026-11-04.png`,
SHA-256 `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
The author directly inspected its Wall panel: a near-black field, cream pixel
figure, crimson diagonal marks and hard cell edges. Only that contrast, rhythm
and palette direction inform this new composition; the figure and lettering
are not reproduced.

After-show Bloom is an original eight-petal stepped cream flower with a crimson
seed, bent stem, asymmetric red leaves and sparse pixel sparks. Broken red
corner rhythms frame it without a decorative left rail or enclosing logo.
No face, person, avatar, copyrighted band logo, reference pixels or legacy
character artwork is reused. No approval, operator identity, exclusive-rights
warranty or provisioning authority is asserted.

Index 0 is deliberately `#171617`, the actual opaque background/erase color.
Index 1 is cream `#F1E8D8`, index 2 crimson `#C72D3B`; supporting fold/core
shades and unused palette capacity are explicit in the 16-entry array. Those
nonzero roles are authoring choices, not server defaults. Order is immutable
for this exact bound pack and must never be silently reordered.

| Index | Actual RGB | Authoring role |
|---|---|---|
| 0 | `#171617` | Background and erase destination; the only producer-fixed role. |
| 1 | `#F1E8D8` | Main cream petals and stem. |
| 2 | `#C72D3B` | Crimson core, leaves and detached corner strokes. |
| 3 | `#30282B` | Reserved charcoal supporting shade. |
| 4 | `#D8C9B5` | Cream fold shadows and quiet sparks. |
| 5 | `#821F30` | Deep red underfolds. |
| 6 | `#FFF7E7` | Petal glints. |
| 7 | `#AD9886` | Reserved paper/taupe shade. |
| 8 | `#4A2B34` | Reserved plum shadow. |
| 9 | `#E3555A` | Crimson highlights. |
| 10 | `#685058` | Reserved muted supporting shade. |
| 11 | `#D77769` | Small warm core accent. |
| 12 | `#827873` | Reserved neutral shade. |
| 13 | `#D5AD95` | Reserved warm supporting shade. |
| 14 | `#242024` | Reserved near-black supporting shade. |
| 15 | `#E8D9BE` | Reserved warm cream supporting shade. |

Reserved entries are valid explicit editable palette capacity. The pack does
not require every nonzero slot to be painted, and the server must not compact
unused slots or turn these authoring labels into additional backend semantics.

## Qualification and delivery

The exact six-file pack contains editable `source.svg`, raw top-left row-major
`pixels.bin`, ordered `palette.json`, opaque native `preview.png`, closed
`manifest.json`, and actual-provenance/checks `README.md`. The independent checker
and original recipe live in the evidence directory.

Actual stored manifest SHA-256:
`3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce`.
This binds the complete exact manifest bytes, not a prettified reserialization.

| File | Bytes | SHA-256 |
|---|---|---|
| `source.svg` | 55,012 | `d61e130214820d959158a012d691ef5edfcb7bbfab52e46296a72eddea476be2` |
| `pixels.bin` | 16,384 | `f29eaf4ea632e2a34b6dd4fdfdc6f8a3d07ab780af96e19c2a64c28faf42c721` |
| `palette.json` | 211 | `9d6f27f9d60773877053459e47766017d0be0f3f4c7f889fff97ffca60e2247d` |
| `preview.png` | 1,390 | `05a0d8b1f974675ea93b601b5c7403d198be6aab94fc9aad9bb7960306e96432` |
| `manifest.json` | 1,424 | `3f1aa318f0e4b7d8fa7f079b1101f7386619291a8dbb3bb04ef70820952f34ce` |
| `README.md` | 4,406 | `58bc04bc0f527f2212199ad2ea13fd343b9d1df1bd3af2681ec0beb12514be37` |

The admitted `qualify.py` round exited **0**. Its actual machine result is
[qualification.json](evidence/wall-initial-motif-v1/qualification.json): closed
manifest/provenance/file keys, exact ordered 16-entry uppercase palette,
16,384 unsigned indices 0–15, complete opaque closed SVG with 1,243 editable
integer rectangles, every SVG cell equal to its binary index, and every native
RGB pixel equal to palette reconstruction. The PNG CRC/structure checks pass;
every pixel of the proof's 2× artwork region also matches the native source.
All seven negative rejection probes pass. No application test matrix was run.

The eight actually painted indices are 0, 1, 2, 4, 5, 6, 9 and 11. Background
index 0 occupies 11,628 cells; unused slots are retained in their original order.
Full counts and all six final file pins are in the machine result.

The author directly opened and inspected both actual images:
[native preview](../../../frontend/public/wall/initial-motif-v1/preview.png) and
[390px art-only proof](evidence/wall-initial-motif-v1/art-pack-390.png).
[Visual findings](evidence/wall-initial-motif-v1/visual-inspection.md) record
legible contrast, crisp stepped geometry and explicit proof-only labels. No
artwork correction was indicated; the images were not regenerated afterward.

## Bounded execution and concrete fix

The first SourceA offer expired before acceptance; its guard refused before any
worker started. A subsequent actual SourceB job
`WALL-MOTIF-ART-128-01-SOURCE-B-202611041724Z` authored the images, then stopped
before qualification on a guardian accounting race: the fast child had exited,
but a still-visible zombie PID made an unavailable libproc RSS record look like
a live measurement failure. No art or format pass was claimed from that job.
Its raw [accept](evidence/wall-initial-motif-v1/resource-accept.json) and
[failure result](evidence/wall-initial-motif-v1/resource-result.json) remain
unchanged. Its guardian and worker were independently confirmed gone.

The concrete fix checks/reaps an already-finished child before classifying a
missing RSS record, while still rejecting an unreadable live process. Retained
child peak RSS is included. No monitor subprocess or parallel worker is spawned.
Under a new offer, only the remaining checker ran; authoring/rendering was not
repeated. Frozen guard SHA-256:
`8f40918bfbe839ecf0766f20dc84ecd249add077c55c96ec6becc812d2781ed1`;
checker SHA-256:
`42042663c530d45d4683d37c99e37709936018f3318c9d0cf1914596253b4db9`.

Actual successful job `WALL-MOTIF-ART-QUALIFY-SOURCE-B-202611041729Z` accepted
at `2026-11-04T17:29:40.523321Z`, FULL at `17:29:49.687436Z`, exit **0**;
see [resource result](evidence/wall-initial-motif-v1/resource-wall-motif-art-qualify-source-b-202611041729z-result.json).
Baseline included the entire checkout (143,634 B), entire shared Git including
private worktree Git (1,087,922,059 B), retained art and failed-job evidence:
1,088,065,693 B, with a nonshrinking whole cap of 1,100,648,605 B. Conservative
other-job reserve was 16 GiB; fresh free space was 27,044,765,696 B, leaving
9,852,313,600 B after that reserve and the complete own 12 MiB growth allowance,
above 9 GiB. The guard enforced 12 GiB start / 8 GiB live floors and reserve.

Observed aggregate RSS peak 50,249,728 B; maximum guard interval 144.342 ms;
positive growth 130,913 B; whole bytes at FULL 1,088,196,606 B; new durable
bytes 122,148 B at FULL. Limits were 512 MiB / at most three processes with one
sequential worker, 4 MiB durable / 8 MiB temporary / 12 MiB all positive growth,
120s work + 60s cleanup, each subprocess at most 60s, and 250ms guard. No temp,
cache, dependency links, ports, browser, server, DB, build target or installation
was created. Guardian and worker exited; SourceB was explicitly returned to
Social immediately at FULL, independently of documentation or publication.

Only these new pack/document/evidence paths are handed directly to Backend202
and Root153. Root is the publisher; no additional review or acknowledgment gate
is introduced. Canonical publication is the remaining delivery receipt, not
permission to use the pack on a live event.

The 390px proof is ordinary art presentation labeled **ART PACK ONLY / NOT A
LIVE WALL**. It cannot prove server-authoritative state or application behavior.
Producer runtime implementation/validation and local operator selection remain
separate: bind a real canonical event, exact manifest digest and qualified pack,
background 0, explicit cooldown 0–3,600,000 ms, and explicit EDITABLE/PAUSED/FROZEN.
Only atomic pristine revision 1 and exact same-seed pristine retry are allowed;
existing content/history/checkpoints/different seed must be refused. This author
does not implement, authorize or claim execution of that producer boundary.
