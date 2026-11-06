# F2 - Code quality review

- Final verification task: F2 (`.omo/plans/mcr-finish-project.md`).
- Delivery range reviewed: `9185c9922` (parent of the earliest W0 commit `8e5cc757f`) through `33141de3f` on `master`; 214 commits, 1828 changed entries (`A 1478 / M 340 / D 1 / R 9`, 152,015 insertions, 1,801 deletions).
- Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-f2`, branch `codex/mcr-exec-f2`, HEAD `33141de3f`, clean before and after the review.
- Tools: Node `v26.7.0`, TypeScript `6.0.3` (`tsc -b`), oxlint `1.72.0`, Rust `1.88.0` (pinned toolchain) with `cargo-clippy 0.1.88`, Python 3 guards.
- Verdict: **NOT APPROVED as-is**. No hidden skipped/disabled tests or suppressed compiler errors were found, but `cargo clippy --all-targets -- -D warnings` is not green (1 pre-existing deny-by-default error plus 26 delivery-introduced warnings), one accepted idempotency gap contradicts the IS-4 server-side idempotency row, and one red test is excluded from CI instead of being reconciled. Details and verdicts below.

## 1. Diagnostics

| Tool | Scope | Exit | Errors | Warnings | Introduced by this delivery | Pre-existing |
| --- | --- | --- | --- | --- | --- | --- |
| `tsc -b --pretty false` (`pnpm typecheck`) | whole frontend project | 0 | 0 | 0 | 0 | 0 |
| `oxlint src vite.config.ts` (`pnpm lint`) | whole frontend | 0 | 0 | 36 | 12 | 24 |
| `cargo clippy <12 touched packages> --all-targets` (JSON) | touched crates + all workspace dependents | 101 | 1 | 335 raw / 197 unique | 26 unique | 171 unique (38 in changed files on old lines, 133 in untouched files) |
| `cargo clippy ... --all-targets --keep-going -- -D warnings` | same | 101 | 18 distinct denials in 6 dependency crates before dependents were skipped | - | see below | - |
| `scripts/mcr/check-plan.py` | plan table | 0 | 0 | 0 | - | - |
| `python3 -m unittest scripts/mcr/tests/test_boundaries.py` + `node --test scripts/mcr/tests/ui-origin.test.mjs` | architecture/boundary guards | 0 | 0 | 0 | - | - |
| `generate-openapi.py --check`, `generate-mcr-domain.py --check`, `validate-domain.py` | generated contract drift | 0 | 0 | 0 | - | - |

Notes:

- TypeScript LSP (`lsp_diagnostics` on `frontend/src/app/WorldShell.tsx`) returned no diagnostics; the project-wide `tsc -b` run is the authoritative TS diagnostic source and is clean.
- Clippy covered all 12 touched packages and their dependents: `concert-service` (205 messages), `api-gateway` (111), `auth-service` (56), `post-service` (30), `concert-domain` (20), `concert-application` (37), `platform` (18), etc.

### 1.1 Deny-by-default clippy error (pre-existing, not introduced)

```
services/post-service/src/concert_feed/read_tracking.rs:1 clippy::eq_op
  triggered at services/post-service/src/concert_feed/read_tracking_tests.rs:71
  assert_eq!(include_str!("read_tracking.sql"),
             include_str!("../../migrations/0032_concert_feed_read_tracking.sql"));
```

Both files are byte-identical (`sha256 96b5477f...4fc4` at base and at HEAD), so clippy constant-folds the two `include_str!` values and flags `eq_op` (deny by default). The test file is unchanged by this delivery (`git diff 9185c9922 33141de3f` shows no change), therefore **pre-existing**. It makes any `cargo clippy` run fail even without `-D warnings`. Fix: compare a runtime-loaded digest (or drop the duplicated SQL file), not two compile-time identical literals.

### 1.2 `-D warnings` gate

`-D warnings` turns every warning into an error. With `--keep-going`, the run exited 101 after 18 distinct denials in `notification-application` (3), `post-application` (4 distinct / 5 messages), `concert-domain` (4), `profile-service` (4), `auth-application` (2), and `gateway-application` (1); the codes are `too_many_arguments`, `large_enum_variant`, `nonminimal_bool`, `manual_range_contains` and `upper_case_acronyms`; all 18 sit on pre-existing lines (17 untouched files plus one old line in a changed file), so the deny run exposes inherited lint debt only. Dependent crates (`concert-service`, `api-gateway`) were then skipped. The full non-denying run shows the complete universe: 197 unique warnings (26 introduced) plus the `eq_op` error. CI does **not** run clippy (`cargo check --workspace --all-targets` and `cargo test` only), so this is quality debt rather than a CI red.

### 1.3 Warnings introduced by this delivery (26 unique)

| File:line | Lint | Note |
| --- | --- | --- |
| `services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs:843` | clippy::useless_format | |
| `services/api-gateway/src/adapters/in/http/oauth_routes/form_post.rs:40` | clippy::uninlined_format_args | |
| `services/api-gateway/src/concert_participant_client/mod.rs:13` | unused_imports | `uuid::Uuid` |
| `services/api-gateway/tests/oauth_form_post_browser.rs:141` | clippy::uninlined_format_args | |
| `services/api-gateway/tests/unit/http_support.rs:85` | dead_code | 3 unused helpers |
| `services/concert-service/src/adapters/out/public_profile/mod.rs:354` | dead_code | `dependency_error_status` unused |
| `services/concert-service/src/adapters/out/relationship_authority/mod.rs:122` | dead_code | `with_query` unused |
| `services/concert-service/src/config/report_runtime.rs:142` | clippy::field_reassign_with_default | |
| `services/concert-service/src/config/world_runtime.rs:360` | clippy::too_many_arguments | 8 args |
| `services/concert-service/src/config/world_runtime.rs:808` | unused_assignments | `last` never read |
| `services/concert-service/src/meeting/account_deletion.rs:27` | dead_code | `writer_users` unused |
| `services/concert-service/src/meeting/account_deletion.rs:128` | clippy::too_many_arguments | 8 args |
| `services/concert-service/src/mural_tools_b/admin.rs:591` | clippy::needless_question_mark | |
| `services/concert-service/src/product_rpc/venue_admission.rs:617` | clippy::useless_format | |
| `services/concert-service/src/product_rpc/venue_meeting_admission.rs:408` | unused_variables | `expected` |
| `services/concert-service/src/product_rpc/venue_product.rs:16` | unused_imports | `Direction`, `VenueLocationAuthority` |
| `services/concert-service/src/product_rpc/venue_product.rs:32` | unused_imports | `MeetingSelection` |
| `services/concert-service/src/product_rpc/venue_product.rs:254` | dead_code | `reviewed` unused |
| `services/concert-service/src/product_rpc/venue_product.rs:537` | unused_variables | `context` |
| `services/concert-service/src/venue_meeting/mod.rs:145` | clippy::manual_range_contains | |
| `services/concert-service/src/venue_notes/mod.rs:47` | clippy::too_many_arguments | 8 args |
| `services/concert-service/src/venue_notes/mod.rs:466` | clippy::too_many_arguments | 8 args |
| `services/concert-service/src/venue_notes/mod.rs:591` | clippy::too_many_arguments | 8 args |
| `services/post-service/src/concert_feed/command_admission.rs:16` | dead_code | unused fields |
| `services/post-service/src/concert_feed/command_runtime.rs:41` | unused_mut | |
| `services/post-service/src/concert_feed/command_runtime.rs:73` | unused_mut | |

Verdict per group: dead_code / unused_imports / unused_variables / unused_mut are genuine cleanup defects (low risk, must be removed before a `-D warnings` gate can be adopted). `too_many_arguments` are trait/transaction seams; the new `#[allow(clippy::too_many_arguments)]` on `venue_meeting::record` suppresses one of them instead of matching the surrounding `#[allow]` policy, so it is inconsistent (minor).

Pre-existing warning mass (171 unique) is concentrated in `api-gateway/tests/unit/http_support.rs` (30), `world/admission_bridge.rs` (11), `relationship_authority/world_projection.rs` (10), `product_rpc/venue_product.rs` (8), plus 133 untouched-file warnings. These are inherited, not introduced; they still block a repository-wide `-D warnings` gate.

## 2. Skip / disabled-test and suppression audit

Search scope: added lines of `9185c9922..33141de3f` plus whole-tree counts for `.skip`, `.todo`, `xit`, `it.skip`, `.only`, `fdescribe`, `fit`, `@ts-ignore`, `@ts-expect-error`, `eslint-disable`, `oxlint-disable`, `#[ignore]`, `#[allow]`, `unwrap_or_default()`, `let _ =`.

| Pattern | Added in delivery | Per-pattern verdict |
| --- | --- | --- |
| `#[ignore]` | 21 attributes on 21 newly added tests in 10 files | Justified with a named reason on every attribute; all attach to test functions **added in the same hunk** (no previously-enabled test was disabled). Execution evidence exists for each group (see below). |
| `.skip(`, `it.skip`, `xit(`, `.todo(`, `.only(`, `fdescribe(`, `fit(` | 0 | PASS |
| `@ts-ignore` / `@ts-expect-error` | 0 added (9 pre-existing) | PASS. Pre-existing sites are test-only JS-oracle imports with explanatory comments. |
| `eslint-disable` / `oxlint-disable` | 0 added (8 pre-existing) | PASS. Pre-existing `react-hooks/exhaustive-deps` sites; oxlint is the actual linter. |
| `#[allow(clippy::...)]` | 2 attributes, both in `venue_meeting/mod.rs` | Minor: suppression of new `too_many_arguments` on `record(...)`; restructuring or a documented seam exemption is preferable. |
| `unwrap_or_default()` | 9 added | 8 benign `Option` defaults (OAuth query/URI parsing, bucket default, test timing); 1 defect: `service_policy/mod.rs:218` applies `unwrap_or_default()` to a `Result` row read, silently replacing a decode error with a default `publication_state` (see F2-05). |
| `let _ =` | 33 added (incl. docs/comments) | Mostly teardown/shutdown (`tokio::time::timeout`, `stop.send`, `axum::serve`, `pool.close`), `OnceLock::set` (single-owner rule), `until()` return value (`until` panics on failure, so nothing is lost), and `writeln!` into `String`. Two exceptions: (a) the 3 venue-note idempotency-key discards (F2-01, documented gap); (b) 4 `#[cfg(test)] probe_visible_in` helpers discard the `set_config` error (F2-07, minor). |

### 2.1 `#[ignore]` groups and their execution evidence

| File (tests) | Ignore reason | Executed by |
| --- | --- | --- |
| `tests/qa05_realtime_relay.rs` (6) | `qa05_realtime_suite.py` marker-owned PostgreSQL | `backend/scripts/mcr/qa05_realtime_suite.py` (`--ignored`) |
| `tests/qa03_races_support/gifts.rs` (3) | owned QA03 controller / restart phase | `backend/scripts/mcr/qa03_races/qualify.py` (`--ignored`) |
| `tests/event_lifecycle.rs`, `tests/event_lifecycle_archive.rs` (3) | `event_lifecycle_pg.py` owned fixture | `backend/scripts/mcr/event_lifecycle_pg.py` |
| `tests/account_deletion_runtime.rs` (1) | fresh PG16/PKI + restart | `backend/scripts/mcr/account_deletion_mount.py`, `account_deletion_smoke.py` |
| `tests/authorization/qa04_matrix.rs` (1) | seven-service stack + owned PG16 | `backend/scripts/mcr/in03_social_gate.py` (`qa04_matrix.py` spec) |
| `tests/private_data_production_native.rs` (1) | marker-owned PostgreSQL fixture | task-14 evidence (`.omo/evidence/mcr-finish-project/task-14-be42.md`) |
| `tests/social_live_support/read_order.rs` (2) | owned native database connection | task-30 evidence (`.omo/evidence/mcr-finish-project/task-30-in03.md`, manual `--ignored --exact` command) |
| `tests/world_runtime_support/postgres.rs` (5 in file, 1 added) | `world_runtime_support/bridge_native.py` fixture | task-10 evidence (`task-10-admission.md`, all five listed `... ok`) |
| `tests/be03_oauth_boundary.rs` (3) | qualified candidate / provider / controller | `backend/scripts/mcr/be03_original_completion.py` (`--ignored`) |

Verdict: **no unjustified skip**. The weekly cost is that none of these runs in CI (they need runner-owned databases/stacks); that is a coverage-wiring limitation recorded here, not a hidden skip.

### 2.2 Frontend test selection

- `frontend/vitest.config.ts` (W0 commit `8e5cc757f`) excludes 6 `node:test`-style qualifier files (`src/**/qualifier.test.ts`, `map-facade-derived-v3/foundation.test.ts`) that vitest cannot execute (`import test from 'node:test'`, pinned Node 24.22.2). They are not wired to any other runner, so 6 qualifier suites currently run nowhere. Verdict: **defect F2-04** (coverage gap), not a silent pass.
- CI additionally excludes 3 files at runtime: the C07 social-client pin test (its recorded manifest still names the pre-move generated registry hash) and two operations-composition abort tests (documented flake: 53/54 then 54/54). The flake exclusion is justified and documented; the C07 exclusion is a **red test hidden from CI instead of being reconciled** (defect F2-03). The collected-file floor (408) prevents silent mass drops.

## 3. Test-weakening review

- `crates/concert-application/tests/commands.rs`: route-count assertion updated 62 -> 69, but the line is duplicated (`assert_eq!(OPERATIONS.len(), 69);` twice). Defect F2-06 (cosmetic, no coverage loss).
- `backend/scripts/mcr/test_world_admission_smoke.py`: exact last-migration pin replaced by `0005` presence plus `len > 5`. The assertion's stated purpose is migration *reservation*, which survives; the exact ordering pin is loosened (note only, since the ledger legitimately grew past 0005).
- `api-gateway/tests/integration/gateway_profile_route_matrix.rs`: oversized avatar now expects `413 PAYLOAD_TOO_LARGE` instead of `400`, with the reason-body assertion retained and the new `oversized_ticket_body_is_413_before_any_owner_read` test adding owner-call assertions. Strengthened.
- `api-gateway/tests/support/concert_world_http.rs`: shared `json()` helper now embeds the response body in the status assertion. Strengthened.
- `api-gateway/tests/unit/http_auth_routes.rs`: cookie equality softened to exact prefix `oauth_state_x=v1.` (version-aware), with wrong-cookie cases still asserted. Acceptable.
- `frontend/src/app/__tests__/routes.test.ts`: literal `panelRoutes.length === 41` / `adminPaths.length === 9` replaced by IA-derived set equality (`new Set(publicPaths) == screens...`, every required IA path asserted). Strengthened.
- No removed assertion was found that has no replacement in its file or an equivalent strengthened assertion elsewhere; the remaining "dropped" candidates were reformatting, helper renames, or art-hash repins (`photo2-*`: message/'Map resource unavailable' and pinned digest updated with the art revision).

## 4. Scope-creep review

- Lane boundary: exactly one non-merge commit touches both `backend/` and `frontend/` (`0a08d55a5 docs(venue): freeze location, invitation and notes policy`). It changes the venue contract family (`backend/docs/api/paradise-city-v1/*`, `docs/mcr/contracts/paradise-city-v1/*`) plus the central generated registry `frontend/src/shared/api/domain-operations.generated.ts`. Both are recorded single-writer scopes: the venue amendment assigns the backend contract families to the venue contract owner (`docs/mcr/venue-feature-amendment-2026-11-05.md:64`) and `docs/mcr/scrum/world.md:1374` grants only generated-registry regeneration via the existing generator. The drift check passes, so the generated change is mechanical. Verdict: justified, no creep.
- Shared contracts: `backend/proto/concert/v1/product.proto` (+16), `backend/proto/post/v1/concert_feed.proto` (+41), `backend/proto/relationship/v1/relationship.proto` (+5) are **additions only** (0 deletions) and are authorized by their delivering plan rows (49 mutation envelope, 51 closure/archive, bilateral friend authority). No lockfile change accompanies them (`backend/Cargo.lock` unchanged).
- Manifests/lockfiles: no root manifest, workspace manifest, `Cargo.lock`, `pnpm-lock.yaml` or `pnpm-workspace.yaml` edits. `backend/services/concert-service/Cargo.toml` adds the already-declared workspace dependency `tower` (commit `40f85ad5f`); acceptable, but it is a service-level manifest edit with no separate transfer note.
- Root composition: `frontend/vite.config.ts` (+ virtual venue-art plugin) and `frontend/vitest.config.ts` (setup + exclusions) were changed by the planned W0 build-restoration task; `.github/workflows/mcr-contracts.yml` -> `mcr-ci.yml` is planned task 48. `infra/mcr/*` is planned task 13. No unplanned root composition change found.
- Process artifacts: `.omo/boulder.json` (agent run state, includes session ids), `.omo/plans/mcr-finish-project.md`, `.omo/ulw-execute/ledger.jsonl`, and 1,294 evidence files are committed as part of the delivery. They do not affect the product; note only that `boulder.json` is tooling state and may be excluded from a release artifact.
- Cross-directory imports were checked by the boundary guard (`test_boundaries.py`, exit 0): no frontend import of backend implementation and no backend dependency on frontend rendering.

## 5. Findings and verdicts

| ID | Finding | Origin | Verdict |
| --- | --- | --- | --- |
| F2-01 | Venue place-note create/reply/withdraw accept an `idempotency_key` and discard it (`venue_notes/mod.rs:913,1103,1176`); no receipt table exists, so a duplicate submit creates a duplicate note. Documented as a gap in `task-21-venu-n01.md:116-124`, but the plan's IS-4 row states idempotency is enforced server-side. | Introduced (documented gap) | **Must fix before release** or obtain an explicit recorded scope decision. Not a hidden defect. |
| F2-02 | `cargo clippy --all-targets -- -D warnings` fails: 1 pre-existing deny-by-default `eq_op` error + 197 unique warnings, 26 introduced by this delivery. CI does not run clippy. | 1 pre-existing error; 26 introduced warnings | **Fix before adopting a clippy gate**; the eq_op is a one-line test fix, the 26 introduced warnings are cleanup. Not CI-blocking today. |
| F2-03 | C07 social-client pin test is excluded from CI because its recorded manifest hash is stale (generated registry moved). A red test is hidden rather than reconciled. | Pre-existing record drift, surfaced by this delivery | **Must reconcile before release** (evidence-consistency debt; hand to F5 release audit). |
| F2-04 | 6 `node:test` qualifier suites are excluded from vitest and wired to no other runner, so they never execute in CI or any script. | W0 build restoration | **Should fix**: add a `node --test` step or delete the dead suites; do not leave them silently unrun. |
| F2-05 | `service_policy/mod.rs:218` reads `publication_state` with `try_get(...).unwrap_or_default()`, silently turning a DB decode error into a default state; adjacent fields use `.ok().flatten()`. | Introduced | **Minor defect**: use `map_err(read_error)?` as `policy_revision` already does. |
| F2-06 | Duplicate identical assertion in `crates/concert-application/tests/commands.rs:31-32`. | Introduced | Cosmetic; remove one line. |
| F2-07 | 4 `#[cfg(test)] probe_visible_in` helpers ignore the `set_config` result; a failed authority grant silently makes the probe's observation meaningless. 3 production `OnceLock::set` / 1 teardown `pool.close` ignores are intentional. | Introduced | Minor; make the test helper fail loudly. |
| F2-08 | New `#[allow(clippy::too_many_arguments)]` on `venue_meeting::record` suppresses a warning inconsistent with the rest of the crate. | Introduced | Minor; restructure or document the seam exemption. |
| F2-09 | 21 `#[ignore]` attributes on 21 newly added tests across 10 files; all attach to newly added tests with named fixtures and all have recorded execution evidence. No `.skip`/`xit`/`.todo`/`.only`/`@ts-ignore`/`eslint-disable` additions. | Introduced, justified | PASS |
| F2-10 | Contract/manifest/lockfile edits: proto additions only; no root manifest or lockfile change; the one cross-lane commit and the generated registry change are covered by recorded single-writer grants; guards and contract drift checks pass. | Delivery-wide | PASS (no scope creep) |

## 6. Commands run (exit codes)

```
pnpm install --frozen-lockfile                                  # 0
pnpm typecheck        (tsc -b --pretty false)                   # 0
pnpm lint             (oxlint src vite.config.ts)               # 0, 36 warnings
rustup run 1.88.0 cargo clippy -p concert-service -p api-gateway -p post-service \
  -p concert-application -p concert-domain -p platform -p auth-service \
  -p friend-service -p friend-application -p gateway-application \
  -p migration-runtime -p test-support --all-targets --message-format=json   # 101
rustup run 1.88.0 cargo clippy <same> --all-targets --keep-going -- -D warnings  # 101
python3 scripts/mcr/check-plan.py                               # 0
python3 -B -m unittest discover -s scripts/mcr/tests -p test_boundaries.py   # 0
node --test scripts/mcr/tests/ui-origin.test.mjs                # 0
python3 backend/scripts/docs/generate-openapi.py --check        # 0
python3 backend/scripts/docs/generate-mcr-domain.py --check     # 0
python3 scripts/mcr/validate-domain.py                          # 0
```

## 7. Final verdict

F2 does **not** approve the candidate. Blocking items: F2-01 (server-side idempotency for venue notes contradicts IS-4 unless a recorded scope decision says otherwise) and F2-03 (a red evidence-pin test excluded from CI). Quality-gate items that should be fixed but do not change delivered behavior: F2-02 (clippy error + 26 introduced warnings), F2-04 (6 unrun qualifier suites), F2-05/F2-06/F2-07/F2-08 (minor). No unjustified skipped tests, no suppressed TS errors, and no undisclosed scope creep were found.
