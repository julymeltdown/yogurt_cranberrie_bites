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
| C04 | IN_PROGRESS | [Domain review candidate](domain-contract.md), [target/query follow-up](evidence/domain-targets.md), [combined assembly](evidence/domain-assembly.md) and [internal keyed fingerprint correction](evidence/receipt-fingerprint-review.md) `9044417`: 162 definitions, 106 operations, 25 closed query schemas, 75 semantic rules, 15 inherited mappings, 36 payload fixtures, 47 domain and 15 assembly tests. Final/core pipeline emits 132 HTTP operations without changing frozen C03. Full context coverage, deletion/legal/retention, client qualification and independent reviews remain; not frozen or runtime-qualified. |
| C05 | ACCEPTED | [Frozen realtime v1.0.0](realtime-contract.md): strict frame/HTTP schemas and admission/stream rules, 24 fixtures and 31 regressions. [BE/FE rechecks](qa/realtime-contract-review.md) find no remaining scoped issues after corrections. Runtime ticket, socket, queue, capacity and race tests are NOT_RUN. |
| C06 | CONTRACT_READY | Five asset schemas, six golden JSON fixtures and 21 checks pass; all three themes share geometry and avatar identities. Pixel alignment/art/venue truth remain NOT_RUN. |
| AR03 | IN_PROGRESS | [User-selected character source pack](character-art.md), revision 3: twenty unique JPEGs (ten base sheets plus ten additions); the first batch's duplicate stays archived. Source catalog and frontend lookup preserve both selected batches. Previous character art is prohibited. Transparent cutouts, layer-contract revision and direction/walk atlas remain. |
| C08 | ACCEPTED | [Origin gate](qa/ui-origin.md), empty exact allowlist, 14 negative/positive test cases and current source scan pass. Modified copies and visual similarity require manual review; QA02 is not accepted. |
| BE01 | ACCEPTED | [Integrated foundation review](evidence/core-foundation-integration.md): 1,071 pinned files imported, full workspace builds, six separate services and exact owned process/port/database map exercised without source-default DB targets. |
| BE02 | ACCEPTED | [Integrated tooling review](evidence/privacy-local-stack-integration.md): native migration/restart persistence, owned foreground stack and private mail, 55 helper and 16 development tests, reviewed image/protobuf/SQL paths. Docker execution and production roles remain NOT_RUN and must be qualified before packaging/BE44. |
| BE03 | INTEGRATED | [Email ingress review](evidence/browser-auth-integration.md), candidate `689e4d2`: parent reran 221 Gateway tests, nine harness tests and 53 real native HTTP checks with three mail deliveries and cleanup PASS. Frozen email/account mapping, browser guards and invalid-credential handling delivered. Real browser cookie policy, configured OAuth and complete BE03 acceptance remain open. |
| BE04 | IN_PROGRESS | [Identity foundation](evidence/identity-integration.md) and [real Auth-owner read](evidence/auth-identity-owner-integration.md), `c300a23`: parent verified 8 value/23 retained application/89 helper/22 real PostgreSQL tests, all 13 SQLx migrations, restart and cleanup. Bounded Auth-owner goal COMPLETE; original identity goal remains BLOCKED. Authenticated RPC/Gateway/Concert composition and event policy remain; production authority stays unavailable. |
| BE05 | ACCEPTED | [Integrated foundation review](evidence/concert-core-integration.md), source/workspace/doc `6a08701`/`f5b188b`/`376cf21`: typed hexagonal values/clock/policy ports, constrained event/participant/avatar schema, 12 Rust + 12 Python tests and three real PostgreSQL tests. Parent reran owned migration/reapply/schema-damage refusal, health/outage/restart/graceful cleanup. No product API or invented authority; BE04/bootstrap/production remain separate. |
| BE15 | IN_PROGRESS | [Reviewed internal receipt/outbox foundation](evidence/command-receipts-integration.md), `7cbb97e`/`c2a2c30`: 33 combined deterministic Rust, 82 Python helper and 17 real PostgreSQL tests pass; owned migration failure/recovery, restart persistence and cleanup pass. Review fixes cover keyed private fingerprints, post-authorization expiry and result/outbox revision equality. Bounded goal COMPLETE; production producer/Auth/public API/key configuration/runtime/load gates remain NOT_RUN. |
| FE05 | ACCEPTED | [Integrated evidence](evidence/ui-primitives.json) and [independent review](qa/ui-primitives-review.md): 22 DOM tests, strict build/lint, real Chrome keyboard/focus/dismissal and 44px targets at four viewports. [Style correction](evidence/ui-style-correction.json) removes prohibited left accents and offset shadows; build/lint and all four Chrome viewports pass again. Physical devices and screen readers remain separate QA work. |
| FE01 | ACCEPTED | [Integrated review](evidence/core-foundation-integration.md): 144 transport/error tests plus 22 existing UI tests, strict build/lint; memory-only session, bounded read refresh, cancellation, safe error/upload ports and stale-identity isolation. Real cookies and auth journeys remain BE03/FE09 work. |
| FE02 | ACCEPTED | [Integrated privacy review](evidence/privacy-local-stack-integration.md): actual QueryClient/observers, canonical private keys, cancellation plus eviction, commit fencing and cleanup ports; 45 new tests and combined 189-test transport/privacy run pass. UI/event/real-server owner wiring remains. |
| FE07 | MOCK_READY | [Integrated shell review](evidence/world-shell-integration.md), candidate `d7909c0`: 276 tests, frozen build/lint/origin scan, four installed-Chrome viewports and production-entry isolation pass. Review fixes block obsolete initial loader dispatch and bound scroll history. Actual Phaser/socket/camera preservation and device qualification remain; no real-world or IA acceptance. |
| FE08 | MOCK_READY | [Integrated public-entry review](evidence/public-entry-integration.md), `e7de0f3`: 410 parent tests, build/lint/origin/boundary, four-width Chrome/reduced-height and production isolation pass. Canonical redirect and safe synthetic document presentation delivered. Approved publication/legal/contact/retention inputs and real-service/device/IN01 gates remain open. |
| FE09 | MOCK_READY | [Integrated review](evidence/session-sign-in-integration.md), candidate `a9e130e`: 354 tests including 78 FE09, strict build/lint/origin/boundary checks and four-width native Chrome/cross-tab/production-isolation checks pass. Review fixes preserve valid session on 403 while 401 clears. Production automatic restore/refresh stays disabled; real BE03 Secure-cookie/revocation/rotation, event/world and IN01 gates remain open. Delivered increment goal completed; task not accepted. |
| FE10 | MOCK_READY | [Integrated review](evidence/signup-verification-integration.md), `f825b31`/`f498194`/`7bfea96`: 737 combined frontend tests, build/lint/C03/C08/boundary/plan and four-width enrollment/sign-in/recovery browser suites pass. Three concrete review fixes delivered; bounded goal COMPLETE. Anonymous verification is event-independent; production signup/eligibility/consent/live mail/device/vertical gates remain unqualified. |
| FE11 | MOCK_READY | [Integrated recovery review](evidence/password-recovery-integration.md), `d290391`/`4360d58`/`4550029`: 669 combined tests, strict build/lint/C03/origin/boundary/plan and four-width native recovery plus retained sign-in suites pass. Source fragment intake, canonical confirmation fencing and safe callback presentation delivered; bounded goal COMPLETE. Real mail/BASE_URL/Secure cookies/OAuth/device/IN01 remain NOT_RUN. |
| FE14 | IN_PROGRESS | New tool-confirmed ACTIVE bounded goal on isolated `fe-runtime`, base `244d35a`: Phaser lifecycle, strict existing map/theme/resource consumption and synthetic native-browser fixture assigned. No production shell, avatar, real map/venue or whole-world acceptance. |
| FE16 | MOCK_READY | [Reviewed transport integration](evidence/world-transport-integration.md), `e3a4d84`/`5863f91`: 568 combined frontend tests (158 transport), build/lint/type-drift/origin/plan and installed-Chrome five canonical traces pass. Gap recovery and pre-send room-switch failure findings corrected. Bounded goal COMPLETE; production lifecycle, replication and real-server/IN01 remain NOT_RUN. |

All unlisted task statuses remain NOT_STARTED. Accepted required tasks: **11/112**;
accepted total: **11/118**. No required IA entry or IN01–IN05 integration gate has
been accepted. Real identity/provider flows, Concert product APIs, multiplayer, transactions,
privacy/revocation, physical place verification, original production assets, real
devices, load/recovery and deployment remain outstanding. Concert foundation
startup is verified within BE05; product APIs/identity are not.

## Integration and ownership

The main checkout integrates reviewed commits serially. Two user-supplied chats and two newly created
visible Codex chats use disjoint backend/frontend worktrees and requested
`gpt-6.1-sol` / high settings. Chat count does not imply four active writers:
the authenticated Auth RPC has a clean committed handoff under parent review.
FE10 and the independent event/RPC-prerequisite QA review have tool-confirmed
COMPLETE bounded goals and stopped checkouts. The original BE04 chat is blocked
on shared contracts. Completed chats are reused only for useful, disjoint work.
Previous subagents were stopped
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

FE11, the Auth-owned read and FE10 finished bounded review with COMPLETE goals
and stopped clean checkouts. The authenticated Auth RPC handoff `9bf3233` remains
under parent integration review with its goal ACTIVE. Independent QA review
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


FE14 runtime/resource work and actual-handler QA now have new tool-confirmed
ACTIVE goals on fresh frontend and QA worktrees. Auth RPC R1 was a test setup
race; its test-only fix is integrated for final parent qualification. The initial
combined RPC candidate passes five real RPC and 22 retained PostgreSQL cases;
final corrected-source evidence and independent review disposition remain open.
The parent also corrected the BE15 harness's stale candidate pins after `5c02a31`;
all 98 helper tests, 138 contract regressions and planning checks pass at `d197379`.
No production moderation, identity admission or release gate is thereby enabled.
