# Execution record

Updated 2026-11-03. This record supersedes intake-only status statements in the
historical assessments. `generated/` remains the original planning baseline.
Acceptance below applies only to the stated task criteria and candidate paths;
it is not feature integration or release acceptance.

| Task | State | Evidence and remaining scope |
|---|---|---|
| C01 | ACCEPTED | [Source map](source-map.md), two reviewed per-file inventories and exact import whitelist. Headless candidates need their own later extraction approval/tests. |
| C02 | ACCEPTED | [Foundation qualification](evidence/foundation.md): pinned frozen installation, strict FE build/lint, pinned source and imported full BE workspace builds. Docker execution is NOT_RUN and is not claimed by this minimal build task. |
| C03 | ACCEPTED | [Frozen core v0.1 contract](core-contract.md), exact inherited auth mapping, public/private DTOs, 33 examples and 23 tests; three scoped reviews. Email/core consumers can proceed. Optional OAuth exposure remains explicitly disabled until a separate handoff revision; runtime implementation is NOT_RUN. |
| C04 | IN_PROGRESS | [Domain review candidate](domain-contract.md), initial implementation `b615e35`, [target/query follow-up](evidence/domain-targets.md) `d0649ec`, and [combined assembly](evidence/domain-assembly.md): 162 definitions, 106 operations, 25 closed query schemas, 75 semantic rules, 15 exact inherited mappings, 35 payload fixtures, 46 domain and 14 assembly tests. The retained final/core pipeline emits 132 HTTP operations without changing frozen C03. Full request/response context coverage, deletion/legal/retention contracts, client qualification and independent reviews remain; not frozen or runtime-qualified. |
| C05 | ACCEPTED | [Frozen realtime v1.0.0](realtime-contract.md): strict frame/HTTP schemas and admission/stream rules, 24 fixtures and 31 regressions. [BE/FE rechecks](qa/realtime-contract-review.md) find no remaining scoped issues after corrections. Runtime ticket, socket, queue, capacity and race tests are NOT_RUN. |
| C06 | CONTRACT_READY | Five asset schemas, six golden JSON fixtures and 21 checks pass; all three themes share geometry and avatar identities. Pixel alignment/art/venue truth remain NOT_RUN. |
| AR03 | IN_PROGRESS | [User-selected character source pack](character-art.md), revision 3: twenty unique JPEGs (ten base sheets plus ten additions); the first batch's duplicate stays archived. Source catalog and frontend lookup preserve both selected batches. Previous character art is prohibited. Transparent cutouts, layer-contract revision and direction/walk atlas remain. |
| C08 | ACCEPTED | [Origin gate](qa/ui-origin.md), empty exact allowlist, 14 negative/positive test cases and current source scan pass. Modified copies and visual similarity require manual review; QA02 is not accepted. |
| BE01 | ACCEPTED | [Integrated foundation review](evidence/core-foundation-integration.md): 1,071 pinned files imported, full workspace builds, six separate services and exact owned process/port/database map exercised without source-default DB targets. |
| BE02 | ACCEPTED | [Integrated tooling review](evidence/privacy-local-stack-integration.md): native migration/restart persistence, owned foreground stack and private mail, 55 helper and 16 development tests, reviewed image/protobuf/SQL paths. Docker execution and production roles remain NOT_RUN and must be qualified before packaging/BE44. |
| BE03 | INTEGRATED | [Email ingress review](evidence/browser-auth-integration.md), candidate `689e4d2`: parent reran 221 Gateway tests, nine harness tests and 53 real native HTTP checks with three mail deliveries and cleanup PASS. Frozen email/account mapping, browser guards and invalid-credential handling delivered. Real browser cookie policy, configured OAuth and complete BE03 acceptance remain open. |
| BE04 | IN_PROGRESS | [Reviewed identity foundation](evidence/identity-integration.md), `eba858f`: 30 deterministic parent tests pass, including 18 new identity cases; seven-path boundary and 28 hashes match. Ports/consumer behavior MOCK_READY; owner-backed suspension/provider proof and authenticated Concert service edges remain integrator prerequisites. Production authority stays unavailable and goal active. |
| BE05 | ACCEPTED | [Integrated foundation review](evidence/concert-core-integration.md), source/workspace/doc `6a08701`/`f5b188b`/`376cf21`: typed hexagonal values/clock/policy ports, constrained event/participant/avatar schema, 12 Rust + 12 Python tests and three real PostgreSQL tests. Parent reran owned migration/reapply/schema-damage refusal, health/outage/restart/graceful cleanup. No product API or invented authority; BE04/bootstrap/production remain separate. |
| BE15 | IN_PROGRESS | Separate receipt/outbox lane at `376cf21`, active persistent goal confirmed. Internal design review and pinned C04 candidate inputs; real PostgreSQL crash/replay/lease qualification and committed review remain. Central prerequisite `f74fb66` stays isolated until complete implementation can compile. |
| FE05 | ACCEPTED | [Integrated evidence](evidence/ui-primitives.json) and [independent review](qa/ui-primitives-review.md): 22 DOM tests, strict build/lint, real Chrome keyboard/focus/dismissal and 44px targets at four viewports. [Style correction](evidence/ui-style-correction.json) removes prohibited left accents and offset shadows; build/lint and all four Chrome viewports pass again. Physical devices and screen readers remain separate QA work. |
| FE01 | ACCEPTED | [Integrated review](evidence/core-foundation-integration.md): 144 transport/error tests plus 22 existing UI tests, strict build/lint; memory-only session, bounded read refresh, cancellation, safe error/upload ports and stale-identity isolation. Real cookies and auth journeys remain BE03/FE09 work. |
| FE02 | ACCEPTED | [Integrated privacy review](evidence/privacy-local-stack-integration.md): actual QueryClient/observers, canonical private keys, cancellation plus eviction, commit fencing and cleanup ports; 45 new tests and combined 189-test transport/privacy run pass. UI/event/real-server owner wiring remains. |
| FE07 | MOCK_READY | [Integrated shell review](evidence/world-shell-integration.md), candidate `d7909c0`: 276 tests, frozen build/lint/origin scan, four installed-Chrome viewports and production-entry isolation pass. Review fixes block obsolete initial loader dispatch and bound scroll history. Actual Phaser/socket/camera preservation and device qualification remain; no real-world or IA acceptance. |
| FE08 | MOCK_READY | [Integrated public-entry review](evidence/public-entry-integration.md), `e7de0f3`: 410 parent tests, build/lint/origin/boundary, four-width Chrome/reduced-height and production isolation pass. Canonical redirect and safe synthetic document presentation delivered. Approved publication/legal/contact/retention inputs and real-service/device/IN01 gates remain open. |
| FE09 | MOCK_READY | [Integrated review](evidence/session-sign-in-integration.md), candidate `a9e130e`: 354 tests including 78 FE09, strict build/lint/origin/boundary checks and four-width native Chrome/cross-tab/production-isolation checks pass. Review fixes preserve valid session on 403 while 401 clears. Production automatic restore/refresh stays disabled; real BE03 Secure-cookie/revocation/rotation, event/world and IN01 gates remain open. Delivered increment goal completed; task not accepted. |
| FE16 | IN_PROGRESS | Separate mock/native transport lane at `376cf21`, active persistent goal confirmed. Frozen C05 and FE01 are ready; owned WebSocket parity/cleanup tests are assigned. Production lifecycle, replication and real-server/IN01 qualification remain separate. |

All unlisted task statuses remain NOT_STARTED. Accepted required tasks: **11/112**;
accepted total: **11/118**. No required IA entry or IN01–IN05 integration gate has
been accepted. Real identity/provider flows, Concert product APIs, multiplayer, transactions,
privacy/revocation, physical place verification, original production assets, real
devices, load/recovery and deployment remain outstanding. Concert foundation
startup is verified within BE05; product APIs/identity are not.

## Integration and ownership

The main checkout integrates reviewed commits serially. The two user-supplied chats and two newly created
visible Codex chats now own four disjoint backend/frontend assignments in separate worktrees,
using requested `gpt-6.1-sol` / high settings. Previous subagents were stopped
before ownership transferred; their unfinished work was preserved. The
[chat ownership register](chat-coordination.md) gives exact paths and handoffs.
The integrator owns common contracts and alone pushes master.

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

The current implementation queue is BE04 canonical identity, BE15 internal
command receipts/outbox, FE08 public entry/documents and FE16 mock/native world
transport. Each chat confirmed an active persistent goal. The user explicitly
removed any fixed two-backend/three-frontend quota; the [capacity assessment](parallel-capacity.md)
keeps FE11 queued without a writer. BE03 browser/provider gates remain open.
The integrator retains shared Auth/Profile authority and C04/C06 contracts;
BE06/bootstrap follows qualified authority. No runtime, OAuth or release
acceptance follows from completed component/contract checks.
