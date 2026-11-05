# IN02 current continuation (2026-11-05)

Original task IN02 (specification row 564) is **IN_PROGRESS**. Actual two-client service/browser acceptance is **NOT_RUN**. Developer204 owns implementation, relevant validation, fixes, documentation and a scoped committed handoff to the standing publishing developer412e. No king/SL review, acknowledgment or separate QA stage is required. The original unfinished FE10 goal remains truthfully preserved; it is not completed by this work.

The first focused Source round, `IN02-FOCUSED-CLIENT-SOURCE-20261104T2347Z`, ran against `780acd5038ba555265e35e9d044c685f3f4ded50`. Its command exits were `[0, 1, 1, 0]`: 83 helper checks passed; six affected Vitest files had 18 passing and 50 failing cases; TypeScript stopped at TS5112 before checking types; lint passed for the eight original source/test paths. The feature fixtures returned Party DTOs for the new canonical `/api/auth/enrollment/admission` prerequisite, so Auth correctly rejected those responses before protected feature reads. This round is **FAIL**, not acceptance. Its raw logs, original intake and release receipt remain preserved locally in `source-round-01/` under the IN02 evidence directory.

Root153 subsequently stopped and transferred only the two existing Party fixture harnesses for an explicit synthetic canonical admission response. The fix intercepts that exact GET before custom feature handlers, runs the real Auth decoder and admission state transition, and records admission requests separately from existing feature-request assertions. It changes no Auth or feature controller semantics. New navigation cases require admitted/loaded prerequisites and a visible positive link before testing revocation. The corrected explicit-file TypeScript command uses TypeScript 6's `--ignoreConfig`. These corrections were qualified under the separate round-2 allocation below; the old offer remains closed.

Round 1 reached FULL release at `2026-11-04T23:51:43.057456Z`, with no owned process, dependency link or scratch/cache left. Peak new accounting was 13,670,645 bytes, peak owned processes six and peak RSS 893,026,304 bytes. The prior 208-byte/four-entry external uncertainty remains untouched. Helper and unchanged-file lint results are retained. Every actual IN02 service/browser flow remains **NOT_RUN**.

Corrected source `fa1e566673d61c071e2f6ce50b50a0f8a7a6d91a` is **SOURCE_QUALIFIED** under `IN02-CORRECTED-SOURCE02-20261105T0004Z`: all six affected test files and 68 cases passed (13 new navigation cases plus 55 original Party regressions); focused strict explicit-file TypeScript passed; lint with `--deny-warnings` passed for the four changed fixture/test paths. All three exits were zero, without a repair replay. All 1,790 frozen input pins matched before and after execution. This was a focused typecheck, not a full application build. The unchanged 83 helper checks and prior eight-path lint remain retained results.

Round 2 accepted at `2026-11-05T00:07:14.293188Z` and reached FULL release at `2026-11-05T00:07:26.839739Z`, before the original 180-second WORK and 240-second FULL deadlines. Both guards completed initial samples before workload children. Conservative combined peaks were 15,557,100 new bytes, seven owned processes and 1,193,771,008 RSS bytes; the nonshrinking WHOLE floor remained 3,504,899,426 bytes and 39,539 entries. No owned process, borrowed dependency link, scratch/cache, browser, server, database or target remained. Direct release went to allocator21d and queued successor PI08 before documentation. Exact commands, source postimages, binary/control pins, original failed-history hashes and release evidence are recorded in `docs/mcr/evidence/in02-journey/source-validation.json`.

The next real composition dependency is a **missing upstream authority producer**, confirmed by clock owner53d at canonical `6b6c94d246dcbde75763c2b139d0c7754fd795a0` and Gift owner267. The actual community owner exposes its existing clock, reviewed public-place choices and journal; it supplies no rally, pickup-place or pickup-meeting authority. `MeetingEntryFeatures` still mounts with `release={null}`. Declared optional ports and DTO types are not production producers. A genuine same-scope source must supply current authorized audiences/read-write decisions, meeting and per-place revisions, expiry/cutoff and withdrawal semantics. Public choices, MY PLAN and virtual coordinates cannot establish those facts.

Gift pickup GET does revalidate the authorized actor, current active HELD/READY reservation, private meeting, current map and reviewed VERIFIED OPEN physical place, exact internal per-place revision and DB expiry/cutoff. Its DTO supports a bound **current-read snapshot**; it does not grant future-write authority or an invented transport lifetime. New/edited meeting authoring still lacks a public per-place revision producer. FE19's response revision is a catalog revision; the community projection's map revision is the actual map revision; neither is a per-place revision. Code issuance additionally requires READY state and pickup start. The concrete missing interfaces and source-owner disposition are recorded in `missing-authority-producer.json` beside the validation record. Root153 routed producer/contract work to backend Root1fc and retains final constructor and meeting-release mounting. No inert ready shim or broader Root/controller change was authored under the narrow future wiring grant.

Source custody starts at `adec636683a37d9799e409f718a9aa15e60f00bd`, branch `codex/mcr-fe-IN02-meetup-pickup`, in the existing exclusive developer204 checkout. The initial three harness/document files matched canonical `7f05f596cb64ae7f415d0822addf4b3e1fe77a44` byte for byte. Exact preimages, assigned paths and original completion criteria are recorded in `docs/mcr/evidence/in02-journey/assignment.json`.

Root153 explicitly stopped and transferred only these application hunks, together with new focused tests in each feature's `__tests__/in02-navigation/` directory:

- PartyDetail: current decoded MY PLAN ownership opens **Manage party**; ownership or ACTIVE membership opens **Open rendezvous**. Pending, removed or absent membership grants neither member entry nor owner tools.
- WorldTools: **Check-in** uses the existing `/world/check-in` ShellLink and navigation/icon pattern.
- PartyRally: **View current meeting guidance** links only the server-decoded beacon for the same current party/event/meeting revision while current physical meeting authority exists.

No controller, Auth, World engine, CSS, contract, manifest, lockfile or backend ownership transfers with these hunks. Root's canonical `37add6d996eacb7674c1c220150870d7090d5e09` PartyDetail has an additional selected-pack portrait signature/row absent from the safe base. Publication must preserve that portrait hunk and apply only the additive navigation changes. Borrowed `frontend/src/app/ports.ts` and `frontend/src/world/composition/policy.ts` remain excluded.

The existing journey now navigates through real same-document controls and verifies the World host stays connected. Its intentional reload explicitly signs in again through the saved destination before checking the original command key; it never assumes a refresh cookie restores an anonymous in-memory session. Current-owner reads use existing read controls or real list/re-entry links. The new-key reservation probe uses the actual latest GiveawayPublic revision and requires `DUPLICATE_ACTIVE_RESERVATION`, rather than accepting a stale-CAS conflict. A recipient's owner-management probe requires generic 404 `RESOURCE_UNAVAILABLE` without private fields. One guaranteed-different code attempt must yield a durable FAILED `PICKUP_CODE_INVALID` invocation with unchanged READY reservation/revision and stock before the original valid code is used. That negative attempt intentionally spends admission/rate budget; a used-up fixture or other denial is a separate truthful outcome. Duplicate claim checks bind the durable pickup receipt, terminal reservation revision and returned collection time; a new BE15 invocation receipt may differ. Revocation uses an actual authenticated captured read after observing cached-guidance withdrawal; an anonymous reload is not authorization evidence.

Expiry observation includes the full request interval and timestamp quantum using a monotonic clock. This is a harness observation only, not a product-clock qualification or physical-arrival grant. Node-only contract/World decoder loaders require `TMPDIR` to name an allocated current-user-owned mode-0700 directory and `MCR_IN02_PRIVATE_CACHE` to be strictly inside it. No default Vite cache may be written through borrowed dependencies.

## Scoped current configuration

The original IN02 top-level input retains its exact version-1 keys shown in the historical section below. Its `worldConfigPath` may now point to an exact version-2 private configuration containing: `version: 2`, `isolated: true`, allowed `origin`, event UUID, 40-hex `releaseRevision`, exactly two distinct real `{email,password,userId}` accounts, and hash-pinned `serviceReceipt: {path,sha256}`. This scoped configuration has **no artReview field**. Service descriptor bytes must exist, be private and match the pin. Account zero owns both disposable resources; account one is the applicant/recipient. Main owns the single pending physical-input question; no synthetic place, generated raster or virtual position replaces reviewed physical authority.

For this version-2 world configuration, the private pinned prerequisite receipt has exactly:

```text
kind: mcr-in02-prerequisites-v2
status: READY
isolated: true
origin, eventId, releaseRevision: exact current configuration bindings
expiresAt: future UTC timestamp covering the bounded run
dependencies: {BE19: READY, BE23: READY, BE24: READY, FE23: READY, FE25: READY, FE26: READY}
resources: {partyId, giveawayId, placeId, mapRevision, placeRevision, meetingSha256}
serviceReceipt: {path, sha256}: exact same serving-cohort descriptor pin
```

READY declares usable isolated inputs for an attempt; it is not task acceptance or a runtime PASS. The actual run still verifies independent real logins, actor/event Bootstrap, real C05 presence/movement, current resource/clock/physical bindings and all original journey assertions. An unrelated completed IN01 art/theme/customization suite is not a prerequisite. Version-1 inputs/receipts retain their historical validation for compatibility; do not manufacture legacy art or IN01 reports. Invalid/missing input is NOT_RUN; a missing expected control or failed assertion after usable input is FAIL.

Focused source validation is complete independently of the unavailable serving cohort and affected physical-positive inputs. No current same-service cohort, actors, disposable resource IDs or accepted native execution window have been delivered to this owner. No browser, database, Vite server or native journey has been launched by this continuation. The task remains **IN_PROGRESS** through publication and the original real-flow criteria; source qualification does not close those criteria or the preserved FE10 goal.

## Historical source qualification (2026-11-04)

The following original delivery records are preserved as historical evidence. Their old root-composition and full-IN01/art prerequisites do not describe the current scoped continuation. Canonical `bd087fde` already composes the community-entry routes. Historical path ownership and validation counts apply to that original source cut only.

# IN02 meetup and pickup harness

## Delivery and acceptance

The bounded harness is implemented and validated offline. **Real IN02 acceptance is NOT_RUN.** No real account login, browser context, service mutation, physical meeting, reservation, or pickup was executed during this delivery. The two executed Playwright runs intentionally rejected missing or unqualified configuration before allocating a browser.

Scope comes from IN02 in `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, with `docs/mcr/project-assessment.md` and `docs/mcr/execution-plan.md` read as implementation guidance. IN02 requires the real two-client journey; helper tests, discovery, and an application build cannot accept that gate.

Assignment base: `ef9422c4d83f8555ee5b48ef693d37fd5bf87e0c`; branch: `codex/mcr-qa-in02-resume`. Only these files belong to this patch:

- `frontend/e2e/real/meetup-pickup-helpers.ts`
- `frontend/e2e/real/meetup-pickup.spec.ts`
- `docs/mcr/qa/meetup-pickup-in02.md`

The committed handoff identifies the final commit and its ancestry. The integrator owns integration and master push. No application code, backend, contract, router, manifest, lockfile, infrastructure, original artwork, or `docs/source/` file is changed.

## Provenance and dependencies

The interrupted harness was recovered byte for byte from its private recovery receipt before further edits. Its original source baseline was `edc460943496011873f90f9d60a5931a9570042a`. Recovered hashes were:

| File | Bytes | Recovered SHA-256 |
| --- | ---: | --- |
| `meetup-pickup-helpers.ts` | 18662 | `6a76f39d5a5dbaad96cace43b567bd93d67ceca9a45840e4fa1bea678c98d2fe` |
| `meetup-pickup.spec.ts` | 36040 | `4dd5690defe51520ca6b0b0829fca0a8bee68bcef30cca8a8ed9c82b57951d2a` |

The former checkout stayed read-only. The recovered files were subsequently corrected and validated against the new assignment base above. This is newly authored QA code; it contains no copied legacy rendering or artwork. Existing IN01 configuration, decoder and passive WebSocket observer modules are read-only imports. IN04 is not used as a prerequisite bypass. Existing party and FE20 strict beacon validators are loaded in Node through a non-listening Vite middleware loader; no test module or mock is injected into the product.

The party contract interpreter intentionally rejects unsupported `oneOf` keywords. Beacon audiences therefore use FE20's existing strict union validator, including duplicate explicit-user rejection, rather than weakening that interpreter. New runtime dependencies and dependency installs: none. The locally authorized dependency symlink is removed before handoff.

Contract snapshots at the assignment base:

| Input | SHA-256 |
| --- | --- |
| C04 `operations.json` | `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839` |
| C04 `policy.json` | `81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb` |
| `party.schema.json` | `7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03` |
| `beacon.schema.json` | `68e9249c79644f7f3c137c7eddf3bd1977a33223de5e9dbd7d2c236e20dbf486` |
| `giveaway.schema.json` | `3a5af6301bb9514623aa7d6e68b3ee27b4b89709836801f4c10741f149a7577d` |
| Generated `mcr-domain.openapi.json` | `8c761311d8df7059b48f6c58826565e039bd952481257575ac576815ea9296ac` |

Every runtime report also hashes its own harness, the imported IN01 modules, these contracts, the actual feature validators, and current root router/provider files. `declaredRelease` is an operator declaration bound to prerequisite receipts, not remote deployment attestation.

## Intended real-service behavior

The spec uses two fresh independent browser contexts with actual email/password login, validates returned identities and event bootstrap, and observes actual C05 presence in one room. It then performs the following ordered checks through product controls and actual HTTP responses:

1. Verify composed management/reservation controls and dedicated resources: owner-only open party with capacity two, one initial member, active meeting window, and one available giveaway unit. Verify actual reviewed-place and party-audience options before community mutations.
2. Recipient applies; owner approves the exact pending application. Re-read the real server member list and require active recipient membership.
3. Owner explicitly acknowledges and creates a private party-member beacon. Validate exact owner/event/place, map/place revisions, audience, private description and unexpired bounded lifetime. Recipient must see the actual authorized guidance.
4. Read a rally tied by the server to that beacon. Send actual virtual movement input and observe changed coordinates on the other client. Re-read the rally and require no physical-arrival change.
5. Require the HERE action to stay disabled before explicit physical self-report acknowledgment. Submit the current meeting revision and observe the recipient's HERE state from the owner client. This exercises a declared self-report; it does not establish a person's actual venue attendance.
6. Forward the recipient's original real reservation request to the service, confirm its durable effect, probe same-key replay and new-key duplicate rejection, then abort only the browser's original response. No fixture body is supplied. Require stock to move from one available unit to exactly one reserved unit.
7. Verify the session journal contains only the eight allowed metadata fields and the original key. Reload the actual page, explicitly query original-command status, reconcile the authorized reservation, require journal removal and zero new reservation POSTs, and recheck unchanged stock.
8. Read the actual private pickup response and require current reviewed physical-place authority in the product panel. Request a short-lived code without exporting its plaintext. If asynchronous issuance confirms the command without returning plaintext, read the current plan and express one separate rotation intent with a new key. If that rotation also yields no plaintext, stop with NOT_RUN rather than invent or replay a code.
9. Owner confirms collection using the transient recipient code. Forward the real claim response, reconcile a pending original claim through its status control when necessary, and probe both same-key and new-key repeats. Require the same durable claim receipt, unchanged terminal reservation revision, and one claimed unit with no extra stock consumption. Recipient must lose code and pickup-detail panels after reading current state.
10. Remove the recipient through the owner confirmation UI. Require already-cached beacon guidance to disappear without recipient navigation or reload. Then reload and require a generic no-store unavailable response, avoiding a private-resource existence leak.
11. Close both contexts, require passive socket counts to reach zero, close the browser and clear observer memory.

Only the two bounded reservation/claim probe groups bypass product controls. They forward captured real protected requests to the same configured origin, with no redirect, automatic retry or fabricated response. Credentials, authorization headers, code plaintext, private guidance, resource IDs and raw bodies are never placed in the harness report. These are genuine service calls and can mutate the isolated fixture. The owner must reset/dispose that fixture after a real run; the harness does not erase its audit trail or restore consumed inventory.

## Operator-owned prerequisites

Do not run this against live user data. An operator must supply an authorized isolated event, two distinct test identities, current same-candidate IN01 PASS evidence, qualified BE19/BE23/BE24/FE23/FE25/FE26 evidence, reviewed physical-place authority and dedicated disposable party/giveaway resources. The operator must inspect the evidence before issuing a qualification receipt; the harness cannot independently certify the receipt issuer.

Set `MCR_IN02_CONFIG` to an absolute path to a private, current-user-owned regular JSON file. The configuration and IN02/IN01 receipt/report files reject symlink leaves and group/world access. Use directories mode 0700 and files mode 0600. Input has exactly these keys; angle-bracket values below are placeholders and must not be treated as qualifying evidence:

```json
{
  "version": 1,
  "worldConfigPath": "<absolute private IN01 configuration path>",
  "prerequisiteReceipt": { "path": "<absolute private qualification receipt path>", "sha256": "<64 lowercase hex digits>" },
  "partyId": "<dedicated UUID>",
  "giveawayId": "<dedicated UUID>",
  "place": { "id": "<reviewed UUID>", "mapRevision": 1, "placeRevision": 1 },
  "meeting": { "instructions": "<reviewed private guidance>", "identification": "<reviewed identification>", "ttlSeconds": 900 }
}
```

`worldConfigPath` uses the unchanged IN01 schema: version 1, isolated true, allowed origin, event UUID, 40-hex release revision, exactly two distinct `{email,password,userId}` accounts, and hash-pinned service/art receipts. Account zero owns the party and giveaway; account one is the recipient. TTL must be 120–900 seconds. The meeting and pickup windows, reservation and qualification must remain valid throughout the run.

The hash-pinned qualification receipt must contain exactly:

```text
kind: mcr-in02-prerequisites-v1
status: PASS
isolated: true
origin, eventId, releaseRevision: exact IN01 configuration values
expiresAt: current future UTC timestamp
dependencies: {BE19: PASS, BE23: PASS, BE24: PASS, FE23: PASS, FE25: PASS, FE26: PASS}
resources: {partyId, giveawayId, placeId, mapRevision, placeRevision, meetingSha256}
in01: {path: absolute report path, sha256: lowercase SHA-256}
```

Resource values must exactly match the configuration. `meetingSha256` hashes UTF-8 `JSON.stringify(config.meeting)` in the input's property order. The pinned IN01 report must have kind `mcr-in01-world-gate-v1`, overall PASS, matching declared release and PASS for all 12 IN01 checks. Missing, stale, mismatched or incomplete qualification produces nonzero NOT_RUN before browser allocation. A subsequent failed assertion is FAIL; later steps remain NOT_RUN. PASS requires every step, including teardown, to pass.

At this base, the root does not compose the community-entry management/rally/reservation/pickup adapters, and current physical-place authority remains a prerequisite. No qualifying operator configuration was supplied. Do not manufacture a receipt to work around either limitation. The integrator must complete composition and provide fresh service evidence before a real attempt.

## Commands and observed validation

Run from repository root with the retained frontend dependencies and Node 24.22.2. No dependency installation or browser download is required. Native Chrome is used; `MCR_CHROME_EXECUTABLE` can select an existing executable.

```sh
node frontend/e2e/real/meetup-pickup-helpers.ts --self-test
node frontend/node_modules/typescript/bin/tsc --noEmit --strict --noUncheckedIndexedAccess --exactOptionalPropertyTypes --noUnusedLocals --noUnusedParameters --module ESNext --moduleResolution Bundler --target ES2022 --lib ES2022,DOM,DOM.Iterable --jsx react-jsx --resolveJsonModule --allowImportingTsExtensions --skipLibCheck frontend/e2e/real/meetup-pickup.spec.ts frontend/e2e/real/meetup-pickup-helpers.ts
frontend/node_modules/.bin/oxlint frontend/e2e/real/meetup-pickup.spec.ts frontend/e2e/real/meetup-pickup-helpers.ts
node frontend/node_modules/@playwright/test/cli.js test --config frontend/e2e/real meetup-pickup.spec.ts --list
node frontend/node_modules/typescript/bin/tsc -b frontend --pretty false
python3 scripts/mcr/check-plan.py
python3 scripts/mcr/check-boundaries.py --base ef9422c4d83f8555ee5b48ef693d37fd5bf87e0c --lane frontend --allow-doc docs/mcr/qa/meetup-pickup-in02.md
```

For an authorized real run, use a freshly created private output directory and private umask. Do not enable trace, screenshots, video, HAR, request logging or DOM/error-body exports:

```sh
umask 077
node frontend/node_modules/@playwright/test/cli.js test --config frontend/e2e/real meetup-pickup.spec.ts --workers=1 --retries=0 --reporter=line --output="$MCR_IN02_PRIVATE_OUTPUT"
```

`MCR_IN02_PRIVATE_OUTPUT` must be an operator-created absolute directory with mode 0700. The harness writes its report mode 0600 and attaches only the redacted report. Keep all runner diagnostics private; verify resulting directories mode 0700 and files mode 0600 before sharing even the sanitized result. The report contains fixed check codes, counters, stock counts and hashes, without raw assertion errors. Contexts close before the final stable classification error is thrown.

Observed on 2026-11-04:

| Check | Exit | Evidence / limitation |
| --- | ---: | --- |
| Native helper self-test | 0 | 57 offline checks PASS; explicitly reports real gate NOT_RUN |
| Explicit strict E2E TypeScript | 0 | Covers both E2E files, which application tsconfig does not include |
| Scoped oxlint | 0 | No remaining warnings after cleanup |
| Playwright discovery | 0 | Exactly one test in this file |
| Application TypeScript build mode | 0 | Current application compiles; no runtime implication |
| Vite production build from `frontend/`, external temporary outDir | 0 | 405 modules; existing large-chunk advisory; no deployed service |
| Planning provenance check | 0 | Source archive checksums unchanged; all 112 required tasks and 61 required IA entries preserved |
| Missing `MCR_IN02_CONFIG`, real Playwright runner | 1, expected | NOT_RUN / MCR_IN02_CONFIG_REQUIRED; zero contexts, mutations, probes |
| Private unqualified JSON `{ "isolated": false }`, real Playwright runner | 1, expected | NOT_RUN / INVALID_CONFIG; zero contexts, mutations, probes |
| Three-path frontend boundary check and `git diff --check` | 0 | Assigned paths only; no cross-lane import |
| Actual two-client service journey / device / load | NOT_RUN | No qualified isolated runtime, credentials or physical authority supplied |

The negative runs' reports and runner logs are kept outside Git under a private temporary validation directory; all of its directories/files were verified 0700/0600. They are diagnostic rejection evidence, not IN02 PASS evidence. No retry converts missing prerequisites into a skip or success.

## Remaining acceptance boundary

The harness covers two roles and one dedicated unit. Its same-recipient duplicate requests are not five independent competing recipients, a PostgreSQL race suite, a wrong-code/guess-limit suite, a mobile device result or a load test. BE24 qualification is a prerequisite here; this journey does not independently test image upload. Ordinary party/giveaway authoring is outside this dedicated preprovisioned-resource journey. Beacon/rally linkage must come from actual server authority; the harness does not infer a link from recency or virtual coordinates. A bounded source delivery can complete while IN02 remains NOT_RUN.
