# FE39 wall control and event closure

Date: 2026-11-04, Asia/Seoul. Task **BLOCKED** on the remaining retry contract;
the wall controls and closure progress/recovery have **MOCK_READY** evidence.
The persistent unbudgeted goal is **BLOCKED**, with its full scope unchanged.
Required explicit partial-failure job retry awaits the delivery-owned
C04 amendment being coordinated with BE41. Progress refresh is a read, never a
substitute for that required mutation. No scope reduction is claimed.

## Ownership and provenance

- Chat `01a1a213-61d6-7a20-a8e7-476649312f8c`, title
  `mcr(fe) FE39 wall control and event closure`.
- Isolated worktree `/Users/lhs/Desktop/mcr-worktrees/fe-wall-closure`, branch
  `codex/mcr-fe-FE39`, exact base `cd7192febf9c2764169b55d63a70d7e00d13fd08`.
  The previous FE33 checkout is stopped and untouched. No FE33 dependency import
  was needed; the current editor and shared client are read-only dependencies.
- Exclusive writes: `frontend/src/features/admin/mural/**`,
  `frontend/src/features/admin/closure/**`, and this document. No root router,
  admin registry, shared API, contract, manifest, lockfile or backend is authored
  by this patch. Delivery alone integrates and pushes to `julymeltdown/mcr/master`.
- Requirements: specification FE39, O07/O09, §§7, 8.4, 12 and 15. FE38 event/place/
  audit work stays independently owned. This patch introduces no artificial
  matching-backend or whole-task dependency barrier.
- Pinned C04 v0.1.2, REVIEW_CANDIDATE, blobs at base:

| Source | Git blob |
| --- | --- |
| `contracts/domain/v1/mural.schema.json` | `1134e67f1dcb5786ed83001f34a976a92fa3f4f0` |
| `contracts/domain/v1/admin.schema.json` | `a4fc40b18b155b959b3f32fb61c039b8a12ad054` |
| `contracts/domain/v1/operations.json` | `7ee7b48d3ec85241ad448035268c8f17afd9369f` |
| `contracts/domain/v1/policy.json` | `35e3e398d55a94d8f00c5dff3123c1f56deacb35` |
| `frontend/src/shared/api/http.ts` | `64e89f6f2aec1b864ecc0ca51a51e004418b4f2f` |

All controllers, rendering and adapters are newly authored. There is no copied
legacy UI, character art or replacement art. Tests import owning contract JSON
and the complete/false-success closure and empty-region fixtures by reference.
Closed DTO keys are checked against owning definitions; additional checks enforce
the declared nonempty rectangles and truthful closure completion policies.
`docs/source/`, original hashes, generated planning and retained dependencies are
preserved. No new dependency, storage, socket, game instance or role endpoint exists.

## Implemented behavior

`createMuralControl` and `WallControl` export route-ready controls for O07:

- Read the current authorized administrative wall state, revision, snapshot ID
  and locked half-open rectangles. Missing authority returns unavailable.
- Prepare and separately confirm PAUSE, RESUME or terminal FREEZE with a reason
  and exact current revision. Freeze explicitly warns that it cannot resume or
  restore. A prepared action sends nothing; cancel sends nothing.
- Lock/unlock a nonempty half-open region within 128×128 coordinates. State and
  revision come from the server response; client controls are not authorization.
- Restore only while PAUSED. The confirmation names the complete selected
  snapshot UUID and exact expected wall revision, states that pixels are copied
  into a newer revision, and preserves PAUSED. There is no invented snapshot
  history/list endpoint: the current snapshot is prefilled, and a reviewed
  snapshot ID can be supplied; the server verifies its ownership/content.
- Accepted changes advance exactly one revision; reads cannot rewind or change
  equal-revision data. FROZEN remains terminal within this authorized lifetime.
  Production delivery must feed server reset invalidations to incumbent FE33.

`createEventClosure` and `EventClosure` export O09 behavior:

- A fresh OPEN/LIVE event context and nonempty reason prepare the exact event
  revision and `CLOSE_EVENT` confirmation. Explicit confirmation issues one
  `closeConcertEvent` with a new immutable idempotency key.
- Display a server-accepted job as accepted work, not completed cleanup. Show
  all seven required named steps and PENDING/SUCCEEDED/FAILED states; partial
  failure is visible. COMPLETED is rejected unless every unique required step
  succeeded. Job ID/event/revision are bound and completed steps cannot regress.
- Explicitly refresh an authorized job using `getConcertClosure`; a known job ID
  can be reopened after panel navigation/reload without creating another job.
  Progress refresh never dispatches or retries a mutation.
- Mutation retry is presently unavailable rather than invented. BE41's proposed
  existing-route START/RESUME request union must be pinned by Delivery before
  adoption. A RESUME would be a separately confirmed command with a fresh key,
  exact job/revision and renewed operator/initiator authorization; original
  command replay would still only recover acceptance. This remains required work.

Both features retain one immutable command while sending, pending or unknown.
Duplicate clicks, refresh, remount of a panel and original-status checks never
issue another POST. `getConcertCommandStatus` binds actor/event/operation/key,
unexpired receipt, original success status and expected resource kind. Wall
receipts bind the mural event aggregate and next revision; closure receipts bind
the job. SUCCEEDED metadata is followed by the authorized resource reread and
cannot fabricate an outcome or current state. Pending/retired/expired/missing/
malformed results retain uncertainty. Idempotency conflict/expiry is unknown,
not evidence that an earlier destructive command failed. Verified definitive
4xx rejection clears the preview; wall changes require reread and closure needs
a newer qualified event revision before another attempt. No raw server text is
displayed. HTTP 429 delays subsequent explicit requests without scheduling retry.

## Authority and composition

Delivery supplies the existing canonical HttpClient/PrivacyPort plus an injected
`OperatorContextPort` (`read`, `subscribe`). Its structural value is
`{scope: PrivacyScope, role: 'OPERATOR', roleRevision, eventRevision, phase}`.
The context owner must return null when its fresh server-qualified event role is
missing, expired, revoked or unavailable, and emit on lease/phase/revision change.
It must not infer OPERATOR from hidden UI, a client preference, an Auth/global
role or an unrelated capability. FE38 was sent this exact seam for root mapping;
no imports from its uncommitted code are required.

`qualifiedContext` checks the exact current canonical actor/event/session/policy
scope and safe revisions. Every response verifies the captured context, path,
HTTP status, actor/event/generation and closed payload before publication.
Role/identity/policy changes abort and erase private resource, draft, job and
command state. Event revision/phase changes invalidate drafts/resources and late
responses, while retaining only the original ambiguous command metadata for
read-only resolution under the renewed context. No action is silently replayed.
Private state is in memory only; no local/session storage, persisted query data,
analytics payload or production debug global is added.

`Recheck operator access` reads only the injected fresh context and reopens
eligible controls after the role owner publishes renewed authority. It creates
no role endpoint or role request. An existing job can then be reloaded by its
exact ID; missing authority remains visibly unavailable. Throttling honors the
full server delay up to the shared 24-hour bound, is scoped to canonical privacy
identity, and is erased on privacy change.

Construct `createAdminTransport` once with the canonical client, privacy and
qualified context; pass it to `createMuralAdminApi`/`createClosureApi`, then the
matching controllers. Call `start` after admission, retain controllers through
panel navigation, and `dispose` at shell/context ownership end. Recreate after
privacy change rather than reviving an old scope. Delivery alone mounts the
components at `/admin/mural` and `/admin/archive` and owns all root registration.
No route is installed by this patch. Server fresh authorization, audit receipts,
atomic revisions and actual closure work remain owning-service responsibilities.
Dispose the shared feature transport after both controllers release it; this
unregisters its private throttle cleanup.

## Verification

Commands ran in the assigned worktree with Node 24.22.2 at
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (`NODE` below).
The authorized nontracked `frontend/node_modules` symlink uses Delivery's
qualified installation; no install or dependency mutation was performed.

| Command | Exit | Evidence |
| --- | --- | --- |
| `NODE frontend/node_modules/vitest/vitest.mjs run frontend/src/features/admin/mural/__tests__ frontend/src/features/admin/closure/__tests__` | 0 | 60 tests / 4 files: contract fixtures/parity, actual canonical HTTP, explicit commands, pending/unknown/no replay, forged receipts/results, exact snapshot, locks, stale revisions, partial closure, server denial, role/privacy eviction/renewal, delayed response, timeout and identity-scoped full-delay throttling. |
| `NODE frontend/node_modules/typescript/bin/tsc --project frontend/tsconfig.json --pretty false --incremental false` | 0 | Whole frontend strict types. |
| `NODE frontend/node_modules/oxlint/bin/oxlint frontend/src/features/admin/mural frontend/src/features/admin/closure` | 0 | Scoped lint, zero warnings/errors. |
| `NODE frontend/src/features/admin/mural/__tests__/browser-smoke.mjs` | 0 | Actual Chrome 156.0.8078.80, 360/390/430×844 touch/mobile/reduced-motion emulation. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/archive provenance PASS; application acceptance is separate. |
| `python3 scripts/mcr/validate-domain.py` | 0 | 164 schemas / 108 operations / 41 fixtures; REVIEW_CANDIDATE, runtime NOT_IMPLEMENTED, freeze NOT_ACCEPTED. |

Browser report and nine PNGs are under
`frontend/src/features/admin/mural/__tests__/evidence/browser/`. Source hashes
bind feature code, test HTTP fixture, browser harness and C04 inputs to the base.
The 390px restore confirmation was visually inspected. Native DOM checks cover
labeled fields/status, keyboard confirmation and focus on the review region,
44px minimum controls, no horizontal overflow, exact snapshot/revision text,
pause/resume/freeze, lost-wall-command recovery without replay, visible failed
closure step, progress reread and cross-panel privacy eviction. The fixture
global exists only in the test entry point. These are qualified frontend fixture
checks; they do not certify backend moderation or IN04.

Committed increment ancestry starts at `75856239d273e14a21a17f4c6248af35fe7e7007`
(parent `cd7192f`). A following correction qualifies renewed operator access and
full-delay private throttling; its browser report records parent `7585623` and
the exact tested source hashes. Delivery has the initial committed handoff;
the required resume amendment remains a separate unfinished increment.

## Remaining dependency and blocked audit

Three consecutive goal turns encountered the same external dependency. Fresh
repository inspection at clean `680915dc382927591084d881f75fc94966ae43db` confirms
that `CloseEvent` is still closed to `{expected_revision, confirmation:
CLOSE_EVENT, reason}`. Its policy permits initiation only in OPEN/LIVE; a
partial closure has already made the event READ_ONLY. The status-recovery policy
explicitly says that status alone does not authorize manual replay. No committed
START/RESUME amendment exists in the inspected local contract history.

BE41 has proposed an explicit resume shape on the existing POST route and keeps
its HTTP schema gate closed pending Delivery's pin. Delivery owns the shared
contract/schema/registry changes; this worker is not authorized to invent that
wire capability or modify shared paths. All independent assigned work is
committed in `7585623` and `680915d`; the temporary dependency symlink has been
removed and the checkout is clean. No extra approval, live-backend or whole-task
barrier is being imposed. A committed, coordinated resume amendment is the
external change needed to resume this assignment.

Remaining required work: adopt the committed explicit closure-resume contract,
add meaningful failed-step retry/ambiguous-resume fixtures and browser checks,
update this record, commit final handoff and resolve concrete findings. The goal
stays unfinished while blocked. Production root composition, real multiuser moderation/restore,
durable backend closure, IN04/QA09, native devices, assistive hardware and load
remain **NOT_RUN**, separate from task-local fixture criteria.
