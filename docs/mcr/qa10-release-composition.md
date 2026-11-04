# QA10 same-candidate evidence composition

Status: **IN_PROGRESS**. This implements the functional remainder
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
assignment. Focused source self-validation awaits its separate named allocation;
no prior runtime/build offer is reused. Commands, exits, source pins, scoped
commit and direct Root publication receipt will be recorded after that run.
