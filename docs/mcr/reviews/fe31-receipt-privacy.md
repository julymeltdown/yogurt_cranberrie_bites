# FE31 independent receipt and privacy review

Review disposition: **PASS for the targeted FE31 client receipt/privacy criteria**
on fix candidate `61136cdc10a82a74bd0288a5f0b443a9cf2c4696`. All eight initial
findings are resolved by source fixes and unchanged independent probes. Initial candidate
`310dd65d655c72865ca448da89b30a308ed6cc82`, QA branch `codex/mcr-qa-FE31`,
checkout `/Users/lhs/Desktop/mcr-worktrees/qa-social-sharing`. This sole QA reader
writes only this report. Application fixes belong to the FE31 owner; no source,
fixture, contract, manifest or lockfile was edited by QA. The former FE29/FE30
writer was stopped at clean `3477041` before this assignment.

## Authority, ancestry and method

Read root/frontend working agreements, specification §§2–5 and 8.3, FE31/IN03,
assessment, execution and current parallel-delivery rules. Scope is a targeted
client review, not main/SL approval or real-service acceptance. Verified ancestry:
`5c95ed4` → `c5a23cb0416e2b3ab8addecf4ca4fe2ae39a5445` (separate canonical
Auth single-attempt sensitive-read prerequisite) →
`b5c7468dc1f864199e09ebf59cce4b95086482fb` (actual-fetch-signal fixture repair) →
`bc65a30187799d3a65ed38003e659558f0ced1c5` → reviewed `310dd65`.

Review directly inspected closed DTO decoders, controller/runtime, consent and
handle UI, canonical privacy/HTTP/Auth dependency, frozen social/common schemas,
operations and policy. Independent probes live only at
`/tmp/mcr-fe31-qa.a62uiW/`; this directory is ephemeral and is not release evidence
storage. Reproduction outlines and observed outcomes are recorded below so the
findings do not depend on that directory surviving. No real/private user data was
used: account names and IDs are synthetic fixture values.

Delivery authorized one native Chrome slot and a nontracked `frontend/node_modules`
symlink to its existing qualified install. All QA caches/artifacts are redirected
to owned temporary paths. No dependency installation or shared source mutation.
Node 24.22.2, retained frontend lockfile; native Chrome 156.0.8078.80. Browser/server
were closed and slot explicitly released after the baseline run.

## Initial candidate findings

Line references below identify the original `310dd65` source. Fix/recheck results
are recorded separately for the exact subsequent candidate; they are not inferred
from these baseline results.

| ID | Priority | Original source | Reproduction and observed failure |
| --- | --- | --- | --- |
| R1 | P1 | `model/controller.ts:127` / `locked` / `dispatch` | Prepare a consented create, return 202 PENDING with original key, then a valid FAILED VERSION_CONFLICT receipt. Select consent and create again. Observed **two POSTs with a new key**, expected one. FAILED clears `command` and unlocks; C04 status recovery explicitly says terminal feedback does not permit minting a replacement key. |
| R2 | P2 | `model/controller.ts:110` | Return SUCCEEDED receipt created 08:00, expired 08:30, clock 09:00. Observed mutation `succeeded`, expected preserved uncertainty. Validation checks expiry after creation but never against current time; retired/expired metadata cannot unlock new-key inference. |
| R3 | P1 | `model/controller.ts:120` / `loadRequest` | Create receipt names requestID with minimum revision 2. Resource reread returns that ID and current requester, but recipient 99, intro 98, introRevision 99, recipient-handle-version 99 and revision 1. Observed ready detail with this mismatched DTO. Receipt success fires an unbound reload after unlocking; reread checks only ID/participation. Bind original recipient/intro/consents/versions and receipt minimum before materializing verified current state. Stale post-revoke APPROVED reads need the same protection. |
| R5 | P1 | `model/controller.ts:132`–138 / `model/runtime.ts:39` | A subscriber changes canonical account upon `sending`. Submit accountA's private handle draft. Observed one PUT with **new-account bearer token and accountA's old plaintext draft**. Notification invalidates the frozen command scope, but runtime captures the replacement scope before dispatch. Expected zero PUTs; fence immutable command scope/version after callbacks and at request construction. |
| R6 | P1 | `model/controller.ts:31` /34 | Load/reveal a synthetic private name; register a throwing subscriber before a mounted `useSyncExternalStore` view. Apply canonical `privacy.changePolicy(...,'block')`. Internal snapshot clears, but mounted output still shows `private_name`. Both cleanup/sync notification loops stop before React's subscriber. Isolate every observer so one failure cannot preserve another observer's secret DOM. |
| R7 | P1 | `model/controller.ts:218`–221 / `schedule` | Qualified sensitive port decodes an allowed name expiring at clock+1s, then returns after clock reaches that exact expiry while request remains valid. Observed ready secret `short_lived_name`, expected null. Final commit rechecks request expiry but omits returned reveal expiry; timer ignores that already-past deadline. Recheck both at final publication. |
| R8 | P2 | `model/controller.ts:114` | Original create202 followed by FAILED reason `MURAL_COOLDOWN`. Observed accepted `failed`, expected reject/uncertain: globally registered reason is not registered for the original create operation. Validate per-operation reasons and receipt state invariants. |
| R9 | P2 | `ui/SocialSharing.tsx:21` | With HandleSettings mounted, open accountA removal confirmation, switch canonical account to B, then load B handles. Observed old Confirm removal still present. Bind confirmation intent to canonical scope/resource version and discard it on invalidation. No automatic deletion is claimed. |

R4 control: loss of access during an already-dispatched pending mutation does
become recoverable `uncertain` under the original identity/key. This independent
probe passed; no defect is claimed for that path.

All eight failures were sent directly to FE31 owner and delivery with exact
source/reproduction and the temporary probe/config paths. The owner acknowledged
R1/R2/R3/R5/R6 and began scoped fixes; R7/R8/R9 were sent as additional reproductions.
QA has applied no application fix. Initial probe suite: nine tests, eight
expected-behavior failures and one passing control, exit 1.

## Baseline validation and observations

| Check | Exit / result |
| --- | --- |
| Independent existing combined test selection | 0, 384/384 tests; source selection below |
| Nine independent adversarial probes | 1, eight failures above / R4 passed |
| Native fixture replay at 360/390/430 | 0; 33 requests, 7 writes, 3 explicit reveals per width; zero page errors |
| Source boundary/style inspection | No backend imports, browser storage/Query insertion, auto-follow action, decorative left border/rail or legacy artwork in the feature |

Exact commands (pinned Node binary prepended to PATH):

```sh
node frontend/node_modules/vitest/vitest.mjs run --config /tmp/mcr-fe31-qa.a62uiW/baseline.config.mjs src/features/social-sharing src/features/auth/__tests__/sensitive-read.test.ts src/features/auth/__tests__/domain-request.test.ts src/features/auth/__tests__/session.test.ts src/features/auth/__tests__/enrollment.test.ts src/shared/api/__tests__ src/shared/query/__tests__ --reporter=json --outputFile=/tmp/mcr-fe31-qa.a62uiW/baseline-results.json
node frontend/node_modules/vitest/vitest.mjs run --config /tmp/mcr-fe31-qa.a62uiW/vitest.config.mjs --reporter=json --outputFile=/tmp/mcr-fe31-qa.a62uiW/initial-results.json
node /tmp/mcr-fe31-qa.a62uiW/browser-initial.mjs
python3 scripts/mcr/check-plan.py
```

The planning check exited 0 and preserved all 13 source-archive hashes; its scope
is planning/provenance only. Initial test runner setup needed explicit Vite
filesystem allow paths for macOS `/private/tmp`; an earlier zero-test setup error
was discarded. The recorded adversarial result is an actual executed nine-test
suite, not that setup failure.

The combined selection used `vitest run` with one worker for
`src/features/social-sharing`, Auth `sensitive-read`, `domain-request`, `session`,
`enrollment`, `src/shared/api/__tests__` and `src/shared/query/__tests__`.
Configuration was temporary, with `root` set to the QA frontend and a temporary
cache. JSON output: `baseline-results.json` in the owned temporary directory.

The independent native replay copied the committed smoke runner into the temporary
directory, changing only absolute source/output/cache paths. It reads unchanged
candidate UI/controller/canonical session and serves deterministic same-origin HTTP.
It passed unchecked requester choices, metadata-only create, recipient selected
subset plus separate reciprocal consent, explicit keyboard reveal, hide, revoke
confirmation/scrub, uncertain status without replay, expiry, normalized handle
replace/remove, policy cleanup, empty Query/mutation/storage and no overflow.
Baseline output/screenshots: `native-initial/`. These passing happy-path fixtures
do not negate the independently reproduced failures above.

Canonical Auth prerequisite inspection and existing negative tests confirm that
both `requestSensitive` and generic request routing for `revealConcertSocialHandle`
use one protected no-store attempt, without 401 refresh/replay. The feature refuses
reveal when no qualified sensitive port is supplied. Existing tests cover unmount,
StrictMode, stale logout/event/policy/revoke/expiry responses, wrong counterpart/
version, private/unknown DTO fields, terminal states and cursor loops. Public
metadata omits handles; reveal stays transient feature state and never enters a
Query cache. Backend current-authority rechecks remain essential regardless.

## Fix candidate and independent recheck

Resumed the existing aligned unbudgeted goal after recovery; `get_goal` returned
actual ACTIVE. The preserved report hash was
`a376dc254d8631e7aa069a43e940ca1c4164ed1509dc89d78fd46991169c3dd3`.
Inspected the exact source-fix commit and fast-forwarded with
`git merge --ff-only 61136cdc10a82a74bd0288a5f0b443a9cf2c4696` (exit 0), retaining
the report and authorized ephemeral dependency symlink. This commit is a direct
child of 310dd65. Its six changed paths are the feature controller, runtime, UI,
controller tests, component tests and existing lane documentation. No contract,
Auth prerequisite, lockfile, backend or root composition changed. QA did not
patch any application file or import an uncommitted writer snapshot.

The **same nine-probe file** ran unchanged against 61136cd: **9/9 PASS, exit 0**.
The original executed result is **8 failures/1 passing control, exit 1**. No test
was removed or assertion weakened. R3's preserved assertion already waits for
`detail.phase` to leave loading and checks `detail.data === null`; its actual
fixed outcome is `detail.phase=error`, null data and an uncertain original
command. No readiness-wait adaptation was needed or made during this recheck.

| Probe | 310dd65 | 61136cd | Source correction inspected at 61136cd |
| --- | --- | --- | --- |
| R1 | FAIL, second POST/new key | PASS, one POST | `controller.ts:159` retains the original command in blocked state; write locking includes the retained command. Current activity inspection remains possible. |
| R2 | FAIL, expired receipt accepted | PASS, uncertain | `controller.ts:118` rejects receipt expiry at/before the current clock. |
| R3 | FAIL, mismatched/stale DTO materialized | PASS, null/error plus uncertainty | `controller.ts:139` awaits an authorized resource read and validates original relationship, introduction, per-platform versions, reachable state and receipt minimum revision before success; persistent metadata guards also reject later stale projections. Handle reads enforce platform/version/configuration. |
| R4 | PASS control | PASS control | Access interruption still preserves recoverable uncertainty and original command identity. |
| R5 | FAIL, old plaintext sent under new identity | PASS, zero PUTs | `controller.ts:165` captures the command before notifications and fences after hide/sending; runtime retains required scope and verifies its lease before request creation/dispatch. |
| R6 | FAIL, private DOM retained | PASS, mounted output hidden | `controller.ts:33` notifies an observer snapshot with per-observer exception isolation, including cleanup/dispose. |
| R7 | FAIL, already-expired name displayed | PASS, null secret | `controller.ts:261` checks the returned reveal deadline again before publication, alongside the request deadline. |
| R8 | FAIL, unrelated reason accepted | PASS, uncertain | `controller.ts:122` uses the original operation's registered reasons instead of the global registry. |
| R9 | FAIL, old confirmation transferred | PASS, no old confirmation | `SocialSharing.tsx:21` binds pending removal to canonical scope/access lifetime and handle version; revoke confirmation similarly binds scope/request revision and rechecks at click. |

The affected owned feature test selection was also independently executed on
61136cd: **75/75 PASS, exit 0**. The implementation owner's separate 405-test
combined run, typecheck and scoped lint are recorded in its committed lane
record; they are attributed to that owner, not claimed as a new QA reproduction.
The earlier independent 384-test combined baseline remains pinned to 310dd65.

Exact recheck commands, from the QA repository with pinned Node on PATH:

```sh
node frontend/node_modules/vitest/vitest.mjs run --config /tmp/mcr-fe31-qa.a62uiW/vitest.config.mjs --reporter=json --outputFile=/tmp/mcr-fe31-qa.a62uiW/fixed-results.json
node frontend/node_modules/vitest/vitest.mjs run --config /tmp/mcr-fe31-qa.a62uiW/baseline.config.mjs src/features/social-sharing --reporter=json --outputFile=/tmp/mcr-fe31-qa.a62uiW/fixed-owned-results.json
```

| Retained temporary artifact | SHA256 |
| --- | --- |
| Unchanged independent `probes.test.ts` | `4868302feb9f700cc737c309432c35c73bda79d65c5a68b4ae7c6525555d8feb` |
| Original nine-test `initial-results.json` | `1829230c90e155d0ac427d3458425bd2e70e54dab09de2acb546d124ca325a62` |
| Fixed nine-test `fixed-results.json` | `f2def5d54ed93281945103bc46a9d5fcde087da17042254aefd901cce61d20e0` |
| Fixed 75-test `fixed-owned-results.json` | `ca96027c89b2276f675618ef343817592656d9d7ee6d93a52fe78a7fb934cb53` |

The original independent native 360/390/430 replay is baseline 310dd65 evidence.
It is **not** relabeled as 61136cd browser evidence. Separately, the source owner
committed refreshed native evidence as `b118083` (only lane documentation and
`__tests__/evidence/browser-report.json`). QA inspected that committed report and
verified that every recorded source hash matches the unchanged corresponding
file at 61136cd. The owner records exit 0, Chrome 156.0.8078.80, all three widths,
33 requests / 7 writes / 3 explicit reveals per width, no browser errors or
private Query/storage entries, and an isolated feature build passing. These are
owner-executed receipts, not an independent QA browser rerun. No second QA browser
lease/run was started during this recheck. CSS is unchanged; the scoped UI
lifecycle/confirmation fixes are exercised by the same independent probes and
the 75 feature tests.

## Criteria disposition

| Reviewed client requirement | Authoritative evidence | Disposition |
| --- | --- | --- |
| Requester opt-in, recipient offered subset and separate reciprocal consent; no plaintext before approval | Closed C04 decoders, initial independent native flow and existing tests; fixed 75-test selection | PASS |
| Explicit transient reveal/hide/revoke, stale handle/version and expiry boundaries | Controller guards, wrong-counterpart/version/terminal/late-response tests, R3/R7 and fixed 75 tests | PASS |
| Original key/scope recovery without replay or replacement-key inference | R1/R2/R3/R4/R8 unchanged recheck and immutable command/receipt source inspection | PASS |
| Canonical user/event/policy/logout invalidation and mounted private-state cleanup | Original late-response tests plus R5/R6/R9 unchanged recheck and required-scope runtime | PASS |
| Sensitive401 no refresh/replay, no Query/storage insertion | Unchanged Auth prerequisite, independently passing baseline sensitive tests/native empty caches and direct source inspection | PASS |
| UI lifecycle and no prohibited left accents | R6/R9 and fixed component tests; unchanged CSS inspection and baseline native rendering | PASS |

No concrete client finding from this targeted allocation remains open. This
clears the reviewed FE31 client criteria on 61136cd; it does not add an approval
gate or claim a corresponding backend prerequisite. Integrator-owned routing and
real vertical-flow qualification remain separately owned.

## Pinned input hashes

| Input at 310dd65 | SHA256 |
| --- | --- |
| C04 social schema | `b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2` |
| C04 common schema | `303f3019611c636679282d6c29106deab83d7c0c9717bd8b9ba526d2ff865a35` |
| C04 operations | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| C04 policy | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |
| Canonical Auth session | `dae6680d1f4a0600b6d3012639fd82c9317d63d1a55eb856e5c4dc81f9a5dc53` |
| Retained pnpm lockfile | `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6` |
| Social controller | `86e629eed8ea9bcbb32cb3c3967896fbce8b6aed236bf29aed521a0d470cefe2` |

The social/common/operations/policy, canonical Auth and lockfile hashes above
were rechecked and are unchanged at 61136cd. Fixed feature hashes:

| Input at 61136cd | SHA256 |
| --- | --- |
| Social controller | `a798c5f50cf278a998517c552fc678b818c04ba569e3ff53174560677697e18c` |
| Social runtime | `999c074719e88f889130fd7726dbccbc969d2cab15c36d0813f1f54001652b53` |
| Social UI | `51b410255fd7a2bc21c34291d7fb091b830bd6b22e44f5016c7dd8415753cfb6` |
| Closed social decoder, unchanged | `eba5001637bc1b1cc5fcb98ec764cc5aa1ac4faf171f8fabac519e2f676e5426` |
| Sharing CSS, unchanged | `4213c99408d0feaaf2a8e39b4ad5876bc7fadfc8123bcdd03562f2ab0c47823c` |

## Decision and remaining qualification

Initial 310dd65 was not cleared because of eight concrete client failures.
Fix candidate 61136cd is **cleared for the targeted FE31 client receipt/privacy
criteria**: eight findings resolved and the control retained, all nine unchanged
probes and 75 affected feature tests pass. Application status remains
MOCK_READY/CONTRACT_READY; this report-only review is complete. No generic
main/SL code approval is needed. Final delivery is the one-file report commit on
top of 61136cd, handed directly to the current Main/integrator and Social; the
former delivery chat is retired. The authorized ephemeral dependency symlink is
removed before the clean stopped handoff; no source or dependency artifacts enter
the report commit.

Real gateway/DB, backend reciprocal grants/current authorization, genuine provider
or account data, block propagation across two real users/HTTP/WS, physical mobile
devices, QA04 and IN03 are **NOT_RUN**. Native synthetic two-actor fixture behavior
is client MOCK_READY/CONTRACT_READY evidence. FE31 explicitly permits frozen
contracts and deterministic fixtures without corresponding backend completion;
its own client criteria have been dispositioned above on actual fixture/source
evidence after the concrete fixes passed.
Those fixtures cannot certify real multiuser IN03, QA04, device or release gates.
