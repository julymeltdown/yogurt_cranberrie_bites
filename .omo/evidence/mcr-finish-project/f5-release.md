# F5 - Release audit

- Task: F5 (`Release audit`) of the final verification wave.
- Candidate: `dcac6b20d05c3aff8ab5bea6516d22a1c59fc972` (`master`), worktree
  `/Users/lhs/Desktop/mcr-worktrees/exec-f5`, branch `codex/mcr-exec-f5`.
- Audit window: 2026-11-06T18:52Z-19:04Z; initial reconciliation at 18:57:45Z,
  CI state re-read through the run's failure conclusion at 19:02:46Z and
  immediately before commit.
- Machine record: `.omo/evidence/mcr-finish-project/f5-release.json`.
- Refreshed IN05 snapshot: `.omo/evidence/mcr-finish-project/task-47-in05-refresh.json`
  (generated at this candidate by the row-47 checker with the candidate and output
  paths changed; the original `task-47-in05.json` is preserved byte for byte).
- Release decision: **`NOT_RELEASE_READY`**.

This audit re-ran the mechanical plan/register checks, read every final-wave
artifact (F1-F4), and re-checked the CI workflow and its hosted run history. It
did not re-execute F3's live scenarios or F2's toolchain runs; those results are
carried with their candidates and limits.

## 1. Release decision

**`NOT_RELEASE_READY`.**

The candidate is not releasable for six independent reasons, any one of which
is sufficient:

1. Fourteen implementation rows are recorded gaps (`[~]`) with named, still-open
   work (section 7). None is a user-authorized scope cut.
2. F3's privacy scenario **FAILED**: QA04 aborted twice at
   `DELETE /api/relationships/blocks/{target_user_id}` with a 504 while the
   standalone IN03 gate returns 200 on the same route. The role x surface
   privacy matrix therefore has no completed run (section 5.3).
3. F2's F2-02 remains open: `cargo clippy` is red (one pre-existing
   deny-by-default `eq_op` error plus 26 delivery-introduced warnings) and CI
   runs no clippy step (section 5.2).
4. F4 assessed 20 ideal-state elements as 10 delivered / 10 partial / 0 fully
   missing, and **no IS row is wholly delivered**. The 10 partial elements
   produce follow-ups 52-66 (section 9).
5. The NOT_RUN inventory is material: venue reachability, mobile visual and
   physical devices, QA04 cells and WebSocket phase, the deletion saga, QA10
   release composition, QA08 capacity and the compose/restore drill (section 8).
6. The hosted `mcr-ci` run at this candidate is red: guards and release jobs
   passed, but the backend test job failed on
   `concert-application --test intro` (nanosecond vs microsecond cursor clock)
   and the frontend test job failed 33 of 5726 tests because the shallow
   checkout breaks provenance `git show` calls and the job installs no Python
   `referencing` dependency (section 6.4). These are the first hosted-only
   failures found in the final wave.

## 2. Plan-row reconciliation

Counted directly from `.omo/plans/mcr-finish-project.md` at this candidate
(every checkbox line, regex `^\s*- \[[ x~]\]`):

| Scope | `[x]` DONE | `[~]` GAP | `[ ]` OPEN | Total |
| --- | ---: | ---: | ---: | ---: |
| Rows 1-51 | 37 | 14 | 0 | 51 |
| Audit rows F1-F5 | 3 (F1, F2, F4) | 1 (F3) | 1 (F5, this audit) | 5 |
| Whole plan | **40** | **15** | **1** | **56** |

The task brief expected 41 `[x]` / 14 `[~]` and asked for verification. That
pair is **not reproducible at this candidate**. Rows 1-51 are 37/14; including
the audit rows gives 40/15/1. The likely origin of 41/14 is counting F5 as
closed (41 `[x]`) while omitting F3's `[~]`; closing F5 in the plan would give
41 `[x]` / 15 `[~]` / 0 open, still not 14 `[~]`. F3 was set to `[~]` by
`dcac6b20d` ("close F3 with the privacy-matrix gap"), so 15 gaps is the
current state. The implementation gap set remains exactly the 14 rows in
section 7.

Rows 47 and 48 are **DONE** at this candidate:

| Row | State | Delivering commit | Evidence |
| --- | --- | --- | --- |
| 47 | `[x]` | `7bbcba836` (ledger) | `task-47-in05.md` exists; ancestry PASS |
| 48 | `[x]` | `bbf9da478` (ledger) | `task-48-ci.md` exists; ancestry PASS |

All 40 closed rows (37 numeric + F1/F2/F4) pass the refreshed mechanical
evidence + ancestry check; 0 fail.

## 3. Named obligations

The definitional counts are unchanged and all PASS in the refreshed checker:

| Obligation | Required | Accounted | Status | Evidence |
| --- | ---: | ---: | --- | --- |
| Original task IDs | 112 | 112 | PASS | register has 112 unique IDs; `generated/TASKS_EN.json` has 112 originals plus 6 extension IDs (EX01-EX06) |
| Active original tasks | 111 | 111 | PASS | 112 minus the one deferred ID QA08 |
| Required IA entries (P0) | 61 | 61 | PASS | `generated/IA_EN.json` P0 set equals `functional_delivery_ia`; P1 excluded: M10, R02, X02 |
| Design families | 12 | 12 | PASS | PI01-PI12 in the functional delivery plan |
| Venue deliverables | 8 | 8 | PASS | VENU-ART01, VENU-BE-L01, VENU-FE-L01, VENU-BE-M01, VENU-FE-M01, VENU-BE-N01, VENU-FE-N01, VENU-IN01 |
| Extension IA states | 10 | 10 | PASS | owner map in `task-46-w04.md`; 10 mapped, 0 UNMAPPED, no URL added |
| Recorded implementation gaps | 14 | 14 | PASS | rows 11, 14, 16, 28, 30, 32, 35, 36, 37, 41, 42, 43, 45, 51 |

The refreshed checker's `gap_14` status is `FAIL` only because the checker
asserts that the whole 56-row plan contains exactly 14 gap rows; F3 is now a
15th (`[~]`) audit row. The implementation gap set itself is exactly the 14
rows above. This is an assertion-scope limit, not a new gap.

Completion is separate from the definitional counts and is **not** complete:

| Measure | Current | Source |
| --- | --- | --- |
| Active original tasks DONE | **65 / 111 (58.6%)**, 46 active remaining | register row-level states, recomputed by the refresh |
| Register state mix | DONE 65, IN_PROGRESS 39, OPEN 3, WAITING_INPUT 3, REOPENED 1, DEFERRED_BY_USER 1 (= 112) | `docs/mcr/task-progress-2026-11-04.json` |
| Design families | **10 / 12**; PI08 (World HUD/era) and PI11 (Map/places/rooms) open | latest plan checkpoint |
| Venue deliverables | **1 / 8**; only VENU-ART01 is DONE, seven remain | latest plan checkpoint |
| VENU-IN01 comparisons | 23 PASS / 2 DIFFER / 23 NOT_REACHABLE of 48; extension IA 3 PASS / 0 DIFFER / 7 NOT_REACHABLE of 10 | `task-28-venu-in01/report.json` |
| Extension IA states | **10 / 10 mapped, 0 UNMAPPED; 10 / 10 captures and verdicts still NOT_RUN** | `task-46-w04.md`; row 28 |

The stale plan narrative ("63/111 active DONE") predates the AR02/AR04
closures; the register now totals 65. The plan's design table still shows all
12 families as IN_PROGRESS, which contradicts the latest checkpoint's 10/12;
F5 states the checkpoint value and did not re-derive per-family acceptance.

### IN05 snapshot refresh

The original IN05 snapshot was taken at candidate `7d14251bb` and recorded
35 DONE / 14 GAP / 7 OPEN, with rows 47 and 48 still OPEN because their
delivering commits landed after that snapshot. F1 flagged this.

The refreshed snapshot at `dcac6b20d` records:

| Snapshot | Candidate | Rows 47/48 | DONE / GAP / OPEN (56 rows) | Register mix |
| --- | --- | --- | --- | --- |
| `task-47-in05.json` (historical) | `7d14251bb` | OPEN / OPEN | 35 / 14 / 7 | 65 DONE, 1 deferred |
| `task-47-in05-refresh.json` (F5) | `dcac6b20d` | DONE `7bbcba836` / DONE `bbf9da478` | 40 / 15 / 1 | 65 DONE, 1 deferred |

The refreshed decision remains `NOT_RELEASE_READY`. Its `qa08.status: PASS`
is the checker's deferral predicate (the task is correctly marked
`DEFERRED_BY_USER` with no evidence); it is not a capacity pass and F5 records
QA08 as `DEFERRED_BY_USER` / `NOT_RUN`.

## 4. QA08

**`DEFERRED_BY_USER` / `NOT_RUN`. No capacity claim is made.**

- Register state: `DEFERRED_BY_USER`; recorded state `IN_PROGRESS`; evidence `[]`.
- Scope that remains unmeasured: the 200-socket/four-room scenario, mobile
  rendering and bundle/memory ceilings, slow-network stress, reconnect storms,
  and published capacity caps.
- Reopen condition (verbatim): "A named user decision re-authorizes performance
  qualification; then re-run the QA08 capacity/soak suite (including the
  200-concurrent-user scenario) on a fresh candidate with measured reports and
  publish the result as a new task delta. Until then no capacity claim is
  permitted."
- No 200-concurrent-user result, throughput number, or capacity ceiling is
  claimed anywhere in the final wave, including F3 and F4.

## 5. Final-wave results carried into this decision

### 5.1 F1 - plan compliance (COMPLIANT-WITH-RECORDED-GAPS)

`f1-compliance.md` audited all 51 evidence paths at `33141de3f`: 51/51 present
at the plan path, 0 substitutions, 0 unaccounted row states. It ran **54
ancestry checks** (47 distinct ledger commits covering 48 rows, plus 3 for row
11 and 2 each for rows 47 and 48) with **0 failures**, and found 14/14 gap rows
name the missing piece. F5's refresh independently re-verified the 40 closed
rows at `dcac6b20d` with 0 failures.

### 5.2 F2 - code quality (NOT APPROVED as-is)

Ten findings were recorded over `9185c9922..33141de3f`. Disposition at this
candidate:

| Finding | Status at `dcac6b20d` |
| --- | --- |
| F2-01 venue note idempotency | Resolved on the shipped product path. `product_rpc/venue_product.rs` dispatches to `create_note_receipt` / `create_reply_receipt` / `withdraw_note_receipt`, which replay by key and digest; F3 measured same-key create replay 201 with the same id/revision and a changed body under the same key -> 409. F2's cited `let _ = idempotency_key` lines are the legacy `PlaceNoteUseCases` trait methods, which are not the HTTP dispatch path and are called only by tests. No post-range code change removed them. |
| F2-02 clippy | **OPEN.** `read_tracking_tests.rs:71` still compares two identical `include_str!` values (`clippy::eq_op`, deny by default); the 26 delivery warnings were not cleaned; `.github/workflows/mcr-ci.yml` has no clippy step. |
| F2-03 C07 red pin excluded from CI | Resolved by `2bf45d2e1`/`d2f8bcd36`: both pin records advanced to `2c45873e...`, provenance recorded in `docs/mcr/evidence/contracts/qa01-coverage/pin-update-2026-11-06.json`; QA01 coverage and the C07 compatibility test now run blocking (`71e54aa73`). |
| F2-04 six `node:test` suites run nowhere | **OPEN** (non-blocking quality debt; carried in NOT_RUN). |
| F2-05 `unwrap_or_default` decode masking | Open, minor. |
| F2-06 duplicate assertion | Open, cosmetic. |
| F2-07 test helpers ignore `set_config` errors | Open, minor. |
| F2-08 `#[allow(too_many_arguments)]` inconsistency | Open, minor. |
| F2-09 ignored tests justified | PASS; no unjustified skip. |
| F2-10 no scope creep | PASS. |

### 5.3 F3 - real manual QA (PASS except the privacy matrix, FAIL)

Candidate `71e54aa73`, development bundle `NOT_ADMITTED`; QA08 capacity, IN03
cohort admission and release qualification explicitly not claimed.

| Scenario | Verdict | Measured |
| --- | --- | --- |
| IS-1 two-client meetup/pickup | PASS | 15/15; capacity, version conflict, party full, sold out, pickup code, same-key replay, new-key replay 403 |
| IS-2 mural convergence/moderation | PASS | 39/39; 2 NOT_RUN sub-items (wall-clock undo expiry, device/visual) |
| IS-3 admin closure/archive | PASS at reached `READ_ONLY` | 51 true, 2 null; `/archive` 404 because the job never reaches `ARCHIVED` |
| IS-4 venue invitation/notes | PASS | 28/28; note-read replies 503 (N01 path not composed) |
| Privacy spot-check | **FAIL** | IN03 27/27 PASS; QA04 aborted |

The privacy failure is the release-blocking one. QA04 measured 96 cells
(74 PASS / 22 NOT_RUN / 0 FAIL) and then aborted twice, once on the default
gateway budget and again with `GATEWAY_REQUEST_TIMEOUT_MS=5000`, at its own
state-restore step:

```
require(unblocked[0] in (200, 204), f"qa04-restore-unblock-{unblocked[0]}")
-> Qa04Failure: qa04-restore-unblock-504
route: DELETE /api/relationships/blocks/{target_user_id}
```

The gateway logged `downstream request timed out`
(`2026-11-06T18:43:36.786475Z`; first attempt `18:41:11.236692Z`). The same
unblock route returns 200 in the standalone IN03 gate, so the 504 is specific
to the matrix's preceding seven-service state, not a global outage. Media
privacy cells (14 + 1 probe) and venue note-read cells (7) are NOT_RUN, the WS
ticket/subscription/replay phase and the final denial audit were never reached.
F3's row is correctly `[~]` with the privacy gap as its open piece.

### 5.4 F4 - ideal-state fidelity (10 delivered / 10 partial)

Four IS rows, 20 elements: **10 DELIVERED, 10 PARTIAL, 0 NOT DELIVERED; 0 IS
rows wholly delivered.** The 10 partial elements map to 15 proposed rows
(52-66). IS-3's CI element is partial both because the excluded gates remain
and because no full check-suite entry point exists; IS-4's authorization
element carries the 23 NOT_RUN QA04 cells and the unconverged deletion saga.

## 6. CI reconciliation

### 6.1 Workflow state

`.github/workflows/mcr-ci.yml` has four blocking jobs (`guards`, `frontend`,
`backend`, `release`) on push and pull_request, no `continue-on-error`, pinned
Node 24.22.2 / pnpm 11.7.0 / corepack 0.35.0 / Rust 1.88.0 / PostgreSQL 16.

After `71e54aa73` the reconciled C07 pin and QA01 coverage run as blocking
steps: the guards job runs `scripts/mcr/contracts/c07/test_inventory.py` and
`scripts/mcr/contracts/qa01-coverage/run.py`; the frontend vitest run includes
`src/shared/api/__tests__/c07-social-client/compatibility.test.ts` because the
only vitest exclusions are the two composition files below.

### 6.2 Remaining documented exclusions

| Exclusion | Exact scope | Reason |
| --- | --- | --- |
| Timing-sensitive operations-composition test 1 | `src/app/operations-composition/__tests__/composition.test.tsx`, "aborts pending settings and ignores its late completion after canonical policy replacement" | Pre-existing timing flake; suite measured 53/54 then 54/54; excluded rather than retried so a flake cannot mask a real failure |
| Timing-sensitive operations-composition test 2 | `src/app/operations-composition/__tests__/reports-production/composition.test.tsx`, "aborts a pending list read and rejects its late completion across policy change" | Same |

No exclusion is a warning, skipped assert, or `continue-on-error`; the two
files are not selected at all, and the collected-file floor is 409 (411 minus
the two excluded files).

### 6.3 Backend environment-gated exclusions

- `notification_binding` tests in `services/concert-service` need
  `NB01_OWNER_ROOT` and panic before reaching product code without it.
- The consumer probe needs `CONCERT_CONSUMER_TEST_URL`.
- Consequently the backend test job runs only 16 core crates; no
  `concert-service` suite (venue, mural, closure, social, feed) runs in CI.
- The release job treats exit 2 with `"status": "INCOMPLETE"` as a notice, not
  a failure, until a QA10 evidence manifest exists; that is a third
  non-blocking path beyond the two documented test exclusions.

### 6.4 Local command exits and hosted runs

Task 48 ran every workflow command locally and recorded exit codes in
`task-48-ci.md` (plan, guards, generators, C07 inventory, route parity,
frontend install/typecheck/lint/build/one-file vitest, `cargo check`, one core
crate test, release INCOMPLETE path, clean tree). It could not run the full
frontend suite, the 16-crate backend test job, or the release PASS path, and it
stated that no hosted runner existed at that time.

That last statement is **stale at this candidate**, and F5 records the measured
reality instead of repeating it:

- `gh run list --repo julymeltdown/mcr` shows **8 `mcr-ci` runs** total:
  7 completed `cancelled` (concurrency cancellation as master advanced) and
  1 completed `failure` (`42274582726`, head `dcac6b20d`, started
  2026-11-06T18:52:14Z, concluded 19:02:46Z).
- At 18:57:45Z run `42274582726` had `plan, guards and contract drift` =
  **success** (18:54:30Z, covering C07 inventory and QA01 coverage) and
  `QA10 release-evidence consistency` = **success** (18:53:09Z);
  `frontend install, typecheck, lint, build and test` and
  `backend check and core-crate tests` were still in progress.
- At 18:58:49Z the backend job **failed** in `Cargo test (core crates)`:

```
test authenticated_cursor_binds_request_position_and_lifetime ... FAILED
panicked at crates/concert-application/tests/intro.rs:193:5:
assertion `left == right` failed
  left:  Ok(CursorWindow { upper: 2026-11-06T18:58:49.405844Z, ... })
  right: Ok(CursorWindow { upper: 2026-11-06T18:58:49.405844563Z, ... })
```

  Cause: `cursor::encode_authenticated` serializes `timestamp_micros()` and
  `decode_authenticated` reconstructs microsecond precision, while the test
  builds `now = Utc::now()` (nanoseconds on the Linux runner) and compares the
  token round trip to the untruncated window. The same test passed in the
  macOS-local recipient-intro evidence. Product wire precision is intentional;
  the test needs `trunc_subsecs(6)` (or comparison on truncated fields). Cargo
  check succeeded before the failing test.
- At 19:02:46Z the frontend job **failed**: `Test Files 16 failed | 397 passed
  (413)`, `Tests 33 failed | 5679 passed | 14 skipped (5726)`. Typecheck,
  lint, build and the collected-file floor (413 >= 409) passed before the test
  step. The failure blocks are dominantly two wiring classes:
  (a) 26 provenance/preservation tests call `git show <historical sha>:path`
  and fail because the job checks out shallow (`actions/checkout@v4` without
  `fetch-depth: 0`) - "exists on disk, but not in '<sha>'";
  (b) 8 Python-oracle tests fail with `ModuleNotFoundError: No module named
  'referencing'` because the frontend job installs no Python contract
  dependencies. The blocks include file-level duplicates; vitest reports 33
  failed tests. Neither class executed its assertion against product behavior.
- **No `mcr-ci` run has a green conclusion on any candidate.** The hosted
  blocking-run requirement is therefore still open (follow-up 61), and the two
  hosted job failures add release blockers of their own (F5-CI-01 and F5-CI-02
  in section 7).
- The 46 successful hosted runs in the repository belong to the separate
  "MCR contract generation" workflow, not `mcr-ci`.

## 7. Gap list

Fourteen named implementation gaps plus the F3 privacy gap, F2-02, and
F5-CI-01/F5-CI-02 discovered by this audit. Owners follow the IN05 owner map
and the F2/F3/F5 findings.

| # | Gap | Owner | Status / closure condition |
| --- | --- | --- | --- |
| 11 | Media delivered; feed mutation transport gated on the shared proto envelope | Feed/Post owner | Row 49 landed the envelope and routes; row text not re-closed. Re-verify at the shell level (follow-up 53) |
| 14 | BE42 coordinator + sealed apply delivered; serving binary does not mount it | Account/Concert deletion owner | Row 50 mounted it; live seven-service convergence unproven (follow-up 65) |
| 16 | BE41 route + read-only proofs delivered; closure producer and archive endpoint unmounted | Concert lifecycle owner | Producer delivered by row 51; archive remains unreachable (follow-up 59) |
| 28 | VENU-IN01: 23/48 cases and 7/10 IA states NOT_REACHABLE while `friend_in` returns 503 | Venue Map/Friends authority owner | Bilateral friend authority + full re-run (follow-up 56); 2 DIFFER need a Photo 5 product decision |
| 30 | IN03: 27/27 executed pass; provider, reveal/revoke, venue and client-cache cases NOT_RUN | IN03 social owner + external provider/claims/mail owner | Provider inputs + venue rerun + browser/WS client (follow-up 64) |
| 32 | IN01 deltas: D2 policy frames and feed CONTENT NOT_RUN (Social transport unset, feed mode off) | World/Feed composition owner | Compose both transports (follow-up 53) |
| 35 | AR01 venue provenance inputs not supplied; three schema gaps need a contract decision | AR01 venue-provenance owner + external venue operator | External inputs; no reviewed place provenance yet |
| 36 | QA06: 95 PASS / 111 NOT_REACHABLE / 206; physical iOS/Android NOT_RUN | QA06 visual owner | Replace blocked producers, re-capture, run physical devices (follow-up 55) |
| 37 | Reviewed character atlas produced but not served; production activation NOT_RUN | World release/atlas activation owner | Serve reviewed manifest; prove no fallback reachable (follow-up 57) |
| 41 | QA09 seven scenarios pass; live deletion saga NOT_RUN (four steps RETRY with Dependency unavailable) | QA09 recovery owner | Converge the saga and re-run (follow-up 65) |
| 42 | QA10 release composition INCOMPLETE at `25b2d356f`; all 112 task and 61 IA bindings unbound | QA10 release-composition publisher | Re-run at the final SHA with owner receipts (follow-up 66) |
| 43 | FE33/FE34 closed; FE24 gift media/motif producer not composed (403/403/400, productionBinding null) | Frontend gift owner + BE24 Gateway media owner | Compose upload/finalize/read and bind motif (follow-up 52) |
| 45 | No approved FE10 eligibility/legal text; only a `productionApproval=false` dev publication | FE10 legal/eligibility owner + external approved-publication requester | External legal input; then bind and re-test (follow-up 62) |
| 51 | Closure stops at READ_ONLY because FREEZE_MURAL..CLEANUP have no composed LifecycleOwners; `/archive` 404 | Event lifecycle owner | Compose lifecycle owners; archive 200 (follow-up 59) |
| F3 | Privacy matrix FAIL: QA04 aborts twice at `DELETE /api/relationships/blocks` with 504 while IN03 standalone returns 200; 22 NOT_RUN cells, WS phase and denial audit unreached | Relationship-authority owner (concert-service block mutation) + QA04 matrix owner | Fix the seven-service unblock path or the matrix restore step, then complete QA04 to 0 FAIL / 0 NOT_RUN and re-run the denial audit |
| F2-02 | Clippy red: pre-existing `eq_op` in `read_tracking_tests.rs:71` plus 26 delivery warnings; no clippy step in CI | Backend lint owner; CI gate owner once green | Fix the test comparison and warnings, then add a blocking clippy step |
| F5-CI-01 | Hosted backend test red: `concert-application --test intro::authenticated_cursor_binds_request_position_and_lifetime` compares a nanosecond `Utc::now()` with the microsecond-truncated cursor round trip; fails on Linux, passed on macOS | BE27 intro test owner (concert-application) | Truncate the test clock to microseconds (or compare truncated fields), re-run `cargo test -p concert-application --test intro`, and record a green hosted backend job |
| F5-CI-02 | Hosted frontend test red: 33 failed tests in 16 files. 26 provenance tests fail because the shallow checkout has no historical commits for `git show <sha>:path`; 8 Python-oracle tests fail with `ModuleNotFoundError: No module named 'referencing'` | CI workflow owner (frontend job) + test owners (map/world/venue provenance and contract oracles) | Add `fetch-depth: 0` to the frontend checkout, install `scripts/mcr/requirements-contracts.txt` (or the exact oracle requirements) in the frontend job, and re-run the hosted vitest step to green |

## 8. NOT_RUN inventory

Only measured-and-unrun items are listed; nothing here is inferred as PASS.

| Area | State | Detail / blocker |
| --- | --- | --- |
| QA08 performance | DEFERRED_BY_USER / NOT_RUN | 200-socket/four-room, mobile FPS/bundle/memory, slow network, reconnect storms; no capacity claim |
| VENU-IN01 venue comparisons | 23 NOT_REACHABLE + 2 DIFFER of 48; 7 NOT_REACHABLE of 10 IA states | `friend_in` fail-closed 503; Photo 5 vs Photos 1/4 unapproved |
| QA06 mobile visual | 95 PASS / 111 NOT_REACHABLE / 206; physical iOS and Android NOT_RUN | Blocked producers listed in row 36; Android captures from an earlier session not re-run |
| IN03 social/privacy | 9 NOT_RUN items | Four provider cases, age-provider positive, reveal/revoke, comments read, blocked-comment filter seed, client-cache/WS frames |
| IN01 deltas | D2 policy frames and feed CONTENT NOT_RUN | `CONCERT_SOCIAL_MODE` unset with no probe decoder; feed product mode off; `refreshObservation` unobservable in the 900 s window |
| QA04 privacy matrix | Aborted; 22 NOT_RUN cells; WS ticket/subscription/replay and denial audit unreached | 504 at unblock state restore; media 15 cells (14 + probe), venue note reads 7, feed cache eviction 1 |
| IS-2 mural | 2 NOT_RUN sub-items | Wall-clock undo expiry (>600 s) not waited; no device/visual run |
| IS-3 closure | Archive 200 NOT_RUN | Job stops at READ_ONLY; later lifecycle owners absent |
| IS-4 venue notes | Blocked note-read denial NOT_RUN | Reads are 503 for every role (N01 retention provider absent) |
| QA09 deletion saga | NOT_RUN | CONCERT_PRIVATE_DATA/SESSIONS/SHARING/TICKETS RETRY attempt 3, `Dependency unavailable` |
| QA10 release composition | NOT_ASSESSED / INCOMPLETE | No same-candidate receipts; gates NOT_RUN/BLOCKED; 112 task and 61 IA bindings unbound |
| CI | Hosted `mcr-ci` red on both test jobs; full frontend suite and 16-crate backend test job unrun locally | Run `42274582726`: guards/release success, backend failed on the intro cursor test (F5-CI-01), frontend failed 33/5726 tests on shallow-history and missing Python `referencing` (F5-CI-02); backend job covers 16 crates only; release PASS path unexecuted |
| F2-04 qualifier suites | 6 `node:test` files run nowhere | Excluded from vitest and wired to no other runner |
| Compose/deployment | Compose execution and restore drill NOT_RUN | No Docker daemon on the host |
| Register-owned gates | IN02 OPEN, IN03 WAITING_INPUT, IN04 OPEN, IN05 OPEN, AR01/FE10 WAITING_INPUT | No completed integration/release gate established by current receipts |

## 9. Follow-up backlog (52-66)

F4's 15 proposed rows remain the backlog; F5 adds no new row numbers. F5 maps
them to this audit's gaps:

| Row | Title | Gap link |
| ---: | --- | --- |
| 52 | Compose and authorize the BE24 gift media and motif producer end to end | 43, F3 media cells |
| 53 | Prove authorized feed content in the shipped shell | 11, 32, IS-1 feed |
| 54 | Prove the Wall B editor tools surface and close PI09 | 36, IS-1 wall |
| 55 | Close the QA06 mobile visual acceptance residual and physical devices | 36 |
| 56 | Complete the bilateral venue friend authority and re-run VENU-IN01 | 28 |
| 57 | Activate the reviewed character atlas in the served runtime | 37 |
| 58 | Compose the operator report-decision and moderation commands | IS-2 reports |
| 59 | Compose the lifecycle owners so closure reaches ARCHIVED and `/archive` serves 200 | 16, 51 |
| 60 | Add one full-suite entry point and run it from a clean clone | IS-3 |
| 61 | Wire the excluded CI gates and prove a hosted blocking run | CI exclusions, backend env gates, F2-02/F2-04, F5-CI-01, F5-CI-02 |
| 62 | Bind the approved FE10 eligibility and legal publication | 45 |
| 63 | Compose the place-note read path and drive QA04 to zero NOT_RUN | 30, 43, F3 privacy |
| 64 | Complete IN03's provider-dependent, client-cache and WS-frame checks | 30 |
| 65 | Converge the live seven-service account-deletion saga | 14, 41 |
| 66 | Re-run QA10 composition against the final release SHA | 42 |

## 10. Reconciliation limits

1. The expected 41 `[x]` / 14 `[~]` count was not reproducible; the measured
   values are 37/14 for rows 1-51 and 40/15/1 for all 56 rows (section 2).
2. F2-01 is resolved on the product dispatch path, not by a post-range code
   change. The legacy `PlaceNoteUseCases` implementations still discard the
   idempotency key; they are not reachable from the product HTTP dispatch.
3. The plan's design-family table still lists all 12 families as IN_PROGRESS;
   `Design 10/12` comes from the latest responsible-owner checkpoint. F5 did
   not re-derive per-family acceptance.
4. No hosted `mcr-ci` run is green. The run at this candidate completed
   `failure`: guards and release passed, while the backend and frontend test
   jobs failed for the portability/wiring reasons in F5-CI-01 and F5-CI-02.
   This differs from task 48's "no hosted runner" statement, which was true
   when written.
5. F5 did not re-execute F2's clippy/typecheck runs, F3's scenarios, F4's
   documentary mapping, QA10, the compose/restore drill, or QA08. Their
   recorded candidates and limitations are carried above.
6. The refreshed IN05 checker's `qa08.status: PASS` is a deferral predicate;
   QA08 itself remains `DEFERRED_BY_USER` / `NOT_RUN`.
7. The hosted job failures (F5-CI-01, F5-CI-02) were discovered by this audit,
   after F2's local review and task 48's local command table. F5-CI-01 is a
   test-portability defect; F5-CI-02 is CI wiring (shallow checkout and missing
   Python oracle dependency). Neither changes product behavior, but both keep
   CI red until fixed.
8. The hosted frontend run executed 33 failing tests whose assertions never
   reached product behavior because of the wiring faults. F5 therefore makes no
   positive product claim from the 5679 passing tests either.

## 11. Decision rationale

`NOT_RELEASE_READY` follows from the recorded evidence: 14 named implementation
gaps, a FAILED privacy matrix with an unreached WebSocket/denial phase, an open
clippy gate, zero wholly delivered ideal-state rows, a substantial NOT_RUN
inventory, and no completed green CI run. No recorded evidence supports a
release claim, and no user decision in this delivery authorizes a scope cut
beyond the QA08 deferral.
