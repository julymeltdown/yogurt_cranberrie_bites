# Contract validation environment follow-up

Date: 2026-11-04 KST. Result: **two previously failing UUID/provenance regressions
PASS with the repository's declared Python dependencies**. This is an environment
correction, not an application-source fix or a full-suite green claim.

UIV04's wider frontend run found five failures, including two tests in
`frontend/src/world/__tests__/runtime.test.ts` named
`matches C06 positive UUID/provenance semantics with consistent references`.
Both also failed on its untouched base. Their Python subprocess loaded the asset
validator without the required `date-time` and `uri` format checkers. The existing
validator correctly refused to proceed and identified
`scripts/mcr/requirements-contracts.txt` as the required environment input.

Main created a separate local virtual environment at
`/Users/lhs/Desktop/mcr-tools/contracts-validation-venv`, using the already
declared requirements. Global Python, frontend dependencies, manifests, lockfiles,
application code and the validator were not changed. The resolved environment is
jsonschema 4.26.0, referencing 0.37.0, rfc3339-validator 0.1.4,
rfc3986-validator 0.1.1, six 1.17.0, attrs 26.1.0,
jsonschema-specifications 2025.9.1 and rpds-py 2026.9.1.

## Executed checks

The tested application revision was `970b2728d5397ef98ad9cd6dfb74a6b5e110b9ec`,
which includes the integrated UIV03 and UIV04 presentation changes. The environment's
`bin` directory was prepended to the test process's `PATH`, so its existing
`python3` subprocess used the declared validators. Node was the retained 24.22.2
binary. The temporary retained-dependency link was removed after execution.

| Check | Result |
| --- | --- |
| Install existing `scripts/mcr/requirements-contracts.txt` into new isolated venv | Exit 0 |
| Whole frontend strict TypeScript, incremental output disabled | Exit 0 |
| Venv Python `scripts/mcr/validate-assets.py` | Exit 0; six metadata documents, CONTRACT_READY only |
| Vitest `src/world/__tests__/runtime.test.ts -t 'matches C06 positive UUID/provenance' --configLoader runner --no-cache --maxWorkers=1` | Exit 0; exactly two affected tests passed, 133 unrelated tests skipped |

The exact commands, requirement-file hash, tested revision and log hashes are in
the local receipt
`/Users/lhs/Desktop/mcr-tools/contracts-validation-evidence/receipt.json`.
The accompanying logs preserve validator limitations: atlas images, actual resource
bytes, venue truth, art and mobile acceptance are **NOT_RUN**. The fixture validator
does not approve production artwork.

For subsequent local validation, reuse this venv read-only by prepending its
`bin` directory to the validation process's `PATH`; do not install packages into
global Python or an unrelated worker's environment. A new machine should create
its own isolated environment from the repository requirements.

## Remaining failures

UIV04's original expanded suite remains a recorded failed run (2,204 passed,
five failed, 14 skipped). Its direct-beacon Back-navigation failure also reproduced
on the untouched base; transport epoch recovery and room-switch timing failures
passed a targeted rerun. None of those three is declared fixed here. The exact
failures and base/changed-tree comparisons remain in
[UIV04 evidence](../frontend/me-visual.md). No broad suite was repeated merely to
replace the original result. Their owning implementation/test paths need targeted
diagnosis under coordinated ownership before release acceptance.
