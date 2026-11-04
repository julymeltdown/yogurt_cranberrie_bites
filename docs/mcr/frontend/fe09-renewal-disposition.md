# FE09 expiry, refresh and bootstrap disposition

Original FE09 frontend implementation criteria remain **SATISFIED, task-local**.
No confirmed original-criterion gap was found at the qualified source. This
delivery changes no product code. The new combined boundary evidence is
**MOCK_READY**; it does not reopen the prior production-refresh disposition or
claim whole FE09 release, IN01, live expiry or device acceptance.

## Assignment and current goal

Exact base: `39c68d1dcd85a618ec0c56287d8451c6ffe13dbd`, independently fetched and
matched by a separate remote master lookup before authoring. Isolated checkout
`fe-auth-natural-renewal-204`, branch `codex/mcr-fe-FE09-renewal-disposition`.
The checkout was clean and all four new prefixes absent. Initial actual
`get_goal` returned null. A new unbudgeted aligned goal was created for chat
`01a1a204-c583-7670-965a-4891b8b1289e`, `createdAt=1793778508`; immediate and
subsequent readbacks returned ACTIVE. The tool exposes no separate goal UUID.
It remains active through committed Root153 publication, concrete owner fixes,
fresh independent ancestry/all-payload verification and clean STOP/lease return.
A sent handoff is not publication or completed goal evidence.

Community is sole allocator. Root recorded this exact lease before writes:
minimal confirmed lifetime/renewal/bootstrap hunks in existing `session.ts` only
if warranted, plus the four new test/e2e/evidence/report prefixes. No such product
hunk was warranted. Old Meet/FE32 branches and returned leases stay untouched.
No other Auth model/UI/old test, shared HTTP/privacy/cookie policy, App/main,
router, contract, backend, manifest, lockfile or original source was edited.
Only designated Root153 publishes this assignment; no king/SL review gate exists.

## Original criterion finding

Authorities read before implementation: specification §7.2 and original FE09 row,
project assessment, execution plan, current `production-refresh.md`, historical
`session-sign-in.md` and `auth-contract-review.md`, canonical generated public
core contract, and BE03 browser refresh contract. Analysis/history does not
silently amend the specification. Current code, transport, privacy, cookie
policy, bootstrap decoder, production entry and sign-in rendering were also read.
All source hashes/revisions are in the evidence provenance file.

| Original requirement | Current source and disposition |
| --- | --- |
| Memory-only canonical credential | `createMemorySession` holds bearer/user/event in closure; identity replacement advances generation and aborts the previous signal. No credential persistence or second Auth owner is added. |
| Single-flight refresh / concurrent401 | Production `main.tsx` enables `qualifiedRefresh:true`. Controller `refreshFlight` coordinates cookie policy; FE01 joins/latches refresh by generation/token version. Delayed old-token401s reuse the newer token. Only one safe protected GET replay is permitted. Retained native production-refresh proof remains VALID at its documented source/producer pins. |
| Explicit refresh/bootstrap adapter | Controller exposes coordinated refresh and `loadBootstrap`; bootstrap uses the agreed protected endpoint, strict contract/context/bundle checks, abort and generation fences before returning a nonsecret shell projection. This is an explicit adapter, not a promise of startup cookie restoration. |
| Safe bootstrap and truthful states | Bootstrap503 maps to shell unavailable while authenticated identity remains current. Bootstrap401 after bounded recovery clears authority and reports unauthenticated. Malformed/cross-context bootstrap fails closed. Ordinary feature403 preserves identity as a denied operation rather than pretending credentials expired. |
| Login/logout/provider availability | Current sign-in renders capability, invalid-input, credentials, throttling, ended-session and unconfirmed logout states; provider controls remain disabled honestly. Local logout revokes before server/lock completion; only expected204 confirms server logout. Retained sign-in/provider qualification is not relabeled a new browser run. |
| Complete private cleanup and cancellation | Identity revocation aborts scoped transport/privacy work, scrubs active observers before query eviction, removes mutation cache, and executes registered draft/subscription cleanup. Late login/refresh/bootstrap cannot reauthorize a changed generation. Existing world scope-abort cleanup remains consumer-owned. |
| Safe replay/privacy | Mutations and explicit secret reads never use GET recovery. No auth endpoint enters the generic handler; refresh409/uncertainty is not retried and removes authority. No cookie/bearer sharing or inferred recovery from a peer's rotated cookie. |

The reported observations are supported behavior, not a demonstrated original
criterion failure: `start()` performs capabilities discovery; it does not restore
a cookie. `expireIn()` clears memory authority at the issued local lifetime;
it does not initiate renewal. Thus a natural local expiry normally prevents a
later request from reaching401 recovery. Refresh is reactive to an eligible
protected GET401 while local identity still exists, or its explicit coordinated
entry; proactive renewal and reload resume are not implemented or promised here.

Neither original FE09 nor §7.2 explicitly requires proactive scheduling,
automatic cookie restoration after reload, or successful renewal at natural
local expiry. The current owner report explicitly describes those exclusions.
No fresh source/contract counterexample was supplied by Social, and the current
combined boundary tests passed. Therefore no speculative `session.ts` rewrite,
frontend decoder fix or new successful-renewal requirement is justified.
The constructor flag's old test-only comment is historical wording superseded
by `production-refresh.md` and actual main composition; no cleanup-only model
edit is acquired under this minimal confirmed-gap lease.

## Fresh targeted evidence

Newly authored fixture suite:
`frontend/src/features/auth/__tests__/fe09-renewal-disposition/boundaries.test.ts`.
It uses actual controller/transport/QueryPrivacy implementations, public-contract
fixtures and the retained isolated CookiePolicy helper. It is synthetic HTTP,
not native cookie, browser reload, real JWT, elapsed expiry or real-service proof.
No clock/TTL change is made by the six new cases.

1. Qualified startup and a freshly created controller remain anonymous until
   explicit login; refresh/bootstrap cannot silently restore anonymous authority.
2. Cancelling bootstrap during a shared rotation rejects that bootstrap without
   replay, while another read renews once and retains the current private observer.
3. Held old refresh completion cannot overwrite a subsequent explicit login
   after logout, revive the old generation or clear the new identity.
4. Bootstrap/read409 fanout makes one refresh dispatch, no read replay, scrubs
   observer/cache/draft/subscription authority, refuses anonymous retry and still
   permits explicit logout. The isolated policy does not certify native peer locks.
5. Bootstrap503 truthfully reports shell unavailability while canonical Auth
   stays current and another safe read remains possible.
6. Bootstrap401 remaining after one refresh makes exactly two bootstrap attempts,
   then clears authority rather than looping.

All final checks exited0. Node24.22.2 was the retained pinned executable; no
installation, dependency resolution, lockfile or configuration change occurred.
Run commands from `frontend/` except the final planning check:

| Command | Result |
| --- | --- |
| `node node_modules/vitest/vitest.mjs run src/features/auth/__tests__/fe09-renewal-disposition/boundaries.test.ts --cache=false --maxWorkers=1` | PASS; 6 tests /1 file |
| `node node_modules/vitest/vitest.mjs run src/features/auth/__tests__/session.test.ts src/features/auth/__tests__/sensitive-read.test.ts src/features/auth/__tests__/cookie-policy.test.ts src/features/auth/__tests__/domain-request.test.ts --cache=false --maxWorkers=1` | PASS; 75 tests /4 files |
| `node node_modules/typescript/bin/tsc --project src/features/auth/__tests__/fe09-renewal-disposition/tsconfig.json --pretty false` | PASS; strict noemit across imported source |
| `node node_modules/oxlint/bin/oxlint src/features/auth/__tests__/fe09-renewal-disposition` | PASS |
| `python3 scripts/mcr/check-plan.py` | PASS; planning only |
| `git diff --check` | PASS before handoff |

The initial new-suite invocation from repository root also passed6 cases; the
recorded reproduction/log uses the frontend invocation with retained config.
No check failed. The retained session suite includes its pre-existing one-second
synthetic expiry fixture. That is deterministic unit coverage, never genuine
900-second evidence. Existing production-refresh401/409/native-policy evidence
remains separately attributed; its 624 regressions and backend12-phase receipt
were not blindly rerun or relabeled current in this task.

Evidence: [checks](../evidence/fe09-renewal-disposition/checks.json),
[source pins/provenance](../evidence/fe09-renewal-disposition/source-provenance.json)
and five compressed logs. Product build, native browser, genuine elapsed expiry,
real service, physical devices and IN01: **NOT_RUN** here. No product code changed,
so a new production build or browser slot would not resolve a further identified
original-criterion risk. No broad repository-green claim is made.

## Genuine-window request: transparent authority correction

The actual Main coordination source was verified by BackendSL21d:
chat `01a19e5c-e9c6-7973-a53b-a6d19141620f`, turn
`01a1a56d-26d2-77e3-9344-83b07fe83d62`, send call
`exec-28e92149-b52d-4aea-aa16-66447e353651`. It requested resource coordination
with Root1fc and original IN01 consumer202 for remaining genuine expiry/refresh
coverage **under applicable existing task/gate criteria**, with no forced expiry,
synthetic401 or shorter passing slice and no extra unrelated criterion.

Earlier relays overstated that coordination as a new human requirement and
Community temporarily assigned a mandatory new15-minute follow-up to this same
owner/goal. It was preserved as pending, never performed or falsely completed.
After21d checked the source, Community withdrew that overstated allocation.
Main then explicitly clarified at07:55UTC that the direction was conditional on
EXISTING requirements, not a new human requirement, and withdrew the proposed
long-runtime follow-up as a required gate. No900-second IN01 duration, newly
successful automatic renewal, proactive refresh or cookie/reload restoration is
required. This correction supersedes the earlier relay; the historical correction
is preserved without treating that relay as direct human authority. No goal was
reset, paused, recreated or prematurely completed to relabel the scope.

Current disposition: genuine-window observation **NOT_RUN, informational only**.
No further Main reconciliation or runtime reservation is required for this bounded
delivery. A genuine expiry/cleanup observation, if encountered during an otherwise
justified existing test, must report the actual behavior honestly; its absence
must not independently block original IN01. No slot, active qualified real Auth
descriptor or account-entry handoff was supplied or invented. Root1fc owns backend
availability/runtime;202 remains archived original IN01 consumer and was not woken
for this correction. The [conditional observation notes](../../../frontend/e2e/fe09-renewal-disposition/proof-plan.md)
are preserved as future reference only, not an executed runner, required gate,
assignment, or retained source/runtime lease after STOP.

ActorA customization GET/me504/noPUT remains BackendRoot's separate diagnosis.
Fourteen backend serial/concurrent200 probes do not establish App-activeWorld
behavior or resolve that failure. No frontend decoder defect or whole IN01
closure is inferred. Original required scope stays intact; no product scope cut,
new duration gate or waived established criterion is claimed.

## Delivery boundary

Commit this exact scoped report/tests/evidence to Root153 for normal publication,
then independently fetch/compare remote ancestry and every latest delivered file.
At this handoff those steps remain pending and the same goal remains ACTIVE.
Any concrete finding in owned files returns to this implementation owner; no
extra king/SL technical review stage is requested. The remote receipt and actual
goal closure/STOP are reported through the normal handoff, without a receipt commit.
