# QA10 release source checks

This implements **QA10-RELEASE-SOURCE-01**, a bounded contribution to original
QA10 (§13 of `MCR_IMPLEMENTATION_PLAN_V2_EN.md`). It checks evidence consistency;
it does not run application tests or decide IN05. Full QA10/release acceptance
remains **INCOMPLETE**. Original QA08 remains **PARTIAL**, irrespective of this
checker's regression results.

Assignment base: `bcbcf3edf4eba23a3011358018142574077d1426`.
Existing exclusive checkout: `fe-ref-gifts`; branch
`codex/mcr-qa-QA10-release-source`. Actual goal inspection returned no unfinished
goal; an unbudgeted concrete delivery goal was created at epoch1793803778.
Only new `scripts/mcr/release-check.sh`, `scripts/mcr/release_checks/**` and this
document are authored. Root1fc transferred those paths and retains final release
composition. No application, registry, QA01, CI, manifest/lock or historical
evidence is changed. The completed bounded QA08 delivery is preserved.

## Commands and outcomes

Use Python3's standard library and Git; no installation is needed:

```sh
scripts/mcr/release-check.sh --candidate FULL_40_CHARACTER_HEAD
scripts/mcr/release-check.sh --candidate FULL_40_CHARACTER_HEAD \
  --manifest docs/mcr/evidence/release/CANDIDATE/source-input.json
python3 -B -m unittest discover -s scripts/mcr/release_checks -p 'test_*.py' -v
```

The wrapper resolves its own repository root. Paths inside the input are relative
to that root. The checker only prints JSON to stdout; it does not write outputs,
install dependencies, invoke receipt commands, build, launch browsers/Cargo/live
services, fetch, publish or update task/acceptance ledgers. Redirect output only
to the caller's assigned evidence path. `-B` prevents bytecode writes.

| Exit | `source_checks` | Overall `status` | Meaning |
| --- | --- | --- | --- |
| 0 | PASS | PARTIAL | Supplied bounded source/test/production/trace evidence is internally consistent; full release is still INCOMPLETE |
| 2 | NOT_RUN or INCOMPLETE | INCOMPLETE | Input, tests, trace, retained production bytes or skipped cases leave a bounded gap |
| 1 | FAIL | FAIL | Candidate mismatch, contradictory PASS, invalid/tampered evidence or another reported rule violation |

`release_status` is **always INCOMPLETE**. The checker independently lists
QA02-manual, QA03–QA09 and IN01–IN05 as NOT_RUN **by this offline checker**;
this does not revoke a historical owner's acceptance or substitute it for
same-candidate evidence. QA01/QA02/BE43 receipts and all required task/IA,
operator, device and live-service dispositions still belong to release
composition. Missing prerequisites cannot be cleared by this source-only result.

## Input envelope

The new local consumption schema is `mcr.qa10.release-source.v1`. It is not a
replacement central registry or acceptance ledger. Unknown envelope fields are
rejected so that an ignored `status:PASS` or live-acceptance claim cannot be
mistaken for a checked field.

```json
{
  "schema": "mcr.qa10.release-source.v1",
  "candidate": {"commit": "FULL_SHA", "tree": "FULL_TREE_SHA", "dirty": false},
  "release_status": "INCOMPLETE",
  "receipts": [{"path": "docs/mcr/evidence/release/CANDIDATE/tests.json", "sha256": "SHA256"}],
  "production": null,
  "task_links": [{"task_id": "QA10", "commit": "SOURCE_COMMIT",
    "paths": ["scripts/mcr/release-check.sh"], "receipt_ids": ["source-tests"]}]
}
```

All placeholders must be replaced with actual full identifiers/hashes. Supplying
`production:null` deliberately leaves `production.NOT_RUN`; the example is not
a passing release input. The deterministic fixture builder in `test_check.py`
creates complete positive and independently corrupted negative inputs, including
retained production bytes, without requiring application execution.

Candidate must equal actual current HEAD, its actual tree, and a clean tracked
worktree. An ancestor is insufficient. Source and contract pins are checked
against Git blobs and SHA256; regular-file modes are required. The selected
candidate and tracked cleanliness are checked again after consumption. Untracked
evidence may exist, but is never executed as source; the input's candidate tree
is the source authority. No historical mode silently certifies current HEAD.

Each artifact reference is exactly `{path,sha256}`. Artifacts are explicit
sanitized reports/logs or emitted release outputs. Traversal, absolute paths,
symlinks, `.git`, `.env*`, common credential directories and key-file names are
refused before reading. No environment values or credential stores are read.
Matched secret values, raw logs and arbitrary exception messages are never
printed; findings use fixed rule identifiers. Limits are32MiB per ordinary file/blob,96MiB for the existing frontend
Vitest JSON report only,128MiB total artifact reads (including compressed input
and decompression),128MiB Git batch output,
and30seconds per Git operation. These are parser bounds, not a runtime resource
allocation or an application capacity claim.

## Command evidence and genuine-test consistency

A new producer may supply `mcr.qa10.command.v1`:

```json
{
  "schema": "mcr.qa10.command.v1", "id": "source-tests", "kind": "tests", "status": "PASS",
  "candidate": {"commit": "FULL_SHA", "tree": "FULL_TREE_SHA", "dirty": false},
  "same_candidate_end": true,
  "command": {"argv": ["node", "--test", "frontend/example.node.mjs"],
    "cwd": ".", "attempted": true, "exit_code": 0, "signal": null},
  "source_pins": {"frontend/example.node.mjs": {"git_blob": "BLOB_SHA", "sha256": "SHA256"}},
  "log": {"path": "docs/mcr/evidence/release/CANDIDATE/tests.log", "sha256": "SHA256"},
  "test_files": ["frontend/example.node.mjs"],
  "report": {"format": "tap", "artifact": {"path": "docs/mcr/evidence/release/CANDIDATE/tests.tap", "sha256": "SHA256"}}
}
```

`kind` is `tests`, `source` or `build`. PASS requires explicit attempted execution,
integer exit0, no signal, nonempty argv/cwd, source pins, log pin and report pin.
NOT_RUN/BLOCKED receipts retain their status with an owner and reason, and leave
an INCOMPLETE gap. Missing/skipped cases cannot become PASS through a zero exit.

Tests support Node TAP (`format:tap`) and Vitest JSON (`format:vitest-json`).
Pinned `test_files` must be dispatched in argv relative to cwd. TAP needs actual
test points, ordered top-level plan, complete counts, nonzero passing cases and
no bailout/failure; footer-only fabricated PASS is rejected. Suite/skip/todo
counts are checked and skips remain an INCOMPLETE gap. Vitest needs exact named
file coverage, nonempty per-file assertions, actual status/count agreement,
successful suites and nonzero passing cases. This is bounded report consistency,
not an assertion that every project test was covered by one generic receipt.

`source`/`build` reports use `format:execution-json`; their pinned JSON must repeat
the exact candidate, PASS and identical command object. These are producer
records, not execution by this consumer. Backend-specific runner reports are not
silently interpreted as TAP/Vitest or declared qualified; their owner can provide
an explicit producer format in a separately authorized extension.

These checks catch contradictory or missing execution evidence. They **cannot
prove a coherent report was not forged**: local unsigned artifacts are not a
trusted attestation service. An owner must actually execute the commands and
preserve the evidence. The consumer never claims cryptographic execution proof.

## Existing frontend producer

The consumer also accepts the already-published `mcr.qa10.frontend.v1` receipt
unchanged. It checks exact candidate/tree/end fence, PARTIAL disposition, all ten
required gate IDs, attempted/exit/signal truth, nonempty counts, every artifact
and command-log digest, and source/contract Git/SHA256 pins. Archived
`vitest.json.gz` is boundedly decompressed and compared against the original
uncompressed digest. Raw evidence is never rewritten.

Candidate frontend sources independently reconstruct the producer's Node,
Vitest, Playwright and unsupported-runner discovery conventions. Inventories
must agree with both the pinned inventory artifact and the candidate; deleting
a test from a report/inventory does not silently reduce coverage. Actual
Vitest/Node reports and dispatched file paths are checked again. Unsupported
runner groups are rejected; Playwright/live cases remain producer limitations.

The existing producer deletes its build directory. Its audit asset hashes can
therefore validate only the historical recorded inventory. The consumer checks
that audit and known emitted mock module paths, but explicitly reports
`production_bytes:NOT_RUN`. A fresh scan requires retained emitted bytes and a
same-candidate build receipt; old `candidate-980fc4d8` evidence cannot qualify
the newer current HEAD. A read-only compatibility check of that historical
receipt validated its1,941 source/contract pins and3,525 test assertions under
the historical candidate only. It does not qualify the current candidate. The
initial32MiB decompression guard rejected its83,822,272-byte Vitest JSON; the
known-format96MiB allowance corrected that owned compatibility defect. Both
failed and successful compatibility records are retained.

## Production exclusion and task trace

For a retained-output scan, `production` contains `build_receipt`, `root`,
`modules` and `assets`. The named receipt must be `kind:build` and its pinned
execution JSON must contain an identical `production` object excluding
`build_receipt`. The root must be below `docs/mcr/evidence/release/`. Every emitted
file below it must appear exactly once as a pinned root-relative artifact;
extra/omitted files, symlinks or missing bytes fail. No source/config/home tree
is recursively scanned for secrets.

The scan rejects known emitted fixture/mock/MSW module paths, mock transport or
worker markers, private-key headers, JWT-shaped literals and selected literal
credential assignments. Tests use only obviously synthetic markers. A clean
scan covers those rules over supplied outputs; it is not a complete secret,
obfuscation, source-map, dependency, authorization or dynamic-mode audit. The
producer's module list is a recorded build observation, not independently
reconstructed bundler execution.

Task IDs and required scope come from the named candidate's canonical
`generated/TASKS_EN.json`, not a new registry or mutable progress count.
Task-linked paths must fit their canonical owned paths (including the specified
CD/CA/CS/GW/FS/CT/TEST aliases), be present in the referenced verified receipt's
source pins, and have identical blobs at the named ancestor source commit and
candidate. Unknown/duplicate tasks, missing receipt IDs and stale source links
fail. Output is SOURCE_LINKED/acceptance NOT_ASSESSED; task completion is never
inferred. Required tasks without a source link remain explicitly listed, and
IN05 still needs all112 required task/61 IA dispositions.

## Validation and delivery record

The first26 deterministic regressions passed. A subsequent inventory-hardening
edit introduced a Python indentation error; the test command exited1 before
collection. That error was corrected and the same26 tests passed again.
The expanded30-test suite then passed in5.957s (Python3.14.3), as did bash
syntax and the unchanged planning-provenance check. Actual current-candidate CLI
receipts are recorded after source freeze. No application/runtime/live validation
is implied.

Frozen source `24fa26b7a9f4934bf8f02e59c1cdd6d3cc039a02` completed30
regressions in6.622s. Its actual CLI returned exit2/INCOMPLETE without a manifest
and exit1/candidate.mismatch for the historical frontend envelope. These are
recorded at [source-01 validation](../../scripts/mcr/release_checks/evidence/source-01/validation.json),
with exact command logs and both compatibility outcomes. The later archive-bound
correction and its new negative regression receive a new frozen-source record;
no earlier result is overwritten.

The new Python consumer is authored for this scope. Its frontend compatibility
semantics use the read-only published `frontend/e2e/release/qa10/model.mjs` and
`run.mjs` protocols at the assignment base (Git blobs
`d9341cc8ba1b8c45860b7cf37b014b5bdc3a3f66` and
`6d39dfc0a575d632a74d872f46a14e4d7a727536`). No frontend source is copied or
modified. Task aliases and acceptance limits come from specification§§9/13/15/16.

Tests create disposable miniature Git fixture repositories with fixed author
dates and synthetic contents, not additional MCR checkouts/worktrees. They
exercise positive consumption, candidate/tree/dirty/end-fence mismatch, stale
ancestor/source links, missing or fabricated PASS counts, zero/failed/unattempted
commands, hashes, omitted outputs, mock and synthetic secret markers, safe paths,
task ownership/evidence linkage, skips, frontend inventory/archive consumption,
and CLI exit semantics. No real credential is opened or printed.

Implementation commits and the final concrete handoff go directly to Root1fc
for normal serialized publication. There is no king/SL/QA review or ACK gate.
The owner resolves concrete defects and independently verifies published
ancestry plus exact owned file blobs/modes. Root retains final release
composition and original QA10/IN05 acceptance.
