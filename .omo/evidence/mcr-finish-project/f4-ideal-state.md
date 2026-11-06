# F4 - ideal-state fidelity check

- Row: F4 (`Ideal-state fidelity check`)
- Candidate: `33141de3f` (`master`), worktree `/Users/lhs/Desktop/mcr-worktrees/exec-f4`,
  branch `codex/mcr-exec-f4`
- Executed: 2026-11-06
- Method: documentary mapping. The IS definitions and the plan's Success-criteria
  table were read from `.omo/plans/mcr-finish-project.md`; every delivering row's
  evidence file was then opened at this candidate and the cited machine reports,
  gate logs and database observations were checked against what the evidence
  actually records. No gate was re-executed here; F3 owns the live manual QA and
  F5 owns the release audit.
- Result: 4 IS rows / 20 IS elements: **10 delivered, 10 partially delivered,
  0 not delivered**. No IS row is wholly delivered. The 10 partial elements
  produce 15 proposed task rows (52-66).

The check deliberately does not upgrade a row because its plan checkbox is
`[x]`. A row is only counted as delivering an IS element when the cited artifact
shows the behavior the element names. Where an artifact proves part of an
element, the element is `PARTIAL` and the exact shortfall is carried into a
numbered task proposal below.

## 1. Ideal-state definitions (verbatim)

From `.omo/plans/mcr-finish-project.md` lines 26-35:

| Row | Statement | Reason |
| --- | --- | --- |
| IS-1 | An attendee completes world presence, meetup, giveaway, feed, wall and profile flows on a phone with no dead ends | This is the product's core promise and the IN-series gates' subject |
| IS-2 | An operator handles reports, wall control, notices and event closure with audited, truthful states | Operations and archive are required surfaces (O01-O09, X01) |
| IS-3 | A maintainer clones the repo and runs one check suite; CI blocks regressions; deployment is documented | The current session found a broken build and 376 failing tests CI never caught |
| IS-4 | Consent, blocks, expiry, idempotency and authorization are enforced server-side on HTTP, WS and caches | Client-only hiding cannot satisfy the privacy contract |

The plan's Success-criteria table (lines 679-684) binds each row to delivering
todos and proving artifacts:

| IS | Delivering todo(s) | Proving QA scenario | Evidence named by the plan |
| --- | --- | --- | --- |
| IS-1 | 7-17, 19-32 | Two-client IN02/IN04 + Playwright real world flow at 360/390/430 | `task-29-in02.md`, `task-31-in04.md` |
| IS-2 | 15, 16, 38-42 | Admin report/closure API + browser run on the same candidate | `task-15-be38.md`, `task-42-qa10.md` |
| IS-3 | 1-6, 44, 48 | Clean clone check suite + CI green + deployment doc review | `task-4-candidate.md`, `task-48-ci.md` |
| IS-4 | 7, 14, 23, 31, 39 | Privacy matrix over HTTP/WS/cache with blocked/suspended/expired actors | `task-23-venue-authz.md`, `task-39-qa04.md` |

## 2. Plan state read at this candidate

`grep '^- \[.\].' .omo/plans/mcr-finish-project.md` at `33141de3f`:

- 37 rows closed `[x]`, 14 rows recorded as gaps `[~]`, 5 audit rows open:
  F1-F5. The gap rows are 11, 14, 16, 28, 30, 32, 35, 36, 37, 41, 42, 43,
  45, 51.
- IN05's own per-row table (`.omo/evidence/mcr-finish-project/task-47-in05.md`)
  was captured at candidate `7d14251bb` and still shows rows 47 and 48 as OPEN;
  they were closed by `ef02302d4` and `33141de3f` respectively. The IN05
  per-row verdicts for every other row are used unchanged here.
- IN05's release decision remains `NOT_RELEASE_READY` (35 closed / 14 gap /
  7 open at `7d14251bb`; 37 / 14 / 5 open audit rows at this candidate).

### Evidence inconsistencies found while reading (recorded, not normalized)

1. `task-39-qa04.md` line 8 still says "Result: **FAIL** - the matrix
   executes but exposes a real WebSocket authorization gap", while the later
   correction section in the same file records the post-fix run
   (`87 PASS / 23 NOT_RUN / 0 FAIL`, line 192-193, 217). This check uses the
   post-fix run, which is the run the plan row 39 relies on; the stale header
   is a documentation defect, not a second failure.
2. `task-14-be42.md` says "Status: PASS" for the sealed deletion apply, but
   plan row 14 is `[~]` and IN05's gap list records "serving convergence
   unproven". Task 50 later mounted the runtime and verified it. The row's
   remaining gap is the live seven-service convergence measured as NOT_RUN in
   QA09, not the sealed apply itself.
3. The central register aggregate (`docs/mcr/task-progress-2026-11-04.json`:
   65 DONE / 39 IN_PROGRESS / 3 OPEN / 3 WAITING_INPUT / 1 REOPENED /
   1 DEFERRED_BY_USER) counts the 112 original task IDs. It is not a count of
   this plan's 56 rows and is not used as evidence of plan-row delivery here.
   IN05 already records this distinction.
4. QA04's evidence records a pre-existing failing venue test
   (`venue_authz_matrix.rs:2107`, stale `venue/location` expectation) that is
   not a QA04 cell failure. It is carried below as a test-hygiene shortfall,
   not as an IS-4 privacy failure.
5. The plan's F4 row is still `[ ]`; this document is its evidence, not a plan
   edit. The plan is not modified by this check.

## 3. IS -> rows -> evidence verdict

| IS | Delivering rows (state at this candidate) | Decisive artifacts | Verdict |
| --- | --- | --- | --- |
| IS-1 | 7, 9, 12, 13 `[x]`; 11, 14, 16 `[~]`; 19-27 `[x]`; 28 `[~]`; 29 `[x]`; 30 `[~]`; 31 `[x]`; 32 `[~]`; 33-34 `[x]`; 35-37 `[~]`; 43 `[~]`; 49-50 `[x]` | `task-29-in02.md` (15/15 PASS); `task-31-in04.md` (39/39 PASS); `task-32-in01-delta.md` (D1/D4/D5/R1 PASS, D3 content NOT_RUN); `task-43-fe-closure.md` (FE33/FE34 producer-backed, FE24 blocked); `task-36-qa06/README.md` (95 PASS / 111 NOT_REACHABLE / 0 FAIL); `task-28-venu-in01/report.json` (23 PASS / 2 DIFFER / 23 NOT_REACHABLE, IA 3/0/7) | **PARTIAL** |
| IS-2 | 15 `[x]`, 16 `[~]`, 31 `[x]`, 38 `[x]`, 39 `[x]`, 40 `[x]`, 41 `[~]`, 42 `[~]`, 51 `[~]` | `task-15-be38.md` (operator persistence, non-operator 403, read routes); `task-31-in04.md` (admin pause/resume/lock/restore/freeze); `task-51-closure-producer.md` (close 200, member 403, post-close 409, archive 404); `task-38-qa03.md`, `task-39-qa04.md`, `task-40-qa05.md`, `task-41-qa09.md`, `task-42-qa10.md` | **PARTIAL** |
| IS-3 | 1-6 `[x]`, 44 `[x]`, 47 `[x]`, 48 `[x]` | `task-4-candidate.md` (411 files / 5683 passed, typecheck/build/lint, clean tree); `task-48-ci.md` (4 blocking jobs, local command runs, no hosted run); `task-44-deployment.md` (runbook + variable cross-check) | **PARTIAL** |
| IS-4 | 7 `[x]`, 14 `[~]`, 23 `[x]`, 31 `[x]`, 39 `[x]`, 50 `[x]` | `task-7-be37.md` (six read entries on real PG); `task-23-venue-authz.md` (46-cell matrix); `task-39-qa04.md` (110 cells: 87 PASS / 23 NOT_RUN / 0 FAIL post-fix); `task-30-in03.md` (27/27 checks, 5 recorded NOT_RUN); `task-50-deletion-mount.md` | **PARTIAL** |

## 4. Element-level assessment

### IS-1 - attendee on a 360-430px phone

| # | Element | Verdict | Proof read | Shortfall |
| --- | --- | --- | --- | --- |
| 1.1 | World presence (admission, tickets, two clients, movement, reconnect) | DELIVERED | `task-32-in01-delta.md` D1: two real accounts, two tickets (2 issued / 2 consumed), mutual membership, bidirectional movement (7 inputs, acks 9/8, frames 20/16), real proxy TCP cut -> fresh authorized stream with both players; `task-40-qa05.md` six real-socket failure modes PASS | none |
| 1.2 | Meetup (plan, apply, accept, real pickup) | DELIVERED | `task-29-in02.md`: 15/15 checks true, cleanup PASS; `task-41-qa09.md` QA09-BASELINE-PICKUP and QA09-PENDING-PICKUP PASS with one-unit consumption and idempotent replay | none |
| 1.3 | Giveaway (discovery, detail, reservation, pickup, media) | PARTIAL | `task-41-qa09.md` real giveaway/reservation/pickup PASS; `task-38-qa03.md` gift last-slot/stock-1/claim/cancel races PASS; `task-43-fe-closure.md` FE24 **NOT CLOSED**: metadata 403 PERMISSION_DENIED, content 403, bytes 400 INVALID_INPUT, productionBinding null; `task-36-qa06/README.md` PI04 21 NOT_REACHABLE | Gift media/motif producer not composed/authorized end to end (task 52) |
| 1.4 | Feed (read, post, edit, delete, comments, reactions) | PARTIAL | `task-49-feed-mutations.md`: all eight event-scoped post/comment/reaction routes resolve to `Operation::FeedCommand`, envelope + receipts delivered; `task-30-in03.md` feed read 200 with author present, blocked author filtered; `task-32-in01-delta.md` D3 `NOT_RUN (content)` because `CONCERT_PUBLIC_FEED_MODE=off` | Shipped shell never rendered authorized feed content against the real stack (task 53) |
| 1.5 | Wall (paint, erase, fill, undo, sync, moderation) | PARTIAL | `task-31-in04.md` 39/39 checks true incl. convergence, fill limits, cooldown 429, budget 429, undo guards, admin deny/pause/lock/restore/freeze, world socket frames; `task-43-fe-closure.md` FE33 real snapshot/operations at rev 1, 360/390/430 | QA06 PI09 editor/palette 6 NOT_REACHABLE; `/mural/tools` was recorded 403 for both principals in `task-32-in01-delta.md` limits and the IN04 report logs the requests without asserting their status (task 54) |
| 1.6 | Profile (read, edit, preferences, conflict) | DELIVERED | `task-43-fe-closure.md` FE34 real stack: GET /me 200 rev 1, PATCH 200 rev 1->2, stale PATCH 409 VERSION_CONFLICT at 360/390/430 | none |
| 1.7 | Phone, 360-430px, no dead ends | PARTIAL | `task-36-qa06/README.md`: 206 cases, 95 PASS / 111 NOT_REACHABLE / 0 FAIL at 360/390/430 desktop Chrome; physical iOS NOT_RUN, physical Android NOT_RUN, Android emulator captures from an earlier session and not re-produced at this candidate; `task-28-venu-in01/report.json` 23/48 venue cases and 7/10 IA states NOT_REACHABLE; `task-37-atlas.md` reviewed atlas not served by the runtime (activation NOT_RUN) | Visual acceptance incomplete (task 55), bilateral venue authority missing (task 56), character atlas not activated (task 57) |

### IS-2 - operator surface

| # | Element | Verdict | Proof read | Shortfall |
| --- | --- | --- | --- | --- |
| 2.1 | Reports (queue, detail, decision) | PARTIAL | `task-15-be38.md`: `admin/reports` and `admin/reports/{id}` are classified to `OperationsOperation::ReportQueue`/`ReportDetail`, the seven read templates are mounted, real Postgres operations suite passes; `task-39-qa04.md` report reads are exercised | `decideConcertReport` and `moderateConcertUser` are registered rows (`commands/registry.rs:320`, `:326`) and mounted gateway templates (`command_routes.rs:94`, `:98`), but `product_rpc::operation` has no branch for them (`report_admission.rs` owns only Create/Own; `operations_admission.rs` owns only GET report queue/detail) and falls through to `permission_denied("Unsupported product operation")` at `product_rpc/mod.rs:211`/`:254` (static read of the dispatcher at `33141de3f`; no runtime probe in this check); the row-15 regression test `every_registered_admin_route_reaches_the_operations_owner` lists only the implemented routes. An operator therefore cannot act on a report or moderate a user through the public surface (task 58) |
| 2.2 | Wall control (pause, resume, freeze, lock, restore) | DELIVERED | `task-31-in04.md` checks `member_admin_denied`, `member_pause_denied`, `pause_blocks_write`, `resume_allows_write`, `lock_blocks_write`, `restore_exact`, `restore_advertised_snapshot`, `freeze_blocks_write`, `frozen_terminal` all true; admin mutations write a non-pixel revision, checkpoint, `RESOURCE_INVALIDATED` outbox row through the BE15 receipt transaction | none |
| 2.3 | Notices (create, publish, read) | DELIVERED | `task-15-be38.md`: `createConcertNotice`/`updateConcertNotice`/`listConcertAdminNotices`/`listConcertNotices`/`getConcertNotice` reach the operations owner; real PG test persists `notice row PUBLISHED`; non-operator and revoked operator get `Denied` -> 403 | none |
| 2.4 | Event closure and archive | PARTIAL | `task-16-be41.md`: real-PG route test CLOSE_EVENT 200 -> READ_ONLY, same-key byte-identical replay, later closure write 409 INVALID_TRANSITION, job PENDING -> ARCHIVED; `task-51-closure-producer.md`: operator closure 200, member 403 PERMISSION_DENIED, post-closure write 409, forged/expired 401, OPERATOR and READ_ONLY rows persisted; `task-41-qa09.md` QA09-PARTIAL-CLOSURE PASS | `/archive` is unreachable: FREEZE_MURAL..CLEANUP have no composed `LifecycleOwners`, the job stops at READ_ONLY, and both authenticated and anonymous reads return 404 RESOURCE_UNAVAILABLE (`PgPublicArchive::page` requires ARCHIVED) (task 59) |
| 2.5 | Audited, truthful degraded states | DELIVERED | `task-31-in04.md` receipt/outbox per mutation and truthful 409/429; `task-15-be38.md` durable operator role row and `PgOperationsReview` (no positive default); `task-39-qa04.md` truthful 503/404 bodies with no private fields; `task-41-qa09.md` outbox DELIVERED with one consumer receipt after restart | none |

### IS-3 - maintainer and CI

| # | Element | Verdict | Proof read | Shortfall |
| --- | --- | --- | --- | --- |
| 3.1 | Clone the repo and run one check suite | PARTIAL | `scripts/mcr/check.sh` supports `plan`, `frontend`, `backend`, `guards`, `contracts`, `foundation`; `task-4-candidate.md` records the individual commands at exit 0 (411 test files / 5683 passed, typecheck, build, lint, `cargo check --workspace --all-targets`, clean tracked tree); `task-48-ci.md` defines the four-job workflow | No single entry point runs the full gate set: `check.sh foundation` runs plan/guards/contracts/frontend build/backend build but no Vitest and no `cargo test`; the test-bearing commands exist only as CI job steps that have never run on a hosted runner (task 60) |
| 3.2 | CI blocks regressions | PARTIAL | `task-48-ci.md`: `.github/workflows/mcr-ci.yml` with four blocking jobs (guards, frontend, backend, release), no `continue-on-error`, pinned Node/pnpm/Rust/Postgres, route-parity broken-probe exits 1 in a throwaway worktree, local runs of each command | Four exclusions remain: the C07 compatibility pin (measured red), two operations-composition abort files (documented flakes), the `notification_binding`/consumer tests (missing runner roots), and the QA01 coverage checker (not wired, pinned-input drift measured). The backend job runs only 16 core crates, not the concert-service suites. No GitHub Actions run exists; every workflow command was run locally, the full frontend suite and the 16-crate backend test job were never executed (task 61) |
| 3.3 | Deployment documented | DELIVERED | `task-44-deployment.md`: `docs/mcr/ops/deployment-runbook.md`, ten-entry Compose topology, loopback smoke profile, every secret named without a value, mTLS per workload, migration/backup/restore/rollback, QA08 deferred; variable cross-check against committed config objects | Compose execution and restore drill remain NOT_RUN (no Docker daemon), which the row declares; the IS says "documented", and the document exists and is cross-checked |

### IS-4 - server-side consent, blocks, expiry, idempotency, authorization

| # | Element | Verdict | Proof read | Shortfall |
| --- | --- | --- | --- | --- |
| 4.1 | Consent (enrollment policy, acknowledgements) | PARTIAL | `task-30-in03.md` real enrollment consent + admission, and `task-32-in01-delta.md` pre-world policy 200 with `acceptedRevision == policyRevision`; `task-45-fe10.md` records `BLOCKED_ON_EXTERNAL_INPUT` with enrollment returning 503 POLICY_UNAVAILABLE without a publication, and the row-45 gap records 422 ACKNOWLEDGEMENTS_REQUIRED for missing/extra acknowledgements once a policy exists | No approved FE10 eligibility/legal text exists; only a `productionApproval=false` development publication is used and is not legal approval (task 62) |
| 4.2 | Blocks | DELIVERED | `task-7-be37.md`: committed block and suspension remove all six read entries (presence, party, gift private, public profile/feed, intro, notification routing) on real PG and restore after unblock; `task-39-qa04.md` block cells pass with no private rows; `task-30-in03.md` `/me/blocks` list/block/unblock and blocked-author feed filtering PASS | none |
| 4.3 | Expiry (leases, tickets, reservations, notes) | DELIVERED | `task-23-venue-authz.md` expired location lease -> 404 with `EXPIRED` snapshot; expired invitation decision 409; expired note/message absent from reads; `task-38-qa03.md` claim/cancel/expiry races with observed blockers; `task-40-qa05.md` ticket replay 4401 and stale stream 4409 | none |
| 4.4 | Idempotency | DELIVERED | `task-38-qa03.md` claim/cancel, release/re-reservation, mid-flight receipt restart; `task-31-in04.md` `recovery_succeeded`, `recovery_no_duplicate`, `recovery_charges_unchanged`, `recovery_same_receipt`; `task-41-qa09.md` same-key replay returns the original receipt; `task-16-be41.md` byte-identical closure replay | none |
| 4.5 | Authorization on HTTP, WS and caches | PARTIAL | `task-39-qa04.md` post-fix 110-cell matrix: 87 PASS / 23 NOT_RUN / 0 FAIL; WS ticket/subscription gap fixed (`stranger`/`admin` now 403, replay 4401); `task-30-in03.md` 27/27 checks; `task-23-venue-authz.md` 46 real-PG cells with no-store headers and same-instance revoke/block | 23 QA04 cells NOT_RUN: media upload/read 15 (producer 403), venue place-note reads 7 (N01 read-retention path not composed -> 503), feed cache eviction 1 (warm feed had no removable post); IN03 records `client_cache_and_ws_frames` NOT_RUN plus four provider-dependent cases NOT_RUN; the live seven-service deletion saga never converged (task 63, 64, 65) |

## 5. Counts

| Measure | Count |
| --- | ---: |
| IS rows | 4 |
| IS elements assessed | 20 |
| Delivered | 10 |
| Partially delivered | 10 |
| Not delivered | 0 |
| IS rows wholly delivered | 0 |
| Proposed task rows | 15 (52-66) |

## 6. Shortfalls and proposed task rows

Each proposal follows the plan's row shape: one numbered row, an explicit
"what to do / must not do", the owning lane, and the evidence the row would
need. Numbers continue after the plan's last numbered row (51); F1-F5 remain
audit rows and are not renumbered.

### 52. Compose and authorize the BE24 gift media and motif producer end to end

- Proposed row text: `52. Compose the BE24 gift media upload, finalize and authorized read, and bind the approved motif mapping`
- What to do / Must NOT do: compose the BE24 Gateway HTTP upload/finalize/authorized-read path for a real canonical Auth/Friend caller and bind the approved giveaway motif mapping; do NOT substitute fixtures, do NOT accept a truthful-unavailable state as closure, do NOT invent motif art.
- Closes / extends: IS-1 giveaway, IS-4 media authorization; plan row 43 (FE24 residual), QA04 media cells.
- Owner lane: backend (concert-service owner-media composition + Gateway media transport); frontend gift lane consumes the binding.
- Evidence required: real caller PUT/POST upload -> finalize -> GET metadata/content 200 with receipts through the Gateway; non-owner and blocked caller 403/404 with no private metadata; `productionBinding` non-null and motif id recorded; GiftDetail renders the media at 360/390/430; the 15 QA04 media cells convert from NOT_RUN to PASS; `task-43-fe-closure.md` FE24 row changes from NOT CLOSED.

Current shortfall evidence: `task-43-fe-closure.md` (metadata 403, content 403, bytes 400 INVALID_INPUT, productionBinding null, motif NOT_READY); `task-39-qa04.md` NOT_RUN list (media 14 + media probe 1).

### 53. Prove authorized feed content in the shipped shell

- Proposed row text: `53. Prove authorized feed content and the full post lifecycle in the shipped shell with CONCERT_PUBLIC_FEED_MODE=read`
- What to do / Must NOT do: run the shipped shell against the real seven-service stack with the public feed mode enabled and assert authorized `/feed` content, thread, create/edit/delete, comments and reactions; do NOT use `CONCERT_PUBLIC_FEED_MODE=off` or synthetic fixtures for this claim.
- Closes / extends: IS-1 feed; IN01 D3 content; plan row 49's shell-level proof.
- Owner lane: frontend shell (Feed panels) with backend feed composition already delivered by rows 30/49.
- Evidence required: real-stack browser run at 360/390/430 showing `GET /feed` 200 with only authorized posts, author absence for a blocked author, create -> 200 receipt -> visible, edit/delete -> receipt and removal, comment/reaction round trip; IN01 delta D3 content `PASS`; captures retained.

Current shortfall evidence: `task-32-in01-delta.md` D3 `PASS (panel), NOT_RUN (content)`: "No `/feed` request is issued because `CONCERT_PUBLIC_FEED_MODE=off`".

### 54. Prove the Wall B editor tools surface and close PI09

- Proposed row text: `54. Prove GET mural/tools reachability and close the PI09 Wall B editor visual states at 360/390/430`
- What to do / Must NOT do: assert the `/mural/tools` status for admitted members, strangers and suspended callers in the real IN04 stack, and capture the editor grid, palette, tools and confirmation states; do NOT treat an unasserted request log as reachability proof.
- Closes / extends: IS-1 wall; QA06 PI09; IN04 tool-surface coverage.
- Owner lane: backend (Concert mural tools producer) + frontend (Wall B editor) + QA/visual.
- Evidence required: an IN04 gate check that asserts `/mural/tools` 200 for both admitted members and 403/404 for non-members; PI09 captures with PASS verdicts at 360/390/430; QA06 PI09 6 NOT_REACHABLE -> 0.

Current shortfall evidence: `task-32-in01-delta.md` limits ("`GET .../mural/tools` returns 403 for both principals throughout the run; the Wall tool surface ... remains a separate defect"); `task-31-in04-report.json` logs 21 `/mural/tools` requests but the 39 asserted checks contain no tools-status check; `task-36-qa06/README.md` PI09 `NOT_REACHABLE 6`.

### 55. Close the QA06 mobile visual acceptance residual and physical devices

- Proposed row text: `55. Close QA06 to zero NOT_REACHABLE states and capture physical iOS and Android devices`
- What to do / Must NOT do: reach each of the 111 NOT_REACHABLE IA/family states with a real producer or an explicitly approved exclusion, re-run the 206-case matrix at 360/390/430, and capture physical iOS and Android; do NOT claim acceptance from desktop Chrome touch viewports or earlier-session emulator captures.
- Closes / extends: IS-1 phone/no dead ends; plan row 36.
- Owner lane: QA/visual lane (`mcr(qa)`) with the named producers for gifts, venue, wall, auth/legal and archive.
- Evidence required: updated `task-36-qa06/report.json` with 0 NOT_REACHABLE or a user-approved exclusion list; per-state captures; physical-device model/OS/browser/DPR identifiers and captures; `physicalIOS` and `physicalAndroid` no longer NOT_RUN.

Current shortfall evidence: `task-36-qa06/README.md` (95 PASS / 111 NOT_REACHABLE / 206; `physicalIOS`/`physicalAndroid` NOT_RUN; Android emulator provenance "earlier capture session", candidate UNRECORDED).

### 56. Complete the bilateral venue friend authority and re-run VENU-IN01

- Proposed row text: `56. Implement the bilateral venue friend authority and re-run all 48 VENU-IN01 comparisons`
- What to do / Must NOT do: replace the fail-closed `PgVenueLocationAuthority::friend_in` stub with the real bilateral friendship policy, then re-run the 48 comparison cases and 10 extension IA states; do NOT approve the provisional Photo 5 variant without a user decision.
- Closes / extends: IS-1 meetup/venue reachability; plan row 28.
- Owner lane: backend venue (VENUE-FRIEND authority + L/M/N composition); frontend venue consumes it; QA/visual re-runs the matrix.
- Evidence required: `friend_in` reads real friendship rows (no 503), a real two-account venue run proves friend location, sharing audience, meeting, invitation and conversation; 48/48 cases with 0 NOT_REACHABLE; 10/10 IA PASS at 360/390/430; the Photo 5 decision recorded.

Current shortfall evidence: `task-28-venu-in01/report.json` (`PASS 23 / DIFFER 2 / NOT_REACHABLE 23`; IA `3 / 0 / 7`; `VENUE_FRIEND_V1: NOT_REACHABLE: PgVenueLocationAuthority::friend_in returns VenueLocationError::PolicyUnavailable (503)`).

### 57. Activate the reviewed character atlas in the served runtime

- Proposed row text: `57. Serve the reviewed character atlas from the production runtime and prove no fallback art is reachable`
- What to do / Must NOT do: compose the reviewed manifest v2 into the served stack so the world, profile and gift surfaces render it; do NOT fall back to legacy or synthetic character art and do NOT edit the supplied originals.
- Closes / extends: IS-1 world/profile visual fidelity; plan row 37; character-art agreement.
- Owner lane: world release/atlas owner + backend composition (served bootstrap); frontend renderer already wired.
- Evidence required: served runtime reports the reviewed manifest hash; a real-stack world/profile/gift capture shows the reviewed atlas; a scan proves no legacy/synthetic art reachable; `production activation` recorded as PASS rather than NOT_RUN.

Current shortfall evidence: `task-37-atlas.md` (`Current stack serves it: NO`; `Production activation: NOT_RUN`; manifest is `production_eligible: true`, `fallback: NONE` but only a runtime test fixture).

### 58. Compose the operator report-decision and moderation commands

- Proposed row text: `58. Compose decideConcertReport and the user/content moderation commands through the public operator surface`
- What to do / Must NOT do: classify the registered `decideConcertReport`, `moderateConcertUser` and `moderateConcertContent` routes into closed producers with operator-role and reviewed-evidence checks; do NOT let them remain unclassified `Unsupported product operation`, and do NOT weaken role evidence to make a test pass.
- Closes / extends: IS-2 reports; plan row 15 residual; GAP-3.
- Owner lane: backend (concert-service product dispatch + operations/moderation producers); gateway templates already exist.
- Evidence required: real HTTP operator decision 200 with receipt/outbox and audit row, member 403, unknown report 404, duplicate key replay byte-identical; user suspend/unsuspend 200 with audit; content moderation 200; an `every_registered_admin_route_reaches_its_producer` test covering the three routes; browser operator run at 360/390/430.

Current shortfall evidence: registry rows `commands/registry.rs:320` (`decideConcertReport`), `:326` (`moderateConcertUser`), `:332` (`moderateConcertContent`); gateway templates `command_routes.rs:94`, `:98`, `:102`; `report_admission.rs` owns only Create/Own; `operations_admission.rs` owns only GET report queue/detail; fallthrough `permission_denied("Unsupported product operation")` at `product_rpc/mod.rs:211`, `:254`; the row-15 test lists only implemented routes.

### 59. Compose the lifecycle owners so closure reaches ARCHIVED and /archive serves 200

- Proposed row text: `59. Compose FREEZE_MURAL..CLEANUP lifecycle owners so a closure reaches ARCHIVED and /archive serves permitted data`
- What to do / Must NOT do: compose the missing `LifecycleOwners` for FREEZE_MURAL, SNAPSHOT, EXPIRE_GRANTS, EXPIRE_BEACONS, EXPORT and CLEANUP (single-bridge rule, reviewed retention), so a close job advances past READ_ONLY to ARCHIVED and the PUBLIC archive projection serves permitted fields; do NOT expose private or deleted content, do NOT return 404 as a stand-in for a working archive.
- Closes / extends: IS-2 event closure; plan rows 16 and 51.
- Owner lane: backend event lifecycle (Concert lifecycle + cleanup live).
- Evidence required: live operator close -> job steps complete -> phase ARCHIVED; authenticated and anonymous `GET /archive` 200 with permitted fields only; post-closure write 409; forced cleanup fault leaves a retryable job; restart resumes pending cleanup; private/deleted content absent; task-51 gap removed.

Current shortfall evidence: `task-51-closure-producer.md` gap ("FREEZE_MURAL..CLEANUP have no composed LifecycleOwners ... /archive reads truthfully return 404 RESOURCE_UNAVAILABLE").

### 60. Add one full-suite entry point and run it from a clean clone

- Proposed row text: `60. Add a single full-suite check entry point and record a clean-clone run at the release SHA`
- What to do / Must NOT do: extend `scripts/mcr/check.sh` (or an equivalent documented script) to run the full CI gate set - plan, guards, contracts, frontend frozen install/typecheck/build/lint/Vitest, backend check plus tests, route parity and release composition; do NOT leave the test suites reachable only as CI YAML steps.
- Closes / extends: IS-3 one check suite; plan rows 6 and 48.
- Owner lane: CI/infra lane (shared scripts and workflow composition - integrator-coordinated).
- Evidence required: the script's own log on a fresh clone at the release SHA with every stage exit 0 and timings; docs point maintainers at it; a broken-probe per stage proves the entry point fails when a gate fails.

Current shortfall evidence: `scripts/mcr/check.sh` scopes (`foundation` = plan + guards + contracts + frontend build + backend build, no Vitest and no `cargo test`); `task-48-ci.md` jobs contain the test commands but were only run locally.

### 61. Wire the excluded CI gates and prove a hosted blocking run

- Proposed row text: `61. Remove the four CI exclusions and record a hosted GitHub Actions run that blocks on each gate`
- What to do / Must NOT do: reconcile the QA01 pinned inputs and the C07 compatibility pin, make the two operations-composition abort files deterministic, supply the runner roots for `notification_binding`, and run the backend job across the concert-service suites; do NOT re-add exclusions, `continue-on-error`, skipped asserts, or a reduced crate list to go green.
- Closes / extends: IS-3 CI blocks regressions; plan row 48.
- Owner lane: CI/infra lane with the owning test owners (frontend composition, contracts, notification/consumer).
- Evidence required: the four formerly excluded targets pass locally and in CI; a hosted run id/URL on a pushed candidate with guards/frontend/backend/release green; a broken-probe per job (renamed route, failing test, dirty generated file, stale receipt) each blocks; the pinned Node/pnpm/Rust/Postgres versions recorded from the run.

Current shortfall evidence: `task-48-ci.md` exclusion table (C07 pin measured red; QA01 coverage exit 1 with pinned input drift; composition abort flakes; notification roots missing) and "What could not run here" (no hosted runner; full frontend suite and 16-crate backend test job not run).

### 62. Bind the approved FE10 eligibility and legal publication

- Proposed row text: `62. Bind the approved FE10 eligibility and legal publication to signup and verification`
- What to do / Must NOT do: obtain the approved eligibility/legal/consent text and version from the user/operator, bind it to the enrollment policy and acknowledgements, and remove the dev-only publication from any acceptance path; do NOT invent text, a version or an effective date, and do NOT treat `productionApproval=false` as legal approval.
- Closes / extends: IS-4 consent; plan row 45; GAP-5.
- Owner lane: FE10 legal/eligibility owner + external publication requester; auth-service binding; frontend auth consumes it.
- Evidence required: real signup -> verification -> sign-in with the approved text and recorded policy revision; acknowledgement mismatch 422; no-policy 503 no longer observed for the approved publication; duplicate/throttle cases pass; the publication reference and hash recorded.

Current shortfall evidence: `task-45-fe10.md` (no approved text; 503 POLICY_UNAVAILABLE without publication; only a `productionApproval=false` dev publication exists).

### 63. Compose the place-note read path and drive QA04 to zero NOT_RUN

- Proposed row text: `63. Compose the N01 place-note read-retention path and re-run QA04 with zero NOT_RUN cells`
- What to do / Must NOT do: compose the prepared N01 native read-retention path so list/thread/replies return composed reads for every role, add a warm feed containing a removable blocked-author post for the cache-eviction cell, and re-run the full matrix; do NOT count a 503 as a privacy pass and do NOT replace a missing cell with a fixture.
- Closes / extends: IS-4 authorization on HTTP/caches; plan rows 21/39; QA04 remainder.
- Owner lane: backend venue N01 + feed cache composition; QA re-runs the matrix.
- Evidence required: notes list/thread/replies 200/404/403 by role with no private text; cache-control no-store and same-instance block/expiry removal; feed eviction journal before/after with the blocked post actually removed from a warm feed; QA04 110/110 cells with 0 NOT_RUN and 0 FAIL.

Current shortfall evidence: `task-39-qa04.md` NOT_RUN list (venue notes 7; feed eviction 1) and the note that the N01 prepared read-retention path is not composed (503 for every role).

### 64. Complete IN03's provider-dependent, client-cache and WS-frame checks

- Proposed row text: `64. Complete the IN03 provider, client-cache and WebSocket policy-frame checks when inputs exist`
- What to do / Must NOT do: with the authorized age-verification provider/claims, external OAuth/mail credentials and the Social policy transport keys, convert the five recorded NOT_RUN cases (four provider-dependent plus the client-cache/WS-frame case) into executed checks; do NOT claim a cache or socket fence from a backend-only gate.
- Closes / extends: IS-4 consent/authorization over caches and WS; plan row 30.
- Owner lane: external-inputs requester + backend social/age providers + frontend client-cache/WS probe.
- Evidence required: provider-signed age claims with positive and reveal/revoke cases; external OAuth and mail/SMS cases; browser client-cache invalidation after block/suspend/expiry; `policy.baseline`/`policy.invalidate` frames on the real socket; IN03 report with 0 NOT_RUN.

Current shortfall evidence: `task-30-in03.md` NOT_RUN rows (`age_provider_positive`, `social_reveal_revoke`, `external_oauth_provider`, `external_mail_or_sms`, `client_cache_and_ws_frames`) and `result_scope = PASS_WITH_RECORDED_NOT_RUN`.

### 65. Converge the live seven-service account-deletion saga

- Proposed row text: `65. Converge the live seven-service account-deletion saga and re-run QA09-DELETION-SAGA`
- What to do / Must NOT do: resolve the `Dependency unavailable` retries so CONCERT_PRIVATE_DATA/SESSIONS/SHARING/TICKETS reach terminal COMPLETE on the live composition, then re-run the QA09 deletion scenario; do NOT claim convergence from the isolated owner mount and do NOT rewrite a terminal receipt on replay.
- Closes / extends: IS-4 privacy/expiry; plan rows 14 and 41; GAP-3.
- Owner lane: account-deletion owner (Concert + Auth saga).
- Evidence required: eight saga steps terminal with attempt counts and last_error empty; zero private rows for the deleted user; a revoked session 401; restart replay idempotent with one terminal receipt; QA09-DELETION-SAGA PASS with before/after rows.

Current shortfall evidence: `task-41-qa09.md` (`QA09-DELETION-SAGA: NOT_RUN`; "CONCERT_PRIVATE_DATA/SESSIONS/SHARING/TICKETS stay RETRY attempt 3 with last_error Dependency unavailable"), `task-14-be42.md` residual serving convergence, `task-50-deletion-mount.md` (isolated mount only).

### 66. Re-run QA10 composition against the final release SHA

- Proposed row text: `66. Re-run QA10 task-to-commit composition and release checks against the final release SHA`
- What to do / Must NOT do: refresh the 51-row task trace and all receipts on the final release candidate and publish a release-check composition with no missing in-scope bindings; do NOT reuse evidence from another candidate.
- Closes / extends: IS-2/IS-3 release evidence; plan row 42.
- Owner lane: QA10 release-composition publisher.
- Evidence required: `release-check.sh --candidate <final-sha> --mode composition` with `release_decision` assessed (not INCOMPLETE), 0 unbound in-scope bindings, every artifact hashed at that SHA, and the task/IA bindings resolved or explicitly deferred.

Current shortfall evidence: `task-42-qa10.md` (candidate `25b2d356f`, `release_decision: NOT_ASSESSED`, `composition_checks: INCOMPLETE`, no same-candidate owner receipts, gates NOT_RUN/BLOCKED, all 112 task and 61 IA bindings unbound).

## 7. What this check did not do

- It did not execute any gate, browser, database or device run. Every verdict
  above is a reading of the cited artifact at its cited candidate; F3 is the
  live manual QA and F5 is the release audit.
- It did not modify the plan, the register or any evidence file other than this
  document. The F4 row stays `[ ]` until the owner of the final verification
  wave records the audit result.
- It did not upgrade a `[~]` row because its plan checkbox was `[x]`, and it did
  not downgrade a row whose evidence records a later correction than its stale
  header. The disagreements are listed in section 2.
- QA08 remains `DEFERRED_BY_USER` / `NOT_RUN`, and no capacity claim is made.
