# Execution record

Updated **2026-11-04T11:27:19+09:00**, from published history through `980fc4d8` and existing developer/SL delivery reports. This is a management reconciliation, not a code, query, visual or test review. All **112 required tasks and 61 required IA entries** remain in scope; the six optional EX tasks are excluded from the denominator.

## Current task accounting

**The old 11/112 number counted historical formal acceptances; it did not count all implemented work. The central ledger failed to incorporate later task-local completion dispositions.**

| Original-task disposition | Count | Meaning |
| --- | ---: | --- |
| Recorded acceptance dispositions | **12** | Eleven historical ACCEPTED records plus the implementation owner's explicit original FE08 acceptance. |
| Additional task-local criteria closed | **21** | Existing dispositions plus original BE12 movement and FE07 shell criteria; exact owner states are preserved, with no new king/SL technical sign-off. |
| Other implementation evidence | **65** | Bounded implementation, fixtures or component delivery exist; original-task completion is partial or not yet reconciled. |
| Status not yet reconciled | **9** | Insufficient current per-task disposition in this reconciliation; not assumed unstarted. |
| Open integrated-product/release gates | **5** | IN01–IN05 have no completed gate receipt in the current reports. |
| **Required total** | **112** | Every original required ID appears once in the linked register. |

There are **33 recorded task-criteria outcomes** (12 recorded acceptance dispositions plus 21 additional task-local closures), **not only 11 tasks with implementation**. The latest reconciliation adds original **BE12 and FE07**, following FE14, FE19, QA01, QA02, FE09, FE15 and QA07, from explicit implementation-owner dispositions and published receipts. FE07 and FE15 retain their exact **INTEGRATED** states and explicit original-criterion closure; they are not relabeled as formal ACCEPTED dispositions. This is not a claim that 33 tasks were newly accepted today, nor an effort-weighted completion percentage. The 65 other evidence-bearing tasks are not silently counted as complete. A completed worker goal, a new commit, a test count or an assistance artifact adds no original task IDs.

The [112-row task register](task-progress-2026-11-04.json) records each original ID, its evidence pointers and the distinction above. Its null states mean unreconciled completion, not NOT_STARTED. No current full-product completion percentage is asserted.

### Completion dispositions reconciled from owner records

[Specification section 12](../../MCR_IMPLEMENTATION_PLAN_V2_EN.md#12-frontend-implementation-tasks) permits frontend contract fixtures and deliberately does not make corresponding backend implementation a prerequisite. Live integration requirements remain in their own tasks/gates. Pure helper/contract tasks likewise use their own original criteria, without an invented IN05 prerequisite.

| Tasks | Existing disposition and delivery record |
| --- | --- |
| **FE22, FE23, FE25, FE26** | Explicit SATISFIED task-local table in [Community's published tracker](scrum/community.md#frontend-task-disposition-against-specification-section-12). Published integration ancestry includes `e82a220`, `aa103a8`, `48bb3e8`, `95d9f37`, `941d36c` and final KST correction `d8778b8`. The old publication-pending notes are historical; these integration commits are in the current remote history. |
| **FE27, FE28** | Social SL's retained task-local fixture-complete dispositions and [Feed/thread delivery record](frontend/social-feed.md), with published `cf7836a`, `a413e69`, `5bc0055`, `b392f56`. Live Post/media/write authority and IN03 remain separate. |
| **FE29, FE30** | Social SL's existing task-local criteria disposition and [introduction/profile record](frontend/introduction-profile.md), delivered through `a3bdb38`. Original author commits need not themselves be ancestors after recorded integration. Actual social/age/public-profile producers remain open. |
| **FE40** | [Archive owner record](frontend/archive.md) explicitly records the complete assigned read-only feature and its task-local fixture criteria implemented. Real closure/publication/media and integrated-product acceptance are separate. |
| **FE08** | The [Landing owner record](frontend/reference-public.md) explicitly declares original frontend criteria ACCEPTED in `0c76e427`, with required artwork `b01eb708` delivered. This is owner-declared task-local acceptance, not a new king/SL review. Production legal/contact content and its document-provider composition remain separate A07/BE38 dependencies. |
| **BE25** | The policy implementation owner reconciled the original pure-policy scope and reports no remaining criterion; the old goal is actually COMPLETE. Published `3810016a` and [combined qualification record](backend/intro-age-combined-qualification.md) support the disposition. A separate canonical-authority goal is ACTIVE; its provider/DB/RPC work does not keep original BE25 open. |
| **FE33** | The [mural lifecycle owner record](frontend/mural-entry-lifecycle.md), published `7c2f3af7`, explicitly declares original frontend convergence, rejected-preview rollback and restore-invalidation criteria SATISFIED under frozen contracts/fixtures. The bounded goal is complete; separately assigned caller assembly and actual backend/IN04 integration remain open. |
| **FE20** | The [meeting-beacon owner record](frontend/meeting-beacon.md), published `6c908472`, explicitly declares original frontend criteria SATISFIED at MOCK_READY. Its remaining photo criterion now includes authorized upload/save/fresh-controller reread and privacy/recovery behavior against frozen contract fixtures. Actual backend durability, server authority, root admission and IN02 remain separate. |

| **FE14, FE19** | The [root/Map owner reconciliation](frontend/reference-scene-root.md#explicit-original-criterion-reconciliation), published through `10784b26`, declares the client/source-fixture criteria satisfied after all-era root adoption. Existing Map/Places recovery/source/closure/help evidence is preserved. The historical FE15 remainder is now closed by the later owner record below; the current FE07 shell criterion alone does not close original FE07. Actual Product admission and full reference fidelity remain separate. |
| **QA01** | The [contract-coverage owner record](evidence/contracts/qa01-coverage/README.md), implementation `6e2038fc`, evidence `19d7441f` and publication `bd103c10`, explicitly declares original QA01 criteria SATISFIED for pinned canonical inputs at CONTRACT_READY. The same goal is actually COMPLETE, followed by null/clean STOP. This does not accept whole C07, live producers or IN gates. |

| **QA02** | The [UI-origin audit owner record](evidence/ui-origin/README.md), published `11ca3a3e`, declares original criteria SATISFIED at its pinned candidate after source comparison and native presentation review across 59 route patterns and all 61 IA entries. Goal complete and checkout clean. This closes the original origin audit, not final fidelity, functional route readiness, QA06 or IN gates. |

| **FE09** | The [production refresh owner record](frontend/production-refresh.md), source `dc88c248`, receipts `f96ea11b` and publication `62b4bf80`, declares original frontend criteria SATISFIED. Production caller activation and browser-policy cases close the remaining original caller behavior; actual goal complete and no active goal remain. Frontend fixtures and backend real-service evidence stay separate, and broader test failures are retained rather than claiming a green full suite. |

| **FE15** | The [application-input owner record](frontend/fe15-application-input.md), source `977b773d` / `50d950d7`, support `416b4e5f` / `1b4ac457`, final receipt `accc0dfe` and publication `a84885f2`, explicitly declares the original client/application remainder **INTEGRATED** and closed, with no FE15-owned behavior remaining. The owner reports native browser qualification at 360/390/430, goal COMPLETE then null/clean STOP, and returned ownership. Separate QA07, physical-device, live integration and release requirements remain open under their own tasks. FE07 scroll support is not whole FE07 closure. |

| **QA07** | The [accessibility/input owner record](evidence/accessibility/qa07/README.md), artifact `41f7c64c` and final publication receipt `ced8ec12`, explicitly declares original criteria **SATISFIED at MOCK_READY**. The owner reports keyboard/focus, screen-reader labels, DOM alternatives, reduced motion, trusted pointer cancellation and essential social/pickup flows delivered, with three FE15/FE36 defects fixed by their owners and independently requalified. Actual goal COMPLETE then null/clean STOP and returned ownership are recorded. Actual screen-reader announcements, physical-device, live service and IN/release limits remain separate; no extra original prerequisite is invented. |

| **BE12** | The current World implementation owner explicitly reconciles original authoritative movement-loop criteria as **SATISFIED**, using the retained 26 native deterministic cases and published `2d3faaa6` in the [movement record](backend/world-simulation.md). The historical document's CONTRACT_READY/full-task caveat is preserved as historical evidence; the new disposition closes the original fixed-step movement, speed, collision, spawn/range and negative-case scope. Actual public movement, two-user delivery, new FE17 contract work and IN01 remain separate. No BE10/11/13/14 closure is inferred. |

| **FE07** | The [whole-shell journey owner record](frontend/fe07-shell-journey.md) explicitly declares original client criteria **SATISFIED at INTEGRATED**, at pinned candidate `8cd20466`. Source `009327ca` and evidence `c19ad5b1` were published as `19ad4ca0` / `59de1ac9`, with final receipt `629a9f84`; integrator-confirmed `980fc4d8` preserves all 24 payload pins. The owner confirms the same goal COMPLETE then fresh null/clean STOP and returned ownership. The actual browser router, canonical client Auth ownership and default Phaser qualify the full shell/history/direct-gift journey. Live services, physical devices, art admission and IN/release work remain separate. |

This reconciliation preserves the owners' actual dispositions: FE08 is explicitly owner-declared ACCEPTED, FE07 and FE15 retain their explicit INTEGRATED states and original-criterion closures, and other SATISFIED records are not relabeled as new formal acceptances. Known follow-up requirements, including the later exact visual target, remain tracked even when the original task-local implementation criteria were met.

## Current major feature delivery

| Feature | Published or owner-reported delivery | Required remainder |
| --- | --- | --- |
| Actual Auth → Profile → public bootstrap | Root owner reports published source `e9e65e5d`, integration `14b0548a`: two real canonical Auth principals, actual Profile PUT and public HTTP bootstrap 200 for each; map4 catalog, 27 migrations, positive/negative probes and cleanup qualified by that developer. | Actual public World sockets and the two-client journey remain NOT_RUN. World's current native handoff and Root startup wiring retain named owners; this milestone does not close IN01 or require optional Age/Intro/Gifts/new-art work first. |
| Selected characters and root integration | Canonical avatar persistence `b13d79e6`, participant increment `0313afa5`, whole editor `c3f411ad`, default-root character flow `9fc3a299` / `903773b8`, authorized portrait/media binding `9dacd962`. | Actual live identity/Profile/authority journeys, all original avatar capabilities, directional/walk/layered art and full device/visual acceptance. |
| Ten-screen reference design | Auth refinement `08d1b1c0` and required artwork `1249430f`, Landing `d5ec766d` and artwork `b01eb708`, Map `f4b40808`, Gifts `9055af1e`, Profile/party/feed portrait delivery `9dacd962`; existing browser-fixture records are retained. | Auth and Landing owners report their required artwork increments delivered. Original FE09 production refresh caller is now delivered in `62b4bf80`; FE08 and FE09 task-local criteria are recorded satisfied. The combined live frontend/service journey remains separate. All ten screens still need the requested near-identical final design; fixture success is not complete visual acceptance. |
| Three-era scenery and map | All three era inputs `02320134`, canonical Tiled/loader `aa3e2f3d`, World scene hook `969b3b2f` and Map consumer `4dc12a56` are published. World owner correction `482d02c4` identifies all four current scene targets as virtual concepts. | The source-fixture root/Map integration is now delivered in `10784b26`; final reference fidelity and actual Product/art/theme/release admission remain open. These four virtual records do not establish a physical-venue-review prerequisite. Real physical-place qualification remains separate AR01/beacon/IN02 scope; no REVIEWED status or physical eligibility is invented. |
| Feed and media product APIs | Feed groundwork `02b17e04`, owner media `84126f6a`, Product API transport `49fb637c`, platform authority checkpoint `ba64c62e`. | Actual durable canonical guards, Profile/producer composition, public HTTP and recovery/revocation journeys. Published transport does not establish enabled live Feed/media. |
| Meeting-photo client | Original FE20 client delivery `6c908472` satisfies the owner-reported frozen-contract/fixture criteria; photo upload, attachment, save and fresh-controller reread are included. | Actual meeting-photo producer, owner-media lifetime/read/erase, Product API root admission and IN02/operator/device/multiuser requirements. |
| Meet, Gifts and Wall | Retained community and mural kernels are delivered; reused developers have accepted isolated live-producer assignments. | Genuine public authority, root transport, party commands, reservation/pickup and multiuser mural flows. Newly assigned producers do not close the original feature tasks. |
| World revocation and event closure | Owner replay components and bounded closure `a93a70ec` are published; remote invalidation/combined owner integration continues. | Actual two-account world/revocation, operator/retention and owning-adapter composition, remaining lifecycle/deletion flows. |
| Release | Existing lane-level source/native/browser receipts are preserved. | **IN01–IN05: 0 completed gate receipts established.** Real devices, accessibility, capacity/recovery, qualified real-stack startup, remaining physical-place/operator inputs and full 61-entry IA accounting remain open. |

## Next integrated milestone: authenticated World

Management dependency snapshot **2026-11-04T10:40:12+09:00**, from current implementation-owner and SL receipts. No king/SL code, query, test or visual review was performed. The original IN01 journey remains open; component readiness below does not certify it.

| Required input or journey | Current owner-reported position | Responsible implementation owners / next action |
| --- | --- | --- |
| Browser session and refresh | Backend `86cf6bcb` / `8d283459` is qualified; frontend original FE09 is delivered in `62b4bf80`. Frontend fixtures and backend actual-service evidence remain separate. | Root frontend owner `01a1a153-1227-7e72-801f-a2a6b91981e1` and stack owner `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` coordinate the genuine joined browser/service journey through a fresh explicit handoff. |
| Participant, Profile and avatar persistence | Typed/generation components and Profile preparation/abort increments are delivered; the joined participant save/recovery flow and avatar transaction witness still need their current validation. Participant is compiling; avatar retry is queued after its build-cache correction. | Participant `01a1a209-80da-7e53-a295-b64cfe09c1d7`, Profile `01a1a1fd-6b5d-75b0-8d42-01ade79b9393`, avatar `01a19f83-12cc-7f21-8ad3-55749f012d9b`; finish the actual owner/journal/write-witness handoff. |
| Current selected assets, map and three themes | The current source-bound bundle constructor has an owner-reported passing result using actual resource files. This is an input receipt, not operator admission or full visual acceptance. | Spatial/avatar owner and stack owner retain the current map4 bundle for the first real World journey. |
| World runtime | Last owner report retains 109 passing regressions and 9/11 native cases. The ticket-issuance/member-preservation correction `c13d7799` awaits its fresh allocation and validation. Production startup remains disabled. | World `01a1a20f-8d37-71c3-a429-8a55bb6d6c49` resolves the two remaining cases and hands its concrete startup inputs to the stack owner. |
| Isolated service startup and two-client journey | No completed real two-user World startup/journey receipt has been supplied. | Stack owner prepares the actual isolated configuration and two-account seed; assigned frontend/World developers execute the original IN01 flow when the concrete inputs are ready. No manager test stage is added. |

The next free backend allocations prioritize ready World/participant/avatar-witness dependencies without preempting accepted holders or starving other work. BE26, BE27, social, Gifts, Feed, Wall and optional capability activation are not prerequisites for basic World entry; their required implementation continues independently. Unavailable capabilities stay unavailable.

The artist's new scenery source `492160b1` (`spatial-v4` / `quality-v8`) is separately published. Its 72 additional collision cells and 12 solid props require a new canonical map revision greater than 4 and versioned client adoption; old map4 bytes remain immutable. Frontend root has an actual new active adoption goal under World SL. The backend spatial adoption follows the current avatar-witness work. This visual delivery must not delay the valid current-map IN01 work, and source publication is not final reference fidelity or an original art-task closure.

FE07, FE15 and QA07 continue with disjoint developer ownership. The conflicting IN02 dispatch to the FE07 developer was withdrawn: FE07 is its sole active assignment, with IN02 queued for an actual later transfer. Reported typing/visibility and Back-scroll defects are assigned to the incumbent frontend source writer; the report-dismissal/block-control defect is separately assigned through Social. The broader FE09 diagnostic failures are routed to the responsible lanes, with fixture, collection and environment issues kept distinct from confirmed product defects.

QA04 remains unfinished. Its developer reports publication `129f37ad`, 112 passing fixture cells and 63 unsupported cells, and a blocked persistent goal awaiting the actual principal/seed/HTTP/WS inputs from the social-runtime and stack owners. This is not QA04 or IN03 completion and does not block unrelated frontend work. The existing Social SL retains the concrete dependency and lifecycle follow-up; no goal reset or speculative restart is required.

## Staffing and delivery policy

[Current coordination roles](king-coordination.md) remain authoritative: developers implement, validate, resolve technical interfaces and non-force publish their exclusive scopes directly to master. Kings manage priorities, staffing escalation and major delivery; SLs manage feature dependencies, ready work and worker lifecycle. No king/SL technical review or approval stage is introduced by this report.

Backend staffing/resources SL owns both existing target queues and actual handoffs. Current holders must come from live receipts, not old snapshots in this file. Frontend/art staffing belongs to the frontend king and its three SLs. Finished chats are reused only for ready disjoint scopes or archived with work and unfinished requirements preserved. No chat quota is imposed.

The backend meeting-photo owner has accepted an isolated active assignment supporting original BE09/FE20; Social coordinates the frontend photo continuation. This is assignment progress, not task completion. Contract and fixture work can proceed independently of physical-venue live qualification.

Named future backlog ownership covers the remaining beacon/rally scope, age/introduction/mutual sharing, Feed mutations/comments, notifications, operations/moderation, closure/deletion and real-stack startup. These future assignments are not counted as active or complete. The planning exports remain untouched.

## Historical task acceptance ledger

The following task-level receipts preserve earlier criteria and candidate
revisions. The historical tally was **11/112 required tasks** and **11/118 total**;
this is not a freshly audited completion count and must not be reported as an
unchanging project percentage. This management reconciliation does not silently grant
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
