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
| C04 | IN_PROGRESS | Eight untested domain-schema drafts preserved in the stopped contract worker's isolated worktree; the integrator owns continuation. No frozen domain contract or runtime implementation claimed. |
| C05 | IN_PROGRESS | Integrator authoring realtime frames, ticket/admission/stream rules and negative fixtures in disjoint contract paths. |
| C06 | CONTRACT_READY | Five asset schemas, six golden JSON fixtures and 21 checks pass; all three themes share geometry and avatar identities. Pixel alignment/art/venue truth remain NOT_RUN. |
| AR03 | IN_PROGRESS | [User-selected character source pack](character-art.md), revision 2: ten unique selected JPEGs including the new accessory sheet; the first batch's duplicate is archived. Source catalog and frontend lookup use the latest selection. Previous character art is prohibited. Transparent cutouts, layer-contract revision and direction/walk atlas remain. |
| C08 | ACCEPTED | [Origin gate](qa/ui-origin.md), empty exact allowlist, 14 negative/positive test cases and current source scan pass. Modified copies and visual similarity require manual review; QA02 is not accepted. |
| BE01 | IN_PROGRESS | 1,071 pinned files imported in `909e2bf`, full build passes. Explicit MCR runtime configuration and safe launch qualification remain. |
| BE02 | IN_PROGRESS | [Isolated DB report](evidence/be02-migration-smoke.json): 84 migrations in six stores, ledger checksums and six records survive PG restart; cleanup PASS. 24 guard tests pass. Normal service startup/restart, runtime roles and packaging remain NOT_RUN. |
| FE05 | ACCEPTED | [Integrated evidence](evidence/ui-primitives.json) and [independent review](qa/ui-primitives-review.md): 22 DOM tests, strict build/lint, real Chrome keyboard/focus/dismissal and 44px targets at four viewports. [Style correction](evidence/ui-style-correction.json) removes prohibited left accents and offset shadows; build/lint and all four Chrome viewports pass again. Physical devices and screen readers remain separate QA work. |
| FE01 | IN_PROGRESS | Frontend-only transport worker implementing against frozen C03; strict same-origin paths, memory/session ports, safe 401 replay and cancellation/error cases. |

All unlisted task statuses remain NOT_STARTED. Accepted required tasks: **5/112**;
accepted total: **5/118**. No required IA entry or IN01–IN05 integration gate has
been accepted. Real identity/provider flows, service startup, multiplayer,
transactions, privacy/revocation, physical place verification, original production
assets, real devices, load/recovery and deployment remain outstanding.

## Integration and ownership

The main checkout integrates reviewed commits serially. The two user-supplied
visible Codex chats now own backend and frontend work in separate worktrees,
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

The next implementation queue is guarded MCR runtime configuration/full service
startup for BE02, BE03 auth hardening against the frozen C03 email/core surface,
then typed frontend transport and the persistent shell. C04/C05 domain/realtime
contracts remain required before feature lanes diverge. No runtime, OAuth or
release acceptance follows from the completed component/contract checks.
