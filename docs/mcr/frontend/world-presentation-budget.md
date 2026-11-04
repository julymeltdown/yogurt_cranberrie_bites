# World presentation budget

Status: IN_PROGRESS — validated handoff ready; Root153 publication pending.
This completes the allocated presentation correction and its focused qualification.
Original ten-screen fidelity, public/live admission and physical devices remain
separate open obligations.

## Result

Closed World now gives the retained Phaser canvas the full scene stage. Its four
existing tool destinations use a 44px disclosure over the scene, and the connected
status/virtual-position notice use a compact bottom strip. Controls stay above
that strip. Retry/error text and the original 44px retry action remain readable.

| Viewport | Baseline canvas | Candidate canvas | Baseline zoom | Candidate zoom |
| --- | --- | --- | --- | --- |
| 390×554 | 378×280 | 378×373 | 0.4794520548 | 0.6386986301 |
| 390×844 | 378×570 | 378×663 | 0.6472602740 | 0.6472602740 |

The short viewport recovers 93px / 33.2% canvas height. The existing camera
calculation consequently increases short-viewport zoom by 33.2%; width constrains
the tall viewport's unchanged zoom. No camera formula, authoritative feet,
fixture roster or artwork changes. The supplied reference/baseline/candidate
comparison shows the larger scene and actors, while the six-player fixture
formation and remaining reference differences remain explicit.

## Ownership and provenance

The isolated branch is `codex/mcr-fe-world-reference-budget`, based on published
`d1a5deaae0f6b79b365cb4c451d921711573ab8c`, containing `12a5d394`.
World's pre-edit source grant is `e6312ce472a4b00f64d0c874424c0f715b5e4e89`.
The active unbudgeted goal is `1793766233`; it stays active through publisher
handoff, fresh remote verification and cleanup.

Application changes are limited to the closed-tools branch and local state/imports
in `frontend/src/app/WorldShell.tsx`, closed-stage/tools and composition
summary/controls selectors in `frontend/src/app/shell.css`, and the new
`frontend/src/app/world-presentation/WorldTools.tsx`. Disclosure state lives in
the persistent shell. Four link IDs/routes, six main destinations, skip focus,
Back-to-origin focus and Escape-to-toggle focus remain available.

World separately allocated exactly three existing test-presence sentinels in
`46d3999555e30295afa2b75330f1c42dcdaea180`: one in
`app/__tests__/reference-root/root.test.tsx`, two in
`app/__tests__/default-shell/chrome.test.tsx`. They identify the visible World
tools button instead of its initially collapsed destinations. All original
route/focus/input/one-world assertions remain. New tests and the native driver
use only `app/__tests__/world-presentation-budget/`.

Composition logic, MapScene, spatial-v5 camera, the approved character manifest,
and the existing v5 fixture data remain byte-identical to the base. No panel,
MAP, feed/me/footer, global typography, Auth/socket/resource/DTO/replication,
backend, contract, ART, manifest/lockfile or CI ownership is acquired. There are
no cross-lane imports. `docs/source/` remains unchanged.

QA06's input packet is `e8dac180cd20b9135a32b13243f5f05dccd34de2`, with
artifacts pinned at `e3173efb095f7d8d329f822993a4d88ae21cd81b`.
The driver verifies all five original artifact SHA256 values. The unchanged
reference SHA256 is
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`;
its World crop `(589,3,370,526)` is displayed proportionally. The earlier 281px
raster estimate and approximately 86px source-derived row estimate were diagnosis
inputs, not old retained DOM measurements. Native evidence now directly records
280px before and 373px after.

## Qualification

Selected Node is 24.22.2. Commands and exit/results:

| Check | Command / scope | Result |
| --- | --- | --- |
| Unit/regression | `vitest run`: new tools, reference-root, default-shell/chrome, app/routes, composition, presentation-release | 71 tests / six suites, exit 0 |
| Retained shell | `vitest run src/app/__tests__/shell.test.tsx` | 39 tests, exit 0 |
| Full types | `tsc -p frontend/tsconfig.json --incremental false --pretty false` | Exit 0, no diagnostics |
| Targeted lint | `oxlint` on WorldShell, new helper/tests | Exit 0 |
| Driver syntax | `node --check frontend/src/app/__tests__/world-presentation-budget/native.mjs` | Exit 0 |
| Planning provenance | `python3 scripts/mcr/check-plan.py` | Exit 0; planning only |
| Native matrix | `node frontend/src/app/__tests__/world-presentation-budget/native.mjs` with explicit slot/output environment | Exit 0; 24 cases / five interaction groups |
| Actual optimized app | Retained React/Tailwind Vite build of `frontend/index.html` / `src/main.tsx`, after browser closure | PASS; temporary output, `copyPublicDir:false` |
| Focused replay | Same driver with `MCR_WORLD_BUDGET_FOCUSED=1` and new explicit slot/output | Exit 0; two cases / five interaction groups |

The actual default Root/Phaser fixture uses synthetic source7005 HTTP/Socket
inputs. Baseline loads only the two original WorldShell/CSS files from the exact
base through a test-only Vite plugin; all other application/fixture/art inputs
are identical. Six baseline cases cover 360/390/430×554/844. Eighteen candidate
cases cover those sizes in all three eras. Every steady case has one game,
one canvas and one active fixture socket, unchanged self `(472,440)`, no
horizontal overflow and the complete virtual-position notice.

Native full/half/peek stage/canvas/summary/panel rectangles at 390×554 and
390×844 have maximum before/after delta **0px**. Focused behavior covers four
tools and Back focus, all six destinations, skip focus, Escape, reduced viewport
with physical keyboard input, synthetic server-close/reconnect, actual runtime
resource failure/retry, privacy withdrawal and complete socket/canvas/blob cleanup.
Production-denied fixture mode still opens no game/socket and admits no production
release. Browser exceptions are absent. These are fixture/native checks, not
real backend multiuser acceptance.

## Review fixes and immutable evidence

QA06 first found new CSS selectors reaching nonclosed shells. Commit `a5b475f5`
restored original nonclosed rules and restricts every new scene-span/status/retry
override to `data-panel-size='closed'`. Actual native geometry confirms that fix.

Initial native source is `e93b92ad0058f3ad62d9bc639f7dc8c5df1db23c`.
Its report SHA256 is
`46bfada6aba20bf65bdb5a6d8c67b9ec3353b6685027f8f42eaf8ea608efe8d8`.
The original report/images/review remain immutable. Image review found one
prepaint expanded-tools capture; QA06 also found the reduced 390×420 sample
recorded stale canvas height 373px against stage/controls height 239px. Those two
initial images are excluded from the qualified screenshot set. The added
`native-01/settling-review.json` records both limits without rewriting history.

Focused replay source is `5619da572b7e9ba0b4ca614852a90f65666719a4`.
Its report SHA256 is
`b245a6268c5c936bfba0eb1fe076b7cce7d5c88b06b2c4c0976245753e75173d`.
All ten application/fixture source pins match the first report. The driver polls
actual viewport, canvas/controls rectangles, canvas backing dimensions, camera
dimensions and projection bounds before measurements, then waits two animation
frames and 100ms before screenshots. At 390×420, stage/canvas/controls/camera
heights all equal 239px, stage/canvas bottom is 348px and projection bounds are
`(6,109)-(384,348)`. The tools scene is fully painted. The existing right-side
map edge at this reduced height is preserved; camera/fidelity/device acceptance
is not inferred. QA06 independently verified these saved metrics and images.
The unchanged 24-case matrix and app build were pinned rather than rerun.

Evidence lives in `docs/mcr/evidence/world-presentation-budget/`:
`qualification.json` is the current receipt; `source-ready.json` is the historical
pre-native snapshot. `native-01/` preserves the initial matrix/comparison and
`native-02/` contains the settled focused evidence. `screenshots.json` records the
small owner-qualified screenshot set with absolute PNG paths, source commits,
original write-completion UTC timestamps, privacy review and publication limits.
Only synthetic identity/scene data appears; no real private user data is captured.

## Resources, delivery and limits

The unaccepted 0429 offer was returned without launch. Initial slot
`FE26E-WORLD-PRESENTATION-20261104T0451Z` was explicitly accepted after a fresh
18,095,816,704-byte floor/zero-competing-browser check and explicitly returned.
Measured peak was 178,270,361 bytes (<0.2GiB); durable output before receipts was
14,626,816 bytes. Focused slot `FE26E-FOCUSED-REPLAY-202611040504Z` followed
Motion's actual release, with 41,032,679,424 free bytes and zero competing
validation Chrome roots. Its measured peak including prior task evidence was
145,866,904 bytes; accumulated durable output before final receipts was
20,430,848 bytes (<40MiB).

Both workflows closed their sole owned browser/server and removed their temporary
profile/cache/build directories and borrowed dependency link. Fresh guards show
zero validation Chrome roots and no owned temporary directories; the existing
interactive MCP browser and peers were preserved. No install, Cargo, shared
target mutation or peer interruption occurred. Both slots are explicitly free.

Root153 is the sole serialized publisher. Implementation/evidence commits are
handed off with exact changed paths and source ancestry. No self-push is performed.
Source/publication never implies canonical producer/public registry activation.
Physical iOS/Android, software keyboard/notch behavior, live/backend6 multiuser
flows, complete supplied-art fidelity and the full original ten-screen obligation
remain outside this bounded acceptance and must not be marked complete here.
