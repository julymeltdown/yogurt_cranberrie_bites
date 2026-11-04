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
Large Vitest JSON is stored losslessly as `vitest.json.gz`. Artifact pins retain
the original uncompressed byte digest; the verifier decompresses a JSON archive
when its original file is absent, then verifies that digest. Archiving historical
output preserves its original receipt and all raw report bytes.

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

## Executed candidate receipts

Implementation commits (serial from the assignment base):
`f30f7871e8dafb0a0cac6d04857bc49b9cbf5d72`,
`68c5f48d1dc0f4a2bdbdfd372a51f89b842e788f`,
`0fe5a687522ae2d08a38f7c669fe3814fd68b55e`.
The last commit corrects the verifier's default Git output buffer for existing
3.6 MB source blobs; all inspected blobs remain SHA-256 and Git-blob checked.
The buffer is explicitly bounded at 64 MiB and larger inputs fail verification.

Actual commands were run from the assigned checkout with existing Node 24.22.2,
retained dependency lock SHA-256
`47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`,
and the existing `contracts-validation-venv/bin/python` environment. Each
`candidate-*/receipt.json` contains exact source/contract blobs and digests,
resolved direct tool versions, full command argument arrays and exit codes.
Top-level `run-*.log` records runner process output. These commands used
`run --candidate <named SHA> --output <new absolute candidate directory>
--python <existing environment>`; no install was performed.

| Candidate | Tree | Actual result |
| --- | --- | --- |
| `f30f7871e8dafb0a0cac6d04857bc49b9cbf5d72` | `b1ee4d4b3cd9d9ae11f4ff86041dac964a197f44` | Runner exit 1, FAIL: Vitest 3,416/3,417, 204 files; nine other gates PASS; 19/19 Node tests. |
| `68c5f48d1dc0f4a2bdbdfd372a51f89b842e788f` | `ab3ca81400cc1482354860da7bf2b8cc132eb7c2` | Runner exit 0, PARTIAL: all ten bounded gates PASS; Vitest 3,417/3,417, 204 files; 19/19 Node tests. |

Both candidates passed the initial and final exact commit/tree/clean fences.
Their product tests are unchanged. The first full run failed the case
`preserves direct-beacon back navigation through Meet to World without creating a second world lease`
in `frontend/src/app/meeting-entry/__tests__/root-registration.test.tsx`, with
expected `/world` and observed `/meet`. A separate focused command using the
same Node/Vitest, `run <that file> --configLoader runner --no-cache
--maxWorkers=1 --reporter=json --outputFile=<focused-meeting.json>`, exited 1
with 6/7 passing. The second full run passed without changing that test, which
establishes intermittent behavior rather than proving a repair. All original
FAIL evidence and raw bytes are preserved. Root153 assigned the exact central
test/readiness helper repair exclusively to chat `01a1a204-c583-7670-965a-4891b8b1289e`;
this QA10 writer has no application/test repair rights.

The same candidate receipts recorded 26 direct packages, 764 TypeScript inputs,
1,297 source entries for lint provenance, 243 actually emitted production
modules, 2,827 canonical C08 content files, and 36 source-linked task rows.
The latter leaves FE20, FE25, FE26, FE27, FE28 and FE33 NOT_MAPPED, without
inferring they are unimplemented. Three real-service Playwright suites remain
NOT_RUN. The complete limitations and owning lanes are retained in each receipt.

Consumer verification under final source `0fe5a687` accepts the unchanged
historical FAIL receipt structurally and returns exit 1 with frontend_checks
FAIL; the historical PARTIAL receipt returns exit 0. Neither certifies a later
HEAD. `verify-probes.py` executed ten real CLI probes, all PASS: valid archived
FAIL evidence, wrong expected candidate, missing gate, empty passing gate,
failed process falsely marked PASS, forbidden global acceptance, missing final
candidate fence, altered log, corrupt compressed report and an ancestor passed
to `run`. `negative-probes.json` records actual exits and diagnostics. The initial
consumer ENOBUFS discovery was corrected rather than skipped. Final six harness
self-tests, scoped lint and diff checks passed. Focused raw reproduction output
and the original candidate reports remain alongside these probes.

Publication and qualification on the eventual published repaired candidate are
still pending. The integrator can publish these implementation commits while the
QA10 owner retains responsibility for consuming that candidate and recording a
fresh meaningful run. Existing broad NOT_GREEN is not overwritten, and no full
QA10, QA01, QA02, BE43, real-service or IN05 acceptance is claimed here.

## Published candidate qualification

The designated integrator published the implementation, evidence and sole-owner
readiness repair at **`980fc4d85020f297b36fee8195e1ca92079ff33f`**.
Fresh fetch and `git ls-remote origin refs/heads/master` independently returned
that exact commit. Original worker commits were cherry-picked as follows:

| Original | Published ancestor |
| --- | --- |
| `f30f7871e8dafb0a0cac6d04857bc49b9cbf5d72` | `976508a87a71f5daf88e5b06fb99a09934791815` |
| `68c5f48d1dc0f4a2bdbdfd372a51f89b842e788f` | `ec84cfbb433d9abf0abd4acd55fb736e6c1ef0cd` |
| `0fe5a687522ae2d08a38f7c669fe3814fd68b55e` | `c338b86f2b9e02e84590562fc7714a2e4d6d7b80` |
| `544d45ae01cb3c54465e901bb977c687da6048fb` | `4505971aaffc833c0b825ddb734af1f3cf617601` |

All 48 handoff paths were independently compared byte for byte against the
published candidate. `publication-980fc4d8.json` records their SHA-256 hashes,
mapped ancestry and the repair test hash
`fc117dc9f32ebe79bf1c974d4fdea1f77742918450fe12aa9b2a894b837a0422`.
Original commits remain historical candidates; mapping does not rewrite receipts.

The preserved original branch remains at `544d45ae`. Fast-forwarding safely
refused because of the cherry-picks. In the same isolated checkout, new branch
`codex/mcr-qa-QA10-published-candidate` started at the exact published commit.
No merge, application edit, install, source repair or developer push was used.

The QA10 owner then executed the full `run` command itself with explicit
`--candidate 980fc4d85020f297b36fee8195e1ca92079ff33f`, fresh absolute output
`docs/mcr/evidence/release/qa10/candidate-980fc4d8`, and the same existing Python
validation environment. Actual result: **exit 0, PARTIAL, frontend_checks PASS**.
The initial/final candidate fences both verified commit, clean state and tree
`4e18155702c0907782da77afc9cb442916473967`.

| Gate | Actual exit | Recorded coverage |
| --- | ---: | --- |
| dependencies | 0 | 26 retained direct packages; original lock unchanged |
| python-formats | 0 | 5 installed validator packages and actual format checks |
| typecheck | 0 | 780 TypeScript inputs |
| lint | 0 | canonical source/config plus harness; 1,337 source entries |
| vitest | 0 | **3,503/3,503 tests, 209 files, zero failures/skips** |
| node-tests | 0 | **22/22 tests, 5 files, zero failures/skips/todos** |
| build | 0 | ordinary production entry; 249 emitted source modules |
| ui-origin | 0 | canonical C08 PASS, 3,012 content files |
| production-static | 0 | 249 emitted modules; no narrow mock/credential findings |
| task-trace | 0 | 36 linked source rows; six unmapped rows retained as gaps |

The published run automatically discovered additional tests rather than using
the old fixed inventory. All 209 Vitest and five Node files executed; all three
real-service Playwright suites remain explicitly NOT_RUN. No unrecognized runner
was omitted. The exact commands, source/contract/tool pins, raw lossless report,
logs and artifacts are in the new candidate directory; `run-980fc4d8.log` records
process output. The owner also ran `verify` against this explicit candidate and
receipt; its actual exit/result is retained in `verify-980fc4d8.log`.

This qualifies the assigned frontend candidate contribution, including the
published readiness repair, without upgrading original historical FAIL results.
Full QA10/IN05, real multiuser services, backend release packaging, physical
devices, full secret auditing and complete task/IA acceptance remain with their
existing owners and explicit NOT_RUN/BLOCKED criteria. A later documentation
publication is evidence recording, not qualification of a different code SHA.
