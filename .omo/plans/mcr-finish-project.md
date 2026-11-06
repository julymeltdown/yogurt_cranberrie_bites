# mcr-finish-project - Work Plan

## TL;DR (For humans)
<!-- Fill this LAST, after the detailed plan below is written, so it summarizes the REAL plan. -->
<!-- Plain English for a non-engineer: NO file paths, NO todo numbers, NO wave/agent/tool names. -->

**Who this is for and what changes for them:** Concert attendees get a working mobile world where they can actually meet, trade gifts and paint the wall against real services; operators get audited moderation and closure; maintainers get a reproducible build with CI that cannot silently go red.

**What you'll get:** Every remaining original feature and the eight venue deliverables completed, the real multiuser gates (meetup/pickup, social/privacy, mural) passed, visual acceptance captured, and a release decision backed by one clean commit.

**Why this approach:** The current tree holds verified but uncommitted work, so release evidence is inadmissible until it is cleaned up; and the backend authority hubs (relationship policy, receipts) must land before their many dependents or every later gate re-tests a moving target.

**What it will NOT do:** It will not claim 200-user capacity (explicitly deferred), will not bulk-merge the salvaged branches, and will not add features beyond the authorized venue amendment.

**Effort:** XL
**Risk:** High - missing venue backend, incomplete public composition, and external human inputs (venue provenance, legal text, provider claims).
**Decisions to sanity-check:** QA08 stays deferred; salvage branches are triaged, never bulk-merged; deployment is staged locally with production documented; the current verified work is committed and pushed before new work.

Your next move: run `/ulw-execute` on this plan. Full execution detail follows below.

---

> TL;DR (machine): XL, high risk; clean candidate + authority hubs + route parity + venue L/M/N + IN02/03/04 + visual/QA + IN05 release, QA08 deferred.

## Scope
### Affected user and ideal state
<!-- From the draft ledger: who this touches and how they use the result, then one IS row per property with its reason, then one GAP row per difference with its reason. Every todo below closes a GAP row; ## Success criteria proves every IS row. -->
**Affected user:** A concert attendee on a 360–430px phone who enters the world, sees peers move, plans a real meetup, reserves a giveaway, posts, edits the wall and manages a profile; a moderator/operator who handles reports and closes the event; a maintainer/agent who needs one reproducible check suite and a clean candidate; and a privacy reviewer who needs server-enforced consent, blocks, TTL and authorization.

| Row | Statement | Reason |
| --- | --- | --- |
| IS-1 | An attendee completes world presence, meetup, giveaway, feed, wall and profile flows on a phone with no dead ends | This is the product's core promise and the IN-series gates' subject |
| IS-2 | An operator handles reports, wall control, notices and event closure with audited, truthful states | Operations and archive are required surfaces (O01–O09, X01) |
| IS-3 | A maintainer clones the repo and runs one check suite; CI blocks regressions; deployment is documented | The current session found a broken build and 376 failing tests CI never caught |
| IS-4 | Consent, blocks, expiry, idempotency and authorization are enforced server-side on HTTP, WS and caches | Client-only hiding cannot satisfy the privacy contract |
| GAP-1 | 40 backend tasks are IN_PROGRESS; public composition and workers incomplete | Attendee flows cannot run against real services |
| GAP-2 | IN02 and IN04 real gates not run | Multiuser meetup/pickup and mural are unproven |
| GAP-3 | QA03/04/05/09/10 incomplete | Operator and privacy behavior is unqualified |
| GAP-4 | CI deleted; verified work uncommitted; QA01 pins were stale | Release evidence is inadmissible from a dirty tree |
| GAP-5 | IN03, AR01 and FE10 await external human inputs | Those gates cannot close without them |
| GAP-6 | PI08/PI11 open; QA06 incomplete; character atlas status unverified | Visual acceptance is a release requirement |
| GAP-7 | IN05 open; 112/61 reconciliation pending | There is no release decision |
| GAP-8 | 86 salvaged refs unmerged; some likely contain missing venue/backend code | Recovered work could shorten W2/W3 if verified |

### Must have
- A clean candidate on `master` before any release evidence; the verified frontend/backend work committed and pushed atomically (W0).
- Real public composition: every registered `/api/concert/events/{eventId}/...` command reachable through the gateway, with the route-parity test over `contracts/domain/v1/operations.json` (W2).
- BE37 relationship policy and BE15 outbox/receipts adopted as authority hubs before their 11 and 7 dependents (W1).
- Venue backend L01/M01/N01 implemented with server-side consent, visibility, TTL/revocation and event timezone/endday semantics, then integrated (W3/W4).
- IN02, IN03 (where inputs exist), IN04 passed on real two-client/cohort evidence; IN05 reconciled against 111/111 active tasks, 61/61 IA, 8/8 venue deliverables and 10/10 extension IA, with QA08 explicitly deferred (W5/W8).
- Art/visual acceptance: PI08, PI11, AR01 inputs, QA06 at 360/390/430 plus devices (W6).
- Quality suites QA03/04/05/09/10 passing on the same candidate (W7).
- CI restored last: plan/guards, frontend install/typecheck/build/lint/vitest, cargo check/test, generated-state drift, and release checks (W8).

### Must NOT have (guardrails, anti-slop, scope boundaries)
- No new product features beyond the authorized venue amendment.
- No QA08 pass claim; it stays `DEFERRED_BY_USER` and is never marked complete.
- No automatic merge of salvaged branches; recovery only through an isolated worktree and current checks.
- No schema redesign or unrelated refactor; no force-push; no decorative left accents.
- No client-only substitute for server authorization; no mock evidence for IN02/IN03/IN04.
- No release claim while `git status` is dirty.

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: TDD for new venue backend aggregates/routes and the route-parity test; tests-after for wiring existing producers. Frameworks: Rust `cargo test`, Vitest, Python contract validators, curl/psql, Playwright real-stack specs.
- Evidence: `.omo/evidence/mcr-finish-project/task-<n>-<slug>.md` (n is the todo number) for every todo, containing the exact command, exit code, and the decisive output/capture.
- Integration gates use only real services, real PostgreSQL and two real accounts; no mocks, no fixture-only claims.

## Execution strategy
### Parallel execution waves
> Target 5–8 todos per wave; waves are admission gates, not full barriers.

| Wave | Focus | Todos |
| --- | --- | --- |
| W0 | Clean candidate, baseline commits, salvage triage | 1–6 |
| W1 | Authority hubs: BE37, BE15, BE03, admission/rooms | 7–11 |
| W2 | Public route parity, real stack, BE42/BE38/BE41/BE43, feed mutations | 12–17, 49 |
| W3 | Venue backend policy + L/M/N | 18–23 |
| W4 | Venue frontend integration + VENU-IN01 | 24–28 |
| W5 | IN02 / IN03 / IN04 / IN01 deltas | 29–32 |
| W6 | Art and visual acceptance | 33–37 |
| W7 | QA03/04/05/09/10 | 38–42 |
| W8 | FE closure, deployment docs, CI restoration, IN05 | 43–48 |

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1–3 baseline commits | — | 4, 48 | 6 |
| 4 clean-candidate verification | 1–3 | all later | — |
| 5 salvage triage | 4 | 18–23 | 7–11 |
| 6 CI inventory (write-only, not restored) | 4 | 48 | 7 |
| 7 BE37 | 4 | 11, 18–23, 30, 32 | 8, 10, 11 |
| 8 BE15 | 4 | 12, 13, 38 | 7, 10, 11 |
| 9 BE03 leaves | 4 | 30, 45 | 7, 8 |
| 10 BE10/BE11 | 4 | 29, 32 | 7, 8, 9 |
| 11 BE24/BE31/BE32 | 4, 7 | 29, 30, 32 | 8, 10 |
| 12 route parity + missing mounts | 7, 8 | 13, 29, 30, 31 | 14–17 |
| 13 BE44 real stack | 8, 12 | 29–32, 38–42 | 14–17 |
| 14 BE42 deletion | 7, 8 | 29, 39 | 13 |
| 15 BE38 operations | 7, 12 | 31, 42 | 13, 16 |
| 16 BE41 lifecycle | 8, 12 | 31, 42 | 13, 15 |
| 17 BE43 hardening | 7, 12 | 32, 42 | 13, 15, 16 |
| 18 venue policy freeze | 5 | 19–21, 24 | 22–23 |
| 19–21 L/M/N backend | 18 | 22–23, 29, 31 | each other (disjoint modules) |
| 22 L/M/N real tests | 19–21 | 24, 29, 31 | 23 |
| 23 venue authz matrix | 19–21 | 31, 39 | 22 |
| 24 shell ports | 18, 22 | 25–28 | 29–32 |
| 25–27 venue FE wiring | 24 | 28 | each other |
| 28 VENU-IN01 | 25–27 | 47 | 29–32 |
| 29 IN02 | 10, 11, 13, 14, 19–22 | 47 | 30–32 |
| 30 IN03 | 9, 11, 12, 14, 23 | 47 | 29, 31, 32 |
| 31 IN04 | 13, 15–17, 19–23 | 47 | 29, 30, 32 |
| 32 IN01 deltas | 7, 8, 10, 12 | 47 | 29–31 |
| 33–37 art/visual | 4; 24–28 for venue visuals | 47 | 38–42 |
| 38–42 QA suites | 13, 29–32 | 47 | 33–37 |
| 43 FE closure | 25–28, 31 | 47 | 44–46 |
| 44 deployment docs | 13 | 47 | 43, 45, 46 |
| 45 FE10 input | external | 47 | 43, 44, 46 |
| 46 W04 + extension IA | 25–28 | 47 | 43–45 |
| 47 IN05 reconciliation | all | 48 | — |
| 48 CI restoration | 6, 47 | none | — |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->
- [x] 1. Commit the frontend build and test repair
  What to do / Must NOT do: commit the verified frontend changes as one atomic commit: generated client moved to `frontend/src/generated/api-client`, generator and C07/QA01 scripts updated, `virtual:venue-root-art` plugin + `src/virtual-modules.d.ts`, route test derived from `generated/IA_EN.json`, admission fixtures, `src/test-setup.ts` (`asyncUtilTimeout: 5000`), vitest collection excludes, and the two product fixes (`Landing.tsx` microtask redirect, gift owner portrait). Do NOT include the backend test fix, QA01 pins or CI deletion in this commit. Push fast-forward to `master`.
  Closes: GAP-4
  Parallelization: Wave 0 | Blocked by: none | Blocks: 4, 48
  References: `frontend/vitest.config.ts`, `frontend/vite.config.ts`, `scripts/mcr/generate-api-client.py`, `scripts/mcr/contracts/qa01-coverage/run.py`, `frontend/src/app/__tests__/routes.test.ts`, `frontend/src/features/public/ui/Landing.tsx`, `frontend/src/features/gift-discovery/ui/reference-presentation.tsx`
  Acceptance criteria: commit contains only frontend/scripts/docs files listed above; `cd frontend && corepack pnpm@11.7.0 run typecheck && corepack pnpm@11.7.0 run build && corepack pnpm@11.7.0 run lint && corepack pnpm@11.7.0 run test` exits 0 with 411 files and 0 failed tests; `git push origin master` accepts fast-forward.
  QA scenarios: happy `corepack pnpm@11.7.0 run test` -> `411 passed (411)` and `0 failed`; failure probe `corepack pnpm@11.7.0 exec vitest run src/app/__tests__/routes.test.ts` with a removed IA path -> test fails (proves the assertion can fail). Evidence `.omo/evidence/mcr-finish-project/task-1-frontend-commit.md`.
  Recommended task executor category: quick
  Commit: Y | `fix(frontend): restore build and test suite at 9185c9922`

- [x] 2. Commit the backend compile fix and the QA01 re-pin
  What to do / Must NOT do: commit `backend/crates/platform/tests/grpc_auth.rs` (Workload::Concert arm) and `scripts/mcr/contracts/qa01-coverage/pins.json` as one atomic commit. Do NOT mix in frontend or CI changes. Push fast-forward.
  Closes: GAP-4
  Parallelization: Wave 0 | Blocked by: none | Blocks: 4, 48
  References: `backend/crates/platform/tests/grpc_auth.rs:34`, `scripts/mcr/contracts/qa01-coverage/pins.json`, `scripts/mcr/contracts/qa01-coverage/run.py:338`
  Acceptance criteria: `cargo +1.88.0 check --locked --workspace --all-targets` (CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/dev-target) exits 0; `python3 scripts/mcr/contracts/qa01-coverage/run.py` prints `"result": "PASS"`; commit contains exactly two files.
  QA scenarios: happy `python3 scripts/mcr/contracts/qa01-coverage/run.py` -> PASS with 198 fixtures / 1727 responses / 150 operations; failure `python3 scripts/mcr/contracts/qa01-coverage/run.py` after altering one input hash locally -> `pinned input drift` (proves the pin guards). Evidence `.omo/evidence/mcr-finish-project/task-2-backend-commit.md`.
  Recommended task executor category: quick
  Commit: Y | `fix(backend): register Concert workload and repin QA01 inputs`

- [x] 3. Commit the CI workflow removal
  What to do / Must NOT do: commit the deletion of `.github/workflows/mcr-contracts.yml` only, with a commit body stating CI restoration is deferred to task 48. Do NOT add a replacement workflow now. Push fast-forward.
  Closes: GAP-4
  Parallelization: Wave 0 | Blocked by: none | Blocks: 4, 48
  References: `.github/workflows/mcr-contracts.yml` (deleted), `docs/mcr/execution-plan.md` CI ownership
  Acceptance criteria: `git show --stat HEAD` lists exactly the deleted workflow; `git status --porcelain` empty after the push.
  QA scenarios: happy `git show --stat HEAD -- .github` -> one deletion; failure `git status --porcelain` non-empty -> task fails. Evidence `.omo/evidence/mcr-finish-project/task-3-ci-removal.md`.
  Recommended task executor category: quick
  Commit: Y | `ci: remove contract-only workflow pending full gate`

- [x] 4. Verify the clean candidate end to end
  What to do / Must NOT do: run the full current check suite against the clean pushed candidate and record every exit code. Do NOT fix anything here; any failure reopens the owning commit task instead of being patched inside this task.
  Closes: GAP-4
  Parallelization: Wave 0 | Blocked by: 1–3 | Blocks: all later waves
  References: `scripts/mcr/check.sh`, `scripts/mcr/check-plan.py`, `scripts/mcr/generate-api-client.py`, `scripts/mcr/contracts/c07/test_inventory.py`, `scripts/mcr/contracts/qa01-coverage/run.py`, `scripts/mcr/release_checks/check.py`
  Acceptance criteria: `python3 scripts/mcr/check-plan.py`, `bash scripts/mcr/check.sh guards`, `python3 scripts/mcr/generate-api-client.py --check`, `python3 scripts/mcr/generate-domain-transport.py --check`, `python3 scripts/mcr/contracts/c07/test_inventory.py`, `python3 scripts/mcr/contracts/qa01-coverage/run.py` all exit 0; `cd frontend && corepack pnpm@11.7.0 run test` exits 0; backend workspace check exits 0; `scripts/mcr/release_checks/check.py` reports `dirty=false`.
  QA scenarios: happy all commands exit 0 on the pushed SHA; failure run `check-plan.py` against a modified IA JSON -> non-zero (proves the suite blocks). Evidence `.omo/evidence/mcr-finish-project/task-4-candidate.md`.
  Recommended task executor category: deep-low
  Commit: N

- [x] 5. Produce the salvage recover-or-reimplement decision table
  What to do / Must NOT do: for each of the 48 remaining tasks (register `IN_PROGRESS`/`WAITING_INPUT`/`REOPENED`/`OPEN`), inspect the matching refs under `refs/salvage/20261106/*` read-only and decide RECOVER (branch implements the task and passes current checks in an isolated worktree) or REIMPLEMENT. Record branch, patch count, touched paths, verdict, and evidence. Do NOT merge anything here; do NOT delete refs.
  Closes: GAP-8
  Parallelization: Wave 0 | Blocked by: 4 | Blocks: 18–23
  References: `/Users/lhs/Desktop/mcr-salvage-20261106/branches.tsv`, `/Users/lhs/Desktop/mcr-salvage-20261106/README.md`, `docs/mcr/task-progress-2026-11-04.json`, candidates `codex/mcr-paradise-city-notes-api-root153`, `codex/mcr-be-product-api`, `codex/mcr-be-BE36-report-completion-03de`
  Acceptance criteria: table has one row per remaining task with a verdict and a cited branch/commit or `REIMPLEMENT`; every RECOVER row names the isolated worktree check command and its result.
  QA scenarios: happy review of 3 sampled RECOVER rows reproduces their check output; failure a RECOVER row that fails its check must be demoted to REIMPLEMENT. Evidence `.omo/evidence/mcr-finish-project/task-5-salvage-table.md`.
  Recommended task executor category: ultrabrain
  Commit: Y | `docs(mcr): record salvage recover-or-reimplement decisions`

- [x] 6. Inventory the CI gates to restore (write-only, no workflow change)
  What to do / Must NOT do: write the exact job list for task 48 in `docs/mcr/execution-plan.md` (plan/guards, frontend install+typecheck+build+lint+vitest, cargo check+test, generated drift, release checks). Do NOT create or modify any `.github/workflows/*` file in this task.
  Closes: GAP-4
  Parallelization: Wave 0 | Blocked by: 4 | Blocks: 48
  References: `scripts/mcr/check.sh`, `.github/workflows/mcr-contracts.yml` (deleted at task 3), `docs/mcr/execution-plan.md`
  Acceptance criteria: the document lists each job, its command, and its blocking condition; `ls .github/workflows` remains empty.
  QA scenarios: happy every listed command was executed successfully at task 4; failure any listed command whose exit code is not 0 must be marked non-blocking with a reason. Evidence `.omo/evidence/mcr-finish-project/task-6-ci-inventory.md`.
  Recommended task executor category: unspecified-low
  Commit: Y | `docs(mcr): inventory blocking CI gates`

- [x] 7. Adopt the BE37 relationship policy across real consumers
  What to do / Must NOT do: finish the canonical Friend block/suspension authority adoption across HTTP, WebSocket and projections; resolve the retained Social16e failures; prove blocked actors are removed from presence, party, gift, feed, intro and report flows. Do NOT introduce client-side filtering as a substitute; do NOT invent a second policy source.
  Closes: GAP-1
  Parallelization: Wave 1 | Blocked by: 4 | Blocks: 11, 18–23, 30, 32
  References: `backend/services/concert-service/src/relationship/`, `backend/crates/concert-application/src/`, `docs/mcr/backend/relationship-policy.md`, register BE37
  Acceptance criteria: a cfg(test)-only probe test drives all six read entries (presence, party, gift, feed, intro, report/notification) after a committed block and a committed suspension on real PostgreSQL and observes cache invalidation; `cargo +1.88.0 test -p concert-service --test world_ws`, relationship_authority, social_live and relationship_smoke pass. SCOPE CLARIFICATION (2026-11-06): the 403/404 route matrix and the two-client feed/nearby QA are owned by tasks 30 (IN03), 32 (IN01 deltas) and 39 (QA04) and are verified there; they are not a second proof burden on this row.
  QA scenarios: happy two-client run where A blocks B -> B disappears from A's nearby/party/feed within one invalidation; failure unblocked control still visible. Evidence `.omo/evidence/mcr-finish-project/task-7-be37.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(concert/relationship): adopt canonical block policy across consumers`

- [x] 8. Adopt the BE15 outbox and command receipt consumers in production
  What to do / Must NOT do: wire the existing receipt/outbox interfaces into the production producers and worker startup; prove idempotent replay and crash recovery on a real database. Do NOT redesign the receipt schema; do NOT enable the worker without bounded claims and lease expiry.
  Closes: GAP-1
  Parallelization: Wave 1 | Blocked by: 4 | Blocks: 12, 13, 38
  References: `backend/crates/concert-application/src/commands/`, `backend/services/concert-service/src/workers/`, `backend/services/concert-service/migrations/0002_command_receipts_outbox.sql`, `docs/mcr/backend/command-receipts.md`
  Acceptance criteria: duplicate command with the same key returns the stored result without a second side effect; killing the worker mid-claim resumes after restart; `cargo test -p concert-application` and the receipt integration suite pass on real PostgreSQL.
  QA scenarios: happy replay of a party command twice -> one row + identical response; failure delete the receipt mid-flight -> command stays unreceipted and retries safely. Evidence `.omo/evidence/mcr-finish-project/task-8-be15.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(concert/receipts): enable production outbox and receipt consumers`

- [x] 9. Publish the BE03 application leaves and finish the OAuth boundary
  What to do / Must NOT do: integrate the 15 unpublished application/shared leaves, complete signup/verify/login/reset/refresh/logout against real Auth, and keep public OAuth OFF unless provider configuration exists. Do NOT tombstone Auth state on denial without the contract decision; do NOT seed production accounts.
  Closes: GAP-1
  Parallelization: Wave 1 | Blocked by: 4 | Blocks: 30, 45
  References: `backend/services/auth-service/`, `backend/services/api-gateway/src/adapters/in/http/`, `docs/mcr/backend/be03-oauth-boundary.md`, register BE03
  Acceptance criteria: real HTTP run covers signup -> verification -> login -> refresh -> logout and invalid credentials/throttle; `cargo test -p auth-service` passes; `cargo test -p api-gateway` passes; public OAuth remains disabled in config.
  QA scenarios: happy `curl -i -X POST /api/auth/login` with a seeded isolated account -> 200 + cookie; failure wrong password -> 401 without enumeration. Evidence `.omo/evidence/mcr-finish-project/task-9-be03.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(auth): complete browser auth contract leaves`

- [x] 10. Complete BE10/BE11 admission and room leases in production
  What to do / Must NOT do: finish one-use world-ticket issuance/consumption and room lease join/switch/leave/disconnect-grace against the canonical session and event; prove replay refusal and capacity refusal. Do NOT create a second admission path; do NOT rely on client-reported room membership.
  Closes: GAP-1
  Parallelization: Wave 1 | Blocked by: 4 | Blocks: 29, 32
  References: `backend/services/concert-service/src/world/admission_bridge.rs`, `backend/services/concert-service/src/world/`, `backend/services/concert-service/migrations/0005_world_admission.sql`, register BE10/BE11
  Acceptance criteria: ticket replay rejected; full room rejects with 409/403; disconnect grace expires and frees the lease; `cargo test -p concert-service --test world_ws` and admission tests pass on real PostgreSQL.
  QA scenarios: happy two clients obtain distinct tickets and both appear in one room; failure the same ticket used twice -> second attempt 401/409. Evidence `.omo/evidence/mcr-finish-project/task-10-admission.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(world): complete one-use admission and room leases`

- [~] 11. Complete BE24/BE31/BE32 media, feed and post composition (media delivered; feed mutation transport gated on the shared proto envelope)
  What to do / Must NOT do: join the media pipeline to canonical Auth/Friend revocation and authenticated bytes; adopt the feed producer through public Gateway/Post composition; finish comment/reaction/owner edit-delete with receipts. Do NOT invent presigned endpoints; do NOT bypass server media authorization.
  Closes: GAP-1
  Parallelization: Wave 1 | Blocked by: 4, 7 | Blocks: 29, 30, 32
  References: `backend/services/concert-service/src/feed/`, `backend/services/post-service/`, `backend/crates/post-application/`, `backend/services/concert-service/migrations/0012_media.sql`, register BE24/BE31/BE32
  Acceptance criteria: authenticated upload -> finalize -> authorized read works; unauthorized read 403; feed create/edit/delete and comment/reaction persist with receipts; `cargo test -p post-service` and feed integration tests pass.
  QA scenarios: happy `curl -i` upload + media read -> 200 with bytes; failure read after revocation -> 403. Evidence `.omo/evidence/mcr-finish-project/task-11-media-feed.md`.
  Recommended task executor category: deep-low
  Scope result: media composition verified and merged; feed mutation transport remains deliberately closed. Gap tracked by task 49.
  Commit: Y | `feat(feed): complete media, post and comment composition`

- [x] 12. Add the route-parity test and close missing public templates
  What to do / Must NOT do: create `scripts/mcr/contracts/route-parity.py` comparing every command path in `backend/crates/concert-application/src/commands/registry.rs` and `contracts/domain/v1/operations.json` with the axum routes under `backend/services/api-gateway/src/adapters/in/http/**`; mount every missing family (beacons, giveaways/reservations/pickup, places, party owner/rally, reports, admin settings/phase/places/notices/closure) with the existing proxy pattern. Do NOT add domain logic in the gateway; do NOT leave a path reachable by a non-concert service by accident.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 7, 8 | Blocks: 13, 29, 30, 31
  References: `backend/crates/concert-application/src/commands/registry.rs`, `contracts/domain/v1/operations.json`, `backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs`, advisory finding 24/56 missing
  Acceptance criteria: `python3 scripts/mcr/contracts/route-parity.py` exits 0 with zero missing templates; unauthenticated request to each family returns the gateway 401 envelope; unknown event returns 404 from the Concert handler.
  QA scenarios: happy parity report `missing=0`; failure temporarily remove one route -> parity exits non-zero. Evidence `.omo/evidence/mcr-finish-project/task-12-parity.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(gateway): mount every registered concert command route`

- [x] 13. Compose the real stack: BE44 compose, smoke script, restart proof
  What to do / Must NOT do: add `infra/mcr/compose.yml` and `scripts/mcr/backend-smoke.sh` (seven services + Redis + PostgreSQL), fresh start, health, migration verify, one public route, restart, and clean shutdown. Do NOT bake secrets; do NOT point at shared databases; keep loopback.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 8, 12 | Blocks: 29–32, 38–42
  References: `infra/mcr/backend/Dockerfile`, `backend/scripts/mcr/dev_stack.py`, `docs/mcr/backend/local-development.md`, advisory finding: no compose/smoke exists
  Acceptance criteria: `scripts/mcr/backend-smoke.sh` exits 0 on a fresh volume set; all `/livez` 200; gateway `/readyz` 200; `trim`/restart preserves data; `docker compose down -v` leaves no containers.
  QA scenarios: happy fresh start -> readyz 200 + bootstrap 200; failure stop one dependency -> readyz reports it down (no false ready). Evidence `.omo/evidence/mcr-finish-project/task-13-stack.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(infra): add reproducible real-stack composition and smoke`

- [~] 14. Complete BE42 sealed account-deletion apply (coordinator + sealed apply delivered; serving binary does not yet mount it)
  What to do / Must NOT do: finish the canonical Auth -> Ticket/Sessions/Sharing/PrivateData sealed apply with its proposal seal and ownership gap resolved; prove deletion converges and revoked sessions cannot read. Do NOT delete data without the sealed receipt; do NOT leave orphaned private rows.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 7, 8 | Blocks: 29, 39
  References: `backend/services/concert-service/src/account_private_data/`, `backend/services/concert-service/migrations/`, register BE42, advisory finding: held/unpublished
  Acceptance criteria: deletion request -> sealed apply -> all canonical references removed; a revoked session gets 401; a second deletion request is idempotent; real PG test proves no orphan rows.
  QA scenarios: happy delete one isolated account -> Auth/Profile/Concert rows gone; failure abort mid-apply -> documented resume/rollback behavior. Evidence `.omo/evidence/mcr-finish-project/task-14-be42.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(account): complete sealed account deletion apply`

- [x] 15. Complete BE38 event settings, notices and flags
  What to do / Must NOT do: finish operator-role checks, reviewed-place binding, notice publication and feature flags through the public gateway; prove a non-operator is rejected. Do NOT allow a client to set flags without role evidence.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 7, 12 | Blocks: 31, 42
  References: `backend/services/concert-service/src/operations_live/`, `backend/crates/concert-application/src/commands/registry.rs` admin routes, register BE38
  Acceptance criteria: operator sets phase/flags/notice -> persisted and visible in bootstrap; non-operator -> 403; `cargo test -p concert-service --test social_policy_runtime` and operations tests pass.
  QA scenarios: happy operator publishes a notice -> `GET /api/concert/.../bootstrap` shows it; failure member token -> 403. Evidence `.omo/evidence/mcr-finish-project/task-15-be38.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(operations): complete settings, notices and flags`

- [~] 16. Complete BE41 event closure and archive (route + read-only proofs delivered; closure producer and archive HTTP endpoint unmounted)
  What to do / Must NOT do: finish phase closure, read-only barrier, archive export and cleanup jobs with partial-failure recovery. Do NOT delete private data before the archive receipt; do NOT allow writes after closure.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 8, 12 | Blocks: 31, 42
  References: `backend/services/concert-service/src/event_lifecycle/`, `backend/services/concert-service/src/event_cleanup_live/`, register BE41
  Acceptance criteria: closure switches the event to READ_ONLY; archive contains only permitted data; writes after closure return 409/403; restart resumes pending cleanup; real PG tests pass.
  QA scenarios: happy close an isolated event -> archive row + read-only bootstrap; failure force a cleanup fault -> job remains retryable. Evidence `.omo/evidence/mcr-finish-project/task-16-be41.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(lifecycle): complete closure, archive and cleanup`

- [x] 17. Complete BE43 boundary and abuse hardening
  What to do / Must NOT do: consolidate body-size, rate, ticket, upload, reveal and WebSocket boundary limits; add negative tests for oversized bodies, replay and throttling. Do NOT weaken limits to pass a test; do NOT add global blanket bans.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 7, 12 | Blocks: 32, 42
  References: `backend/services/api-gateway/src/config/`, `backend/crates/platform/src/`, `backend/services/concert-service/src/world/`, register BE43
  Acceptance criteria: oversized body -> 413; replayed ticket -> 401/409; throttled login -> 429 with Retry-After; tests cover each boundary; `cargo check` and gateway tests pass.
  QA scenarios: happy a normal request still succeeds; failure each abuse case returns the documented status without private metadata. Evidence `.omo/evidence/mcr-finish-project/task-17-be43.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(gateway): harden public boundaries against abuse`

- [x] 18. Freeze the venue policy inputs before any venue code
  What to do / Must NOT do: decide and record the venue semantics the backend must enforce: friendship/viewer authority for location, freshness/retention and pause semantics, event timezone/endday expiry, reviewed-landmark requirement for INVITE, and the optional-invitation policy for MESSAGE. Write `docs/mcr/contracts/paradise-city-v1/policy-freeze.md` and update the venue OpenAPI fields. Do NOT invent permissive defaults; unknown authority must fail closed.
  Closes: GAP-5, GAP-1
  Parallelization: Wave 3 | Blocked by: 5 | Blocks: 19–21, 24
  References: `backend/docs/api/paradise-city-v1/{location,meeting,place-notes}.openapi.json`, `backend/proto/venue/v1/*.proto`, `docs/mcr/venue-feature-amendment-2026-11-05.md`, `docs/mcr/identity-admission-decision.md`
  Acceptance criteria: every field consumed by L/M/N has a stated owner, TTL and fail-closed rule; contract schema checks pass; no unresolved “TBD” remains in the affected fields.
  QA scenarios: happy `python3 scripts/mcr/generate-domain-transport.py --check` and venue OpenAPI validation pass; failure a contract with a missing authority field is rejected by the validator. Evidence `.omo/evidence/mcr-finish-project/task-18-venue-policy.md`.
  Recommended task executor category: deep-high
  Commit: Y | `docs(venue): freeze location, invitation and notes policy`

- [x] 19. Implement VENU-BE-L01 location consent and friend freshness
  What to do / Must NOT do: add the consent, viewer-scope, freshness and revocation domain to the Concert service (migration, aggregate, commands, public routes) with server-time evaluation. Do NOT expose location without an active grant; do NOT persist a location history the contract does not allow.
  Closes: GAP-1, GAP-5
  Parallelization: Wave 3 | Blocked by: 18 | Blocks: 22–23, 29, 31
  References: `backend/docs/api/paradise-city-v1/location.openapi.json`, `backend/services/concert-service/src/relationship/`, `backend/services/concert-service/migrations/` (new `0053_venue_location.sql`)
  Acceptance criteria: grant -> viewer reads fresh location; revoke/expire -> 403/404; paused data never appears current; blocked/suspended viewer fails closed; real PG tests pass.
  QA scenarios: happy grant then `curl -i` location -> 200 with TTL; failure revoke then same call -> 403. Evidence `.omo/evidence/mcr-finish-project/task-19-venu-l01.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue/location): implement consent and freshness`

- [x] 20. Implement VENU-BE-M01 landmark invitations and conversation
  What to do / Must NOT do: add invitation create/accept/decline/cancel bound to a reviewed landmark and revision, plus event-scoped conversation between authorized friends. Do NOT create a global DM; do NOT let invitation linkage grant conversation permission; do NOT allow a non-friend write.
  Closes: GAP-1, GAP-5
  Parallelization: Wave 3 | Blocked by: 18 | Blocks: 22–23, 29, 31
  References: `backend/docs/api/paradise-city-v1/meeting.openapi.json`, `backend/proto/venue/v1/meeting.proto`, `backend/services/concert-service/src/meeting/`, `backend/services/concert-service/migrations/` (new `0054_venue_invitations.sql`)
  Acceptance criteria: invite requires a reviewed landmark; accept/decline transitions are idempotent; conversation only opens for authorized friends in the event; cancel rights follow the producer; real PG tests pass.
  QA scenarios: happy A invites B, B accepts, both exchange one message; failure B is blocked -> invite and message rejected. Evidence `.omo/evidence/mcr-finish-project/task-20-venu-m01.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue/meeting): implement invitations and conversation`

- [x] 21. Implement VENU-BE-N01 place notes and replies
  What to do / Must NOT do: add place-anchored notes with Friends/Everyone audience, 1h/endday expiry on server time, replies, moderation and Meet Here linkage. Do NOT allow a note without a real place; do NOT leak notes to unauthorized audiences or expired readers.
  Closes: GAP-1, GAP-5
  Parallelization: Wave 3 | Blocked by: 18 | Blocks: 22–23, 29, 31
  References: `backend/docs/api/paradise-city-v1/place-notes.openapi.json`, `backend/proto/venue/v1/place_notes.proto`, `backend/services/concert-service/src/community/`, `backend/services/concert-service/migrations/` (new `0055_place_notes.sql`)
  Acceptance criteria: create/list/reply/expire work on real PG; audience filter enforced; expired notes disappear; Meet Here enters the invitation flow and never fabricates arrival; moderation can hide a note.
  QA scenarios: happy Friends note visible to a friend and hidden from a stranger; failure expired note -> 404. Evidence `.omo/evidence/mcr-finish-project/task-21-venu-n01.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue/notes): implement place notes and replies`

- [x] 22. Prove the venue backend on real PostgreSQL and route parity
  What to do / Must NOT do: add venue families to the route-parity test, run L/M/N against a private PostgreSQL cluster, and prove migrations apply from an empty database and after restart. Do NOT use fixtures as producers; do NOT skip the migration ledger.
  Closes: GAP-1
  Parallelization: Wave 3 | Blocked by: 19–21 | Blocks: 24, 29, 31
  References: `scripts/mcr/contracts/route-parity.py` (task 12), `backend/services/concert-service/migrations/`, `backend/scripts/mcr/dev_stack.py`
  Acceptance criteria: `python3 scripts/mcr/contracts/route-parity.py` reports zero missing venues; `cargo test -p concert-service --test venue_*` passes; migrations 0053–0055 apply, verify and survive restart.
  QA scenarios: happy fresh DB -> migrate -> venue tests green; failure drop one migration -> verify-migrations exits non-zero. Evidence `.omo/evidence/mcr-finish-project/task-22-venue-tests.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(venue): prove L/M/N on real PostgreSQL and route parity`

- [x] 23. Prove the venue authorization and privacy matrix
  What to do / Must NOT do: test owner/friend/stranger/blocked/suspended/expired/admin across location, invitations, conversation and notes on HTTP and caches; prove no private metadata leaks. Do NOT accept client-side hiding as evidence.
  Closes: GAP-5, GAP-3
  Parallelization: Wave 3 | Blocked by: 19–21 | Blocks: 31, 39
  References: `docs/mcr/venue-feature-amendment-2026-11-05.md` required flows, `backend/crates/concert-application/src/`, QA04 matrix precedent
  Acceptance criteria: each matrix cell has an asserted status; blocked/suspended actors get 403/404; cache invalidation removes revoked data; no error body contains private fields.
  QA scenarios: happy authorized friend reads; failure each denied role -> documented status. Evidence `.omo/evidence/mcr-finish-project/task-23-venue-authz.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(venue): cover the authorization and privacy matrix`

- [x] 24. Wire the production shell to the venue ports
  What to do / Must NOT do: replace `meeting: null` and the omitted `capabilities`/`phone` ports in `WorldShell` and `venue-root/input.ts` with real adapters that fail closed. Do NOT change route ownership; do NOT invent a second protocol.
  Closes: GAP-1, GAP-6
  Parallelization: Wave 4 | Blocked by: 18, 22 | Blocks: 25–28
  References: `frontend/src/app/WorldShell.tsx:186-187`, `frontend/src/app/venue-root/input.ts:19-21`, `frontend/src/shared/api/paradise-city-v1/`
  Acceptance criteria: shell renders venue panels with real ports; missing authority shows an unavailable state, never a normal state; `tsc -b` and the venue shell tests pass.
  QA scenarios: happy browser mount shows the venue panel with a real capability; failure ports removed -> truthful unavailable panel. Evidence `.omo/evidence/mcr-finish-project/task-24-shell-ports.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue): wire production shell ports`

- [x] 25. Complete the venue Map/Friends client states
  What to do / Must NOT do: implement permission request/denial, friend clusters, selection, request/pause/revoke, stale/offline recovery against the L01 producer. Do NOT show current without freshness; do NOT treat a friendship as location consent.
  Closes: GAP-1
  Parallelization: Wave 4 | Blocked by: 24 | Blocks: 28
  References: `frontend/src/features/paradise-city-map/`, `frontend/src/app/venue-root/`, `docs/mcr/venue-feature-amendment-2026-11-05.md` Friends flows
  Acceptance criteria: denied permission shows a safe state; paused/stale labels are truthful; revoke removes the peer; browser run at 360/390/430.
  QA scenarios: happy share -> friend sees the cluster; failure deny permission -> no location request and a clear message. Evidence `.omo/evidence/mcr-finish-project/task-25-map-friends.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue): complete map and friends states`

- [x] 26. Complete the invitation and conversation client
  What to do / Must NOT do: incoming/outgoing invitations, accept/decline/cancel and the event-scoped conversation, with no bare-user DM entry. Do NOT let opening a screen perform a write; do NOT show invitation context as a conversation permission.
  Closes: GAP-1
  Parallelization: Wave 4 | Blocked by: 24 | Blocks: 28
  References: `frontend/src/features/venue-invitations/`, `frontend/src/features/meeting-conversation/`, `frontend/src/app/venue-community-entry/`
  Acceptance criteria: accept/decline update the producer; cancel respects producer rights; conversation opens only for authorized friends; browser run at three widths.
  QA scenarios: happy invite -> accept -> message round trip; failure blocked friend -> conversation unavailable. Evidence `.omo/evidence/mcr-finish-project/task-26-invitations.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue): complete invitations and conversation UI`

- [x] 27. Complete the place notes client
  What to do / Must NOT do: notes map clusters, list, selection, composer with audience/expiry, replies and Meet Here. Do NOT fabricate timestamps; do NOT allow an audience the server rejects.
  Closes: GAP-1
  Parallelization: Wave 4 | Blocked by: 24 | Blocks: 28
  References: `frontend/src/app/venue-notes-adapter/`, `frontend/src/features/community/` (existing note surfaces), `backend/docs/api/paradise-city-v1/place-notes.openapi.json`
  Acceptance criteria: create note with audience + expiry; replies render; expired notes vanish; Meet Here starts the invitation flow; browser run.
  QA scenarios: happy create -> reply -> Meet Here; failure expired note -> removed from list. Evidence `.omo/evidence/mcr-finish-project/task-27-notes.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(venue): complete notes map, composer and replies`

- [~] 28. Execute VENU-IN01 with the 48 reference comparisons
  What to do / Must NOT do: run the integrated venue flows and capture all 48 comparison cases plus the 10 extension IA states at 360/390/430; record PASS/FAIL per case against the supplied photographs. Do NOT substitute a static screenshot; do NOT claim approval of the provisional Photo5/P09/P10 variant.
  Closes: GAP-1, GAP-6
  Parallelization: Wave 4 | Blocked by: 25–27 | Blocks: 47
  References: `docs/mcr/design/venue-redesign-2026-11-05/reference-manifest.json`, `reference-state-map.json`, `comparison-tracking-2026-11-05.json`
  Acceptance criteria: 48/48 cases have a capture and verdict; 10/10 extension IA states exercised; no material difference left unrecorded.
  QA scenarios: happy all flows complete on real services; failure one denied/expired case missing -> case FAIL and no VENU-IN01 acceptance. Evidence `.omo/evidence/mcr-finish-project/task-28-venu-in01/`.
  Recommended task executor category: deep-low
  Commit: Y | `test(venue): execute integrated flows and reference comparisons`
  Gap (2026-11-06): 48/48 reference cases and 10/10 IA states have captures and verdicts (23 PASS / 2 DIFFER / 23 NOT_REACHABLE; 3 PASS / 7 NOT_REACHABLE), but 23 cases and 7 states are unreachable while PgVenueLocationAuthority::friend_in is a fail-closed stub returning VenueLocationError::PolicyUnavailable (503). The two DIFFER cases are genuine reference-base differences (provisional Photo 5 vs Photos 1 and 4) and remain unapproved. A bilateral friend authority is being implemented; the full matrix must be re-run against it to convert those families (friend location, sharing audience, meeting, invitation, conversation).

- [x] 29. Pass IN02 real meetup and pickup
  What to do / Must NOT do: two real clients complete party request/approval, authorized physical beacon/rally, giveaway reservation and one-time pickup against the real stack. Do NOT infer physical arrival from world movement; do NOT use mocks.
  Closes: GAP-2
  Parallelization: Wave 5 | Blocked by: 10, 11, 13, 14, 19–22 | Blocks: 47
  References: `docs/mcr/qa/meetup-pickup-in02.md`, `frontend/e2e/real/`, `backend/services/concert-service/src/meeting/`
  Acceptance criteria: both clients complete the journey; capacity and stock invariants hold under two concurrent attempts; pickup code is one-time; evidence includes HTTP statuses and DB rows.
  QA scenarios: happy full journey -> accepted/claimed rows; failure three applicants for two slots -> third rejected. Evidence `.omo/evidence/mcr-finish-project/task-29-in02.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(in02): pass real meetup and pickup gate`

- [~] 30. Pass IN03 social and privacy gate (external input dependent)
  What to do / Must NOT do: complete real intro discovery, feed/comment, mutual consent/reveal/revoke, notifications and block propagation; run the authorized age-provider positive cases only when provenance exists. Do NOT invent provider provenance; do NOT substitute fixtures for restricted/revoked/blocked cases.
  Closes: GAP-2, GAP-5
  Parallelization: Wave 5 | Blocked by: 9, 11, 12, 14, 23 | Blocks: 47
  References: register IN03 WAITING_INPUT, `docs/mcr/qa/` social gate evidence, `backend/services/concert-service/src/intro_live/`, `social_live/`
  Acceptance criteria: every non-provider case passes on real services; provider cases are explicitly NOT_RUN with the missing input named; no client-only privacy claim.
  QA scenarios: happy reveal/revoke round trip; failure expired/blocked reveal -> 403/404. Evidence `.omo/evidence/mcr-finish-project/task-30-in03.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(in03): pass the social privacy gate where inputs exist`
  Gap (2026-11-06): 27/27 executed checks pass on a real stack with exact envelopes (follow/request/decline/accept, private-profile fence and invalidation, block/unblock with journal rows, blocked follow 403, feed excluding deleted and blocked authors, wall outsider 404, 11-denial payload audit with zero private-metadata leaks, and the blocked-read feed now 200 with items=[] instead of 503). NOT_RUN with named blockers: four provider cases (age provider/claims, OAuth provider, external mail/SMS), age-provider positive and social reveal/revoke, venue meeting/location cases (the branch predates the venue dispatch merge and must be re-run), no blocked-comment seed for blocked_comment_author_filter, and no browser/WS client for client_cache_and_ws_frames.

- [x] 31. Pass IN04 multiuser mural and moderation gate
  What to do / Must NOT do: multiple real clients edit the Wall B surface (opaque erase, four-connected fill ≤256, eligible undo), with cooldown, shared 512-cell budget, moderation pause/restore/freeze and convergence. Do NOT allow private history leakage; do NOT skip the uncertain-outcome recovery case.
  Closes: GAP-2
  Parallelization: Wave 5 | Blocked by: 13, 15–17, 19–23 | Blocks: 47
  References: `docs/mcr/qa/in04-wall-b.md`, `backend/services/concert-service/src/mural_seed/`, `frontend/src/features/mural/`, `docs/mcr/design/` Wall reference
  Acceptance criteria: two clients converge on the same revision; cooldown/budget rejections are explicit; restore and freeze behave; evidence includes DB revision history and socket frames.
  QA scenarios: happy two clients edit and converge; failure third edit over budget -> rejected with no state change. Evidence `.omo/evidence/mcr-finish-project/task-31-in04.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(in04): pass multiuser mural and moderation gate`

- [~] 32. Re-run IN01 deltas for changed features
  What to do / Must NOT do: re-run the two-account world gate for features changed since the historical IN01 acceptance (admission, policy frames, feed, theme, panels). Do NOT re-claim unchanged history.
  Closes: GAP-2
  Parallelization: Wave 5 | Blocked by: 7, 8, 10, 12 | Blocks: 47
  References: `docs/mcr/qa/in01-world-gate.md`, `frontend/e2e/real/world.spec.ts`
  Acceptance criteria: the changed-feature delta list is explicitly enumerated and each item passes on the same candidate.
  QA scenarios: happy two clients join, move, open panels, keep one socket; failure reconnect after kill -> recovery. Evidence `.omo/evidence/mcr-finish-project/task-32-in01-delta.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(in01): re-run deltas for changed features`
  Gap (2026-11-06): D1 admission, D3 panel, D4 theme, D5 panels and the socket-kill reconnect PASS on real browser evidence with one socket per client and zero invalid frames; D2 (policy frames) is NOT_RUN because the Social policy baseline transport is not composed in this gate (CONCERT_SOCIAL_MODE unset with no probe decoder), and feed CONTENT is NOT_RUN because the Feed product mode is off. Sustain refreshObservation was not observable inside the runtime window (900 s token TTL).

- [x] 33. Finish PI08 Photo2 HUD and camera
  What to do / Must NOT do: close the remaining HUD/camera/Photo2 renderer and registry work against the reference; keep the qualified partial art tuple. Do NOT reopen AR02/AR04 criteria; do NOT admit unqualified pixels.
  Closes: GAP-6
  Parallelization: Wave 6 | Blocked by: 4; 24–28 for venue visuals | Blocks: 47
  References: `docs/mcr/progress.md` PI08 row, `frontend/src/features/parade-ink/world-hud/`, `frontend/src/app/world-entry/`
  Acceptance criteria: HUD matches the reference at 360/390/430; renderer source/release pins are consistent; browser captures recorded.
  QA scenarios: happy three-width HUD capture; failure absent release artifact -> truthful unavailable state. Evidence `.omo/evidence/mcr-finish-project/task-33-pi08/`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(ui): finish PI08 Photo2 HUD and camera`

- [x] 34. Finish PI11 rooms, nearby and help
  What to do / Must NOT do: pass the 390 mandatory switch, 430 and short-height cases; keep map/reference obligations separate. Do NOT change the reference scene geometry.
  Closes: GAP-6
  Parallelization: Wave 6 | Blocked by: 4 | Blocks: 47
  References: `docs/mcr/progress.md` PI11 row, `frontend/src/app/world-entry/root-features`, `frontend/src/features/world-help/`
  Acceptance criteria: 390/430/short captures pass; no legacy layout regression.
  QA scenarios: happy three captures; failure short-height overlap -> FAIL. Evidence `.omo/evidence/mcr-finish-project/task-34-pi11/`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(ui): finish PI11 rooms, nearby and help`

- [~] 35. Obtain and integrate AR01 venue provenance inputs
  What to do / Must NOT do: request the dated event-day layout, meeting/accessibility review, named operator, publication expiry and closure responsibility; publish only qualified place records and keep unknown ones unavailable. Do NOT fabricate venue data.
  Closes: GAP-5
  Parallelization: Wave 6 | Blocked by: external input | Blocks: 47
  References: register AR01 WAITING_INPUT, `docs/mcr/backend/places-*.md`, `docs/mcr/venue-feature-amendment-2026-11-05.md`
  Acceptance criteria: inputs are documented with provenance; unqualified candidates remain unavailable; VENU place records match the supplied review.
  QA scenarios: happy reviewed place shows verified state; failure missing review -> unavailable. Evidence `.omo/evidence/mcr-finish-project/task-35-ar01.md`.
  Recommended task executor category: deep-low
  Commit: Y | `docs(venue): register AR01 provenance and qualification`
  Gap (2026-11-06): no reviewed AR01 inputs supplied. Binding map and exact missing inputs recorded in .omo/evidence/mcr-finish-project/task-35-ar01.md; unverified/closed places stay refused at domain, DB, repository and application layers (fail-closed proven). Three schema gaps need a contract decision, not data: no place publication-expiry field, no closure-owner field, no event-day layout date.

- [~] 36. Pass QA06 mobile visual acceptance
  What to do / Must NOT do: compare all 12 mapped families and all 61 retained IA entries at 360/390/430 plus iOS/Android devices where available; record PASS/FAIL per case. Do NOT accept a single surface as coverage.
  Closes: GAP-6
  Parallelization: Wave 6 | Blocked by: 4; 25–28, 33–34 | Blocks: 47
  References: `docs/mcr/qa/reference-integrated.md`, `docs/mcr/design/reference-target-2026-11-04.png`, `docs/mcr/design/venue-redesign-2026-11-05/reference-state-map.json`
  Acceptance criteria: each family and IA entry has a capture and verdict; device runs recorded with versions; no material difference left unrecorded.
  QA scenarios: happy full matrix; failure an overlapping/blank capture -> FAIL. Evidence `.omo/evidence/mcr-finish-project/task-36-qa06/`.
  Recommended task executor category: deep-low
  Commit: Y | `test(visual): pass QA06 reference acceptance`
  Gap (2026-11-06): the matrix is honest but incomplete - 206 desktop case-widths at 360/390/430 (95 PASS, 0 DIFFER, 111 NOT_REACHABLE, 0 FAIL) plus 36 Android-emulator captures from an earlier session (not re-run at the delivered commit; physical iOS/Android NOT_RUN with no simulator or device). Blocked families with named producers: reviewed siteplan/landmark (W02/W03/W07/M06), community clock/entry authority (G01-G07, M01-M05), world mural-scene release (R01, PI09), operator/moderator authority (O01-O09), safety service (U08/S01), requests/social provider (U04/U05), email/OAuth/legal/archive (A03/A04/A06/A07/X01). Family capture coverage is 8 distinct cases and 15 aliases (identical bytes to their IA base); only W01-parade, PI08-danger-era and the two PI01 chrome cases add pixels. Two 44px defects were fixed (profile back-world link 21px, admin check-access 25px).

- [~] 37. Verify the character atlas production status
  What to do / Must NOT do: confirm the produced atlas/manifest matches the selected source pack and is actually served; if composition is only available through a reviewed derivative, document the admission path. Do NOT use legacy or synthetic character art.
  Closes: GAP-6
  Parallelization: Wave 6 | Blocked by: 4 | Blocks: 47
  References: `frontend/src/entities/avatar/character-source-pack.json`, `docs/mcr/character-art.md`, `frontend/public/world/avatars/`
  Acceptance criteria: atlas manifest serves and renders in world/profile/gift surfaces; fallback remains prohibited; source hashes match.
  QA scenarios: happy avatar renders in three surfaces; failure missing manifest -> truthful unavailable. Evidence `.omo/evidence/mcr-finish-project/task-37-atlas.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(avatar): verify production atlas admission`
  Gap (2026-11-06): reviewed motion manifest mcr_selected_motion_reviewed_20261104 and its 1280x1856/576-cell atlas are produced, hash-pinned, guarded and loader-wired, but are NOT served: compose sets no CONCERT_GRPC_ADDR and none of the six CONCERT_* release settings, the reviewed bootstrap exists only as a runtime test fixture, and reviewed-handoff.json records production_activation NOT_RUN. Production activation and a served-renderer capture remain open with the World release path (row 51). Evidence: .omo/evidence/mcr-finish-project/task-37-atlas.md; guard commit cc9691cec.

- [x] 38. Pass QA03 database race suite
  What to do / Must NOT do: run real PostgreSQL interleavings for party capacity, stock reservation/release/claim, idempotent receipts and restart. Do NOT use sleeps as synchronization; use barriers and db constraints.
  Closes: GAP-3
  Parallelization: Wave 7 | Blocked by: 13, 29–32 | Blocks: 47
  References: `backend/services/concert-service/tests/`, register QA03
  Acceptance criteria: every interleaving has an asserted invariant; no test relies on wall-clock ordering; suite passes on a clean candidate.
  QA scenarios: happy concurrent approval keeps capacity; failure double-claim -> one succeeds, one conflicts. Evidence `.omo/evidence/mcr-finish-project/task-38-qa03.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(qa03): pass database race suite`

- [x] 39. Pass QA04 privacy and authorization matrix
  What to do / Must NOT do: cover owner/member/stranger/blocked/suspended/expired/admin across HTTP, WebSocket and caches, including Wall history and Gifts media. Do NOT rely on client checks.
  Closes: GAP-3
  Parallelization: Wave 7 | Blocked by: 13, 29–32 | Blocks: 47
  References: `docs/mcr/qa/` privacy precedent, register QA04
  Acceptance criteria: each cell asserted; private fields absent from all denied responses; cache eviction proven.
  QA scenarios: happy authorized read; failure each denied role -> documented status. Evidence `.omo/evidence/mcr-finish-project/task-39-qa04.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(qa04): pass privacy and authorization matrix`

- [x] 40. Pass QA05 realtime failure suite
  What to do / Must NOT do: real pairs for delay/gap, slow client, foreground, ticket replay, restart and room switch. Do NOT simulate server frames in place of a real socket.
  Closes: GAP-3
  Parallelization: Wave 7 | Blocked by: 13, 29–32 | Blocks: 47
  References: `docs/mcr/qa/qa05-frontend-recovery.md`, register QA05
  Acceptance criteria: each failure mode recovers to a fresh snapshot; no duplicate actors; ticket replay rejected.
  QA scenarios: happy reconnect after restart; failure stale ticket -> 401/409. Evidence `.omo/evidence/mcr-finish-project/task-40-qa05.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(qa05): pass realtime failure suite`

- [~] 41. Pass QA09 recovery and lifecycle rehearsal
  What to do / Must NOT do: six real recovery scenarios: DB restart/backup restore, outbox replay, pending pickup, deletion convergence, partial event closure, and service restart. Do NOT accept a plan-only rehearsal.
  Closes: GAP-3
  Parallelization: Wave 7 | Blocked by: 13, 14–17, 29–32 | Blocks: 47
  References: register QA09, `backend/scripts/mcr/dev_stack.py`, `docs/mcr/qa/`
  Acceptance criteria: each scenario has state before/after and a restored invariant; evidence includes DB rows and logs.
  QA scenarios: happy restore -> data consistent; failure kill mid-outbox -> replay completes. Evidence `.omo/evidence/mcr-finish-project/task-41-qa09.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(qa09): pass recovery and lifecycle rehearsal`
  Gap (2026-11-06): seven scenarios are measured and pass - outbox replay (READY|1 to DELIVERED|2 with one consumer receipt), baseline pickup, gateway restart (event/giveaways/reservations/outbox ids/receipts/closure rows unchanged), pending pickup under a terminated lock backend, immediate postmaster stop with new PID and intact committed rows, logical backup (783,491 bytes, sha256 bdcf7958...) restored with a post-backup mutation table absent, and partial closure resumed atomically to READ_ONLY|2 with 409 on post-closure writes. Deletion convergence is NOT_RUN as a live saga: with managed deletion configured on both sides, CONCERT_PRIVATE_DATA/SESSIONS/SHARING/TICKETS stay RETRY attempt 3 with last_error Dependency unavailable while FRIEND/NOTIFICATION/POST/PROFILE succeed, so the isolated owner mount PASS is not convergence and is labelled as such.

- [~] 42. Pass QA10 candidate provenance and release checks
  What to do / Must NOT do: produce the same-candidate task-to-commit trace, frontend/backend/device evidence and release-check composition for the final SHA. Do NOT reuse evidence from another candidate.
  Closes: GAP-3
  Parallelization: Wave 7 | Blocked by: 13, 29–32 | Blocks: 47
  References: `scripts/mcr/release_checks/check.py`, `docs/mcr/qa10-release-composition.md`, `docs/mcr/qa10-release-source.md`
  Acceptance criteria: `scripts/mcr/release-check.sh --candidate <sha> --mode composition ...` reports `release_decision: NOT_ASSESSED` with no missing bindings for the in-scope items; every task row maps to a commit.
  QA scenarios: happy composition passes for the candidate; failure a stale receipt -> exit 1. Evidence `.omo/evidence/mcr-finish-project/task-42-qa10.md`.
  Recommended task executor category: deep-low
  Commit: Y | `test(qa10): compose same-candidate release evidence`
  Gap (2026-11-06): the composition checker and a 51-row task-to-commit trace are delivered for candidate 25b2d356f (109 resolved commits, all ancestors; seven SHAs mentioned in evidence are unmerged and never used as delivering). release-check reports release_decision NOT_ASSESSED with composition_checks INCOMPLETE because no same-candidate owner receipts exist for backend/frontend/service/device/operator/source, the gates BE43/QA01-07/QA09/IN01/IN02/IN04 are NOT_RUN and IN03 BLOCKED, and all 112 task and 61 IA bindings are unbound. The run must be refreshed against the final release SHA; IN05 owns the decision.

- [~] 43. Close FE24, FE33 and FE34
  What to do / Must NOT do: FE24 protected gift media/motif binding, FE33 authoritative mural serving/config/adapter, FE34 profile preferences and conflict behavior. Do NOT reopen historical closures without a changed criterion.
  Closes: GAP-1
  Parallelization: Wave 8 | Blocked by: 25–28, 31 | Blocks: 47
  References: register FE24/FE33/FE34 rows, `frontend/src/features/gift-discovery/`, `frontend/src/features/mural/`, `frontend/src/features/me/`
  Acceptance criteria: each remains truthful with real producers; no fixture-only acceptance; browser checks at three widths.
  QA scenarios: happy gift detail media + mural sync + profile save; failure unavailable producer -> truthful state. Evidence `.omo/evidence/mcr-finish-project/task-43-fe-closure.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(frontend): close FE24, FE33 and FE34`
  Gap (2026-11-06): FE33 (authoritative mural serving/config/adapter) and FE34 (profile preferences with 409 conflict) are closed against real producers with three-width browser evidence. FE24 (protected gift media/motif binding) remains open: BE24 Gateway HTTP media upload/finalize/authorized-read is not composition-qualified for a real caller (scripted probe: metadata 403 PERMISSION_DENIED, content 403, bytes 400 INVALID_INPUT), productionBinding is null and the motif mapping is NOT_READY; GiftDetail truthfully shows the unavailable state and invents nothing.

- [x] 44. Document deployment and production inputs
  What to do / Must NOT do: write the staging/production runbook: mTLS material, TLS termination, provider endpoints, operator roles, limits, backup/restore and the deferred QA08 note. Do NOT claim production capacity or invent credentials.
  Closes: GAP-5, GAP-7
  Parallelization: Wave 8 | Blocked by: 13 | Blocks: 47
  References: `infra/mcr/`, `docs/mcr/backend/local-development.md`, task 13 compose, task 41 recovery
  Acceptance criteria: a new operator can start staging from the document; every required secret is named without a value; known gaps listed.
  QA scenarios: happy follow the runbook on a clean machine profile; failure a missing secret -> startup refuses with a named error. Evidence `.omo/evidence/mcr-finish-project/task-44-deployment.md`.
  Recommended task executor category: unspecified-low
  Commit: Y | `docs(ops): add staging and production runbook`

- [~] 45. Obtain FE10 eligibility/legal publication and bind it
  What to do / Must NOT do: request the approved eligibility/legal/consent text and version, then bind signup and verification to the persisted policy. Do NOT ship fixture acknowledgements.
  Closes: GAP-5
  Parallelization: Wave 8 | Blocked by: external input | Blocks: 47
  References: register FE10 WAITING_INPUT, `backend/services/auth-service/` enrollment policy, `frontend/src/features/auth/`
  Acceptance criteria: signup -> verification -> sign-in works with real policy text; duplicate/throttle cases pass; policy version recorded.
  QA scenarios: happy real signup; failure duplicate email -> neutral response. Evidence `.omo/evidence/mcr-finish-project/task-45-fe10.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(auth): bind approved eligibility and consent publication`
  Gap (2026-11-06): no approved FE10 eligibility/legal text supplied. Corrected behaviour: the enrollment endpoint returns 503 POLICY_UNAVAILABLE without a publication, while 422 ACKNOWLEDGEMENTS_REQUIRED applies to the retired signup route and to missing/extra acknowledgements once a policy exists. Binding map and exact missing inputs in .omo/evidence/mcr-finish-project/task-45-fe10.md; only a productionApproval=false dev publication exists and it is never legal approval.

- [x] 46. Resolve the W04 invitation policy and close the 10 extension IA states
  What to do / Must NOT do: obtain the undecided W04 policy answer, then verify the 10 extension IA states have a route/state owner. Do NOT invent an invitation authority.
  Closes: GAP-5, GAP-7
  Parallelization: Wave 8 | Blocked by: 25–28 | Blocks: 47
  References: register W04 note, `docs/mcr/functional-delivery-plan.md` W04 row, `generated/IA_EN.json` extension rows
  Acceptance criteria: W04 policy recorded; 10/10 extension IA states mapped to implementing screens; no route silently redefined.
  QA scenarios: happy each extension state reachable in the app; failure an unmapped state -> FAIL. Evidence `.omo/evidence/mcr-finish-project/task-46-w04.md`.
  Recommended task executor category: deep-low
  Commit: Y | `docs(venue): resolve W04 policy and map extension IA`
  W04 decision (user, 2026-11-06): use the plan proposal - invitee consent, exactly one inviter grant per party, capacity 8, 24-hour expiry, host can revoke. Recorded as the W04 policy for row 46.

- [x] 47. Reconcile IN05 and publish the release decision
  What to do / Must NOT do: reconcile 111/111 active tasks, 112 original IDs, 61/61 IA, 12/12 design families, 8/8 venue deliverables and 10/10 extension IA on the final candidate; record QA08 `DEFERRED_BY_USER/NOT_RUN`, operators, limits and known gaps. Do NOT claim full 112 or 200-user capacity.
  Closes: GAP-7
  Parallelization: Wave 8 | Blocked by: all | Blocks: 48
  References: `docs/mcr/task-progress-2026-11-04.json`, `generated/TASKS_EN.json`, `generated/IA_EN.json`, `docs/mcr/functional-delivery-plan.md`
  Acceptance criteria: a reconciliation table covers every ID with a state and evidence; the release decision is explicit; no task is DONE without its evidence.
  QA scenarios: happy reconcile a clean candidate; failure a task without evidence -> stays IN_PROGRESS. Evidence `.omo/evidence/mcr-finish-project/task-47-in05.md`.
  Recommended task executor category: ultrabrain
  Commit: Y | `docs(release): publish IN05 reconciliation`

- [x] 48. Restore blocking CI last
  What to do / Must NOT do: add `.github/workflows/mcr-ci.yml` with the gates inventoried in task 6, make them blocking, and prove a green run on the final candidate. Do NOT reintroduce contract-only CI.
  Closes: GAP-4
  Parallelization: Wave 8 | Blocked by: 6, 47 | Blocks: none
  References: `docs/mcr/execution-plan.md` CI inventory, `scripts/mcr/check.sh`, task 4 commands
  Acceptance criteria: the workflow runs plan/guards, frozen install, typecheck, build, lint, vitest, cargo check/test, generated drift and release checks; it fails on an intentionally broken probe; the final candidate is green.
  QA scenarios: happy CI green on master; failure open a branch with a broken test -> CI red. Evidence `.omo/evidence/mcr-finish-project/task-48-ci.md`.
  Recommended task executor category: quick
  Commit: Y | `ci: add blocking build, test and contract gates`

- [x] 49. Add the qualified post mutation envelope and event-scoped feed routes
  What to do / Must NOT do: design the qualified mutation envelope in `backend/proto` (shared contract), implement `PostTransport::execute`/`get_receipt` for create/edit/delete/comment/reaction, mount the event-scoped routes in the gateway, and prove them on the composed real stack. Do NOT open a route without authorization and idempotency; do NOT invent a private envelope.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 12, 13 | Blocks: 30, 31
  References: `backend/proto/`, `backend/services/concert-service/src/feed/owner.rs`, `.omo/evidence/mcr-finish-project/task-11-media-feed.md`
  Acceptance criteria: duplicate key replay returns the stored result; create/edit/delete/comment/reaction persist with receipts; route-parity and architecture tests pass; real HTTP proof on the composed stack.
  QA scenarios: happy create -> edit -> delete a post through the gateway with a real token; failure blocked/unauthorized -> 403/404 with no row. Evidence `.omo/evidence/mcr-finish-project/task-49-feed-mutations.md`.
  Recommended task executor category: deep-high
  Commit: Y | `feat(feed): add qualified post mutation envelope and routes`

- [x] 50. Mount the account-deletion runtime and prove the end-to-end request
  What to do / Must NOT do: invoke `AccountDeletionRuntime::compose` from the serving `main.rs`, mount the deletion route behind the canonical auth boundary, and prove a real deletion request converges to the sealed ACK with revoked-session 401. Do NOT weaken the sealed apply; do NOT invent a second coordinator.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 14 | Blocks: 41
  References: `backend/services/concert-service/src/config/private_data_runtime.rs`, `src/config/account_deletion_runtime.rs`, `main.rs`, task 14 evidence.
  Acceptance criteria: a real HTTP deletion request returns the ACK; the serving binary composes the coordinator (grep-both test); restart does not duplicate effects.
  QA scenarios: happy delete -> ACK + zero private rows; failure revoked session -> 401. Evidence `.omo/evidence/mcr-finish-project/task-50-deletion-mount.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(account): mount the sealed deletion coordinator in serving`

- [~] 51. Compose the closure producer and the archive HTTP endpoint
  What to do / Must NOT do: add `Operation::Closure` to `product_rpc` using the Ops-lane `concert_event_operator_roles` schema (migration 0076, single source; no duplicate role migration or config allowlist) and the reviewed-retention provider, thread the World `AdmissionBridge` (single-bridge rule), and mount the `/archive` read endpoint. SCOPE CLARIFICATION (2026-11-06): the frozen contract declares `getConcertArchive` PUBLIC, so the archive is public and refuses only before the archived phase (404/409); private fields and deleted content are never returned. Authorization applies to the admin closure command only.
  Closes: GAP-1
  Parallelization: Wave 2 | Blocked by: 16, 13 | Blocks: 47
  References: `backend/services/concert-service/src/event_lifecycle/`, `src/product_rpc/`, `src/config/`, task 16 evidence.
  Acceptance criteria: authenticated operator closure returns the ACK through the public route; `/archive` serves only permitted data; post-closure writes 409; real HTTP proof.
  QA scenarios: happy close -> archive fetch 200 with permitted fields; failure member closure -> 403. Evidence `.omo/evidence/mcr-finish-project/task-51-closure-producer.md`.
  Recommended task executor category: deep-low
  Commit: Y | `feat(lifecycle): compose closure producer and archive endpoint`
  Gap (2026-11-06): live close -> /archive 200 is unreachable because FREEZE_MURAL..CLEANUP have no composed LifecycleOwners, so a closure job stops at READ_ONLY and both authenticated and anonymous /archive reads truthfully return 404 RESOURCE_UNAVAILABLE (PgPublicArchive::page requires ARCHIVED). Proven live: operator closure 200, member 403 PERMISSION_DENIED, post-closure write 409 INVALID_TRANSITION, forged/expired 401, OPERATOR|1| and READ_ONLY|2 persisted, deny-by-default settlement gate with exactly one early service, nine gate tests, merged-tree PG lifecycle PASS with the PUBLIC projection proven on real PostgreSQL.

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [x] F1. Plan compliance audit
  What to do: verify every implementation row 1–48 is completed or explicitly dropped, every row's evidence file exists, and no row was silently narrowed. Evidence `.omo/evidence/mcr-finish-project/f1-compliance.md`.
  Recommended task executor category: unspecified-low

- [ ] F2. Code quality review
  What to do: run LSP diagnostics on every changed file, confirm no skipped/disabled tests and no suppressed errors, and review the diff for scope creep. Evidence `.omo/evidence/mcr-finish-project/f2-quality.md`.
  Recommended task executor category: deep-low

- [ ] F3. Real manual QA on the final candidate
  What to do: execute the end-to-end scenarios for IS-1..IS-4 on the final SHA: two-client world + meetup + pickup, mural convergence, admin report/closure, venue invitation/notes, and the privacy matrix. Evidence `.omo/evidence/mcr-finish-project/f3-manual-qa/`.
  Recommended task executor category: deep-low

- [x] F4. Ideal-state fidelity check
  What to do: map each IS row to its delivering todos and captured QA artifact; a shortfall creates a new numbered task, never a note. Evidence `.omo/evidence/mcr-finish-project/f4-ideal-state.md`.
  Recommended task executor category: unspecified-low

- [ ] F5. Release audit
  What to do: reconcile 111/111 active tasks, 112 original IDs, 61/61 IA, 12/12 design, 8/8 venue, 10/10 extension IA, QA08 deferred, CI green on the same candidate. Evidence `.omo/evidence/mcr-finish-project/f5-release.md`.
  Recommended task executor category: ultrabrain

## Commit strategy
- One atomic commit per verified increment, built and tested green on its own; never an omnibus end-of-run commit.
- W0 commits: `fix(frontend): restore build and test suite`, `fix(backend): register Concert workload in grpc_auth test`, `test(contracts): repin QA01 inputs`, `ci: remove contract-only workflow (restore in W8)`, each pushed to `master` fast-forward.
- Later waves: `feat(concert/<area>)`, `fix(...)`, `test(...)` scoped per task; each commit message names the task row and the evidence path.
- No force-push; fetch and reconcile before every push; verify delivered SHA ancestry.

## Success criteria
> One row per IS row; F4 checks delivered behavior 1:1.
| IS | Delivering todo(s) | Proving QA scenario | Evidence |
| --- | --- | --- | --- |
| IS-1 | 7–17, 19–32 | Two-client IN02/IN04 + Playwright real world flow at 360/390/430 | `.omo/evidence/mcr-finish-project/task-29-in02.md`, `task-31-in04.md` |
| IS-2 | 15, 16, 38–42 | Admin report/closure API + browser run on the same candidate | `task-15-be38.md`, `task-42-qa10.md` |
| IS-3 | 1–6, 44, 48 | Clean clone check suite + CI green + deployment doc review | `task-4-candidate.md`, `task-48-ci.md` |
| IS-4 | 7, 14, 23, 31, 39 | Privacy matrix over HTTP/WS/cache with blocked/suspended/expired actors | `task-23-venue-authz.md`, `task-39-qa04.md` |

## Proposed follow-up tasks (F4, numbered, not part of this delivery set)

F4 (ideal-state fidelity) requires every shortfall to become a numbered task rather than a note. These 15 rows are recorded with their lanes and evidence expectations in `.omo/evidence/mcr-finish-project/f4-ideal-state.md`. They are deliberately outside the checkbox set above so the completed delivery is not restated as unfinished; the release decision stays NOT_RELEASE_READY until they are scheduled and delivered.

- [ ] 52. Compose the BE24 gift media upload/finalize/authorized read and bind the approved motif mapping (lane: backend)
- [ ] 53. Prove authorized feed content and the full post lifecycle with CONCERT_PUBLIC_FEED_MODE=read (lane: frontend + backend)
- [ ] 54. Prove GET mural/tools reachability and close the PI09 Wall B editor states (lane: backend + frontend + QA)
- [ ] 55. Close QA06 to zero NOT_REACHABLE and capture physical iOS/Android (lane: QA/visual)
- [ ] 56. Re-run all 48 VENU-IN01 comparisons now that the bilateral friend authority landed (lane: backend venue)
- [ ] 57. Serve the reviewed character atlas from the production runtime (lane: world/atlas)
- [ ] 58. Compose decideConcertReport and the user/content moderation commands (lane: backend)
- [ ] 59. Compose FREEZE_MURAL..CLEANUP owners so closure reaches ARCHIVED and /archive serves 200 (lane: backend lifecycle)
- [ ] 60. Add a single full-suite check entry point and record a clean-clone run (lane: CI/infra)
- [ ] 61. Remove the remaining CI exclusions and record a hosted blocking run (lane: CI/infra)
- [ ] 62. Bind the approved FE10 eligibility/legal publication (lane: external + auth)
- [ ] 63. Compose the N01 place-note read path and drive QA04 to zero NOT_RUN (lane: backend venue)
- [ ] 64. Complete IN03 provider, client-cache and WS-frame checks (lane: external + backend + frontend)
- [ ] 65. Converge the live seven-service account-deletion saga and re-run QA09-DELETION-SAGA (lane: account deletion)
- [ ] 66. Re-run the QA10 composition against the final release SHA (lane: release publisher)

