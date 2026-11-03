# Wall reference target qualification

Assignment: FE-REF-WALL-QA. Bounded fixture presentation qualification is PASS.
The original broader IN04 goal remains BLOCKED and unfinished; this fixture work
does not reset or complete it. Real IN04, physical devices, deployed authority,
actual two-client collaboration and release acceptance remain NOT_RUN.

## Ownership and reference

Assigned checkout `/Users/lhs/Desktop/mcr-worktrees/qa-wall-reference-target`,
branch `codex/mcr-qa-ref-wall`, base
`f837bef676cd26869c26c172a699a91b7842d832`. Authored paths are exclusively
`frontend/e2e/wall-reference/**` and this document. No product, backend, root,
shared contract, manifest, lockfile or art source is authored here. The latest explicit human delivery policy permits this scoped QA owner to
commit, fetch/reconcile and publish its new checkpoint non-force to master;
no King or scrum-lead approval queue applies. The prior real-IN04 checkout and harness are untouched.

The supplied ten-screen PNG was viewed, identified at bottom-row fourth and
verified SHA-256
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
An unchanged evidence copy is `frontend/e2e/wall-reference/reference.png`; the
canonical repository location is
`docs/mcr/design/reference-target-2026-11-04.png`. The approximate Wall region
is manually annotated in the report as x925/y541/width290/height480 within the
1536×1024 image. That annotation is a human comparison aid, not image-recognition
or pixel-exact similarity evidence.

The reference guides window/frame, compact density, canvas-first composition,
right-hand controls, bottom palette and status. Its illustrated title, counts,
level and 00:12 cooldown are not application data. The production 128×128 canvas,
16 server colors and actual controller state are retained. No reference bitmap,
AI redraw, substituted character or screenshot is rendered as the application.
The existing deterministic C04 checker pattern is visibly fixture content; it
is not a reproduction of the illustration or approved product mural artwork.

## Harness

The fixture imports the actual production `MuralSyncPanel` and `MuralEditor`,
canonical memory session, privacy owner, HttpClient, synchronization/editor
controllers, frozen-contract HTTP adapter and existing deterministic C04 test
server. The latter is an in-process `fetch` implementation. It creates no backend
service, real login, database or public mutation. A memory scene sink is used;
the editor's actual HTML canvas is inspected. This does not claim Phaser/world
rendering, production root composition or multiuser networking qualification.

Only the test entry has `window.__WALL_REFERENCE__`, to read controller state,
count fixture HTTP requests and advance/change deterministic fixtures. Production
sources receive no diagnostic globals or writable test hooks. Browser network
requests are restricted to the ephemeral loopback Vite origin. Its development
client WebSocket at that origin's `/` is identified separately; any other socket
is a failure. Tooling query strings are omitted from reports. This exception
never admits a product `/world-ws` connection or external URL.

Retained Node 24.22.2 and dependencies are reused through the explicitly
assigned temporary `frontend/node_modules` symlink to
`/Users/lhs/Desktop/mcr/frontend/node_modules`. No install, dependency copying,
upgrade, shared cache or lockfile write occurs. Vite's cache lives only in the
owned ignored `.cache` directory. The runner enforces an 8GiB free-space floor
before starting and before each new width/capture. The initial launch stopped
at that guard before Vite/Chrome allocation. King's exact sparse-checkout
patterns were then applied to this checkout only, preserving all owned source
and captures. No other worker's files were removed.

Baseline runs read the assigned base's unchanged product files. Candidate runs
require a full final committed author SHA in `WALL_REFERENCE_AUTHOR`. A Vite
pre-load hook reads exactly the author's three assigned presentation blobs from
Git, preserving their original import identities; it never writes them to the
checkout. The report pins their bytes, the author SHA and the actually loaded
paths. Only those three feature paths may be overridden. Shared/root-window styles
remain the exact assigned base `f837bef`; root commit `7201b875` is not imported.
Neither uncommitted author work nor a sent intent is accepted as a source pin. Imported read-only dependency bytes are distinguished
from QA-authored test files. A candidate run without an author pin refuses to
start.

## Native baseline

Executed on actual Google Chrome 156.0.8078.80, desktop headless with
360/390/430×844, touch/mobile emulation, DPR1 and reduced motion. These are native
browser captures, not physical-device evidence. Baseline host framing is an
8px neutral margin; it does not override product selectors.

The corrected baseline command exited 0: all three widths passed the behavior
probes. Its separate `presentation` result is
`REFERENCE_GEOMETRY_MISMATCH`, with six of seven structural reference checks
false at every width. Baseline PASS means interaction qualification only; it
never means visual target acceptance.

| Width | Actual panel height | Canvas offset from panel top | Palette rows / height |
| --- | ---: | ---: | ---: |
| 360 | 1071.38px | 226.47px | 3 / 140px |
| 390 | 1101.38px | 226.47px | 3 / 140px |
| 430 | 1077.19px | 226.47px | 2 / 92px |

Controls are above the canvas, command/synchronization prose is above it, and
the palette starts 32.19px below the raster. The canvas already spans >80% of
the panel width. These measured mismatches were sent directly to the same
presentation owner; they are not an additional approval gate.

Baseline artifacts: `frontend/e2e/wall-reference/evidence/baseline/report.json`
and 24 native PNGs: each width has loaded, preview, cooldown, pending, unknown,
paused, frozen and read-only. Every image has a byte hash in the report, as do
actual product/fixture/contract sources. The first successful probe had an
incorrect socket classification and exited 1 despite all width behavior probes
passing. Its full JSON was overwritten by the already-running corrected rerun;
`first-probe-summary.json` truthfully transcribes the original tool output and
states that it is not a reconstructed full report. No failure is represented as
a product bug or concealed as a visual pass.

## Concrete behavior evidence

The baseline directly exercised:

- Every one of the 16384 confirmed canvas RGBA samples against the actual
  snapshot palette, including opacity; actual raster changes during pan/zoom
  and returns exactly on Fit wall.
- Real palette selection; arrow-key preview, Enter on the canvas and coordinate
  preview cause zero POSTs; Escape cancels and keyboard focus remains visible.
- One explicit keyboard activation of Confirm pixel causes exactly one fixture
  HTTP POST, changes the selected pixel/revision and applies the original
  5000ms fixture cooldown. This is UI state evidence, not server cooldown proof.
- Pending and unknown outcomes survive a snapshot containing the committed
  pixel and an unavailable original-result read. Explicit status/own-result
  GET resolution uses the captured key and never repeats the POST.
- PAUSED/FROZEN/read-only fixture states disable editing and state their actual
  condition. No backend permission is inferred from their appearance.
- Visible controls meet 44×44px; the page has no horizontal overflow. Coordinate
  controls remain reachable at 420px viewport height.

All 16 server palette colors must remain available. The reference's compact
single row cannot display sixteen separate 44px targets simultaneously on a
360px screen. An intentionally horizontally scrollable single row is the
presentation owner's accessibility tradeoff; its actual dimensions,
scroll reachability and keyboard selection passed on the final pin.
No palette colors may be silently removed or 44px targets shrunk to match the
illustration.

## Commands and current limits

For historical reproduction, use a disposable checkout at `f837bef676cd26869c26c172a699a91b7842d832`
and copy only this checkpoint's `frontend/e2e/wall-reference/` directory into it.
The retained runner labels the shared base as `f837bef`; do not run it unchanged
on a newer root and interpret that label as qualification of the newer root.
Use the assigned retained dependencies and Node version, then run:

```sh
export PATH=/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin:$PATH
node frontend/e2e/wall-reference/run.mjs --baseline
node frontend/node_modules/typescript/bin/tsc --noEmit --target ES2022 --lib ES2022,DOM,DOM.Iterable --module ESNext --moduleResolution Bundler --jsx react-jsx --strict --noUncheckedIndexedAccess --exactOptionalPropertyTypes --isolatedModules --verbatimModuleSyntax --resolveJsonModule --skipLibCheck --noUnusedLocals --noUnusedParameters --types node --typeRoots frontend/node_modules/@types frontend/node_modules/vite/client.d.ts frontend/e2e/wall-reference/fixture.tsx
frontend/node_modules/.bin/oxlint frontend/e2e/wall-reference/fixture.tsx frontend/e2e/wall-reference/run.mjs
```

Strict standalone TypeScript and scoped lint exit 0. The first TypeScript probe
omitted Vite's CSS declarations and failed; adding the existing
`vite/client.d.ts` input fixed the command without editing application types.
The first native launch exited 1 at the disk guard; the first allocated native
probe exited 1 for the tooling-socket false alarm; the corrected baseline exits
0 with reference mismatch explicit.

Final native candidate comparison, reference/current image inspection,
scrollable palette reachability and exact source-author pin are complete. No
application defect was found. Production root and
physical-device acceptance, backend authorization/cooldown, capacity/load,
actual multiuser restore/freeze and the real IN04 gate remain NOT_RUN.

## Final candidate qualification

The author delivered source commit
`832c7de9c57f6f8931b5564f61f68d82fa99d89d`, parent `f837bef`, changing exactly
the three assigned presentation files. Its separate tests/evidence/doc handoff
is `4eb2d25290eeb6915c85c24a3d030b181b17fa9c`; that commit is not imported into
this QA checkout. All three served blob hashes and actual Vite load paths were
independently compared to `git show` bytes. The pinned no-emit TypeScript check
also loads the committed editor/panel source through a compiler-host read hook;
it passed without modifying application files.

The first candidate probe passed all eight coarse structural geometry checks
at all widths. These checks are explicitly not full-photo fidelity acceptance.
Compared with the original seven baseline checks, an eighth records the
reference's approximately 1.1 portrait canvas ratio. The canvas remains a true
128-square raster using uniform scale and centered letterboxing; canvas sample
positions are derived from actual CSS/backing dimensions. The probe also passed
all-cell RGBA/opacity/variation, physical corner and blank-letterbox hit tests,
palette access, focus, pan/zoom/fit and zero-POST preview up to its first form
interaction. It then timed out because the QA selector searched for a hidden
spinbutton inside closed details without including hidden content. That is a
harness defect, not an application finding. The corrected harness activates the
actual summary control; it does not force application state or bypass input.

The entire failed first candidate report and its three loaded images are
retained under `evidence/candidate/first-probe/`. A subsequent final rerun was
refused before browser allocation when disk again fell below 8GiB. Only this
stopped harness's 21MiB owned cache was removed; all reference/source/evidence
was retained and the resource owner notified.

| Width | Candidate panel height | Canvas top offset | Canvas size | Palette |
| --- | ---: | ---: | ---: | ---: |
| 360 | 631.13px | 72px | 336×369.59px | one 48px row |
| 390 | 664.13px | 72px | 366×402.59px | one 48px row |
| 430 | 708.13px | 72px | 406×446.59px | one 48px row |

The original reference and the three source-pinned loaded screenshots were
visually inspected. The compact black heading/frame, portrait composition,
right-hand pencil/pan and lower zoom/fit tools, framed palette and bottom status
are substantially closer to the target. Important remaining differences are
explicit: the actual fixture checker data is not the illustration; the factual
128×128 heading replaces its unbacked illustrated title; 16 server colors use a
scrollable row of 44px targets instead of tiny simultaneous swatches; the honest
functional tools do not introduce unsupported eraser, image-import or undo
authority. Root embedding/global navigation, physical devices and the approved
production mural content are outside this fixture comparison. No exact-photo or
whole-product fidelity decision is made.

Final successful candidate commands (each exit 0):

```sh
WALL_REFERENCE_AUTHOR=832c7de9c57f6f8931b5564f61f68d82fa99d89d node frontend/e2e/wall-reference/run.mjs
WALL_REFERENCE_AUTHOR=832c7de9c57f6f8931b5564f61f68d82fa99d89d node frontend/e2e/wall-reference/check-types.mjs
```

The missing-author-pin negative command exits 1 before browser allocation. The
corrected native run passed after a fresh disk-floor check. No product fix has
been requested or concealed.


## Final results and handoff

The final candidate command exited 0 in actual Chrome at 360, 390 and 430px.
All eight coarse structural checks and all interaction checks passed at every
width, with an empty errors array. The presentation result is
`MEETS_STRUCTURAL_GEOMETRY_CHECKS`; this is not exact-photo fidelity acceptance.
All 24 final candidate capture hashes and every recorded source hash were
verified. The final loaded captures are byte-identical to the three inspected
first-probe loaded captures; final 390px preview and unknown captures were also
visually inspected. All nine command flows have exactly one POST each. The 18
observed development sockets are the local Vite client endpoint only.

| Evidence | SHA-256 |
| --- | --- |
| `evidence/baseline/report.json` | `80f639daef077390484826cc0a7cf81a1e574495a3237f6903dbcac1d4eb95e9` |
| `evidence/candidate/report.json` | `0cf6cd9f7abcc4677f0949347d6114b139d62bf587fb7015bc039fca38081144` |

Both evidence paths are relative to `frontend/e2e/wall-reference/`. Each phase
retains 24 state captures. Failed-probe evidence and its limitations remain
recorded separately; no failed capture was renamed into a passing run.

Final verification also includes the author-pinned strict no-emit TypeScript
check, scoped Oxlint, JavaScript syntax checks, missing-pin refusal, planning
consistency and staged whitespace/path-ownership checks. Native captures remain
explicitly pinned to `f837bef` shared/root presentation plus the three served
`832c7de` feature blobs. Later combined root checks reported by Frontendking are
independent evidence and do not become this QA chat's IN04 proof. Reconciliation
with newer master preserves peer history and does not retroactively change the
captured source revision. A later root embedding may differ from these captured
shared/window bytes; rerunning this harness on that root is a new qualification.

The scoped owner publishes only this new QA checkpoint after fetch/reconcile,
without duplicating original author `832c7de`/`4eb2d25` or root commits already
integrated by Frontendking. Commit, remote HEAD and ancestry receipts are sent
with the handoff. Temporary dependency symlink and owned ignored cache are
removed before that handoff; retained evidence is never removed. No live
backend, two-client, device, load or release claim is added. The original
unfinished IN04 goal stays BLOCKED.

At final reconciliation, fetched remote `fb0bfcaa6939d18f6fd4bd7d87d93412d7a7dafe`
retains all three tested feature blobs exactly. It changes three recorded
shared dependencies: `frontend/src/shared/ui/concert-window/index.tsx`,
`frontend/src/shared/ui/concert-window/concert-window.css` and
`frontend/src/shared/styles/base.css`. This evidence is historical qualification
of the explicitly captured feature/base combination, not a fresh rerun of those
new shared bytes. The QA publication preserves those newer peer changes.
