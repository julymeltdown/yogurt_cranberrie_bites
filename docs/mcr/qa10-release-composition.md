# QA10 same-candidate evidence composition

Status: **IN_PROGRESS** for original QA10. The checker implementation was published
as `6d351502bcee839e0836955edbf5eff61ab9b0b7`; its actual-candidate
composition check returned **INCOMPLETE**. The committed report records the
remaining release-input gaps without changing owner task dispositions. This implements the functional remainder
**QA10-COMPOSITION-01**, not a release decision or a new acceptance matrix.
Original QA10 and IN05 remain open until their actual responsible-owner evidence
exists. QA08 remains **DEFERRED_BY_USER**, nonblocking and uncompleted; no capacity
or performance claim follows from this checker.

## Ownership and provenance

Root1fc explicitly grants new `scripts/mcr/release_checks/composition/**`, this
document and `docs/mcr/evidence/release/qa10-composition/**`. Its exact shared
grant is the additive composition mode in `release_checks/check.py` and, only if
needed, the shell entrypoint and existing entrypoint regression. Root stopped
those composition hunks. No application, contracts, registry, central ledger,
CI, manifest, lockfile, historical receipt or frontend checker is changed.
Root is the named publishing developer, without a manager review/ACK stage.

The retained exclusive checkout is `be-world-projection-context`, on
`codex/mcr-qa-QA10-release-wrapper`, based at
`ba314a2c96639b05b6c15c1ead57353719004ffa`. Its historical Profile branch and
unfinished BLOCKED goal are preserved. The goal is incompatible; no reset,
false completion, new goal or new checkout was used to disguise that fact.
Staffing explicitly authorizes this implementation despite that administrative
goal mismatch and confirms no replacement writer or runtime lease.

New Python code composes the published source consumer unchanged. Reused helpers
are the repository/path/hash/pin checks, Node TAP and Vitest readers, frontend-v1
adapter and source evaluator in `scripts/mcr/release_checks/check.py` at the base.
The existing frontend producer remains `mcr.qa10.frontend.v1`; its historical
`980fc4d8` receipt qualifies that candidate only. No receipt is rewritten or
upgraded. `bfddd3d7` changes World visibility documentation and is not QA10
implementation evidence. Original catalogs come from candidate Git blobs at
`generated/TASKS_EN.json` and `generated/IA_EN.json`: 112 required tasks and61
required IA entries. Optional rows stay outside current functional acceptance.

## Invocation and result

```sh
# Original behavior remains the default; explicit --mode source is equivalent.
scripts/mcr/release-check.sh --candidate FULL_HEAD_SHA --manifest SOURCE_INPUT

# New offline composition. Never executes commands contained in a receipt.
scripts/mcr/release-check.sh --mode composition --candidate FULL_HEAD_SHA \
  --manifest docs/mcr/evidence/release/qa10-composition/CANDIDATE/input.json
```

The shell already forwards arguments, so it needs no duplicate implementation.
There is no dependency install, application build, browser/device action,
PostgreSQL connection, network request, provider access or ledger update.
Stdout is a JSON report; no output file is written. Exit0 means all supplied
functional evidence bindings are consistent, not that the checker independently
accepted the product. Exit2 means missing/incomplete evidence. Exit1 means
contradictory, stale, malformed or tampered input. `release_decision` is always
`NOT_ASSESSED`; IN05 belongs to its responsible release implementation owner.

Without an input the output is NOT_RUN/INCOMPLETE with all112 tasks and61 IA
missing. Pending records remain explicit NOT_RUN/BLOCKED, rather than becoming
PASS through a zero command exit. The source-v1 evaluator retains its deliberate
PARTIAL behavior and live-gate placeholders; composition consumes only its
bounded `source_checks` result and supplies its own actual evidence bindings.

## Input protocol

The closed envelope is `mcr.qa10.release-composition.v1`, with exactly:

- `schema`, exact `candidate{commit,tree,dirty:false}` and `source_manifest`.
  The latter is a `{path,sha256}` reference to an existing source-v1 manifest,
  or null to preserve a missing-source gap. Source checks must pass, including
  retained production bytes and mock/recognizable-secret exclusion.
- `receipts`: explicit pinned frontend-v1 envelopes or new composition-evidence
  envelopes. Nothing is discovered by running a recorded command.
- `tasks`, `ia`, `gates`: responsible-owner records referring to the original
  IDs, source commits/paths and verified receipt IDs. These are input bindings,
  not a new progress/acceptance register.
- `deferrals`: the single allowed QA08 deferral described below, or an empty
  array that leaves the required deferral evidence NOT_RUN.

Every record in tasks/ia/gates has exactly
`{id,status,owner,reason,commit,paths,receipt_ids}`. Status is ACCEPTED, NOT_RUN,
BLOCKED, IN_PROGRESS, INTEGRATED or DEFERRED_BY_USER. Only ACCEPTED produces an
owner-recorded binding. Missing rows remain gaps. Every accepted path is present
in a qualified same-candidate receipt and has identical Git blobs at its source
commit and candidate; mere ancestry does not qualify changed code. Task paths
must fit original ownership aliases. IA paths must fit their catalog task links,
and those linked task outcomes must be present. Unknown/duplicate IDs and extra
scope cuts are rejected. Every original ID stays visible, including QA10 and
IN05; this consumer does not fabricate their outcome.

Deferral is exactly `{task_id:"QA08",status:"DEFERRED_BY_USER",blocking:false,
authority:{path,git_blob,sha256}}`. Authority must be the candidate-pinned
`docs/mcr/functional-delivery-plan.md`, retaining its explicit QA08 deferral and
111 active-task amendment. QA08 task/gate records must remain deferred, never
ACCEPTED/PASS. Any other deferred task fails. Both the task row and gate remain
visible. This preserves the actual user decision without allowing an arbitrary
manifest flag to cut required scope.

## Evidence formats and runtime boundaries

New `mcr.qa10.composition-evidence.v1` PASS envelopes have exactly:

```text
schema, id, candidate, same_candidate_end, status, owner,
category, level, command, source_pins, test_files, log, report, context, coverage
```

Category is backend/frontend/service/device/operator; level is
unit/native/live/device/operator. Service cannot be relabeled unit; physical
device/operator categories require matching levels. Each category needs a
nonempty qualified contribution. `command` retains real argv/cwd, attempted=true,
integer exit0 and no signal. Logs/reports are nonempty pinned artifacts. Every
test file is source-pinned and explicitly dispatched. Supported report formats:

`coverage` has exactly task_ids, ia_ids and gate_ids, restricted to the original
catalogs. An accepted binding must be explicitly covered by its immutable owner
report; a manifest label alone cannot reuse an unrelated report. IA bindings
also need live/device level. A frontend-v1 receipt is required as a bounded
contribution; its PARTIAL source result alone grants no task/IA/gate acceptance.

Supported report formats:

- `tap` and `vitest-json` reuse source-v1 validation and can qualify unit level
  only. The published frontend-v1 adapter also qualifies only bounded frontend
  checks, regardless of a producer's claimed live outcome.
- `rust-libtest` reads one integration-target Rust test output. It requires
  `cargo test --test TARGET`, the pinned matching `.rs` test source, named test
  points, one running count, one consistent result summary and nonzero passes.
  Empty/fabricated footer counts, failures, measured benchmarks or duplicate
  case names fail. Ignored cases remain incomplete.
- `playwright-json` verifies exact dispatched source files, nested actual test
  results, suite statistics, nonempty passing cases and no retries, flakes or
  expected failures. Skips remain incomplete; browser execution alone does not
  qualify physical devices or actual services.
- `python-unittest` consumes verbose named test points, one actual running
  summary and consistent OK/skip counts, with explicitly dispatched pinned test
  modules. It qualifies unit level only; a footer without cases is refused.
- `observations-json` is an explicit owner adapter for existing service/device/
  operator procedures. Its closed `mcr.qa10.observations.v1` object repeats the
  exact candidate and actual command, and contains nonempty named
  `cases[{id,status,artifacts}]`. Every case has retained pinned observations;
  status is PASS or NOT_RUN. It is an owner execution record, not execution or
  trusted attestation by this checker. Unit output cannot use this as a live
  shortcut without the additional actual-context bindings.

Pending envelopes contain exactly `{schema,id,candidate,status,owner,reason}`,
with NOT_RUN/BLOCKED and the same current candidate. They never supply proof to
an accepted task/gate binding.

Context is closed and candidate-bound, with exactly candidate, mock_mode=false,
synthetic=false, services, actors, database, device and operator. Unused typed
sections are null. Services record unique IDs, exact candidate source identity,
observed binary SHA256 and pinned deployment/process observations. Actors are
`{fingerprint,observation}` records with distinct SHA256 fingerprints and bound
`{kind:"actor",fingerprint}` observation facts, rather than tokens or raw identities. Live
flows need at least two actual actor observations; false eligibility flags or a
fixture database do not supply provider provenance.

Native context requires actual isolated PostgreSQL engine/version and pinned
observations. Physical context records actual iOS Safari or Android Chrome,
hardware/OS/browser versions, physical=true, widths360/390/430, all three skins
and retained observations. QA06 needs both platforms. Operator context pins
deployment, retention, place-provenance and incident-control dispositions.
Missing real inputs remain NOT_RUN/BLOCKED with the producer; this checker does
not invent settings or request already-pending human answers.

Every context observation uses closed `mcr.qa10.context-observation.v1` with
`{schema,candidate,facts,artifacts}`. Candidate must match; facts must exactly
repeat the relevant service identity/binary hash, database metadata, physical
device metadata or operator setting/VERIFIED disposition. Its nonempty pinned
leaf artifacts retain the producer's actual process probes, SQL/server output,
captures or operator values. A hash of an unrelated nonempty file cannot bind
contradictory metadata. These records still rely on the responsible owner for
truthful execution and actual human/provider provenance.

BE43 and QA01–QA09 (QA08 excepted) plus IN01–IN04 need explicit responsible-owner
gate bindings appropriate to their unit/native/live/device/operator level.
All original task/IA records, production bytes, live/device/operator categories
and user deferral must be supplied before composition can return PASS. PASS
means **owner evidence consistency only**. It never substitutes for the actual
functional gate, private provider/device input, manual visual judgment or IN05.

## Limits and validation

Path traversal, symlinks, secret filenames, duplicate JSON keys, invalid hashes,
source modes, malformed candidate/tree, stale source links and unsafe report
shapes reuse fixed rule-ID failures. Raw report values, credentials, matched
secret text and arbitrary exception messages are not printed. Existing per-file
and artifact/Git limits apply. Source-v1 and composition have independent bounded
repositories (at most128MiB artifact reads each); the known frontend Vitest
archive allowance remains96MiB. No recursive source/home secret scan occurs.

Coherently forged unsigned local records cannot be distinguished from actual
execution by hashes alone. Responsible developers must actually run and preserve
their evidence; actual service identities, provider provenance and physical
device observations remain their duty. The checker validates bindings, not an
attestation chain. A production scan covers its declared recognizable patterns,
not every possible secret/obfuscation or authorization defect.

Focused parser tests use disposable miniature Git repositories, copied original
catalogs and synthetic source/report bytes. Those fixtures are never application
execution or candidate release proof. Positive fixtures must retain 111 active
task bindings, one uncompleted deferral,61 IA bindings and NOT_ASSESSED release
decision. Negative cases cover missing records, stale candidate/service inputs,
unit-as-live, mock contexts, actual actor/device requirements, original-path
ownership, command truth, source hashes and fabricated test counts.

Compilation/application/gates/browser/PG/device validation: **NOT_RUN** by this
assignment. Parser fixtures never certify those actual product outcomes.

## Executed source validation and publication handoff

The final tested source is `b30749b2e039152c82c9a8472dc4e7964fb61788`, descending
from the assignment base. The following retained results are under
`docs/mcr/evidence/release/qa10-composition/`; `self-validation.json` pins all
seven executable/test/controller source files, original raw record hashes and
sanitized retained hashes. Only local checkout/Python-framework prefixes were
replaced. Original records remain preserved locally. No earlier artifact or
historical frontend receipt was rewritten.

| Round | Actual ACCEPT / FULL (UTC, 2026-11-04) | Result |
| --- | --- | --- |
| SOURCE1902 / `round-01` | 19:09:47.994534 / 19:10:34.899920 | 37 tests ran; 13 passed, 24 failed on the shared synthetic frontend inventory. All three retained regressions passed. |
| RETRY1915 / `round-02` | 19:14:19.795819 / 19:15:51.373422 | All 37 tests passed in 90.811s; process exit0. |

The first fixture copied original catalog test paths but omitted those paths
from its frontend-v1 inventory. The fix supplies the complete miniature candidate
inventory, matching Vitest counts and dispatch paths. The production inventory
check remains enforced. The second run reaches the intended positive and
negative bindings: 34 new composition cases plus three relevant retained
source-v1 positive, CLI and frontend-adapter regressions. Exact invocation:

```sh
# Working directory: scripts/mcr/release_checks
python3 -B -m unittest -v composition.test_composition \
  test_check.ReleaseChecks.test_positive_exact_candidate_test_artifacts_production_and_task_link \
  test_check.ReleaseChecks.test_cli_exit_codes_distinguish_partial_incomplete_and_fail \
  test_check.ReleaseChecks.test_existing_frontend_schema_is_consumed_and_archived_json_checked
```

Both rounds separately run the actual external Python CLI. No manifest returns
exit2 with source/composition NOT_RUN; composition lists all112 tasks and61 IA
as missing and leaves `release_decision=NOT_ASSESSED`. A different full candidate
returns exit1 with `candidate.not-current-head`. `git diff --check` against the
assignment base returns0. Exact argv, exits, reports and verbose test points are
retained per round. There is no release-manifest PASS for actual MCR evidence.

Each round had its own explicit allocation and actual epoch, with 180s work,
210s FULL including30s cleanup, NEW32MiB, logs below2MiB, WHOLE6GiB preserving
the prior650272KiB carry, <=4 total processes and RSS512MiB. The corrected
continuity admission required fresh12GiB, >=9GiB after all growth reservations
and live8GiB. Fresh free/reserved bytes were 18,349,715,456/15,497,588,736 in
round1 and17,592,950,784/14,472,388,608 in round2. An independent native libproc
guardian started before each work child and observed no breach. Its maxima
were4 processes/104,742,912 RSS bytes/452,948 new bytes in round1, and4 processes/
95,059,968 RSS bytes/441,157 new bytes in round2. Owned miniature repositories
were removed; supervisor, worker and guardian ended before both literal FULL
notifications. Allocator21d and named publisher Root received the releases;
the next Ops owner received round2's release directly.

The independent SOURCE controller is retained as `source_round.py`; it launches
only these fixed self-validation commands, never commands from input receipts.
Global Git hooks and automatic maintenance are disabled for miniature fixtures.
The original unused1850 offer was definitively declined before children; no
closed round or old runtime/build lease was reused.

The committed handoff routes directly to Root1fc for publication into
`julymeltdown/mcr:master`. The source slice was published by Root as
`6d351502bcee839e0836955edbf5eff61ab9b0b7`, preserving the delivered source
postimages from `007527a5a455340fb27080070c4fe1b34ab6a8f3`. The publisher
receipt confirms remote delivery. The actual-candidate report below is a separate
committed handoff. Source validation does not complete original QA10, IN05 or the
historical incompatible Profile goal. Real responsible-owner service/device/
operator evidence and full functional release acceptance remain open.


## Actual published-candidate execution

Candidate `6d351502bcee839e0836955edbf5eff61ab9b0b7`, tree
`f9b4696543d042919f149b7be4cb0510516d5789`, was frozen with clean tracked
state on `codex/mcr-qa-QA10-candidate-report` in the same exclusive checkout.
The newly authored audit collector/controller were outside that candidate and
SHA-pinned before admission; they are not represented as product code tested
in the candidate. They read immutable candidate source and actual existing
owner evidence. No application, feature matrix, browser, database, device,
build, installation or recorded receipt command was executed.

| Fresh round | Actual ACCEPT / FULL (UTC, 2026-11-04) | Actual result |
| --- | --- | --- |
| `QA10-ACTUAL-CANDIDATE-SOURCE-20261104T1933Z` | 19:35:13.664087 / 19:35:21.723630 | Collector exit1: sparse checkout omitted `.node-version` from the filesystem. |
| `QA10-ACTUAL-CANDIDATE-METADATA-20261104T1955Z` | 19:55:03.130843 / 19:55:11.937719 | Collector exit0; real composition exit2 INCOMPLETE; real historical frontend receipt exit1 `candidate.mismatch`. |

The first failure does not mean the published file was absent. Git contains
`.node-version`, `frontend/package.json` and `frontend/pnpm-lock.yaml`; the
retained sparse checkout marks them skip-worktree. The correction reads their
size-bounded immutable Git blobs without materializing frontend or changing
sparse settings. The failed round was closed and released before requeue.
The second round used a fresh offer, rather than extending the failed lease.
The unchanged checker also refused a noncurrent candidate inside the collector.

Exact successful controller invocation, with the exclusive checkout as `ROOT`:

```sh
exec python3 -B docs/mcr/evidence/release/qa10-composition/candidate_round.py \
  --root "$ROOT" --candidate 6d351502bcee839e0836955edbf5eff61ab9b0b7 \
  --offer QA10-ACTUAL-CANDIDATE-METADATA-20261104T1955Z \
  --all-new-bytes 1543503872
```

The retained `result.json` records exact collector and external CLI argv/exits.
The round's `candidate/input.json` is an actual-input manifest, not the synthetic
complete fixture. It preserves all112 task records and61 IA rows with honest
pending producer envelopes. `candidate/candidate-report.json` is the actual
checker result, byte-identical to `cli-input.log`.

### Result and original QA10 disposition

The actual release composition is **INCOMPLETE** with zero qualified current
release task/IA bindings,111 missing active task bindings,61 missing IA bindings
and the one uncompleted, nonblocking **QA08 DEFERRED_BY_USER** row. All five
candidate producer categories are NOT_RUN because compatible exact-candidate
release envelopes were not supplied. IN03 remains BLOCKED from canonical
WAITING_INPUT; no duplicate operator question is created. Other absent gate
bindings remain NOT_RUN. `release_decision` stays **NOT_ASSESSED**; IN05 retains
its separate owner and NOT_READY release boundary.

These counts do not revoke historical owner acceptance. `task-provenance.json`
preserves the canonical revised states:41 DONE,57 IN_PROGRESS,3 WAITING_INPUT,
7 REOPENED,1 DEFERRED_BY_USER and3 OPEN. It records original owned paths, matching
candidate files, Git blob identities and latest touch commits for88 tasks.
The other24 tasks have no original-owned-path match; they remain explicit
mapping gaps. A touch commit is provenance, never behavior acceptance. The
collector retains190 unique explicit evidence references without rewriting
those owner documents or inferring acceptance from their filenames.

Original QA10 remains **IN_PROGRESS**. The checker implementation and actual
candidate-report execution are delivered, including an honestly incomplete
result. Remaining QA10 qualification requires current retained production bytes,
compatible responsible-owner receipt/criterion bindings and resolution of the
explicit bounded input gaps. It does not wait for a king/SL review, all product
tasks to pass before running the checker, or a duplicate feature matrix. The
next implementation assignment can proceed after this committed handoff.

### Reused evidence and limits

- The real frontend receipt is for `980fc4d85020f297b36fee8195e1ca92079ff33f`.
  Its retained gzip streams to83,822,272 uncompressed bytes with the pinned
  digest. Of its source/contract pins,1799 blobs match this candidate and142
  changed, with none absent. Its small Node TAP reports22 passes/0 skips.
  Vitest3503 is OWNER_RECORDED_NOT_REPARSED; no huge JSON graph or test rerun
  was performed. This historical evidence is preserved and explicitly rejected
  as a current-candidate envelope, never promoted to current product PASS.
- The published37 genuine checker self-tests are reused by matching the actual
  verbose log hash and all seven tested executable/test/controller source
  hashes. This qualifies unchanged tooling only. Five retained visual owner
  references match5/5 hashes; they remain owner evidence, with no new visual
  judgment or physical-device acceptance.
- Source versions declare Node24.22.2, pnpm11.7.0 and Rust1.88.0. The actual
  checker used Python3.14.3 and Git2.55.0 on Darwin. Seven version/lock/schema
  references are hashed; `contracts/generated/mcr-domain.openapi.json` remains
  NOT_RUN with `inventory.file-limit` because it exceeds the2MiB text bound.
  Source pins are not observed running application versions.
- Actual application binary versions, genuine current-candidate product-test
  receipts, production mock scan and production secret scan remain NOT_RUN:
  **CURRENT_RETAINED_BUILD_BYTES_NOT_SUPPLIED**. No scan result is fabricated
  from source manifests. Total collector input reads were90,013,500 bytes,
  within its128MiB bound. Historical receipts and selected-art originals are
  unchanged.

### Resource and retention provenance

Both rounds used the existing independent native guardian, started before any
worker, with180s work/210s FULL, NEW32MiB, logs below2MiB, WHOLE6GiB preserving
prior650272KiB carry, <=4 own ALL processes and RSS512MiB. The1955 offer explicitly
changed the fresh-start floor from12 to11GiB before ACCEPT; reserve9GiB/live8GiB
remained binding. Fresh free bytes were13,365,104,640; all admitted NEW reservations
were1,543,503,872, leaving11,821,600,768 bytes. The controller is a per-round
SOURCE guardian, not the allocator's global capacity controller.

The successful round lasted8.8069s, retained2,998,440 bytes at FULL and reported
no guardian breach. Sampled maxima were4 own ALL processes,118,718,464 RSS bytes,
2,998,270 new bytes and651,465,570 whole bytes. No application resources were
created; supervisor/worker/guardian ended before literal FULL. Root and21d
received the immediate release; next Avatar owner1eed received the literal
D release directly before documentation completion.

The final collector SHA256 is
`99ddd21c8b2c936f1c29f58d3313a48adc4072940f4cc5fbea21cf5862c6972e`;
controller SHA256 is
`a644bc26f83b98f612fd1285b56c9a17ea64493e13dce7a6fc8204ab121a2438`;
reused guardian SHA256 is
`a30910274bd9eb3a98cebc75332292d453c7be2ee6191a3d560f6b54f68850ab`.
`tool-history/` retains both original failed and corrected collector bytes and
both admission-floor controller revisions, with hash provenance. The failed
collector snapshot was reconstructed and verified against its original admission
SHA; it is not claimed as a separately retained original file.

Both named actual-round directories are retained with their original structure.
`actual-candidate-retention.json` pins every original and retained file hash.
Only exclusive-checkout/Python executable prefixes in five control/log files
were sanitized; immutable candidate manifests, reports, artifact paths and
hash bindings are unchanged. Raw records remain locally preserved. The report
is historical evidence for its frozen6d351502 candidate; the later documentation
commit does not silently rebind it to a new HEAD. Publication of this report
routes directly to Root1fc with no manager inspection or acknowledgment gate.
