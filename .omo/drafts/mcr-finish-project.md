---
slug: mcr-finish-project
status: approved
intent: clear
review_required: false
plan_path: .omo/plans/mcr-finish-project.md
plan_sha256: b4036c44d92779ad4b43d3753db5ae481603967c0811ac333e0f85b8c1683b83
review_round_id: null
review_round_limit: 5
pending-action: hand off to /ulw-execute
review:
  plan_reviewer:
    status: skipped_by_user
    workspace_root: null
    runtime_home: null
    target: .omo/plans/mcr-finish-project.md
    round_id: null
    plan_sha256: null
    launch_id: null
    session: null
    result: null
approach: Evidence-grounded, component-locked completion plan: seven independently verifiable components, owner decisions resolved before the brief, dependency-ordered task rows with real-surface QA, and a final verification wave that reconciles all 112 original IDs and 61 IA entries.
---

# Draft: mcr-finish-project

## Affected user and ideal state
IS-1 | A concert attendee on a 360-430px phone enters the world, sees peers move, plans a real meetup, reserves a giveaway, posts, edits the wall, and manages their profile | Every action server-authorized, recoverable after reconnect, and free of dead ends; this is the product's core promise.
IS-2 | A moderator/operator handles reports, pauses the wall, publishes notices, closes the event, and exports the archive | Audited actions, truthful degraded states, and no silent data loss.
IS-3 | A maintainer or agent clones the repository and runs one reproducible check suite; CI blocks regressions | The current session found a broken build and 376 failing tests that CI never caught.
IS-4 | A privacy/security reviewer confirms consent, blocks, TTL/expiry, idempotency, and authorization are enforced server-side across HTTP, WebSocket, and caches | Client-only hiding cannot satisfy the privacy contract.

GAP-1 | IS-1 | 40 backend tasks are IN_PROGRESS; public composition and workers are incomplete; the venue backend is absent | C2, C3
GAP-2 | IS-1 | IN02 (meetup/pickup) and IN04 (multiuser mural) real gates have not been run | C4
GAP-3 | IS-2 | QA03/QA04/QA05/QA09/QA10 are incomplete; moderation and closure paths are unqualified | C6
GAP-4 | IS-3 | CI was deleted; verified changes are uncommitted; QA01 pins were stale at HEAD | C1
GAP-5 | IS-4 | IN03 provider/claim provenance, AR01 venue provenance, and FE10 legal/consent publication are external inputs | C4, C7
GAP-6 | IS-1 (visual) | PI08 and PI11 are open; QA06 visual acceptance is incomplete; character atlas production status is unverified | C5
GAP-7 | IS-2/IS-3 (release) | IN05 is open and the 112-original / 61-IA reconciliation has not been performed | C4
GAP-8 | IS-3 | 86 salvaged refs are unmerged; some likely contain code for exactly the missing venue and backend features | C2, C3

## Components (topology ledger)
C1 | Baseline and delivery mechanics: commit the verified work atomically, keep one reproducible check command, restore blocking CI last | active | git status at 9185c9922; scripts/mcr/check.sh
C2 | Backend production completion: public routes, production workers, and packaging for the remaining backend tasks | active | docs/mcr/task-progress-2026-11-04.json (40 IN_PROGRESS); backend/services/concert-service
C3 | Venue amendment: VENU-BE-L01/M01/N01 plus FE L01/M01/N01 integration and VENU-IN01 comparisons | active | docs/mcr/venue-feature-amendment-2026-11-05.md:35-42
C4 | Integration gates: IN02, IN03 (external), IN04, and the IN05 release decision | active | register integration_gates
C5 | Art and visual acceptance: PI08, PI11, AR01 provenance, QA06, character atlas | active | docs/mcr/progress.md design/VENU status
C6 | Quality suites: QA03/QA04/QA05/QA07/QA09/QA10 plus the QA08 disposition | active | register QA rows
C7 | Operations and external inputs: deployment target, provider credentials, operators, legal/consent publications | active | infra/mcr (Dockerfile only; no compose); register WAITING_INPUT rows

## Open assumptions (announced defaults)
A1 | QA08 remains DEFERRED_BY_USER exactly as the register records | The user explicitly deferred capacity qualification; a later authorization can add it | reversible
A2 | The 86 salvaged refs are reference material, never auto-merged | Merging unverified branches would repeat the delivery problem this plan fixes | reversible
A3 | Deployment is qualified on the local/staging profile first; production target and operators are documented as external inputs | No hosting, TLS, or operator decisions exist in the repo | reversible
A4 | The current verified uncommitted work is committed and pushed as the baseline before new work | 97 modified / 9 deleted / 17 untracked files currently hold the only green frontend and verified backend state | reversible

- Register at origin/master 9185c9922: 63 DONE, 40 IN_PROGRESS, 3 WAITING_INPUT (AR01, FE10, IN03), 2 REOPENED (FE24, FE33), 3 OPEN (IN02, IN04, IN05), 1 DEFERRED (QA08) of 112 — docs/mcr/task-progress-2026-11-04.json.
- Venue amendment is 1/8 (VENU-ART01 only); VENU-BE-L01/M01/N01 require a new backend domain — docs/mcr/venue-feature-amendment-2026-11-05.md:35-42; memory fact: no Rust place-notes/invitations/consent implementation existed at the prior audit.
- Verified in this session at 9185c9922 with uncommitted fixes: frontend typecheck/build/lint 0 and vitest 411 files / 5,683 passed; backend workspace check 0; seven-service stack with gateway /readyz 200; two-client /world-ws movement PASS; QA01 PASS after re-pinning 243 inputs.
- Uncommitted baseline: 97 modified, 9 deleted (CI workflow plus old generated client), 17 untracked — git status.
- No deployment composition exists: infra/mcr contains a backend Dockerfile and a frontend-check Dockerfile only.
- Salvage: 86 refs at refs/salvage/20261106/* (pushed to origin), 25 dirty archives and 8 stash patches under ~/Desktop/mcr-salvage-20261106.
- External blockers recorded by the register: AR01 dated venue provenance, FE10 approved eligibility/legal publication, IN03 authorized claim/provider provenance.

## Decisions (with rationale)
- D1 Definition of done (user, 2026-11-06): functional delivery. Plan for all 111 active original tasks + 8 venue deliverables + IN02/IN03/IN04/IN05; QA08 stays DEFERRED_BY_USER and is never claimed as passed. Rationale: the user chose the register's functional-delivery definition over full capacity qualification.
- D2 QA08 remains deferred (consequence of D1). Capacity benchmarking is out of acceptance scope but recorded as a deferred risk for future authorization.
- D3 Salvage disposition (user, 2026-11-06): bounded triage. Before the backend/venue waves, read-only triage the 86 refs against the missing tasks; recover a branch's code only when it implements the task and passes the current checks in an isolated worktree, otherwise re-implement. No bulk merge.
- D5 Deployment target (user, 2026-11-06): local/staging plus documentation. The plan qualifies the real seven-service stack on loopback/staging and documents production inputs (mTLS material, TLS termination, providers, operators, limits) as pre-launch obligations; IN05 closes on staging evidence plus that documented checklist.
- D6 Advisory verification (ultrabrain lane, 2026-11-06): release evidence is inadmissible from the current dirty tree; W0 must produce a clean candidate. 24 of 56 registered concert command paths have no exact axum template. BE37 has 11 direct dependents; BE15 has 7. Venue backend L/M/N is net-new (DTO-only protos, zero Rust domain matches). The production shell passes `meeting: null` and omits `capabilities`/`phone`.
- D7 Baseline delivery (user, 2026-11-06): commit and push atomically before new work. Step 0 creates scoped commits for the frontend repair, backend compile fix, QA01 re-pin, and CI removal, then pushes to master; later waves build from a clean candidate.

All 112 original task IDs and 61 IA entries (111 active; QA08 per the decision); the 7 remaining venue deliverables; IN02, IN03, IN04 and IN05; art/visual acceptance (PI08, PI11, QA06, AR01 provenance); QA03/04/05/07/09/10; restoration of blocking CI (last, per the user); deployment documentation and operator/provider inputs; a bounded triage of the 86 salvaged refs.

## Scope OUT (Must NOT have)
New product features beyond the authorized venue amendment; QA08 claimed as passed while deferred; automatic merging of salvaged branches without review; schema redesign or unrelated refactors; force-push or history rewrite; client-only substitutes for server authorization; decorative left accents.

Q1 (definition of done) — does "finish" mean the register's functional delivery (111 active tasks + 8 venue deliverables + IN02-IN05, QA08 deferred), or must QA08 capacity qualification also pass? Why: the denominator and IN05 acceptance change materially.
Q2 (salvage usage) — treat the 86 refs as reference only, or run a bounded triage before planning the venue/backend work, or ignore them entirely? Why: VENU-BE-N01/M01 may already exist on paradise-city branches.
Q3 (deployment/ops) — local/staging with documented production inputs, a named production host, or defer deployment entirely? Why: IN05 requires operators and deployment limits.
Q4 (commit policy) — commit and push the current verified baseline as step 0, or leave it uncommitted? Why: 97 modified files currently hold the only green state.

## Approval gate
status: approved
approach: Wave-ordered completion. W0 clean candidate and baseline commits; W1 authority hubs (BE37, BE15, BE03 leaves) with salvage triage; W2 public route parity and real-stack composition; W3 venue backend L/M/N; W4 venue frontend integration; W5 IN02/IN03/IN04; W6 art/visual acceptance; W7 QA suites; W8 IN05 reconciliation. Each todo is gated by predecessor evidence and has agent-executed real-surface QA.
next-action: approved by the user with "Approve, skip review" on 2026-11-06; the plan-reviewer round was explicitly declined; hand off to /ulw-execute.
