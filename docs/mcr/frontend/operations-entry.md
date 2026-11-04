# Common Operations access consumer

Status: MOCK_READY; source-qualified common consumer, production mounting and
actual authority/media integration remain OPEN. Same original FE37 active goal.

Root153 explicitly stopped and delegated ONLY the new
`frontend/src/app/operations-entry/**` common DTO/lifetime adapter and focused
tests, plus this new report and `docs/mcr/evidence/operations-entry/**`.
The exclusive retained checkout is fe-ref-gifts; no new checkout/chat/goal.
Root retains AdminBoundary/router/main/ports and actual app mounting. 53d owns
admin-entry/mural-closure; FE38 source/report/evidence are separately published
and owner092f is released/archived. No shared/backend/contract/lock/CSS edits.

## Contract and consumers

Actual immutable producer contract source is205 commit3a089a3b, parent2eec288c:
getOwnConcertOperationsAccess, admin.OwnOperationsAccessV1 and
OperationsAccessLeaseV1, policy.rules.operations_access_v1 contract_version1.
Its four strict wire tests and schema/generator checks passed according to the
producer. It has no qualified production mount yet. No runtime producer PASS
or production grant is claimed by this consumer. Existing getConcertSettings
supplies OPERATOR event revision/phase; role access adds no event facts.

Source3a089a3b was read directly, including schema, operation, policy and producer
wire tests. The test-only operation.fixture.json copies the exact generated
transport record and records that revision. Positive/negative source fixtures
use the same explicit producer wire payload; only the test module augments its
mock generated registry. Production discovers the actual registered operation;
if the retained generated read set lacks it or its binding differs, no fallback
request is made. No unchecked cast invents an operation in an older union.

The decoder rejects unknown fields, noncanonical identity/decision UUIDs, invalid
roles/revisions/UTC calendar values, actor/event/context mismatch, a different
server_time/checked_at sample and nonpositive/over60-second expiry. Fractional
UTC comparisons also reject submillisecond excess. The factory reuses canonical
Auth request/privacy/session and the existing AuthorityTimeline. It creates no
second Auth, HTTP transport, socket, private cache or persistence.

One fixed full PrivacyScope owns a single explicit bodyless protected no-store
GET. All elapsed request time is charged from request START, including canonical
reauthentication RTT; late/exhausted, backwards or interrupted timing is denied.
Server time is conservatively advanced by elapsed-since-start plus1ms for Date
fraction truncation. A current lease expires before the actual wire expiry; no
wall-clock fallback or receipt-time TTL renewal exists. The browser timeline
withdraws on hidden/frozen/BFCache context or excessive observation gap.

Every refresh synchronously publishes null before issuing a replacement read,
including a replacement whose public Operator fields equal the old lease but
whose decision UUID changes. The canonical scope, each read and every asynchronous
completion are fenced. Privacy/session/route disposal aborts in-flight work and
permanently retires this fixed-scope owner; explicit withdrawal/expiry can be
followed only by another explicit fresh read. No automatic renewal or replay.
403/503/malformed responses leave every role/clock port unavailable, without
retaining server diagnostic details. Cleanup attempts every owned registration.

Common exports: moderator/ModeratorAccess and moderatorNow for FE37;
authority/OperatorAuthorityPort and clock/OperationsClockPort for FE38; read()
returns the immutable exact decision context, plus subscribe/refresh/withdraw/
dispose. 53d confirmed Root must combine current role with independently
qualified getConcertSettings revision/phase for its OperatorContextPort. 092f
confirmed null notification must permanently retire old guards before Root
constructs a fresh consumer. No role-only command, evidence or place grant exists.

## Validation and delivery

Focused fixtures cover closed wire, actual canonical transport, absent/malformed
registry, distinct roles, exact scope, request-start/full RTT, expiry timer,
clock interruption, identical-field decision replacement, original FE37/FE38
consumer withdrawal, no-assignment/HTTP failure, aborts and superseded late data.
They are authored but NOT_RUN. A private source-only guardian copies the already
qualified full-checkout resource accounting into this new evidence prefix, with
unique outputs, no old evidence writes, no install/build/browser/DB. Frozen
candidate and fresh bounded resource offer are required before execution.

The independent earlier FE37 observer fix is canonically delivered at
cf1ca629a33ece977f8b5b1b6faf6152b35e5dbd: all29 exact leased postimages,
65 source/27 native cases. The FE38 custody paths were excluded. Root's canonical
verified receipt is sufficient; no readback/ACK/proof-only commit is needed.
This new common adapter remains unfinished and the same goal remains ACTIVE.


Fresh resource routing also requires the read-only dependency logical charge in
WHOLE. Current node_modules has408,236,853 logical bytes /438,317,056 physical
bytes across11,104 files. The new guardian charges the larger physical/logical
value, plus the read-only Node executable, entire retained checkout (including
all prior evidence/failures), own Git metadata and private TMP. Both logical and
physical dependency figures are recorded; no dependency carry is hidden behind
the missing-only symlink. This remains below the2GiB WHOLE cap before growth;
fresh exact retained/admission/peak figures are required at execution.


## Preserved source failure and actual adapter correction

Allocation OPERATIONS-COMMON-ACCESS-SOURCE-B-20261104T1745Z ran candidate
b4ded009 at2026-11-05 02:45:48 KST. Focused Vitest exited1:37 assertions,
23 reported pass/14 fail, with positive cases proving no HTTP request dispatched.
This is not qualified negative-wire coverage: those rejection assertions could
pass on the earlier transport failure. The entire failed report/log/source pins
are retained under evidence/operations-entry/runs/source-01. Full frontend types
and lint did not run because the guardian stops on the first failure. Actual
FULL02:46:06.680303 KST released every process/link/TMP/cache; Social was notified
immediately for the next fair ready owner, before diagnosis or reporting.

Concrete owned defect: the new adapter passed FE01's refresh option into the
canonical SessionController.request. The actual canonical request API explicitly
omits and rejects that property because Auth owns coordinated read reauthentication.
The adapter now omits it; the full RTT still covers any reauthentication chosen
by canonical Auth. Negative-wire fixtures now assert exactly one actual HTTP
request so early transport rejection cannot make them vacuously pass. No shared
Auth/transport, registry or contract behavior was weakened. Fresh qualification
of all affected common-adapter cases, types and lint is required.


Static diagnosis also corrected the pending-read replacement cleanup order:
retire first removes its own old timeline observer, then aborts the old request.
Otherwise abort synchronously invalidates that timeline and re-enters withdraw,
invalidating the intended replacement request before dispatch. The authored
superseded-response fixture exercises this exact overlap. This correction is
still unqualified until the fresh source retry; no test pass is invented.


## Current source qualification and publication handoff

Fresh allocation OPERATIONS-COMMON-REPAIR-SOURCE-B-20261104T1757Z qualified
clean candidate `a9aa6d3314ca962e5b6e107dcb96d517a8ca5add`. Actual ACCEPT
2026-11-05 02:57:40 KST; work deadline02:59:10; hard FULL02:59:40. Actual
FULL02:58:05.626483 KST. The exact argv, source/readset hashes, exits, byte/RSS/
process samples and cleanup are retained in
`../evidence/operations-entry/runs/source-02/release.json` and its three logs;
`vitest.json` contains all37 passing assertions. No earlier failed result is
relabelled as passed.

| Current check | Result |
| --- | --- |
| Focused common-access semantic fixtures, including actual canonical transport and FE37/FE38 withdrawal | 37/37 PASS, exit0 |
| Full frontend TypeScript, no emit/incremental, allowImportingTsExtensions | exit0 |
| Scoped oxlint with deny-warnings over app/operations-entry | exit0 |

The source fixtures prove closed wire/context binding, absent or malformed
registry denial, shared immutable role/clock projection, exact scope and roles,
submillisecond expiry bounds, full RTT charged from request start, wall-clock
independence, timer/interruption withdrawal, null-before-equal-field decision
replacement, permanent old FE38 guard withdrawal, FE37 no-assignment denial,
HTTP403/503 handling, privacy/session/route/disposal aborts and suppression of
superseded late responses. Negative-wire cases now require one real canonical
fixture HTTP dispatch, closing the prior vacuous-pass weakness.

This is a headless consumer/lifetime change with no new screen or styling.
Meaningful gap-specific coverage is through the real canonical Auth/privacy/
transport and existing FE37/FE38 controllers; no additional unchanged browser
matrix was replayed. The previous FE37 current native27-case result still binds
its unchanged product controller. New live service, runtime-clock/device,
production route, evidence-media and operator acceptance remain NOT_RUN here.

Retained baseline: checkout1,195,208,704 +ownGit1,900,544 +read-only dependency
charge438,317,056 (max logical408,236,853/physical438,317,056) +Node122,564,432
=1,757,990,736 bytes. The baseline includes prior failure/evidence carry.
Peaks: WHOLE1,768,550,224 /NEW10,559,488 /TMP10,543,104 /durable61,440 bytes;
aggregate RSS1,468,547,072 bytes /3 processes. Freeafter20,616,491,008 bytes.
All children, dependency link and private TMP/cache were removed; no browser,
server or listener was created. SourceB was released directly to Social before
reporting/publication, with no resource or manager acknowledgment hold.

Independent common-adapter patch is `f72941b3..HEAD`. Ordered source commits:
`110a676740584cf28ba2c71f503939916498f92f`,
`b4ded009459ea67e9c9dc1b9479596c7eaf69361`,
`68ed887e34cfa41fd83be106e55a90ac368ebd8a`,
`a9aa6d3314ca962e5b6e107dcb96d517a8ca5add`, plus the commit containing this
qualified evidence/report. Earlier FE37 source and FE38 custody are outside this
new patch. Only NEWapp/operations-entry, this report and its NEW evidence prefix
are changed. No uncommitted work, old evidence rewrite, contract/shared/root/
backend/CSS/lock change or additional checkout is part of this handoff.

Qualified production source SHA256:

- index.ts:`d6139ce9279c61b7e430495ba1f2c7a15ec6b5f4157726df5309b625769ce437`
- contract.ts:`c0abb2c88b64ecfff2af859374b2beca0e77be864f3242b19923acfe490347de`
- focused tests:`d7d81bd3b000160fff712323fa9dbc6c220d48bd55761375fb5f2a046e26ac02`

Root153 is the normal serialized non-force publisher. It owns actual route/app
composition and the combination with current authorized getConcertSettings
revision/phase; 53d's closure consumer requires that combined context. Backend205
continues the coherent durable access producer and Media report-purpose/EOF
fence tracing. No stub should be mounted as a qualified producer. The
implementation owner retains concrete consumer fixes. One canonical verified
publisher receipt establishes this scope's delivery; no proof-only commit,
manager review, readback or ACK gate. Same original goal stays ACTIVE through
remaining actual production integration; this source handoff is not full FE37
or whole-project acceptance.


Post-FULL inventory correction: Social reported an additional live backend16e1
workflow reserve2,147,483,648 bytes after this source job had already fully
released. Its target release alone was not workflow FULL. Original admission
receipt is preserved unchanged; this is not a replayed admission or an extension.
Corrected OTHER2,533,359,616 plus own134,217,728 leaves17,976,803,328 bytes
against accepted free and17,948,913,664 against FULL free, both above9GiB.
`source-02/resource-inventory-addendum.json` records the separate retrospective
arithmetic and actual coordination. FULL was sent directly to Social and global
backend resource21d; no replacement frontend job was started from a free source
letter. Global successor205 is coordinated from actual workflow FULL.
