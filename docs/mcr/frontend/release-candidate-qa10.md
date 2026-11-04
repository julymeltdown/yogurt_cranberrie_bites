# QA10 frontend candidate receipt

This is a bounded contribution to original QA10, not a new task, whole-QA10
acceptance or an IN05 release decision. The canonical release wrapper and backend
composition remain with root owner `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`.
`scripts/mcr/release-check.sh` was absent at assignment base; this delivery does
not imply an existing receipt consumer or duplicate that wrapper. Frontend
delivery publication belongs to integrator
`01a1a153-1227-7e72-801f-a2a6b91981e1`; this source writer does not push master.

## Scope and protocol

Assignment base: `e227d12ef4bb3ac92b9fdbbfaf954d4a56050081`, isolated preserved
`fe-ref-customize`, branch `codex/mcr-qa-QA10-frontend-candidate`. Goal created at
epoch `1793757526`. Exact new ownership is `frontend/e2e/release/qa10/**`,
`docs/mcr/evidence/release/qa10/**` and this document. There are no inherited
Customize, mural, moderation, application, shared, schema, manifest/lock or CI
rights. Existing canonical commands and original specification §13 QA10 are
inputs; planning output is not implementation acceptance.

Root1fc additionally transferred exactly one existing test hunk:
`frontend/e2e/reference-integrated/whole/data.test.mjs` receives only
`configLoader:'runner'` in its existing `createServer` options. Community was
notified before editing. Original Git blob
`802218747ca902d440cb7f54d1db6923c87f3ef6` becomes
`4b4ee740f1d6e0fb7032ac64b69e772ca5448cb8`; removing that exact insertion
reproduces all original bytes. This prevents Vite's default config bundle from
writing into borrowed shared dependencies. No probes or filenames change.
`frontend/e2e/reference-integrated/metrics.test.mjs` is unchanged. The earlier
abbreviated `frontend/e2e/whole` path grants no ownership and is not created.

The agreed machine schema is **`mcr.qa10.frontend.v1`**. It contains an exact
`candidate{commit,tree,dirty}`, tool/source/contract pins, real
`commands[{argv,cwd,exit_code,report_hash,nonempty_count,...}]`, per-runner test
inventory, unsupported criteria and explicit `PARTIAL` or `FAIL` status. It
never emits global PASS. A successfully executed bounded frontend set is
`PARTIAL`; unqualified frontend gates produce `FAIL`. Each command separately
records PASS/FAIL/BLOCKED and counts. Unsupported live/device/backend criteria
remain NOT_RUN, with owners and reasons.

## Execution and consumption

From repository root, with existing Node 24.22.2 and the retained dependency
installation available under `frontend/node_modules`:

```sh
node frontend/e2e/release/qa10/run.mjs run \
  --candidate FULL_40_CHARACTER_COMMIT \
  --output /absolute/fresh/evidence/directory \
  --python /absolute/existing/contracts-validation-venv/bin/python

node frontend/e2e/release/qa10/run.mjs verify \
  --candidate THE_SAME_FULL_COMMIT \
  --receipt /absolute/evidence/directory/receipt.json

node --test frontend/e2e/release/qa10/selftest.node.mjs
```

Freeze and commit implementation first. The run requires candidate = actual HEAD,
no tracked changes and no untracked source. It permits only the allocated exact
borrowed dependency symlink and untracked QA10 evidence directory. The output
directory must be new, absolute and outside frontend; existing evidence is never
overwritten. Source state and tree are checked again after commands. An evidence
commit can follow the tested implementation commit, but cannot make that receipt
certify a different candidate. Verification requires the explicit original SHA,
its Git tree/source blob and SHA-256 pins, unchanged artifacts/logs and a complete
nonempty gate inventory. It does not treat ancestor status as candidate equality.
The receipt is transparent local execution evidence, not a signed attestation.

The runner installs nothing and first requires exact installed/candidate pnpm
lock equality and Node engine match. It records resolved direct package versions
and manifest hashes. It executes strict types with incremental writes disabled,
the canonical source/config lint plus the new harness, and an ordinary production
Vite build with private output/cache. Five-minute command limits bound hangs;
failed/aborted commands remain explicit. All generated site/cache output is
removed in `finally`. At least 8 GiB free disk is required.

Tracked test inventory is taken from the actual frozen candidate. Vitest tests
are dispatched explicitly to Vitest; files importing `node:test`, including the
published renamed privacy selftest, run under Node. The JSON/TAP results require
nonempty tests and exact Vitest file coverage. Skipped cases remain named gaps;
the real-service Playwright files are inventoried as NOT_RUN rather than silently
discarded or replaced with fixture tests. Unrecognized runners are NOT_RUN and
prevent qualification. This is a composed runner result, not a claim that the
historical unfiltered Vitest invocation became green.

Canonical C08 `scripts/mcr/check-ui-origin.mjs` is executed unchanged. The build
records actually emitted production modules and script/style/HTML hashes. A
narrow audit rejects emitted fixture/mock module paths and recognizable private
key, JWT and credential literals, without retaining matched credential values.
Passing this scan cannot certify all secrets, dynamic behavior or authorization.
It does not substitute for QA02's visual/manual requirements or BE43 packaging.

Task trace uses canonical `generated/TASKS_EN.json` frontend ownership paths,
existing candidate source paths and their actual latest ancestor commits. The
new QA10 harness is explicitly marked as a supplemental assignment. Unmapped
canonical paths remain gaps; source linkage never means task completion. This
reuses the canonical plan rather than inventing another acceptance registry.

## Provenance and limits

All harness code is newly authored. It orchestrates the retained TypeScript,
Oxlint, Vitest, Vite and Node runners plus canonical C08 guard. No dependencies,
application logic, original art, schemas, generated contracts, package commands
or old evidence are changed. The C06 Python format preflight uses the existing
pinned validation environment; it does not claim complete QA01 generation/schema
qualification. Tool and input bytes are recorded in each receipt.

Meaningful negative probes cover stale/wrong/dirty candidates, missing or empty
test coverage, runner dispatch, skipped-case preservation, emitted mock/secret
markers, missing/duplicate/empty/falsely passing gates, missing end-of-run candidate
fence and unsafe artifact paths. Historical broad NOT_GREEN, live services,
real-user/device gates, complete task/IA dispositions, backend packaging and the
full secret audit remain explicit; their absence does not prevent implementing
this useful frontend contribution.

The owner records actual candidate commands, results, fixes and publication
receipt below after execution. No result is claimed from a planned command.
