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
