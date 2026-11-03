# MCR specification and acceptance assessment

Assessment date: 2026-11-03. Scope: independent document and ledger analysis only. No application feature was implemented, no implementation task was marked complete, and no application build, provider authentication, migration, device, load, recovery, or deployment validation was performed for this report. The parent integrator owns imports, provenance verification, exporter/check runs, the overall assessment, and delivery.

The current user instruction makes **`julymeltdown/mcr`, branch `master`**, the delivery destination. References to `julymeltdown/bigyeting` in the supplied narrative, metadata, task cards, and receipt are historical source assertions, not authority to deliver there. Bigyeting and port-rust remain proposed reuse sources. This report does not modify their imported documents or generated metadata.

## 1. Assessment

The specification describes a substantial event community application: a real authoritative multiplayer world plus parties, physical meeting coordination, giveaways with inventory, introductions and age-policy adapters, mutual handle sharing, feed, collaborative mural, moderation, closure, and deletion. It is not a small world-rendering prototype. Its separation of mocks, contracts, integration, and acceptance is useful, but its task graph alone cannot prove launch readiness or schedule feasibility. The immediate authorized outcome is understanding and assessment; the implementation instructions embedded in source documents do not authorize implementation during this assignment.

The strongest acceptance boundaries are already explicit: one canonical identity, newly authored rendering, service-owned data, real two-user integration, transactional inventory/capacity, recipient-filtered realtime streams, private-by-default handles and locations, operator controls, and same-candidate evidence. The largest remaining risks are underspecified policy/configuration, release checklist coverage beyond graph dependencies, wide shared-path ownership, and beginning late-stage qualification without a stable executable candidate. Sources: [plan §§1–4, 7–9, 15–17](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md); [delivery README, Release-policy clarification and Implementation completion record](../source/mcr_v2_en/MCR_DELIVERY_README_EN.md).

## 2. Independent ledger accounting

I read the supplied JSON directly and used a separate read-only calculation for counts, dependency waves, and IN05 ancestry. I did not run the exporter or its regression tests. These findings establish consistency of the supplied planning data, not execution of the planned application.

| Measure | Independent result | Interpretation |
|---|---:|---|
| Tasks | 118 | C: 8; AR: 4; BE: 44; FE: 41; QA: 10; IN: 5; EX: 6 |
| Launch-required tasks | 112 | Includes IN05, the release decision itself |
| Required tasks before IN05 | 111 | Exactly matches the supplied explicit release-required set |
| Optional tasks | 6 | EX01–EX06 only |
| IA entries | 64 | 61 required; M10, R02, X02 optional |
| Task execution status | 118 `NOT_STARTED` | No task-level application acceptance is claimed by this ledger |
| Dependency links | 297 | No cycle found by independent prerequisite traversal |
| Topological waves | 15 | IN05 in wave 14; optional extensions in wave 15 |
| FE-to-BE prerequisite links | 0 | Supports contract/mocks-first frontend development |

The required IA set also exactly matches the supplied 61-entry release list. These agree with the claims in [VALIDATION_EN.json](../source/mcr_v2_en/generated/VALIDATION_EN.json) and the README, independently of their recorded `PASS` status. Inputs: [TASKS_EN.json](../source/mcr_v2_en/generated/TASKS_EN.json), [IA_EN.json](../source/mcr_v2_en/generated/IA_EN.json), [DEPENDENCIES_EN.json](../source/mcr_v2_en/generated/DEPENDENCIES_EN.json).

**Phase and priority must be kept separate.** The Markdown task table's `P1` for BE01 means phase 1; the JSON assigns BE01 priority `P0`, so it is launch-required. Phase counts are 9 / 35 / 21 / 22 / 25 / 6 for phases 0–5. Do not count only phase-0 rows as required work or treat phase-1 art as optional. Sources: [plan §9 and task tables §§10–14](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md); JSON `phase` and `priority` fields.

**Eight required tasks are outside IN05's transitive dependency ancestry:** FE06 (theme UI), FE08 (public/legal views), FE10 (signup/verification views), FE11 (reset/callback views), FE15 (movement/input), FE22 (party authoring), FE34 (profile/preferences), and FE37 (moderation screens). There are 103 prerequisite ancestors, plus those eight tasks, plus IN05 = 112. This is not a missing-ledger-item defect: all eight are in the explicit 111-task pre-IN05 checklist. It is a concrete reason that “all IN05 dependencies passed” is insufficient. The release owner must review all 111 prerequisite task evidence records and all 61 required IA entries, then record IN05's own decision. Source: JSON dependency/release sets; [README, Release-policy clarification](../source/mcr_v2_en/MCR_DELIVERY_README_EN.md).

The counts are sensible as an inventory, not as effort estimates. IA entries include panels, modals, and global states; 64 entries do not imply 64 independent pages or 64 equally sized deliverables. Tasks are uneven: importing a service workspace, implementing social privacy, and building a single view are not comparable units. Neither 15 waves nor a percentage of task IDs accepted yields a calendar forecast. The source's event date is a planning assertion; this audit has not reverified the event or venue and offers no event-deadline delivery promise. Sources: [plan §§1, 6, 16, 18](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md).

## 3. Scope traps and decisions needed before implementation

| Boundary | Source requirement or ambiguity | Acceptance decision to record |
|---|---|---|
| Reuse versus rewrite | Headless frontend logic may be extracted; all rendering is new. Backend foundations are imported with existing ownership boundaries. | Record source/destination/revision/dependencies/tests for each extract. Hash checks do not replace visual review; renaming old UI is not acceptance. C01/C08/QA02; §§2–4. |
| One world versus several products | Three skins share geometry, membership, collision and interaction IDs; five concepts do not mean five apps. | Test unchanged position, room population, collision, and other users' outfit identity while switching skins. FE06/AR04/IN01; §§1, 5.2. |
| Persistent shell | One Phaser instance and one world socket; direct links and panel state must work. | Define draft safety, browser-back ordering, mount cleanup, identity reset, and background recovery. Persistent does not mean a permanently connected mobile background socket. FE07/FE14/FE17/FE41; §5.1. |
| Initial assets and reduced scope | §5.3 names six presets and item counts; §16 permits reducing asset counts. | Keep these as the baseline until a specific reduced scope is approved and recorded. Do not quietly reduce acceptance or substitute illegible placeholder art for recognizable avatars. C06/AR03/QA06. |
| Venue reality | Approved art does not verify actual gates, routes, or authorized photo use. | AR01/BE08 must distinguish verified physical destinations from concept art and closed places. Recheck operational information before publication; decide who approves it and how stale guidance is withdrawn. §§1, 5.4; O03. |
| Age and signup eligibility | Claim support 14–100 and half-open age filters are implementation semantics, not signup eligibility or concert admission rules. | Name the service eligibility rule, consent/notices versions, provider availability policy, and enabled restricted flows. Keep basic eligible world access separate from unverifiable restricted discovery/sharing. BE25–BE27/FE10/FE30; §8.3. |
| Configured providers | Base auth includes provider discovery/callback states; additional providers are EX04. | Freeze which providers actually launch and verify each advertised one. Missing age/provider credentials must produce unavailable states, not a fake success or an invented verified identity. BE03/BE26/EX04. |
| State transitions | Party arrows are compressed; transfer versus cancellation remains a choice; reservation READY entry is not fully specified. | C04 must enumerate allowed actor/state/command transitions, terminal behavior, owner departure, pause/cancel effects on existing holds, grace period, stock editing, and confirmation-code recipients. §8.2; BE16–BE23. |
| Social consent | Reciprocal grants are specified, but platform selection, request/grant lifetime and membership conditions need exact rules. | Define when both consents exist, what profile/eligibility is required, grant binding to each handle version, expiry, pending-request replacement, and block/unblock behavior. Never imply revocation erases a previously seen handle. BE28–BE30/FE31; §8.3. |
| Operational policy | TTL is initially 15 minutes; retention, cooldown, caps, limits and cleanup are configurable. | Freeze initial values, actor permissions, acceptable propagation delays and measurement thresholds before the relevant acceptance run. “Configurable” alone does not define a test oracle. C04/C05/BE38–BE44; §§5.4, 7.4–7.5, 8. |
| Missing release operations detail | Isolated environments, real-stack startup, backup restore, roles and kill switches are required; a deployed operating environment is not certified by this package. | Before IN05, name the release environment/operator, secret provisioning, enabled features, retention settings, backup/restore procedure, monitoring and recovery/rollback decision process. Do not infer production deployment from local Compose success. BE38/BE43/BE44/QA09/IN05. |

All source/task references in this table refer to [the implementation plan](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md) and corresponding records in [the task ledger](../source/mcr_v2_en/generated/TASKS_EN.json). This table identifies decisions to resolve, not new approved scope.

Explicit exclusions remain exclusions: general DMs, voice/video, resale, payments, continuous GPS, automatic external following, level systems, and five separate UI implementations. Party chat, mural history/souvenirs, offline/PWA extensions and additional cosmetics/providers/discovery are optional after IN05. Optionality of mural *history* does not make the basic mural optional. Source: plan §§14, 16; M10/R02/X02 and EX01–EX06.

## 4. Scheduling and ownership assessment

The graph is structurally coherent and permits useful independent work, but several prerequisites are semantic or operational rather than edges. Start a task when its own inputs are accepted; do not turn diagnostic waves or phase numbers into global barriers. Conversely, a task being graph-ready does not mean its acceptance environment exists.

| Finding | Why it matters | Recommended orchestration rule |
|---|---|---|
| C02 requires a minimal frontend/backend build, while BE01 imports the backend and depends on C02. | In a new destination, literal completion ordering creates a bootstrap ambiguity even though the graph is acyclic. | Decide whether C02 qualifies a pinned isolated source checkout or has staged acceptance with BE01. Record that interpretation; never claim the destination backend built before it exists. |
| C03/C04 precede import completion. | Auth operation IDs and cookie/token semantics must match the actual pinned backend, not a historical route name. | Contract freeze requires inspection of the exact selected import revision and later producer conformance; a proposed route register is not the contract. |
| FE tasks can precede C07; BE25 can be ready before BE05. | This is allowed preparation, but shared harnesses/crate registration may not yet exist. | Use only a frozen fixture subset; distinguish preparation or MOCK_READY from runnable integration. Have the crate owner serialize BE25 placement/registration with BE05. |
| BE37 is a central privacy prerequisite. | Beacon, party, giveaway, media, intro/social, feed, mural and report work rely on policy behavior. | Prioritize canonical identity and BE37 over accumulating endpoints that bypass policy. Do not install permissive production stubs to unblock lanes. |
| BE34 promises atomic outbox updates but does not depend on BE15. | Edge readiness alone does not ensure durable command/outbox infrastructure is available. | Agree the transaction/outbox port with BE15 before BE34 acceptance; use one durable implementation rather than a mural-specific substitute. |
| IN01 lacks FE15 in its ancestry; IN02 lacks FE22. | Movement and party authoring can remain unfinished while nominal gate prerequisites pass. | Include FE15 for IN01's interactive journey and FE22 for an ordinary user-created party in IN02, or record a narrower gate result without calling it full feature acceptance. Keep the release checklist independent. |
| QA02 can run once only the shell/primitives exist; QA10 can run before later integrations. | “All routes” visual-origin review and same-candidate provenance cannot be final at those early points. | Start scaffolding/review early, then rerun applicable checks on the complete candidate. Early QA status is provisional. |
| QA04/QA08 have fewer edges than the full application surfaces they qualify. | A privacy matrix needs actual projections/WS paths; capacity needs a live world and selected build/configuration. | Execute final cross-surface privacy/load acceptance only when those runtime surfaces exist, even if scaffolding is already graph-ready. |
| BE44 depends on closure/security work late in the graph, while IN01 needs real services earlier. | Waiting for BE44 to establish any real-stack workflow delays early integration. | Use BE01/BE02 isolated startup plus a scoped world harness for early IN01; reserve BE44 for the final reproducible full-stack acceptance. Do not mark BE44 done early. |

Sources: task dependencies and acceptance text in [TASKS_EN.json](../source/mcr_v2_en/generated/TASKS_EN.json); [plan §§7, 9–13, 15](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md).

The lane counts include optional work: BE-CONCERT 26 tasks and FE-APP 33, versus BE-CORE 19 and FE-WORLD 9. They expose likely queues but do not measure effort. Backend realtime and transactional domains should be split into explicit modules only if separate owners can avoid common registries/migrations. FE-APP can be divided by feature directories after shared transport, shell and primitives stabilize; adding workers before that increases shared-file contention.

Broad `owned_paths` are proposed responsibility areas, not exclusive locks. BE01's `backend/crates/` and `backend/services/` encompass later work; BE05 encompasses most Concert work; BE03/BE43 overlap Gateway adapters; C03/C04 both own API roots; C07/QA01 overlap contract scripts; FE15's interaction directory contains FE33's mural interaction file. Assign exact files, migration identifiers and registration requests for each bounded unit. One owner serializes root manifests/lockfiles, contract generation inputs, router composition, CI and environment files as required by §15.2. Workers should not all independently “fix” these roots.

## 5. Concurrency acceptance boundaries

These are minimum meaningful acceptance scenarios derived from plan §§7.4–7.5, 8.1–8.4 and BE11–BE23/BE29/BE34–BE41/QA03/QA05/QA09. They are requirements to execute later, not observed results.

| Area | Evidence required beyond a happy path |
|---|---|
| Party capacity | Two approvals for the last slot yield exactly one membership; duplicate applications/decisions preserve one result. Remove/leave/cancel versus approval cannot create an unauthorized or ownerless active party. |
| Giveaway stock | One unit and five concurrent requests yield one hold. Claim/cancel/expiry and worker restart preserve `available = total - reserved_unclaimed - claimed`; stock release happens once; reducing total cannot invalidate committed units. |
| Command receipts | Same actor/operation/key/body returns the original receipt after timeout/restart; changed body conflicts. Define retention and reconciliation after key expiry. Domain commit and outbox enqueue share a transaction; crash/replay cannot lose or duplicate business effects. |
| Room admission | Concurrent joins respect capacity; switch failure preserves valid old membership; switch/reconnect/disconnect leases cannot duplicate membership or leave ghosts. Room ownership is singular across restarts, not just within one happy-path process. |
| Realtime projection | Two differently blocked viewers receive consistent snapshots and per-delivered-stream sequences; gaps/epoch changes resync. Slow clients have bounded queues; durable domain commands are recovered by reads/receipts rather than replayed motion. |
| Privacy races | Define ordering for reveal versus revoke/block/handle replacement, and approval versus member removal. Acceptance must state the authoritative transition point and allowed invalidation delay; test HTTP, active sockets and already-open panels together. Previously disclosed data cannot be retroactively unseen. |
| Mural | Parallel writes cannot bypass a user's cooldown; snapshot/delta baseline is consistent; write versus freeze/restore produces an authoritative revision. Rejected previews roll back; clients converge after a restore into a new revision. |
| Lifecycle | Closure/deletion partial failure and retry preserve completed steps, revoke future access, expose pending failure, and resume cleanup/outbox work. Restore must not silently resurrect grants, sessions, private archive material or committed inventory inconsistencies. |

Use real PostgreSQL, forced transaction interleavings and durable-worker restart evidence where specified. Mock concurrency and sequential assertions do not qualify QA03. The document's 20 Hz simulation, 10 Hz update, 50-user room cap, 200-socket/four-room scenario and 30 FPS target are tuning/qualification targets. Final acceptance needs named device/browser/server configurations, measured ceilings, chosen caps and failure behavior; it cannot claim universal capacity from these numbers.

## 6. Privacy acceptance matrix and unresolved boundaries

QA04's actor matrix must cross owner, approved member, unrelated user, blocked in either direction, suspended user, expired/revoked grant holder and privileged operator with list/detail/direct-ID/mutation/media/WS/cache/archive surfaces. An admin role is not blanket access to handles or unrelated evidence. Establish least-privilege operator roles and receipt redaction before moderation acceptance. Sources: plan §§7–8; BE24/BE26–BE30/BE33/BE36–BE43; QA04; O01–O09.

Required checks include:

- **Identity isolation:** Auth UUID and event scoping throughout; reject callbacks bound to another user; discard previous-identity responses after logout/account switch. Clear sensitive query data, drafts, sockets and context on revocation. Test refresh races without replaying unsafe mutations. FE01/FE02/FE09, BE03/BE04/BE26.
- **Public versus private projections:** No exact verified age, provider claims, email, handles, applicants, recipients, reporter identity or private meeting state in unauthorized lists, HTML, errors, notifications, logs, analytics, image reads or WS payloads. Inspect actual bytes, not just hidden UI. BE09/BE24/BE27–BE33/BE36, QA04.
- **Live policy changes:** Block/removal/suspension/expiry/handle replacement closes or redacts active views and subscriptions and prevents fresh reads. Unblock must not restore prior sharing consent. Policy-service failure denies protected disclosure without inventing eligibility. BE13/BE14/BE18/BE30/BE37, FE31/FE36.
- **Physical meeting control:** Virtual movement never updates a physical beacon or counts as arrival. Audience, source verification, revision, last confirmation, TTL, expiry/revoke/closure must be enforced on reads and active displays. No fabricated GPS distances or route safety. BE08/BE09/BE19, FE19/FE20/FE23/FE26.
- **Retention and archive:** Define concrete retention periods, cleanup ownership, media treatment and permitted archive fields; verify both public archive and linked entity/media paths. Account deletion and restore must obey the reviewed policy. Configurable retention is not evidence that cleanup was executed. BE41/BE42/QA09, FE39/FE40.

Resolve the phrase “membership conditions” for social reveal (§8.3) before C04 is frozen: it does not name a universal party-membership prerequisite for all sharing. Similarly, “unknown verification fails closed for restricted discovery and sharing” needs an explicit matrix of which flows are restricted; it should neither bypass restricted access nor accidentally make verified age mandatory for unrelated basic world use. These are specification decisions, not grounds to invent new eligibility policy in code.

## 7. Review checkpoints and stop/reassign rules

| Checkpoint | Reviewable outcome | Stop or reassign when |
|---|---|---|
| Now: understanding only | Scope, source assertions, destination override, open decisions and planning counts are understood. | Stop before feature coding. Parent owns the overall conclusion and any subsequent implementation authorization. |
| Source/toolchain/contracts: C01–C08 | Pinned reuse/import map; isolated data/process map; C02 bootstrap interpretation; auth/HTTP/WS/assets and frozen fixtures; exact shared-file owners. | Required source/runtime/provider information is absent or interfaces disagree. Reassign idle lanes to deterministic fixtures, design review or policy decisions; do not fork contracts. |
| First bounded vertical slice: IN01 | Two independent real accounts, real backend/world socket, actual art, movement, themes, panels and reconnect; explicit FE15 participation. | Mocks, shared identities, missing authoritative movement, duplicate sockets, ghost membership or identity leakage appear. Pause dependent integration; retain only independent mock/design work. |
| Correctness slice: IN02 + targeted QA03 | Real user party creation/application/approval, private rendezvous and durable reservation/pickup; contested last-slot/stock and restart evidence. | Oversell/overcapacity, uncertain-success UI, unauthorized meeting access or non-durable receipts occur. Reassign to transaction/reconciliation fixes before more feature breadth. |
| Privacy and shared-state slice: IN03/IN04 | Consent/reveal/revoke/block across live clients; author/moderator distinction; mural cooldown/pause/restore convergence. | Any protected data leaks or revocation/convergence fails. Stop the affected lane's acceptance and dependent release work; do not mask defects by hiding controls. |
| Candidate qualification: QA01–QA10 | All required routes/states, actual iOS Safari and Android Chrome, accessibility, limits, load, recovery/deletion/closure and no production mocks; stable candidate/configuration. | Evidence is stale, devices/providers are unavailable, or capacity/recovery is unmeasured. Keep NOT_RUN/BLOCKED and reassign to available independent checks; do not manufacture PASS. |
| Release decision: IN05 | Evidence for all 111 required prerequisite tasks and 61 required IA entries; explicit scope exceptions; candidate delivered to `julymeltdown/mcr` master by the parent. | Any required evidence or operator decision is missing. Record a blocked/no-go decision or seek explicit reduced-scope approval; never rewrite the checklist to create success. |

Suggested work units are one contract subset or coherent vertical behavior with a named owner, affected files, acceptance command(s), evidence and integration checkpoint. Stop repeatedly reassigning a blocked prerequisite to more workers when the missing input is an external provider, venue source, device, policy decision or approval; name the missing input and continue only independent work. Stop integration at the first cross-cutting contract/identity/privacy regression until its owner resolves it. Merge or candidate changes invalidate affected previous evidence; rerun the relevant tests on the integrated candidate rather than summing greens from incompatible revisions.

Scope reductions require a recorded decision and server-enforced feature availability, safe routes/UI, revised documentation and remaining acceptance coverage. Cuts cannot quietly remove the essential real multiplayer, atomic capacity/stock, location distinction, privacy, report/block, reconnect or kill-switch requirements identified in §16. If remaining time or dependencies make the full backlog unrealistic, reassign from optional breadth to these gates and present the unresolved launch scope for decision. This report does not approve a scope cut.

## 8. Evidence status and reporting discipline

The supplied [delivery receipt](../source/mcr_v2_en/DELIVERY_RECEIPT_EN.json) explicitly says application build/tests/deployment are `NOT_RUN`; its historical repository/commits and remote-verification claims require the parent's provenance checks. [Exporter regression results](../source/mcr_v2_en/generated/EXPORTER_TESTS_EN.json) claim nine passing planning-tool probes. Neither that recorded PASS nor this independent ledger arithmetic validates application behavior.

For later execution, retain the source baseline, contract/asset versions, exact candidate/configuration, task/IA IDs, migrations, commands/exit codes, raw evidence paths, limitations and verified delivery ancestry. Keep `MOCK_READY`, `CONTRACT_READY`, `INTEGRATED`, and `ACCEPTED` distinct, as required by plan §15.3. A test file or fixture existing is not a test run; a focused green is not complete release acceptance; a planning import is not C01–C08 implementation completion. No implementation status is changed by this assessment.
