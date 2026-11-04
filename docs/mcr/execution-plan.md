# Parallel execution and review plan

## Current checkpoint

The [current progress reconciliation](progress.md#current-task-accounting) records
22 task-criteria outcomes: 12 recorded acceptance dispositions plus 10 additional
task-local SATISFIED dispositions. The latest owner-declared original-task closures
are FE08 and BE25. Another 75 original tasks
have partial or unreconciled implementation evidence, 10 statuses remain
unreconciled, and all 5 IN gates remain open. The [112-row register](task-progress-2026-11-04.json)
states the evidence and limitations. Historical 11/112 statements later in this
document are not current implementation progress, and no effort-weighted
completion percentage is asserted.

The user's latest 2026-11-04 coordination instruction supersedes historical
single-integrator and per-handoff review rules below. Assigned owners validate,
commit and non-force push their exclusive scopes directly to master. Developers
own all implementation validation: code, queries, tests, runtime evidence and
fixes. They choose and perform the necessary checks and resolve technical
interfaces directly with relevant implementation owners. Kings and SLs must not
inspect, review or validate implementation or queries, rerun tests, or impose a
separate QA, review or approval stage. Kings track priorities, staffing, major
milestones and periodic commit/push progress; SLs track the broad feature picture,
dependencies and staffing. Required scope and developer verification duties remain
unchanged. Shared paths retain one explicit writer and physical build slots remain
coordinated. A king receiving a review or verification request must decline: "I don't perform reviews. The responsible developer owns validation; please resolve this with the implementation owner." SLs likewise do not review code, queries or tests. No work waits for king or SL review. Staffing, priority, ownership conflicts and missing product decisions may still be escalated. See [current delivery policy](king-coordination.md).

## Historical execution plan (superseded where noted above)

Intake and assessment are delivered. Foundation implementation is active: reviewed source mapping, pinned frontend installation/build, imported Rust workspace build and source-origin/boundary guards are available. The [execution record](progress.md) separates accepted foundation tasks, implementation in progress and unrun runtime gates. The original planning exports remain unchanged.

The main chat is the integrator. Backend and frontend development run in separate visible Codex chats, as the user explicitly requested. Each uses an isolated worktree and preserves the user's selected model and reasoning setting. The user withdrew the earlier fixed-model requirement; coordination and continuation calls omit model and reasoning overrides. The [chat ownership register](chat-coordination.md) records the current writers and scopes. Title prefixes are `mcr(be)`, `mcr(fe)`, `mcr(qa)`, `mcr(art)`, and `mcr(pm)`; a title failure is non-blocking. Create workers only for useful independent work and retire them after their handoff or when no ready work remains.

Five existing visible chats are reused for useful parallel work: real HTTP
Auth/Profile integration, BE08 places, FE17 replication, independent frontend
C04 consumer review and QA C04 readiness review. The additional frontend chat
completed FE06 and FE19 review before moving to FE17 implementation. Work proceeds on satisfied prerequisites and bounded contracts;
whole-task acceptance still requires the specified runtime gates. The integrator
publishes reviewed checkpoints while disjoint implementation continues. Actual
goals, exact paths and exclusive build ownership are recorded in the
[ownership register](chat-coordination.md). No fixed chat count is imposed.

## Separate code ownership and workspaces

The user explicitly requires frontend and backend changes to stay separate. The repository keeps both products together for integration, with these boundaries:

| Area | Writer | Boundary |
|---|---|---|
| `backend/` implementation, migrations, backend tests | Assigned mcr(be) worker | Exact assigned paths only; no frontend edits |
| `frontend/` implementation, browser mocks, frontend tests | Assigned mcr(fe) worker | Exact assigned paths only; no backend edits |
| HTTP/proto, realtime, and asset contracts; generated public types | Integrator | Includes inputs under `backend/docs/api/`, `backend/proto/`, and `contracts/`; both lanes submit requirements |
| Central manifests, locks, root routers, CI, scripts, infrastructure | Integrator | Applies even where these files sit inside a lane's directory |
| Art, independent QA, and documentation | Explicitly assigned worker | Exact asset/test/document paths; no implicit ownership of application code |

Each implementation worker receives an isolated Git worktree and task branch, including parallel workers within one lane. The integrator retains the master checkout. Example branches are `codex/mcr-be-BE03` and `codex/mcr-fe-FE09`; record the starting commit with each assignment. Do not launch concurrent implementation in a shared writable checkout.

Frontend/backend communication uses the frozen HTTP/WebSocket contracts and generated public types, without source imports across implementation directories. Frontend code must not receive server configuration, secrets, database access, or server policy implementations. Backend code must not depend on rendering, browser state, or browser mocks. Client form checks support UX; server policy remains authoritative.

Cross-stack work is handed off as separate backend and frontend patches/commits. The integrator checks the changed-path list and cross-directory imports against the assignment before serial integration and combined verification. A required change outside a worker's scope becomes a request to its owner, not an incidental edit bundled into that worker's patch.

## Initial work packages

| Package | Owner | Output and ownership | Readiness / review |
|---|---|---|---|
| C01 backend source map | mcr(be) | Inventory selected Rust workspace, proto/OpenAPI/build paths, migration ownership, source dependencies, and isolated process/port/database mapping | Source revisions pinned; integrator reviews before import |
| C01 frontend reuse map | mcr(fe) | Inventory only candidate headless modules, dependencies, source hashes, destination paths, and regression obligations; enumerate forbidden rendering/assets | Same source baseline; no legacy UI copied into active code |
| C01 consolidation | mcr(pm) | `docs/mcr/source-map.md`, final reuse decision, target path map, and review record | Both inventories reviewed; every proposed reuse has provenance and a test plan |
| C02 reproducible foundation | mcr(pm) with bounded BE/FE workers | Manifests/locks/toolchains, minimal builds, isolated infra, scoped commands | C01 accepted; integrator serializes all central file changes |
| C03/C04 HTTP contracts | mcr(pm), assigned contract writer | Exact inherited auth operations, core/domain schemas, permissions, transitions, retry/idempotency and examples | C03 after C01; C04 after C03; inspect original upstream APIs |
| C05 realtime contracts | mcr(pm), with mcr(be) review | Versioned authenticate/snapshot/input/delta/resync/revoke/room-switch schemas and fixtures | C03 frozen; stream and ticket failure probes reviewed |
| C06 asset contracts | mcr(pm), with mcr(fe)/mcr(art) review | Atlas/map/version contracts, golden assets, anchors/layers/collision/place provenance | C01 accepted; original art remains separate AR work |
| C07/C08 harnesses | mcr(qa) with integrator | Contract generation/drift fixtures, source-origin gate, evidence conventions | Honor each task's listed dependencies; C08 can start after C01, C07 after C03–C06 |

Contract preparation and source reconnaissance may overlap; claiming task acceptance still requires its listed dependencies and evidence. In C02, an isolated staging checkout can qualify the chosen Rust baseline before the reviewed BE01 import. This avoids pretending C02 can build a nonexistent target backend while preserving the ledger's C02 → BE01 order. Record the tested source, import mapping, and later target build separately.

## Feature lanes after their prerequisites are ready

| Lane | First bounded queue | Write boundary |
|---|---|---|
| mcr(be) core | BE01 → BE02 → BE03 → BE04; prioritize BE37 once BE04/BE05/C04 are ready, then BE10 | Imported infrastructure/adapters and assigned identity/privacy modules; central manifest changes submitted to integrator |
| mcr(be) concert | BE05/BE06/BE07/BE08, then BE11–BE14 world; separate party/giveaway modules when ready | Concert domain/application/service modules assigned per task |
| mcr(fe) app | FE01/FE02/FE03/FE05/FE06/FE07 and public/auth views as dependencies pass | New app features, shared headless infrastructure, and new UI primitives; root router composition serialized |
| mcr(fe) world | FE12/FE14/FE15/FE16 → FE17/FE18; editor and mural when ready | Avatar, Phaser runtime, input, rendering, and mock/real transport modules |
| mcr(art) | AR01/AR03/AR04 → AR02 | Reviewed places, original atlases, three theme packs, map assets and manifests |
| mcr(qa) | Contract/source boundary checks, then appropriate QA tasks and independent handoff review | Test/evidence paths explicitly assigned; QA results do not replace the integrator's real-service gates |

This table is a queue summary, not a waiver of `generated/TASKS_EN.json` dependencies. Reserve the central contract roots, service/router composition, manifests, lockfiles, and CI entrypoints for one writer at a time. Split feature domains only after their common types and registration interface are frozen. Keep the number of live agents proportional to ready disjoint tasks; the listed lanes do not require six continuously active agents.

## Handoff and continuation rules

Each worker receives task IDs, pinned source/contract versions, an isolated worktree/task branch, a base commit, exact writable paths, exclusions, required checks, and an expected handoff. The handoff names changed files, behavior, commands and exit codes, evidence paths, known gaps, and the appropriate status. Implementation workers commit only their assigned side to their task branch; only the integrator publishes master.

At each handoff the integrator reads the diff, checks the contract and ownership boundaries, runs affected checks, and inspects failure paths. Closely related fixes go back to the same worker with concrete findings. Independent verification can run alongside implementation when file ownership is separate. Preserve the unaccepted patch and reassign when work diverges from requirements, duplicates another lane, conflicts in shared files, or cannot progress on its prerequisites. Close finished workers after review. Do not keep an idle lane alive merely to maintain parallelism.

The integrator commits/pushes each reviewed bounded unit, fetches master, and verifies the delivered SHA or ancestry. On a non-fast-forward, fetch and inspect concurrent changes, integrate them, and repeat affected checks. Never force-push or change branch protections to bypass a rejection.

## Review checkpoints

| Checkpoint | Required evidence |
|---|---|
| Foundation | C01 source/reuse mapping; C02 reproducible install/build; C03–C06 frozen contracts; C07/C08 fixtures and origin checks |
| IN01 — shared world | Two independent real accounts login, enter the same room, observe movement/avatars, change theme, navigate persistent panels, and recover connection |
| IN02 — meeting and pickup | Real party approval, authorized physical beacon/rally, atomic giveaway reservation, and one-time pickup; virtual movement never creates physical arrival |
| IN03 — social privacy | Feed/introduction/consent/reveal/revoke/block flows against real services, including denied, expired, blocked, and stale-cache cases |
| IN04 — mural | Multiple clients, server cooldown, moderation/pause, restore to a new revision, convergence, and final freeze |
| IN05 — release | Every required task/IA entry, real devices, accessibility, race tests, realtime failures, measured capacity, recovery, deletion, operations, and same-candidate evidence |

IN03 and IN04 proceed alongside the meeting track as their own prerequisites pass. A green unit test or MOCK_READY frontend is useful progress but does not grant INTEGRATED or ACCEPTED. Required scope remains 112 tasks/61 IA entries unless the user explicitly approves and records a cut. Missing real credentials, operator decisions, assets, or device evidence are recorded as bounded blockers while other ready work continues.

Review semantic prerequisites in addition to graph edges: IN01's interactive movement needs FE15, ordinary party authoring in IN02 needs FE22, and BE34's durable mural transaction needs the BE15 outbox/receipt interface. FE06, FE08, FE10, FE11, FE15, FE22, FE34, and FE37 are required even though they are outside IN05's transitive dependency ancestry. The explicit full release checklist preserves them. See the [independent acceptance assessment](../analysis/acceptance-assessment.md).


Strict Gateway JWT context checkpoint: worker `ed2f166` is reviewed and integrated
as `e7d208e`; parent combined 235 tests, three compile-fail doctests and locked
library/binary checks pass. Its goal is COMPLETE and checkout stopped. FE14 is
ACTIVE. Independent report-only QA now audits the C06 format environment and
parent correction; actual checker absence and a pinned-plugin final-LF edge are
reproduced, with fixes reviewed before publication. The next backend opportunity
is the narrow current Auth owner client described in the admission decision,
not an enabled Concert route. Whole project goal remains ACTIVE and accepted
counts remain 11/112 required, 11/118 total.


The C06 tooling correction and independent report now pass parent combined
144 contract regressions at `bab2f1f`; QA bounded goal COMPLETE, no schema/fixture
or acceptance changes. The next BE04 owner-client goal is actually ACTIVE on its
fresh four-path checkout at published `e6efa4c`. FE14 has handed off its fifteen
assigned paths at `a1243ff`, with goal ACTIVE through parent review. Backend and
frontend remain separate; no additional chats are created to meet a quota.
