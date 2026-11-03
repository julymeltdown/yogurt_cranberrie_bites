# Wall and operations delivery

Updated 2026-11-04, Asia/Seoul. This is an execution and ownership snapshot,
not a scope amendment or release decision. Required scope remains 112 tasks and
61 IA entries. Frontend implementation may qualify against frozen deterministic
fixtures without waiting for matching backend completion. Real multiuser IN04
remains NOT_RUN until its separate live journey is executed.

## Current authority and source

- Frontend integration and publication: `01a1a385-9f32-73c2-8715-ff54bd1e0497`.
- Backend, contracts, and build-resource coordination:
  `01a19e5c-e9c6-7973-a53b-a6d19141620f`.
- Wall/operations coordination: `01a1a21d-39ec-7912-84cb-910058e7a110`.
  Main and scrum leads are not mandatory review or approval gates.
- Latest direct human policy, relayed by both kings and verified in
  `origin/master` AGENTS.md at `d8f9f9ac`, permits exclusive scoped owners to
  validate, commit, and publish new scoped checkpoints directly to master.
  Fetch/reconcile peer history, run affected checks, preserve scope, verify
  remote ancestry, and never force-push. Kings are not mandatory publishers or
  per-commit reviewers. Shared-contract and frontend-root ownership stays
  unchanged. Already handed-off Wall/root patches on frontend king's branch
  are published once by that king; their former authors must not duplicate them.
- All three current lane checkouts started at
  `f837bef676cd26869c26c172a699a91b7842d832`. This coordinator writes only this
  document in `codex/mcr-sl-wall-reference`, isolated checkout
  `/Users/lhs/Desktop/mcr-worktrees/sl-wall-reference`.
- The exact new ten-screen reference is preserved by `cdcd834f` at
  `docs/mcr/design/reference-target-2026-11-04.png`, SHA-256
  `d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
  Its bytes were independently extracted and hash-verified. Wall is the fourth
  screen in the bottom row. The previous design is not the current target.
- Specification, assessment, and execution plan have been read. Historical
  intake statements and older staffing tables do not override current explicit
  human instructions. English only; worker model/reasoning settings preserved.

## Active, disjoint assignments

| Task | Owner and isolated branch | Exact write grant | Actual goal state |
| --- | --- | --- | --- |
| FE-REF-WALL, presentation follow-up to FE32/FE33 | `01a1a213-61d6-7a20-a8e7-476649312f8c`; `codex/mcr-fe-ref-wall`; `/Users/lhs/Desktop/mcr-worktrees/fe-wall-reference-target` | `frontend/src/features/mural/editor/MuralEditor.tsx`, `frontend/src/features/mural/editor/editor.css`, `frontend/src/features/mural/sync/MuralSyncPanel.tsx`, new `frontend/src/features/mural/editor/__tests__/reference-target/**`, new `frontend/src/features/mural/sync/__tests__/reference-target/**`, `docs/mcr/frontend/wall-reference-target.md` | Owner verified unbudgeted goal COMPLETE after committed handoff; fresh get_goal now null. Writer STOPPED. |
| FE-REF-WALL-QA, bounded presentation and interaction qualification | `01a1a20f-e2a9-7ca2-a21e-f2bd3a9703be`; `codex/mcr-qa-ref-wall`; `/Users/lhs/Desktop/mcr-worktrees/qa-wall-reference-target` | New `frontend/e2e/wall-reference/**`, `docs/mcr/qa/wall-reference-target.md` only | Broader unfinished real-IN04 goal remains BLOCKED; related preparation continues manually. No competing goal or false completion. |
| Lane coordination and committed evidence handoff | Operations SL above | This document only | Original broad goal verified BLOCKED; explicit new authorization permits useful work manually. No tool capability is claimed to resume it. |

Previous writers were terminal and their old checkouts clean before transfer.
The source author is the original FE32 writer, subsequently FE39 owner. The QA
owner retains its original real-IN04 context. Both accepted the new exact base,
checkout, path boundary, and reference; both were observed active using compact
chat snapshots. Sending an assignment alone was not treated as acceptance.

No implicit grant covers controllers, input logic, HTTP adapters, contracts,
existing tests, root composition, shared UI/styles, manifests, lockfiles, art,
backend code, or another worker checkout. Extra paths require an exact transfer
from their owner. Frontend king publishes the already-integrated Wall/root
patches once; new scoped follow-ups may be published by their exclusive owners
with the latest direct-publication safeguards above.

## Delivery behavior and evidence still required

The target is near-identical composition and density: compact black Wall header,
dominant actual pixel canvas, right-side icon tools and lower zoom controls,
framed palette below, and compact genuine status. Do not substitute the reference
image for a canvas, invent a title, balance, level, cooldown, role, or unsupported
server tool. Preserve the 128 by 128 grid, 16 server colors, preview separated
from explicit confirm/cancel, pan without placement, unknown-result recovery
without mutation replay, privacy fencing, and paused/frozen/read-only behavior.
No decorative left accent rails. Selected character originals remain unchanged;
this assignment does not authorize character editing or replacement.

The author must deliver implementation, meaningful existing mural checks,
TypeScript/lint/build, bounded native 360/390/430 evidence, source/reference pins,
limitations, explicit-path commits, direct handoff, and concrete fixes. Essential
controls remain at least 44 CSS pixels; coordinate and keyboard alternatives can
be compact expandable controls rather than occupying the initial canvas view.

QA uses production editor/synchronization/controller consumers with deterministic
fixtures and nontrivial real canvas pixels. It records native browser/version,
source hashes, viewport geometry, canvas dominance, right tools, palette,
overflow, keyboard/focus, touch targets, preview/no POST, one explicit confirmed
write, unknown-original-result reads without replay, and cooldown/read-only
behavior. Baseline evidence is distinguished from the final author commit.
Author paths consumed as dependencies are identified separately from QA-authored
paths. Concrete findings go directly to the author and are rechecked; QA is not
an additional generic approval gate. Real devices and live IN04 are not implied.

Author task-local work is MOCK_READY and committed; independent QA is
IN_PROGRESS. Integration is verified below; remote publication remains pending.

The author reports the three presentation files implemented and 126 existing
editor/synchronization tests across seven files passing (exit 0). This is an
owner-run intermediate result, not final source/native qualification. QA's first
baseline launch stopped at its disk-floor guard before Vite/Chrome allocation;
no failed launch is counted as browser evidence. Its isolated fixture strict
TypeScript and scoped lint now pass; baseline captures are resuming.

QA subsequently completed baseline behavior at 360/390/430 (exit 0), with six
of seven coarse target-geometry checks failing. At 390px the old panel is
1101px tall, canvas begins 226px below the panel top, and palette occupies three
rows. The coordinator viewed that native screenshot and the supplied target.
The first probe misclassified 18 Vite development sockets; QA corrected its
same-origin development-endpoint classification while retaining the product
socket guard and transcribed the initial failed stdout honestly. That failed
full report had already been overwritten; no retained full-report claim is made.

The author accepted the measured portrait proportion (approximately 274 by
304 in the supplied image), conditional idle preview, and root-height seam.
Its local presentation math keeps uniform pixel scale and centered vertical
offset with matching hit testing; no controller/input ownership expansion was
needed. Final native raster/corner checks remain pending. QA will serve only the
three exact author files through a read-only Git-blob loader, recording hashes
of actually served bytes rather than the old baseline files. Coarse geometry
success alone will not certify exact imagery, icons, or full visual fidelity.

Final product source pin:
`832c7de9c57f6f8931b5564f61f68d82fa99d89d`, parent
`f837bef676cd26869c26c172a699a91b7842d832`. Its exact changed-path list was
independently inspected: only the three granted presentation files. Imports
remain frontend-only. The coordinator compared each committed file SHA-256 to
the actual native report: all three match. Author reports 131 tests across nine
files, full frontend strict TypeScript, scoped lint, production build, planning
check, and whitespace check all exit 0. Native Chrome 156.0.8078.80 passes
360/390/430. The coordinator inspected the 360px loaded PNG and report.

Measured standalone panel heights are 635.53/668.53/712.53px. Canvas starts
72px below the top, measures 340 by 374 / 370 by 407 / 410 by 451px, and occupies
58.8/60.9/63.3 percent of panel height. The framed palette is 62px tall; tools
are 44px and 10px from the right. Checks cover every 16,384 snapshot cell,
uniform raster scale, corner and zoom hit testing, all 16 colors via scroll and
keyboard, no page overflow, explicit one-POST confirmation, cooldown, lost/202
GET-only recovery, paused/frozen/read-only/privacy, and a short 420px viewport
using the root height variable. No uncommitted root implementation was consumed.
The pixel pattern is deterministic fixture data, not a claim that product mural
imagery matches the supplied drawing. Final independent QA is still pending.

Author evidence/documentation commit:
`4eb2d25290eeb6915c85c24a3d030b181b17fa9c` atop the source pin. All 26 changed
paths were checked against the assignment; no path drift was found. The new
tests, 15 PNGs and source-bound report, and
`docs/mcr/frontend/wall-reference-target.md` are delivered. Evidence totals
about 736 KiB. Own 36 MiB build output, cache, and dependency link were removed;
the checkout is clean. Owner reported update_goal COMPLETE and an immediate
COMPLETE get_goal, then corrected its subsequent observation to a fresh null
goal. The terminal completed turn and idle writer were verified. No broad
release or real-IN04 goal was falsely completed. This writer is available for
concrete Wall QA fixes; a new assignment needs an explicit ownership transfer.

Frontend king integrated source as `7b567acb`, author evidence as `fe9cbb2d`,
and the shared root as `deb5865d`; these commits were read from its actual
integration checkout history. All three integrated source files were compared
byte for byte to `832c7de9`, and the delivered author document to `4eb2d252`;
they match. The initial coordinator document is integrated as `7ebf17d9`.
Thus this bounded patch is INTEGRATED locally; publication and combined root
qualification are separate. At that inspection the integrator's origin/master
ref was `1e5022885525ab56f056aa2c24100e18c4bc67ed`, not this source delivery.

Independent QA loaded all three pinned Git blobs, passed committed-byte semantic
TypeScript, and passed eight coarse structural checks at 360/390/430. Its first
candidate behavior probe timed out because its own locator omitted controls
inside closed details. The genuine summary locator is corrected; the full
failed probe and three loaded captures are preserved. The corrected native
rerun was refused before browser allocation when disk again fell below 8 GiB.
The coordinator verified 7,769,484 KiB free and notified both resource owners.
QA removed only its stopped 21 MiB cache, preserving its 3.1 MiB evidence.
Final behavior qualification and QA committed handoff remain pending; no
application failure or independent full-fidelity acceptance is inferred.

The corrected independent candidate subsequently passed all 360/390/430
interaction probes with no runtime errors. Its report records actual served
pin `832c7de9`, shared presentation base `f837bef`, no root override, all three
loaded pinned paths, and structural label `MEETS_STRUCTURAL_GEOMETRY_CHECKS`.
Checks include every authoritative RGBA cell, all palette colors and keyboard
access, physical corner/margin hit testing, genuine pan/zoom raster changes,
explicit one-POST confirmation, fixture cooldown, pending/unknown original-key
GET-only resolution, and paused/frozen/read-only states. Native imagery remains
deterministic test content; exact full-photo fidelity, combined root route,
real devices and IN04 are not certified. QA is finishing its committed report
and qualified publication receipt; its original broader IN04 goal stays BLOCKED.

## Shared shell seam and production dependencies

Root owner `01a1a153-1227-7e72-801f-a2a6b91981e1` retains exclusive
WorldShell/shell CSS, shell chrome, base styles, and shared ConcertWindow paths.
Its reported uncommitted contract removes redundant global header/era/footer in
full mobile panels, uses a flex body with min-height zero, overflow auto and
6px padding, and supplies `--mcr-wall-viewport-height` and
`--mcr-panel-available-height` from the available viewport minus 78px and safe
areas. Automatic `ConcertWindowEmbedding` suppresses the first embedded
feature's duplicate frame/title while retaining a semantic heading; standalone,
nested, or explicitly closable windows retain their own frame/title. No feature
opt-in edit is needed. Root owner subsequently delivered committed pin
`7201b875f9ed6a07a0a77b9ae4aeac2f3f2fbd57` atop `b756e9d9`, reported clean and
STOPPED. Its document is `docs/mcr/frontend/reference-root.md`; it reports 600
tests, types/lint/build and native default-Auth/Phaser checks for eight surfaces
at three widths. Those are root-owner reports, not coordinator reruns or a
combined Wall candidate. Frontend king owns serial integration and combined
route evidence. Standalone Wall source does not import the new shared export.

At base `f837bef`, `frontend/src/app/world-panels/index.tsx` still passes
`release={null}` into `MuralEntryProvider`. Backend kernel qualification alone
does not provide a live browser release. Frontend king and Main own the exact
qualified authority/HTTP/scene composition and production route enablement.
An eventual release needs the actual privacy scope, abort signal, room/epoch/
stream binding, read/subscription authority, write policy, qualified scene
anchor, and incumbent scene on close. Do not fabricate them or broaden the
public realtime contract to bypass missing composition.

Main confirmed the exact dependency request: BE34 kernel delivery `505bb8c2`
has native migration-ledger evidence but does not qualify `MuralEntryRelease`.
The C05 world consumer and Auth/Friend journal/RPC chain are still being
qualified by backend owners. Main will provide exact qualified producer/release
pins; frontend king owns the actual root binding. No extra Operations build or
review gate was requested.

IN04 separately requires multiple real clients, authoritative server cooldown,
moderation/pause, restore to a new revision, convergence, final freeze, and denied
unauthorized moderation. Fixture success does not execute that journey.

## Resource and next-work coordination

- No Cargo, backend services, databases, additional full checkouts, or dependency
  installation/copying in this lane. Use retained pinned Node 24.22.2 and shared
  existing dependencies only through temporary owned links; remove links at
  handoff. Own caches/output paths must not mutate shared dependency caches.
- Fresh disk observation fell to 7.8 GiB. Author and QA were instructed to pause
  new capture/build growth below the agreed 8 GiB floor, preserve work, and
  sparsify their own checkout only. Coordinator sparse conversion preserved
  tracked Git bytes; no other-worker data was deleted. Frontend king provided
  exclusions for backend and historical source/doc PNG captures while retaining
  frontend public originals, source documents, contracts, scripts, root plans,
  and the current reference. Bounded validation resumes only with verified
  headroom; no screenshot/dependency proliferation.
- After each owner applied sparse patterns, author reported 8.7 GiB and QA
  reported 8,956,412 KiB (about 8.54 GiB) free. QA retained only its new harness
  and dependency link, with no tracked edits; its 2.9 MiB footprint was the
  source reference and harness, evidence initially empty. Bounded captures
  resumed with a fresh disk guard before each screenshot.
- FE38 chat `01a1a202-3ff2-7a93-95c3-374c6df97281` is reserved by Social for
  FE-REF-AUTH. An Operations offer to Community was withdrawn before dispatch.
  Community confirmed alternate FEED/GIFTS staffing and needs no Operations
  capacity. FE-REF-PUBLIC remains Social-owned. Do not reassign these writers.
- Next: finish the assigned Wall source, consume its exact commit for final QA,
  resolve concrete findings, deliver committed patches to frontend king, verify
  actual integration/publication ancestry, and recycle finished qualified workers
  only after ownership transfer. Production/live prerequisites are sent to their
  actual owners without blocking useful presentation work.

Coordinator checks: `git diff --check` and
`python3 scripts/mcr/check-plan.py` both exit 0. The latter verifies preserved
source checksums, planning exporters, and required scope accounting only;
application build/tests/deployment remain outside that check.
