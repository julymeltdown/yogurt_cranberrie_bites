# Execution record

Updated 2026-11-04 from published history through `391bc215` and current owner
handoffs. The snapshot below reports major feature outcomes and open delivery
boundaries. Earlier task receipts remain historical evidence; neither their
accepted-task count nor `generated/` is a current project-completion percentage.
All **112 required tasks and 61 required IA entries** remain in scope.

## Current major feature snapshot

| Feature | Delivered evidence | Current boundary and next owner |
| --- | --- | --- |
| Reference frontend | [Checkpoint](frontend/reference-checkpoint-2026-11-04.json) `e9ce93da`: 617 tests, typecheck/lint/build and native default-shell comparisons at 360/390/430px. Later World alignment `fb0bfcaa`, approved Public characters `5008a2ed` and Wall native QA `ce9e8e6c` are published. | **IN_PROGRESS.** These passes apply to their recorded revisions. Contract `3398d833` subsequently exposed frontend decoders that reject `oneOf`, including layered-avatar entry. Frontend king assigned one exclusive union-consumer owner to repair and publish, then Map/root owners rerun affected checks. Current master is not represented as green. Near-identical ten-screen design, full live flows and devices remain unaccepted. |
| Selected character artwork | [Source guidance](character-art.md), derivative integration `26a8eb61`: six source-preserving front-idle presets, transparent cutouts, a shared 384x64 atlas and source/hash evidence. The user's source-preserving reconstruction authorization is resolved. | **IN_PROGRESS.** Client composition and resource caching retain the supplied designs and avoid repeated asset requests. Canonical save/reload and World consumption are being connected. Directional/walk artwork and full layered customization remain unfinished; a historical tool-BLOCKED art goal is not an unresolved permission request. |
| Canonical avatar identity | [Selected-static contract](contracts/selected-static-avatar.md), `3398d833`: disjoint existing layered 1.0.0 and selected whole-character 1.1.0 contracts, six strict preset fixtures and manifest pin. | **CONTRACT_READY.** Backend avatar owner implements persistence/bootstrap/GET; World owner adapts its projection/serializer; frontend owns strict union admission/runtime. Actual new-variant database and end-to-end qualification remain **NOT_RUN**. |
| Auth and Friend revocation replay | [Auth RPC](backend/auth-invalidation-rpc.md), published through `dad5282c`, records 134 Rust and 2 harness tests. [Friend RPC](backend/friend-invalidation-rpc.md), published through `391bc215`, records 98 Rust and 5 harness tests, actual guarded TLS, owner mutations, replay, deadline/cancellation and PostgreSQL restart/cleanup. [Friend journal](backend/friend-invalidation.md) is published through `493958c4`. | **INTEGRATED** for these bounded owner components. Actual deployment and combined World revocation are **NOT_RUN**. The reused Auth chat now owns sealed Concert replay clients; it does not duplicate World code. |
| World revocation and event closure | [World invalidation](backend/world-invalidation.md), published through `a4ae8555`: local durable receipts, native publication fencing, scoped SQL removal and replay/dedup, with 75 pure and 6 native tests. | **IN_PROGRESS.** World owner implements the READ_ONLY transaction barrier and permit drain. Lifecycle owner has a separately committed closure/composite-receipt candidate awaiting that API and its native build turn. Remote replay/checkpoint/readiness and the real multiplayer gate remain unfinished. |
| Participant authority | Isolated participant producer source checkpoints are preserved; its existing chat was recovered and resumed with an actual ACTIVE goal. Static format/proto/ownership checks pass. | **IN_PROGRESS.** Compilation, guarded RPC and native qualification are **NOT_RUN**; the participant owner now holds the released MEDIA build slot and can transfer it directly to the canonical avatar owner if that parser is its actual prerequisite. Current avatar union compatibility is coordinated with the canonical avatar owner. |
| Product release | Bounded source, unit, browser-fixture and private native receipts are recorded in the lane documents. | No new whole-task/IA or IN01-IN05 acceptance is claimed by this snapshot. Real product startup, multiuser authorization/recovery, complete visual fidelity, physical devices, load and deployment require their own evidence. |

## Coordination and resource policy

[Current ownership](king-coordination.md) and `AGENTS.md` implement the user's
latest direction: kings manage major features and periodic commit logs; SLs manage
feature context, dependencies and staffing. Neither role approves or re-reviews
every commit. Exact-scope owners validate and non-force publish directly to
`julymeltdown/mcr` `master`, reconcile peer history and report remote ancestry.
Frontend and backend writers keep separate isolated checkouts and explicit paths.

Reuse existing chats for ready related work. Finished chats without a next task
are stopped/archived after preserving their work; a stale active flag does not
count as progress. The participant chat was recovered through supported chat
lifecycle controls, preserving its branch and documentation. Three superseded or
finished backend/QA chats were archived; frontend staffing remains with its king
and SLs. No additional chat quota is imposed.

Two existing backend build targets remain exclusive physical resource leases.
Source implementation proceeds independently. A lease moves only after actual
handle/process cleanup and a fresh free-space check; no third target is created.
The Auth RPC owner released MEDIA to Friend; Friend has now unconditionally
released it to participant. Participant and the canonical avatar owner may
sequence their real dependency with explicit exclusive lease handoffs. World retains BACKEND for closure qualification, followed
by the dependent lifecycle candidate when that API is delivered. These resource
and dependency waits are not review/approval queues.

## Historical task acceptance ledger

The following task-level receipts preserve earlier criteria and candidate
revisions. The historical tally was **11/112 required tasks** and **11/118 total**;
this is not a freshly audited completion count and must not be reported as an
unchanging project percentage. This major-feature update does not silently grant
new acceptance, reduce scope, or turn fixture/native component tests into live
multiuser or release proof. Current lane reports and the snapshot above take
precedence over stale pending-work descriptions in these earlier receipts.

| Task | State | Evidence and remaining scope |
|---|---|---|
| C01 | ACCEPTED | [Source map](source-map.md), two reviewed per-file inventories and exact import whitelist. Headless candidates need their own later extraction approval/tests. |
| C02 | ACCEPTED | [Foundation qualification](evidence/foundation.md): pinned frozen installation, strict FE build/lint, pinned source and imported full BE workspace builds. Docker execution is NOT_RUN and is not claimed by this minimal build task. |
| C03 | ACCEPTED | [Frozen core v0.1 contract](core-contract.md), exact inherited auth mapping, public/private DTOs, 33 examples and 23 tests; three scoped reviews. Email/core consumers can proceed. Optional OAuth exposure remains explicitly disabled until a separate handoff revision; runtime implementation is NOT_RUN. |
| C04 | IN_PROGRESS | [Domain review candidate](domain-contract.md), [target/query follow-up](evidence/domain-targets.md), [combined assembly](evidence/domain-assembly.md) and [internal keyed fingerprint correction](evidence/receipt-fingerprint-review.md) `9044417`: 162 definitions, 106 operations, 25 closed query schemas, 75 semantic rules, 15 inherited mappings, 36 payload fixtures, 47 domain and 15 assembly tests. Final/core pipeline emits 132 HTTP operations without changing frozen C03. [Readiness audit](qa/domain-readiness-review.md) identifies missing operation fixtures and actual frontend idempotency/error prerequisites. Full context coverage, deletion/legal/retention, client qualification and independent BE/FE reviews remain; not frozen or runtime-qualified. |
| C05 | ACCEPTED | [Frozen realtime v1.0.0](realtime-contract.md): strict frame/HTTP schemas and admission/stream rules, 24 fixtures and 31 regressions. [BE/FE rechecks](qa/realtime-contract-review.md) find no remaining scoped issues after corrections. Runtime ticket, socket, queue, capacity and race tests are NOT_RUN. |
| C06 | CONTRACT_READY | Five asset schemas and six golden fixtures preserve shared geometry/avatar identity. [Format correction](evidence/asset-format-validation.md) and [independent review](qa/asset-format-review.md) close missing checker and final-LF gaps; 27 C06 and 144 combined regressions pass. No schema/fixture bytes changed. Pixel alignment/art/venue truth remain NOT_RUN. |
| AR03 | IN_PROGRESS | [Selected source pack](character-art.md) preserves twenty unique JPEGs. Source-preserving reconstruction is authorized; six front-idle derivatives/atlas are published through `26a8eb61`. Full directional/walk art and layered customization remain. Historical tool-BLOCKED goal state is not a pending user permission request. |
| C08 | ACCEPTED | [Origin gate](qa/ui-origin.md), empty exact allowlist, 14 negative/positive test cases and current source scan pass. Modified copies and visual similarity require manual review; QA02 is not accepted. |
| BE01 | ACCEPTED | [Integrated foundation review](evidence/core-foundation-integration.md): 1,071 pinned files imported, full workspace builds, six separate services and exact owned process/port/database map exercised without source-default DB targets. |
| BE02 | ACCEPTED | [Integrated tooling review](evidence/privacy-local-stack-integration.md): native migration/restart persistence, owned foreground stack and private mail, 55 helper and 16 development tests, reviewed image/protobuf/SQL paths. Docker execution and production roles remain NOT_RUN and must be qualified before packaging/BE44. |
| BE03 | INTEGRATED | [Email ingress review](evidence/browser-auth-integration.md), candidate `689e4d2`: parent reran 221 Gateway tests, nine harness tests and 53 real native HTTP checks with three mail deliveries and cleanup PASS. Frozen email/account mapping, browser guards and invalid-credential handling delivered. Real browser cookie policy, configured OAuth and complete BE03 acceptance remain open. |
| BE04 | IN_PROGRESS | [Joined owner chain](backend/http-owner-chain.md), `ca26683` integrated as `e6c629d`: production Axum extraction/timeout and configured clients reach actual guarded Auth/Profile services and isolated databases. One native scenario with nine controls passes, including negative Auth dispatch suppression, restored-account fresh login, owner outages and original-stop refusal; cleanup passes. HTTP route is test-only/in-process. Signed Concert admission, event authority and whole identity acceptance remain open. |
| BE05 | ACCEPTED | [Integrated foundation review](evidence/concert-core-integration.md), source/workspace/doc `6a08701`/`f5b188b`/`376cf21`: typed hexagonal values/clock/policy ports, constrained event/participant/avatar schema, 12 Rust + 12 Python tests and three real PostgreSQL tests. Parent reran owned migration/reapply/schema-damage refusal, health/outage/restart/graceful cleanup. No product API or invented authority; BE04/bootstrap/production remain separate. |
| BE06 | IN_PROGRESS | Transactional producer integrated `48458d5`; [source-bound native receipt](backend/bootstrap.md) passes in the isolated worker ledger. Combined ledger and guarded startup checks now pass [bounded runtime evidence](evidence/delivery-2026-11-04/bootstrap-runtime.md); actual product release and public live-route qualification remain separate. |
| BE08 | IN_PROGRESS | Bounded implementation and fixes integrated through `5f7eb1c`, independent review through `4d6fc85`, and [retained evidence](evidence/backend-places-integration.md) published `920631d`. Nine focused tests pass. Worker PostgreSQL evidence used direct SQL, not the migration ledger; public routes and whole-task qualification remain open. |
| BE09 / BE19 | IN_PROGRESS | Bounded beacon/rally native stack integrated through `51783cf`, six tests before/after restart and cleanup PASS. Concrete root-aware policy adapter, place projection, public routes and real IN02 remain; no arrival inferred from world movement. |
| BE10 / BE11 | IN_PROGRESS | Isolated admission owner implements one-use tickets and room membership, migration 0005. Native failures are under correction; no public ticket/room/WebSocket or capacity acceptance is claimed. |
| BE16–18 / BE20–23 | IN_PROGRESS | Separate community backend owner; party/gift/maintenance source through `5d8efe7` has bounded compiled/copied-binary native proof; durable-error/current-policy follow-up remains active before final handoff. Migration 0007 reserved; no public command acceptance. |
| BE37 / BE28–30 | IN_PROGRESS | Typed relationship source `b71ae36` compiled and passed eight pure/crypto checks; 20 native producer tests/restart/ledger/cleanup passed and integrated through `914843e`; actor-only lock-order follow-up remains active. Migration 0006 reserved. A separate Auth/Friend authority owner is assigned; a fresh RPC read alone does not establish revocation fencing through a Concert transaction. |
| BE15 | IN_PROGRESS | [Reviewed internal receipt/outbox foundation](evidence/command-receipts-integration.md), `7cbb97e`/`c2a2c30`: 33 combined deterministic Rust, 82 Python helper and 17 real PostgreSQL tests pass; owned migration failure/recovery, restart persistence and cleanup pass. Review fixes cover keyed private fingerprints, post-authorization expiry and result/outbox revision equality. Bounded goal COMPLETE; production producer/Auth/public API/key configuration/runtime/load gates remain NOT_RUN. |
| FE05 | ACCEPTED | [Integrated evidence](evidence/ui-primitives.json) and [independent review](qa/ui-primitives-review.md): 22 DOM tests, strict build/lint, real Chrome keyboard/focus/dismissal and 44px targets at four viewports. [Style correction](evidence/ui-style-correction.json) removes prohibited left accents and offset shadows; build/lint and all four Chrome viewports pass again. Physical devices and screen readers remain separate QA work. |
| FE04 | IN_PROGRESS | Separate media owner also has exact shared `api/http.ts` and HTTP-test ownership for canonical authenticated binary transport. Current C04 same-origin authenticated PUT and credential-free presigned upload semantics remain distinct; no presigned endpoint is invented. |
| FE12 | IN_PROGRESS | Shared selected-source compositor and minimal scene hooks are being validated on the isolated avatar checkout. Synthetic native pixel parity is test evidence; current C06 and product-art availability remain unchanged. |
| FE20 | IN_PROGRESS | Separate text-beacon/explicit-arrival frontend owner. Current C04 contains no photo mutation field; no fake persistence is introduced. Server-qualified audience/place/TTL authority remains required. |
| FE22 | MOCK_READY | Worker `1c38ad4` integrated `e82a220`; parent 8 focused tests pass. [Authoring evidence](frontend/community-authoring.md) reports browser fixtures at 360/390/430/1280, receipt recovery and privacy cleanup. Root composition, real service/multiuser/device acceptance remain open; FE23/25/26 continue independently. |
| FE27 | MOCK_READY | Worker `97ef9fb` integrated `9f72a71`; parent 10 focused tests pass. [Feed evidence](frontend/social-feed.md) covers scoped timeline/notices controller behavior. Root/browser/live acceptance remain open; FE28 text/thread and media qualification continue. |
| FE06 | MOCK_READY | [Theme integration review](evidence/email-and-theme-integration.md): three scoped palettes and atomic Phaser skin replacement; unchanged map/camera/movement, fallback, reentry and resource disposal pass focused262, build/lint and three-width native checks. Parent fixed-source checks pass. Production art/shell wiring, server preferences and real multiplayer remain open. |
| FE01 | ACCEPTED | [Integrated review](evidence/core-foundation-integration.md): 144 transport/error tests plus 22 existing UI tests, strict build/lint; memory-only session, bounded read refresh, cancellation, safe error/upload ports and stale-identity isolation. Real cookies and auth journeys remain BE03/FE09 work. |
| FE02 | ACCEPTED | [Integrated privacy review](evidence/privacy-local-stack-integration.md): actual QueryClient/observers, canonical private keys, cancellation plus eviction, commit fencing and cleanup ports; 45 new tests and combined 189-test transport/privacy run pass. UI/event/real-server owner wiring remains. |
| FE07 | MOCK_READY | [Integrated shell review](evidence/world-shell-integration.md), candidate `d7909c0`: 276 tests, frozen build/lint/origin scan, four installed-Chrome viewports and production-entry isolation pass. Review fixes block obsolete initial loader dispatch and bound scroll history. Actual Phaser/socket/camera preservation and device qualification remain; no real-world or IA acceptance. |
| FE08 | MOCK_READY | [Integrated public-entry review](evidence/public-entry-integration.md), `e7de0f3`: 410 parent tests, build/lint/origin/boundary, four-width Chrome/reduced-height and production isolation pass. Canonical redirect and safe synthetic document presentation delivered. Approved publication/legal/contact/retention inputs and real-service/device/IN01 gates remain open. |
| FE09 | MOCK_READY | [Real local email journey](evidence/email-and-theme-integration.md) now passes 17 actual browser checks, including transmitted revoked-cookie rejection and two-tab invalidation; integrated `2f398a3`/`bed5ffa`. Retained focused auth regressions pass. Secure/TLS, automatic restore/rotation, provider, event/world and IN01 acceptance remain open. |
| FE10 | MOCK_READY | [Real local email journey](evidence/email-and-theme-integration.md) exercises actual signup, delivered verification, wrong-code refusal and successful verification through real forms/services. Production eligibility/consent remains blocked; test enrollment policy is explicitly synthetic. Full production/device acceptance remains open. |
| FE11 | MOCK_READY | [Real local email journey](evidence/email-and-theme-integration.md) opens the actual delivered app reset link, scrubs its fragment, resets password, invalidates the peer and proves the old cookie/password rejected and new password accepted for the same identity. Secure/TLS, OAuth/device/IN01 remain NOT_RUN. |
| FE14 | MOCK_READY | [Reviewed runtime foundation](evidence/phaser-runtime-integration.md), final `77ae6bd`: 871 combined tests, strict build/lint and real Chrome four-width lifecycle/resource matrix pass. Three P2 findings corrected and independently cleared; both bounded goals COMPLETE. Production assets, protected-shell/avatar/world composition, devices and whole FE14 acceptance remain open. |
| FE15 | MOCK_READY | Reviewed movement/camera/interaction implementation `d91ee02` and bottom-edge correction `aec5d17`; independent QA `b381d03` closes visibility and valid-boundary-route findings. Worker 958 tests/22 files (87 FE15), build/lint and four-width native/short-height checks pass. Parent reran affected movement/runtime tests, build/lint and inspected mobile/short screenshots. Actual networking, authoritative physics, selected artwork, devices and full-world integration remain open. |
| FE16 | MOCK_READY | [Reviewed transport integration](evidence/world-transport-integration.md), `e3a4d84`/`5863f91`: 568 combined frontend tests (158 transport), build/lint/type-drift/origin/plan and installed-Chrome five canonical traces pass. Gap recovery and pre-send room-switch failure findings corrected. Bounded goal COMPLETE; production lifecycle, replication and real-server/IN01 remain NOT_RUN. |
| FE17 | MOCK_READY | [Replication integration](evidence/replication-integration.md), worker `bd202ad` integrated as `79e4556`: bounded headless roster/interpolation/input/ack/recovery ownership; parent181combined tests/build/lint and independent probes pass. Actual authoritative physics is unavailable under C05; FE12 rendering/art and production server/shell qualification remain open. |
| FE18 | MOCK_READY | [Bounded integration review](evidence/world-access-integration.md): worker `4bf0413` integrated as `5ef66e1`; parent verified 15 owned paths, source/native hashes, 48 tests, strict build and representative native screenshots. Rooms and nearby consume the existing replication owner and C05 catalog. Invitations, approved avatar rendering, root composition and real-server/IN01 acceptance remain open. |
| FE19 | MOCK_READY | [Places integration](evidence/places-integration.md): map overview, scoped place details and controls help; stale-target and actual reload corrections independently closed. Parent61focused/build/lint pass and all154review source hashes match. Production shell/BE08/venue/device and whole-task gates remain open. |
| FE21 | MOCK_READY | Worker `0497d86` integrated as `78cfb7c`; scoped independent review and parent72affected tests/build/lint pass. Default `/meet` and detail routes now use one shell-owned controller, retain pending commands across tabs and enforce fresh READ_ONLY cancellation decisions. [Composition evidence](evidence/community-shell-integration.md) records worker actual-Phaser fixture and parent default-router native runs. Real backend/SQL/multiuser and whole-task gates remain open. |
| FE24 | MOCK_READY | Worker `5f1005d`/`595c07f` integrated as `3cb3f9c`/`8c33ba3`; scoped QA clear, parent105affected tests/build/lint pass. [Composition evidence](evidence/community-shell-integration.md) connects list/detail/own activity to default routes with policy-bound bootstrap, lifecycle cleanup and390px/wide Chrome checks. No actual backend or reservation-command qualification. |

## Historical delivery notes

The notes below retain their original checkpoint states, review practices and
counts. They are not current staffing instructions, publishing gates or project
progress percentages. The current snapshot and coordination policy above
supersede them.

Reviewed imports through `2b42fd4`: frontend audit/scaffold, backend audit/import,
frontend lock/toolchain configuration, exact auth review, boundary guard and C08
origin guard. The first backend import preserves source whitespace in four files;
the [recorded warnings](evidence/foundation/backend-import-whitespace.log) are not
unrelated edits. No Bigyeting UI or implementation module was imported.

Each later handoff must update this record with actual evidence and limitations;
do not advance statuses simply because a worker reports completion.

FE05 integrated as `ae24437`; its exact worker candidate is `8f9ddf7`. A separate
review reproduced and rechecked the preliminary nested-modal defect, then found
no remaining actionable issue on that candidate. Parent reran all 22 component
tests, strict build/lint and the four-viewport Chrome driver after integration.

FE11, the Auth-owned read and FE10 finished bounded review with COMPLETE goals
and stopped clean checkouts. The authenticated Auth RPC handoff and R1 correction completed parent review
with actual COMPLETE goals. Independent QA review
`e0b37cc`, integrated as `e103da2`, found no actionable defect in the event-scope
correction or Auth prerequisites; its goal is COMPLETE. The actual handler was
outside that review. The original BE04 goal remains BLOCKED. Parent owns the
shared RPC/platform prerequisite `a44f8fa`/`e79fd177` (not published as working
admission), remaining Gateway/Concert composition and the
[admission direction](identity-admission-decision.md).
The user removed any fixed backend/frontend quota; the
[capacity assessment](parallel-capacity.md) records allocation decisions.
BE03 browser/provider gates remain open.
The integrator retains shared Auth/Profile authority and C04/C06 contracts;
BE06/bootstrap follows qualified authority. No runtime, OAuth or release
acceptance follows from completed component/contract checks.


FE14 runtime/resource work has a new tool-confirmed ACTIVE goal on a fresh
frontend worktree. Auth RPC and actual-handler QA goals are now COMPLETE: R1
was a test setup race, its correction is integrated, and final clean `97a698a`
passes eleven controlled cases plus five real RPC and 22 retained PostgreSQL
cases. All source pins, restart/cleanup and independent disposition are verified.
The parent also corrected the BE15 harness's stale candidate pins after `5c02a31`;
all 98 helper tests, 138 contract regressions and planning checks pass at `d197379`.
No production moderation, identity admission or release gate is thereby enabled.


The next BE04 strict Gateway access-context increment is now tool-confirmed
ACTIVE on its fresh worktree at published `b16a679`, alongside FE14. It preserves
legacy JWT behavior and adds only a verified identity/expiry/original-stop
prerequisite; no Concert route or accepting authority is enabled by dispatch.
The completed Auth and QA checkouts remain stopped. Exact exceptions are recorded
in the ownership register. Project goal remains ACTIVE; accepted counts remain
11/112 required and 11/118 total.


Strict Gateway context worker `ed2f166`, integrated unchanged as `e7d208e`,
completed parent source/ownership review and combined 235-test, three-doctest,
library/binary qualification. Its actual bounded goal is COMPLETE and checkout
stopped; the parent owns the retained Rust target. FE14 remains ACTIVE. The idle
QA chat now has an actual ACTIVE bounded C06 format audit on a fresh report-only
checkout: it reproduced missing optional checkers and a trailing-LF plugin edge,
and reviews the parent's isolated correction. No fixed chat quota or acceptance
count changes follow from these assignments.


C06 format correction and independent report are integrated as `6a1e394`,
`04a8268` and `bab2f1f`. Parent final 144 contract regressions pass with exact
source/fixture pins; both findings are closed. QA's actual goal is COMPLETE and
checkout stopped. The backend chat now has a new actual ACTIVE owner-client goal
on fresh `be-gateway-auth-owner`, published base `e6efa4c`; its previous JWT
checkout stays stopped. FE14 handed off clean for review, retaining its ACTIVE
goal. No new chats, acceptance-count changes or production authority activation.
