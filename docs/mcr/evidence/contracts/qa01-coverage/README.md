# Original QA01 contract and fixture coverage

Original specification: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` §13, QA01, dependent
on C07. Required behavior is coverage of every fixture and generated response
shape, drift and missing-operation probes, and nonzero failure for broken schema,
unknown enum and empty coverage.

Task-local disposition: **SATISFIED for the pinned published canonical inputs**.
Evidence class: **CONTRACT_READY / offline contract coverage only**. This records
implementation and validation of QA01; it does not change the central task ledger,
accept C07 as a whole, or certify a producer, live-service flow, device or IN gate.
No remaining QA01 criterion is waived. Exhaustive business-state/value/union
combinations and real response capture are not claimed by schema witnesses.

## Scope and provenance

The exclusive assignment is new `scripts/mcr/contracts/qa01-coverage/**` and new
`docs/mcr/evidence/contracts/qa01-coverage/**`. Original checkout/branch base is
`d273c3771499d8f564e3282d4e9009cdf627a67e`; canonical inputs were frozen after
reconciliation at `d042638789bc84587be7bbbe09b32c324570c75f`. The restored assignment
reused the same goal created at Unix time `1793754012`; its temporary PAUSED state
was reported truthfully rather than replaced with a duplicate goal.

Product root confirmed no pending canonical schema/generator edits for this run.
The source handoff `21d0ac95` is historical ancestry, not a substitute for the
actual current bytes. Last relevant source changes are domain inputs `17fd2e93`,
OpenAPI repair `b9423cab`, frontend projection `65f91166`, and the core/domain
generator change `911f1566`. The unpublished 207 SDK3 GiftPrivate reservation is
not included or fabricated. `pins.json` and the report freeze every actual schema,
operation/policy, fixture, generated artifact, validator and generator dependency
input by SHA-256. Existing canonical sources and generator outputs are read-only.

The implementation is new Python orchestration and deterministic witness
construction. It imports the existing canonical validators directly, reads their
declared source inputs, and invokes existing generators only with `--check`.
No copied application rendering, backend route, schema, generated output,
dependency manifest/lockfile, CI configuration or source archive is changed.

## Coverage inventory

| Surface | Verified coverage |
|---|---|
| Canonical schemas | All 15 files in assets/core/domain/realtime; 225 definitions; draft metaschema and offline references |
| Generated schemas | All 241 components, local references and exact fresh-generator correspondence |
| Operations | All 134 inherited Auth/core/domain/realtime operations, unique IDs and exact method/path; frozen inventory |
| Fixture catalogs | Three canonical indexes, nonempty, unique, safe paths and exact file membership; assets has no canonical index |
| Core fixtures | 33 files, schema plus published identity/operation/status/bundle semantics |
| Domain fixtures | 47 files, expected acceptance/rejection through published schema and semantic oracle |
| Realtime fixtures | 24 files, actual trace/projection/wire-rejection/HTTP validation with expected close codes |
| Asset fixtures | 16 files, including six base, seven selected-static and three illustrated probes; metadata/hash/bundle validation |
| Asset semantic fixture | One file with all 25 named positive/negative scenery cases through the canonical bundle validator |
| Total fixture inputs | 124 JSON files: 121 fixture files plus three checked catalogs; no unclassified file or empty-family success |
| Generated response/media cases | 1,519: 1,513 JSON schema witnesses, one HTML string schema witness, three binary schemas, two bodyless responses |
| Response headers | Each declared header schema resolved, metaschema checked and witnessed; bodyless 302 requires Location |
| Unknown enum responses | 1,167 error-response witnesses reject an unknown `details.reason` after the positive witness passes |
| Generator drift | Core OpenAPI public/backend mirrors; domain OpenAPI public/backend mirrors; frontend transport projection |

The [full report](report.json) lists each fixture and scenery case, every
operation/status/media case, declared headers, actual witness values, schema hashes,
source hashes, environment versions and all generator exit codes. JSON witnesses
may originate from a fixture node or deterministic construction; each is validated
against the full generated target schema. They do not bypass canonical fixture
semantics and are not fabricated service observations. Binary bytes are not
decoded, HTML is not executed and venue/art authority is not inferred.

## Commands and results

Use the retained Python environment with the exact requirements:
`jsonschema==4.26.0`, `referencing==0.37.0`, `rfc3339-validator==0.1.4`,
`rfc3986-validator==0.1.1`, `six==1.17.0`. No install or dependency edit was needed.
For this run, `<contract-python>` was
`/Users/lhs/Desktop/mcr-tools/contracts-validation-venv/bin/python`.

```sh
<contract-python> -B scripts/mcr/contracts/qa01-coverage/run.py --output docs/mcr/evidence/contracts/qa01-coverage/report.json
QA01_WRITE_TEST_RECEIPT=1 <contract-python> -B scripts/mcr/contracts/qa01-coverage/test_coverage.py
python3 -B scripts/mcr/check-plan.py
git diff --check
```

The harness and seven regression tests pass (exit 0). Each of the 12 negative CLI
probes exits **1** with the expected error, not an empty success: broken schema,
unknown enum, empty catalog, missing fixture, unindexed fixture, missing operation,
missing response, generated drift, broken reference, empty responses, empty
generated operations and pinned-input drift. Additional tests reject absent
dependencies/format plugins, duplicate keys/nonfinite JSON and unsafe/duplicate
fixture catalogs. [negative-results.json](negative-results.json) preserves the
positive control and individual negative command/exit/error receipts;
[tests.log](tests.log) contains the test runner result.

The first system-Python attempt failed on missing required optional URI/date-time
plugins, before fixture validation. [initial-run.json](initial-run.json) preserves
that genuine environment failure. The retained pinned environment then passed;
the failure is not relabeled as a contract defect or omitted silently.

The lane-only boundary checker supports frontend/backend implementation leases,
not this expressly assigned central QA script prefix. Exact staged path and
base/delivered scope assertions are used for this assignment instead; no checker
configuration was changed. Publication/source-freeze receipts are recorded in
`validation.json` after final runs. No application build is required for these
Python-only additions; live providers, databases, devices and all IN gates are
**NOT_RUN**. No central acceptance or automatic CI wiring is inferred.
