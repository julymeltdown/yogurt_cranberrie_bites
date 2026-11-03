# FE39 / O07 / O09 explicit closure resume handoff

The remaining FE39 closure-resume increment is implemented and validated against
the published START/RESUME amendment. Commit
`e78d9dd3f49133b1bb266556ecbd36bce124c9b6` contains the closure implementation,
deterministic authenticated HTTP tests, native-browser harness and captured
evidence. This completes the previously blocked task-local frontend increment;
live closure and release acceptance remain separate.

## Assignment, ancestry and provenance

- Delivery repository: `julymeltdown/mcr`, target `master`; only Delivery pushes.
- Assigned checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-closure-resume`, branch
  `codex/mcr-fe-closure-resume`, base
  `911f15665379425afeb52c1401626f39f5298739`.
- Exact implementation ownership: `frontend/src/features/admin/closure/**`, new
  tests under `closure/__tests__/resume/**`, and this document. The integrator
  separately granted exactly `frontend/src/features/admin/mural/__tests__/contract.test.ts`.
- Compatibility commit `45d44fce8c2a9ad346adc01e4d73c1717ddbac90`, parent
  `911f156`, binds the retained START DTO assertions to
  `admin.$defs.CloseEvent.oneOf[0]!`, preserving closed-key assertions. The
  integrator reports this separately integrated as `3f7be97` in published
  `707ea68d585324430144c971724efbf5c73167d6`; this worker did not push.
- Implementation commit `e78d9dd` has parent `45d44fc`. Integrate only the new
  implementation and documentation after the already integrated compatibility
  change. Do not replay the earlier FE39 commits already present in the base.
- The earlier checkout `fe-wall-closure` is read-only. Its initial implementation,
  fixes and historical blocked report are already delivered. This document
  supersedes only its outstanding closure-resume dependency and remaining-work
  statements; it does not rewrite historical evidence.
- The aligned persistent goal was actually `BLOCKED` at recovery. The available
  goal API has no resume transition, so the explicit coordinator instruction
  authorized manual continuation with the new checkout and narrower ownership.
  No duplicate goal or model/reasoning override was introduced. Final completion
  is assessed against the delivered frontend scope, without adding a backend gate.
- No source archive, contract registry, generated API metadata, lockfile,
  manifest, root composition, artwork, backend or infrastructure edit is part of
  this handoff. No Bigyeting rendering is copied. New UI and tests are authored
  here; the existing audited frontend owner/transport/privacy modules are reused
  without modification. Imports use frontend APIs and public contract fixtures.

The governing source revision is `911f156`: read
`docs/mcr/contracts/event-closure-resume.md`, the closed union in
`contracts/domain/v1/admin.schema.json`, operation mode metadata in
`operations.json`, conditional policy in `policy.json`, the six published
START/RESUME fixtures and generated frontend operation metadata together.
`closure/__tests__/resume/evidence/browser/report.json` pins SHA-256 values for
the tested feature, owner, transport, canonical client/privacy, fixture, browser
harness and contract inputs. The report records parent `45d44fc`; its hashes
identify the exact implementation committed in `e78d9dd`.

## Behavior and integration seam

The exported component/controller/API construction and lifecycle are unchanged.
The shell supplies fresh qualified `OperatorContextPort`, canonical HttpClient
and QueryPrivacy, retains one feature controller through panel navigation,
calls `start`, and disposes controller/transport at ownership end. Existing
`closeEventSchema` remains the START-only export. `closureCommandSchema` is the
closed discriminated union and `ClosureDraft` exposes both modes. No new role,
force, retry or status endpoint is invented.

START uses the current event revision in OPEN/LIVE. Explicit resume is prepared
only with current operator authority, READ_ONLY event, exact loaded
PARTIAL_FAILURE job and its safe current job revision. The review names the
job ID, expected job revision and bounded nonblank reason. Cancel sends no POST.
Keyboard confirmation admits one immutable command with a fresh key. The
controller rejects reuse of any key issued during its current private lifetime.
The same existing POST carries
`{job_id, expected_revision, confirmation: "RESUME_CLOSURE", reason}`.

The job DTO intentionally has no initiating-actor field. The client does not
infer initiating ownership from a global role or manufacture an initiator
endpoint. Fresh server Auth, event policy, current operator authorization and
the durable original initiating actor must authorize every job read/resume.
Client controls do not replace those checks. Fixture denial cases exercise this
boundary, without certifying a live server implementation.

Status 200 means admission and a validated current job, not successful cleanup.
The resume response must be the exact event/job, advance the expected job
revision, and preserve previously SUCCEEDED steps. The renderer shows all seven
steps and distinguishes RUNNING, PARTIAL_FAILURE and COMPLETED. Progress reads
do not start or resume a job, and admission alone never labels cleanup complete.

Lost responses, timeout, 5xx and 202 retain the exact original command. A job GET
cannot clear that ambiguity or enable another resume. Explicit status recovery
checks operation/key/actor/event, receipt retention, success status 200, exact
job resource and an advancing accepted revision, then requires an authorized
job read at least as new as the receipt. Forged, retired, expired, missing or
unavailable receipts remain unresolved. Neither GET nor background time admits
new work. Same-key identical-body replay recovers a previous admission rather
than retrying a later failed boundary. A subsequent failure needs another fresh
read, review and key.

A definite server rejection withdraws displayed job data and requires a fresh
authorized read before a new explicit review. Actor/event/policy/role changes
abort and erase job, draft, reason and private command state; late results cannot
repopulate it. Same-owner event revision/phase changes invalidate resources and
drafts while preserving an ambiguous command for read-only resolution, including
after archival. There is no persisted token, private job cache or production
debug global. Test hooks are confined to the test HTML/TSX entry point.

## Validation and evidence

Commands ran in this isolated checkout using retained dependencies and Node
`24.22.2` at `/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`
(`NODE` below). The temporary nontracked `frontend/node_modules` symlink used
the existing installation and was removed before final handoff. No installation
or manifest/lockfile mutation occurred.

| Command | Exit | Result |
| --- | --- | --- |
| `NODE frontend/node_modules/vitest/vitest.mjs run frontend/src/features/admin/mural/__tests__ frontend/src/features/admin/closure/__tests__` | 0 | 118 tests / 7 files. Closed START/RESUME parity and six pinned fixtures; actual canonical authenticated HTTP; exact revision subjects; fresh-key intent; no ambiguous replay; pending/timeout/5xx; forged receipts/results; initiating actor/role/policy denial; monotonic steps; stale CAS; privacy/late publication; full server throttle delay. |
| `NODE frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false` | 0 | Whole frontend strict types, including the new browser fixture and separately authorized compatibility test. |
| `NODE frontend/node_modules/oxlint/bin/oxlint frontend/src/features/admin/closure frontend/src/features/admin/mural/__tests__/contract.test.ts` | 0 | Scoped lint. |
| `NODE frontend/src/features/admin/closure/__tests__/resume/browser-smoke.mjs` | 0 | Actual Chrome `156.0.8078.80`, touch/mobile/reduced-motion emulation at 360/390/430 × 844. |
| `NODE frontend/src/features/admin/mural/__tests__/browser-smoke.mjs` | 0 | Retained wall/initial-closure regression at all three widths. Its historical assertion still checks an ineligible resume under its LIVE fixture context; it does not certify explicit resume. Generated old-path outputs were restored; the new resume report is the retained evidence. |
| `NODE node_modules/vite/bin/vite.js build` (cwd `frontend`) | 0 | Production bundle succeeds; existing large-chunk warning remains. This is bundling evidence, not root-flow acceptance. |
| `python3 scripts/mcr/check-plan.py` | 0 | Planning/provenance PASS; original archive checksums match. |
| `python3 scripts/mcr/validate-domain.py` | 0 | 164 schemas / 108 operations / 47 fixtures; REVIEW_CANDIDATE, runtime NOT_IMPLEMENTED, freeze NOT_ACCEPTED. |
| `git diff --check` | 0 | No whitespace errors. |

The new browser harness checks exact review identity/reason and focus, cancel,
keyboard confirmation, one-admission progress, preserved completed steps,
read-only completion observation, lost-response recovery through status plus
current job GET, a later explicitly confirmed boundary, pending202, stale409,
fresh re-review, role revocation and logout. Every width checks controls at least
44px, no horizontal overflow and no page errors. Twelve PNGs and a source-bound
report are committed under `closure/__tests__/resume/evidence/browser/`.
The 390px resume-confirmation and unknown-command images were visually inspected.
Native validation found a textarea label instability after cancel/remount;
`useId` and explicit `aria-labelledby` now preserve its accessible name. The
passing cancel/review-again browser flow verifies that correction.

## Acceptance boundaries

| Boundary | Evidence state |
| --- | --- |
| Published bounded START/RESUME contract | CONTRACT_READY at assigned base; whole C04 freeze is not implied. |
| FE39 task-local explicit resume behavior | Implemented, committed, fixture-validated; ready for Delivery integration. |
| Native browser/deterministic HTTP flows | MOCK_READY with actual Chrome and canonical frontend transport. |
| Live original-actor server enforcement, database CAS/concurrency and real owner effects | NOT_RUN by this frontend handoff. Fixture effect counters are not live idempotency evidence. |
| Production root/realtime multiuser flow, IN04, QA09, physical device/assistive hardware, load and release | NOT_RUN by this increment; remain owning integration/QA obligations. |

No scope cut or substitute acceptance criterion is introduced. The closure
amendment removed the prior external blocker; completing this frontend increment
does not claim overall task registry, IA, C04 or IN05 release acceptance.
