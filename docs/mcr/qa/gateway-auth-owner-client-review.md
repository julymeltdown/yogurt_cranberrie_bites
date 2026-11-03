# Gateway Auth owner-client independent review

Date: 2026-11-03. **Source/evidence review complete; no actionable bounded
finding identified. Parent handoff/disposition pending.** This is review of an
internal BE04 prerequisite, not whole BE04, real email-only participation,
production owner orchestration or release acceptance.

## Exact candidate and scope

Candidate `868d02c98f69b3c04c0b6e61f06a6408b2cdf747`, sole implementation commit
after `e6efa4c4d6888415835f732de1b1f8c89aa9d460`. QA began clean on
`codex/mcr-qa-gateway-auth-owner` at that candidate in
`/Users/lhs/Desktop/mcr-worktrees/qa-gateway-auth-owner`.

Read authorities include root/backend working agreements, the specification's
canonical identity and BE04 scope, current assessment/execution references,
`docs/mcr/identity-admission-decision.md`, the client handoff document and actual
owning client/context/platform/proto/guard/handler sources. The selected direction
requires fresh authenticated owner reads, one original budget and rechecks before
eventual signed Concert dispatch. It does not make a database observation a
transferable proof or lifetime.

Exactly four worker paths differ from the base:

| Path | Change |
|---|---|
| `backend/services/api-gateway/src/adapters/out/auth_client/concert_admission.rs` | New 252-line production child module |
| `backend/services/api-gateway/tests/concert_admission_transport.rs` | New 920-line guarded transport suite |
| `backend/services/api-gateway/src/adapters/out/auth_client.rs` | Exactly one module export; no legacy behavior edit |
| `docs/mcr/backend/gateway-auth-owner-client.md` | New 175-line handoff |

QA's only tracked output is this report. No application, frontend, backend,
contract, shared fixture, dependency, central document or source-archive edits
were made. No Rust/Cargo command, target access for execution, database, Docker,
service, unrelated Auth suite or build was run. The retained Rust target belongs
to the Profile worker; the parent owns eventual combined Rust qualification.

## Authorization and observation review

`ConcertAccessContext` remains the existing sealed input. Its private fields and
lack of serialization/construction APIs prevent ordinary callers manufacturing
it from unsigned claims. `JwtVerifier::verified_access_claims` actually uses
configured issuer keys and RS256 verification, required audience/issuer/expiry/
subject, access-token type and rejects embedded or remote key references. The
strict projection requires canonical nonnil user/family UUIDs, signed integer
times, original stop and captured monotonic JWT stop. It does not project roles,
provider claims, email or bearer bytes. These sources are unchanged from base.

The new method accepts only a borrowed context (`concert_admission.rs:153–157`).
Actor and session family come from its checked accessors, then canonical UUID
strings are put into the protobuf message. Optional `BindingReference` is closed
EMAIL/GOOGLE/APPLE/X, canonical nonnil UUID and epoch `1..=9007199254740991`;
fields are private and its debug output is fixed/redacted. Its constructor
establishes shape, not current Auth ownership.

`AdmissionObservation` has private fields, no constructor, Clone or serde
implementation. It borrows the exact context passed to the read and retains
the optional requested binding shape without a rebind API. Every exposed
fact/reference/timestamp accessor calls the context check. The six compile-fail
doctests comprise three retained context cases and three new observation cases;
their snippets target private construction and absent serde traits, and the
required imports/dependency exist. QA inspected those tests and their log rather
than rerunning a compiler. No trait/test-only constructor bypass was found.

Inactive account, ended session and NotBound remain explicit facts. The new
module has no boolean admission-grant API and does not invent Profile/event
eligibility or a revocation guarantee.

## Configured transport and absolute time bounds

The method uses `self.client.clone()`, preserving the configured channel,
service identity interceptor, tracing wrapper and `ActorDelegationClient`.
`request_for` supplies a delegated actor extension. The existing delegation
client collects and signs the actual protobuf frame with audience Auth, method,
actor and exact body digest; the new code does not manufacture an independent
signer or send browser credentials. Trace production code records bounded
method/outcome/duration, not peer diagnostic strings or payloads.

At `concert_admission.rs:163–179`, checked context and its captured
`AuthorityRpcDeadline` are clipped by the retained GatewayAuth local ceiling
(900ms edge minus its existing 100ms margin), then by an already-present task
deadline. The request timeout is recomputed against the absolute stop. One
`timeout_at(stop, generated_future)` under `call_deadline::scope` encloses
readiness, encoding, body collection/signing, dispatch and response decoding.
The configured trace wrapper propagates the remaining scoped time; nested scope
cannot extend an outer deadline. There is no cache, retry or refreshed context.

After the await, the client rechecks context and absolute stop before projecting,
then checks again before return. Late noncooperative completion cannot become a
successful observation merely because its future returned ready. Observation
accessors subsequently recheck the borrowed context. A successful database read
does not reset that context or establish a new observation TTL.

These bounds were traced through actual `AuthorityRpcDeadline`, context,
call-deadline, generated-client configuration, signer and server guard sources.
The new suite separately exercises readiness and held/delayed/blocked replies;
it does not separately induce slow signing or provide an explicit shorter-outer-
task-deadline transport case. Source checks cover their enclosure/clipping;
inspected logs are not independent runtime qualification of every timing branch.

## Response and safe-error projection

The per-call generated-client clone caps decoded message length at **128 bytes**
before protobuf discards unknown fields. Only 1/NotRequested with no requested
binding, or 2/Bound and 3/NotBound with a requested binding, can project.
Unspecified/unknown/inconsistent states fail. `DateTime::from_timestamp_micros`
rejects unrepresentable observation times; zero/old representable times remain
observations. No unstated age, future-clock or freshness policy is introduced.

Upstream unauthenticated/permission-denied results map to fixed Denied;
deadline errors stay explicit, decode/size-like statuses become InvalidResponse,
and other upstream errors become fixed Unavailable. Raw status messages/details
are dropped. The new module stores no private claims, diagnostics or bearer, and
its observation/debug surfaces are redacted. An owner denial, an owner outage and
a negative fact cannot silently become a positive grant.

## Test assertion audit

The controlled generated server is behind the public production guard,
registered caller registry and real delegation verification. Before counting a
read its handler requires `VerifiedCaller(Gateway)`, matching `VerifiedActor`,
private `VerifiedReadAuthority` and a still-live original deadline. Tests do not
construct those private authority/context internals. Their local-parity signer
and RSA fixture are explicitly test-controlled, not production credentials.
The policy fixture matches the installed Auth ReadConcertAdmission subset:
Gateway-only caller, Auth audience and actor field 1. Source pins and the actual
policy/handler were inspected, not only the suite's string-based policy assertion.

The sixteen cases cover canonical request fields and absent ordinary binding,
changed negative owner replies with two observed calls, exact optional binding,
shape bounds, state/timestamp failures, real wire-size overflow with an unknown
protobuf field appended to the frame, private peer errors, wrong configured
token/caller/key, raw actor/body/audience/signature/proof/deadline attacks,
earlier waits, accessor expiry, delayed/noncooperative replies, readiness,
dropped-future cleanup/recovery, signed wall/monotonic JWT expiry and outage.
Invalid requests assert zero counted owner calls; malformed replies and private
failures assert fixed results, not just a generic error. New production code
contains no test-only acceptance gate.

The cancellation fixture checks its owned active-handler counter returns to
zero; it does not certify instantaneous database cancellation. The monotonic
JWT test advances past both JWT and local edge stop, and is not isolated proof
that JWT wins an otherwise live edge deadline; the handoff acknowledges this.
The distinct wall-expiry test holds an actual dispatched read and asserts signed
wall expiry is reached before the 800ms edge ceiling.

## Independently verified evidence

External receipt:
`/Users/lhs/Desktop/mcr-build/gateway-auth-owner-handoff.json`, SHA-256
`d0d24fa77e7e9982a7bf6e38387216260cf0ebc44f2c431b600c65660c2a06cd`.
QA recomputed and matched **all four delivered hashes, nineteen provenance
hashes, four log hashes and one planning-file hash**. Every provenance input
also matches its bytes at the implementation base. Git confirms exact four-path
scope and the one-line export; no protected input drift is hidden in the patch.

| Delivered source | SHA-256 |
|---|---|
| Existing `auth_client.rs` with module export | `dcae79cd9b51699c680f108fe15adbed9b349a3666793959adef7f30793fad5a` |
| New `auth_client/concert_admission.rs` | `ca7c7b73f89b0497ac5a59305131058f4739831c02c75e2d587642a5f35a781a` |
| New transport test | `0c616aa54d8ce75b4e7c59ef18151b480c57fc7c994c1619d4cfbab163417daf` |
| Worker handoff document | `ed7d4e29c44bf19f5cf863f4a3d26c8cf28c637fcb9f9b50ac38885ff7b4a8b0` |

Inspected receipt-bound final logs show **251 passing tests**: 16 new transport,
13 strict context, 209 retained Gateway unit and 13 verification transport; no
failed/ignored/filtered cases. They also record six compile-fail doctests,
locked library check and binary build passing without warnings. These are
worker executions inspected by QA, **not QA reruns**. Receipt command exit codes
are reported as recorded; source hashes anchor their claimed candidate.

The initial development log preserves E0271 fixture response composition and
E0505 borrow errors. The separate transport log preserves **14 passed / 1 failed**
at context creation in the wall-expiry test. Final source initializes the cached
RSA fixture before choosing one short expiry, then issues one token without
retrying a denial. The final sixteen-test log includes that corrected case.
No initial failure is relabeled as a pass or hidden by the combined count.

QA independently ran only these read-only checks, each exit **0**:

```sh
python3 -B scripts/mcr/check-plan.py
python3 -B scripts/mcr/check-boundaries.py --base e6efa4c4d6888415835f732de1b1f8c89aa9d460 --lane backend --allow-doc docs/mcr/backend/gateway-auth-owner-client.md
git diff --check
```

Hash/source verification and these outputs are retained under the owned temporary
directory `/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-qa-gateway-auth-owner-775e8x2b/`
as `verification.json`, `plan.log`, `boundaries.log` and `whitespace.log`.
These checks prove provenance/scope, not runtime Auth correctness.

## Disposition and limits

**No actionable finding in the assigned candidate was identified.** No code
correction is requested by this source/evidence review. Remaining HTTP original-
entry capture, key refresh/withdrawal across awaits, real Auth owner/database,
Profile/event eligibility, final signed Concert dispatch, TLS, revocation/epoch
fencing and stream invalidation are explicitly unqualified composition work,
not newly discovered implementation defects in this four-path prerequisite.
The unchanged unrelated Docker-only Auth fixture failure is not rerun or certified.

The previous FE14 goal was COMPLETE. This assignment's new unbudgeted persistent
goal was created and actually returned ACTIVE. It remains active until the sole
report is committed and the parent accepts or returns concrete review fixes.
Only the parent integrates/pushes; no whole BE04/C03/release acceptance follows.

## Follow-up: parent P2 shared edge admission and accounting

The parent subsequently identified a retained resource-policy regression in the
same candidate. The initial review above and commit `ad4e963` are preserved as
historical evidence; their no-finding disposition does not resolve this later
finding. QA independently agrees with the parent finding. The initial review
missed this path despite tracing the deadline behavior.

**P2 — restore the shared GatewayAuth in-flight guard and edge accounting.**
At candidate `868d02c`, `concert_admission.rs:206–213` directly awaits a scoped
`timeout_at`. It never acquires a shared edge permit or records edge exhaustion
or refusal counters. Retained Auth operations use `BoundedGrpcCallExt` in
`grpc_call.rs`, which delegates through `platform::grpc_deadline::with_deadline`
to `with_deadline_on(EdgeLimiter::global())`. That owning helper rejects before
polling the call if no permit is available, records the refusal, and holds the
permit until completion, error, timeout or cancellation. It also records local
and peer deadline exhaustion. `Edge::GatewayAuth.max_in_flight()` is 256.

The channel construction does not supply an equivalent guard:
`grpc_tls::with_edge_deadlines` sets only request/connect timeouts. Neither the
configured identity, tracing nor delegation wrappers acquire this semaphore.
Consequently, holding all retained GatewayAuth permits still allows the new
admission method to dispatch, bypassing the process-wide resource boundary and
its telemetry. A bounded per-call duration does not enforce an in-flight ceiling.
This is an implementation defect in the assigned prerequisite, not deferred
production composition. No Rust/runtime reproduction was performed by QA.

The parent assigned the worker a correction within its existing four-path
scope: immediate shared permit and retained metrics, the same captured absolute
stop, release on every completion/cancellation path, no queue/retry, and
meaningful tests without races on the process-global pool. At that point, exact
correction and evidence review was pending. A new unbudgeted follow-up goal actually returned
ACTIVE; the prior goal remains historically complete. No Rust/Cargo, target,
database, Docker or service ownership was taken for this follow-up.

### QA finding in the initial correction test

QA also identified a false-positive cancellation assertion in the worker's
first uncommitted correction. The process-isolated capacity test reused the
same owner's `Notify` after success, owner-error and timeout calls, each of
which invokes `notify_one`. A stored earlier notification can satisfy the
cancellation phase's `entered.notified()` immediately. On the test's default
current-thread Tokio runtime, the newly spawned read can therefore be aborted
before it is polled or holds a permit; an all-permits-free assertion then passes
without exercising cancellation release. The initial reported passing suite
does not independently prove that phase. QA reported the source-level finding
to the parent, who relayed it to the worker.

Required correction is fresh per-call evidence: observe a new owner call,
an active handler and one fewer available global permit before aborting, then
observe restored capacity and cleanup. QA also requested explicit exact local
and peer deadline-counter assertions, rather than accepting a claimed metrics
behavior solely because the shared helper is called. Final corrected-source and
receipt review is recorded below; no QA Rust execution is implied.

### Exact correction review and disposition

Worker correction **`77890b9acc0edf90a7a9f253eade6a7c0816587d`**, directly after
`868d02c98f69b3c04c0b6e61f06a6408b2cdf747`, resolves the parent P2 and the QA
cancellation-proof finding. QA reviewed all three correction paths: production
child module, its transport suite and the worker handoff. The cumulative change
from implementation base remains the original four assigned paths. No platform,
manifest, lock, contract, router, frontend or protected source changed.

At the parent's explicit instruction, QA cherry-picked only the worker fix into
this isolated checkout as `7859e3f0749db26cbb2b120a825aeaf49c0446f9` for exact
review. That application cherry-pick is worker-authored code, not a QA patch to
integrate. The parent must take only the separate QA report commit from this
branch. All delivered file bytes were independently compared to the worker's
exact committed revision; the worker checkout was clean at that revision.

The corrected call at `concert_admission.rs:211–242` scopes the same captured
absolute stop and calls retained `platform::grpc_deadline::with_deadline` with
the remaining time and the client's existing edge metrics. Its inner async
block contains `timeout_at(stop, generated_call)` plus a late-result check.
Thus `with_deadline_on` obtains the actual global permit **before** polling
readiness, encoding, configured signing or dispatch; saturation returns
immediately, without a queue/retry or a separate limiter. The unchanged owning
helper retains its permit across the awaited full future and drops it on return,
error, timeout and cancellation. There is no permit leak on a projection error
after the helper has returned either.

Absolute expiry is turned into a fixed DeadlineExceeded status **inside** the
helper's accounting. The helper records either its own timeout or the returned
deadline status once, never both. The explicit zero-remaining branch records
once and does not invoke the helper. Retained context and absolute-stop checks
still run after await and before projection/return. The original context is not
renewed. The outer scoped deadline can only shorten the stop; the new test now
exercises that case with a still-live context, improving the initial coverage.

`CapacityExhausted` is a new fixed safe projection of ResourceExhausted, including
local refusal **and peer overload**; the variant alone is not proof of local
no-dispatch. Malformed response state and decode/size failures remain rejected;
the real oversized-frame regression still passes. There is no diagnostic-string
classification or raw peer-message/details exposure. Private context, response
bound, binding consistency and observation projection otherwise remain intact.

The single capacity test reexecutes **itself** with an exact filter and an
environment guard in a child process. That process alone owns its global pool;
there is no separate no-op test entry or public test-only authority/limiter seam.
The exact final assertions establish:

- All 256 permits held: CapacityExhausted, owner calls 0, rejection counter
  exactly 1 and deadline counter 0; dropping the held permits restores 256.
- Success and owner Unavailable: exactly one new owner call apiece and all
  capacity returned; no deadline increment.
- Peer DeadlineExceeded: owner calls 3 and deadline counter exactly 1, with
  capacity restored.
- A 100ms outer task stop while the same context stays live: owner calls 4,
  DeadlineExceeded, deadline counter exactly 2, rejection still 1 and all
  permits returned. There is no renewed edge budget or retry.
- A fresh cancellation harness: owner calls 1, active handlers 1, available
  permits exactly 255 and an unfinished client task **before abort**. After
  awaiting the aborted task, available is 256 and its separate deadline/refusal
  counters remain 0. The owned handler becomes inactive, a subsequent signed
  read succeeds, and the original harness also succeeds with owner calls 5 and
  unchanged exact counters. Stored notifications cannot satisfy this proof.

These assertions and the actual shared helper establish the requested bounded
resource behavior. Handler cleanup is controlled transport evidence; it does
not establish real Auth/PostgreSQL cancellation or production acceptance.

### Final evidence verification

Correction receipt:
`/Users/lhs/Desktop/mcr-build/gateway-auth-owner-r1-handoff.json`, SHA-256
**`fdd24f2f00abada4021ae985baf8918c1c995989a2fe0891c2c6138eecafc45c`**.
QA independently recomputed **46 referenced hashes**: 4 delivered, 22 owning
provenance inputs, 1 original receipt, 9 final logs and 10 correction-history
logs. Delivered bytes match the exact worker commit; all 22 provenance inputs
also match the implementation base. The original receipt still has its earlier
`d0d24fa7...` hash. The initial report's **12,309-byte prefix is preserved byte
for byte** against `ad4e963`.

| Corrected file | SHA-256 |
|---|---|
| Production `auth_client/concert_admission.rs` | `558f41419c9d7f98cbc87e700c0ebd674446f26044dc0441ce370feb1233c132` |
| Transport suite | `3f3c15f2c086ed34c1cbe9ded0c39476f9b4648993ad11234a1439dd974df418` |
| Worker handoff | `c28ca1023552cae1704d1b33b78e51ab3f364a42cb60fca60c74f5f2841ebf04` |

The final hash-bound `gateway-auth-owner-r1-` logs record:

- **252 top-level passing tests**: 17 admission transport, 13 strict context,
  209 retained unit and 13 retained verification transport. The child execution
  passes one capacity case with the other 16 deliberately filtered. It is a
  second execution of the same case, not an extra functional case; no top-level
  cases are failed, ignored or filtered.
- Focused isolated execution: parent 1 PASS and child 1 PASS; each deliberately
  filters the other 16 transport cases.
- Six compile-fail doctests, locked/offline library check, binary build and
  formatting pass; final Rust logs contain no warnings. Receipt commands record
  direct native subprocess exit codes of 0 and Rust 1.88.0/two-job/debug-profile
  settings. QA inspected these results and did **not** execute Rust.

Historical logs preserve three compiler-failing invocations (missing Status and
the helper's Response/Result shape) and an interrupted earlier test. Those old
tee pipelines lacked pipefail; shell exit 0 was not native Cargo success. The
old 253 top-level count included a no-op entry and the stale-notification
cancellation phase. Those claims are superseded by the final source and 252
top-level result; neither old count nor earlier green output certifies the fix.

QA's independent read-only commands returned exit 0: `python3 -B
scripts/mcr/check-plan.py`; the backend boundary checker against `e6efa4c` with
exact allow-doc entries for the worker handoff and this QA report; `git diff
--check`; and `git diff --check 868d02c 77890b9`. The boundary command initially
examined this QA checkout; the exact corrected three-path scope was separately
verified from Git and the receipt, then rechecked after the authorized cherry-pick.
Hash/path/ancestry verification, commands and outputs are retained under
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-qa-gateway-auth-owner-r1-mljn25yq/`.

**Final bounded QA disposition: parent limiter/metrics P2 resolved; QA
cancellation-proof finding resolved; no remaining actionable defect identified
in this correction.** No production or whole-BE04 acceptance is implied.
Slow-signing isolation, real owner/database and later composition limitations
remain as stated above; the earlier missing shorter-outer-deadline runtime case
is now covered. The follow-up goal remains ACTIVE through the separate report
commit and parent review disposition. QA neither used nor acquired the retained
Rust target; its reported release is worker evidence, not a QA process audit.
