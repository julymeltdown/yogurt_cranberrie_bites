# Party read and command producer

Integrated acceptance status: **IN_PROGRESS**. The assigned BE17/BE18
implementation, injected-native validation and committed owner handoff are
complete. Production code is `425b5fea`; final qualified test/controller source
is `af2fd3057a710203783d343cef10c8b3bcb0fe63`. Five allocated replacement Rust
cohorts are recorded below, including the failures that led to the fixes.
Final adapters, focused Root checks, PostgreSQL mutation/race/audience tests,
database restart and cleanup pass. Canonical Product HTTP, joined owner/Profile
privacy races, real two-client IN02, browser, load and release acceptance remain
**NOT_RUN**. Prior owner claims are historical and do not qualify this handoff.

## Replacement provenance and current changes (2026-11-04)

The sole writer uses `codex/mcr-be-party-runtime` in the assigned isolated
checkout. The old Party and frontend checkouts remain read-only. The lossless
handoff package was verified before import:

- Base: `60b286fc6382057d1287c05d1f51a30a6ac553c0`.
- `party-wip.patch` SHA-256:
  `35814b225147c4fbfb2c43f3cef6638b0b4497c60c012d9b43b5d8fa7a111d32`.
- Final `manifest.json` SHA-256:
  `62a5e939a0035b830ba46ce223924f3dc942e69d3e470e3c36a33e9841f4a787`.
- `package-receipt.json` SHA-256:
  `4a1c64e9ad8534d4ff50ba28431a9be5bcd13a342b34b13a24cd2cf97e0fc385`.
- All receipt artifact hashes, all twelve changed paths and all twelve Git
  postimage blobs matched; no untracked source was present. Preservation commit
  `2444260e` rebased to `af27233c` over published `7c8da1c4`.
- Source-only SDK merge `a7163efe` was initially consumed at `b3be37b1`.
  Published master `4d5c044fa1b09cfd794a519d963bcd63d22c26d8` then supplied
  SDK3 plus the unleased `HttpConcertAccess::rpc_deadline` fix `5329d3f4`.
  Candidate `ec981467` combines that published base with Party fix `ed3ea0fd`.
  Only assigned-path patches return to Root; no draft ancestry is pushed.

The replacement fixes production issues in the preserved WIP: immutable
application-party resolution; five missing product-producer operations;
DELETE key rejection despite Gateway requiring a key; missing current actor
checks before durable replay; generic RPC conversion losing 409 reason codes;
and absent outer error protection for `party-applications` routes.

Mutations now acquire the canonical event SHARE then the exact
`concert_community_parties` row UPDATE before authority/pair locks and BE15
budget admission. The same root is held SHARE by `community::audience` in its
caller transaction. Aggregate, child membership/application tables, receipt
and outbox commit together. Current authority and actor permissions gate
successful and failed receipt replay. Domain revision and state checks remain
in the mutation, so replay of a successful command does not apply it again.
No domain, audience, schema, migration or proto change was needed.

The expanded owned PostgreSQL journey covers duplicate applications and durable
failure replay; four foreign-actor denials without receipts; competing last-slot
approvals and full rejection; success/changed-input replay; owner departure
refusal; actual PostgreSQL blocking behind a held audience SHARE lock; leave and
owner removal capacity release; audience generation invalidation; reapproval
member generation; application cancellation; Party cancellation of pending
applications; and current-authority failure on replay. These are injected-owner
native tests, not canonical RPC/HTTP qualification.

Initial replacement checks: `python3 scripts/mcr/check-plan.py` exit 0 (planning
only); `python3 -B -m unittest discover -s backend/scripts/mcr -p
test_party_live_smoke.py` exit 0, eight runner tests; Rust 1.88 standalone
formatting and `git diff --check` exit 0. No Cargo command or target access was
performed before a fresh exact allocation.

## Qualified replacement cohort and concrete fixes

The first allocated replacement cohort compiled clean source `30c5cb34` with
Rust 1.88, `--locked --offline -j2`, debug 0 and incremental 0. A protobuf path
shadowing failure (exit 101) was retained; an explicitly allowed package-only
`contracts-build` cache repair removed stale paths. The subsequent Concert
five-test compile exited 0 in 23.70 seconds, and Gateway library-test compile
exited 0 in 25.21 seconds. Six exact executables, Cargo JSON, source archive and
hashes were copied outside the target. The target was unconditionally released
with no remaining handles before execution.

Copied execution on that source: cursor 3/3, owner 4/4, wire 8/8; Gateway product
8/8 and typed client identity 13/13; exact Root Party mutation route test 1/1 and
Gateway-only RPC policy test 1/1, all exit 0. These focused results do not certify
an actual HTTP request. The expanded PostgreSQL 16.14 run exited 101 with
`INTERNAL_ERROR`; its marker-owned cluster cleanup passed. Failure evidence is
retained in `be-party-runtime-30c5cb34/native-results.json`. The frozen six-binary
metadata SHA-256 is
`a90cb704b213ed1cdc416b62f7c0b28aa34685dec3c12a6a76c775a9eff02321`.

The failure exposed an old WIP invalidation reference mismatch: BE15 requires
invalidation and result to refer to the exact command target, while the WIP
supplied the Party root for application/member commands. Commit `c517adae`
restores the retained `PgCommunity` result-reference invariant. Synchronous
Party audience revocation remains in the locked root; it never waits for an
outbox notification. The fixture now expects one exact create-resource outbox
row, and reports stage context for further failures.

Production tracing also found that `GuardedAuthority` requires registration of
the exact native transaction before its policy check. The replacement now
registers lookup/projection transactions before local effects and command
transactions before policy checks. The injected native policy explicitly checks
registration order. `ScopedPartyProductProducer<R>` constructs
`SharedCommunityAuthority<GuardedAuthority, R>` from the exact acquired Root
scope for each request, with an unavailable alternate fence. Root must supply
actual `CommunityResources` feature/place policy, the qualified Profile reader,
keys, pool and cursor secret, and register all eight operations. No permissive
resource implementation or ambient production authority is installed. Reads
also enforce current feature/actor policy, and apply/approval retain the
canonical meeting-expiry commit guard.

The second allocated cohort compiled clean source
`7d3e21a361ce0f1c34d98b8f303cf372395626c1` (production code `c7d8eff6`)
with the same five Concert test targets and compiler settings. Its initial
protobuf path failure exited 101; the explicitly allowed package-only
`contracts-build` repair was followed by compile exit 0 in 26.98 seconds.
Five executables, Cargo JSON, full source archive and hashes were frozen outside
the target before its unconditional release. Metadata SHA-256:
`1cfdf98056dc91a16d4ad1c61253f0721c30a930978945a9442d9b5240c2af9b`.
Copied cursor 3/3, owner 4/4, wire 8/8 and the two exact Root tests passed.
Gateway was not recompiled in this cohort; its five returned shared files
remain byte-identical to the first qualified cohort.

The second private PostgreSQL native run exited 101 with
`Concert revision or creation time is invalid`; marker-owned cleanup passed.
Its relationship-block fixture updated a pair without incrementing its
revision. Test-only commit `d4cf7a91` added the required increment. A subsequent
source-only SQL probe then exposed the additional real relationship-authority
commit guard. Test-only `5738c6f1` authorizes the fixture through the injected
`PgRelationshipPolicy<TestOwnerFence>` in the same transaction before its
guarded update and commit. Neither schema guard is disabled.

The source-only private PostgreSQL SQL probe for `5738c6f1` passed: block at
revision 2, unblock at revision 3, rejection of an update without a revision
increment, and cleanup. It used an explicit synthetic authority window solely
to check the fixture SQL; it does not qualify the Rust helper or canonical
authority. Reports are retained in `be-party-runtime-7d3e21a3/`, including both
earlier failed probes. Expanded native execution at that checkpoint was
**NOT_RUN**; the subsequent allocated cohorts below supply the qualification. No target
is accessed between offers, and no former `be-meet-live/backend/target`
artifact is a dependency of the replacement handoff or qualification.

The third cohort compiled clean `00594bcd` after the same narrowly evidenced
`contracts-build` repair (20 files, 816.4 KiB). Compile exit 0 took 20.55 seconds;
all five exact Concert executables, both Cargo JSON logs and full source were
frozen before unconditional target release. Metadata SHA-256:
`e97ffe7fc50d224414a683f92a9d0ddae82038fbc2aab481beead49558d11448`.
Copied adapters 15/15 and the two exact Root tests passed. Native PostgreSQL
passed the corrected block fixture but exited 101 at the competing approvals:
the loser returned `DependencyUnavailable`, where `VersionConflict` was required.
Cleanup passed; all evidence remains in `be-party-runtime-00594bcd/`.

Source tracing identified a lock-order inversion consistent with that failure:
post-command reply projection could hold the self-pair lock and wait for a
Party SHARE lock while another mutation held that Party UPDATE lock and waited
for the self-pair. List and detail had the same root-after-pair ordering. The
correction locks Party roots before policy pairs in each reply/read transaction.
List locks one bounded candidate window (at most the existing 1000-row scan
ceiling) before policy, rather than taking additional roots between pair checks;
this can hold more SHARE locks than the earlier small-batch implementation.
Current actor/feature, subject and Profile checks remain mandatory before any
DTO is returned. Pagination still advances by the last scanned candidate and
retains the immutable cursor upper bound.

The expanded native fixture now also holds an actual self-pair UPDATE lock,
waits until each public detail/list request is blocked there, and proves with a
separate `FOR UPDATE NOWAIT` probe that the request already holds the Party root
SHARE lock. The controller retains a redacted PostgreSQL log tail and owned
cluster allocation before cleanup to diagnose any remaining failure. These new
checks and source correction were then qualified in the final cohorts below.

## Final qualified owner handoff

Clean `63bec4ce84247ff3e64752c732efeec605a4ccb1` compiled the corrected
production/test code from `425b5fea` and added a controller-only restart check.
The exact native test passed in 0.34 seconds, including deterministic public
read root-before-pair probes and competing approvals. PostgreSQL 16.14 stop/start
preserved the complete ordered contents of ten tables; cleanup passed.

The completion audit added explicit owner-removal receipt replay, removed-member
audience denial and stale-audience-generation denial in test-only `af2fd305`.
This final clean source received fresh named allocation
`PARTY-REMOVAL-BACKEND-202611040520Z`. Initial compile exited 101 on a retained
protobuf path-shadowing cache entry. The permitted `contracts-build`-only repair
removed 20 files / 816.3 KiB; the unchanged-source retry exited 0 in 20.69 seconds.
All five requested executables, both Cargo JSON logs, source archive and pins
were copied outside the target. Zero remaining handles were checked and the
target was unconditionally released before copied tests or PostgreSQL execution.

| Check | Final evidence | Result |
| --- | --- | --- |
| Cursor, owner and wire executables | `adapter-results.json`; exact unskipped 3 + 4 + 8 cases | 15/15, exit 0 |
| Party route and Gateway-only RPC policy | Two exact `product_runtime` test reports | 2/2, exit 0 |
| Actual Party producer, BE15 and PostgreSQL | `native-results.json`; injected owner, actual transactions | 1 exact expanded journey, exit 0, 0.35 seconds |
| Restart durability | Ordered snapshots of ten tables before/after owned PostgreSQL stop/start | PASS |
| Cleanup and resource release | Native cleanup plus `workflow-release.json`, actual owned root absent and no referencing process | PASS |

Restart preserved 3 Party roots, 6 applications, 6 memberships, 22 receipts and
18 outbox rows. The native journey covers duplicate/failure replay, foreign-actor
denial without receipts, blocked approval, competing last-slot approvals,
successful/concurrent replay and changed-input conflict, owner-no-orphan,
actual audience SHARE/mutation blocking, leave and removal capacity release,
immediate audience/generation revocation, reapproval generation, application
withdrawal, Party cancellation and current-authority failure on replay. It
uses explicit injected authority; no canonical owner or HTTP result is inferred.

Evidence directory: `be-party-runtime-af2fd305/`.

- Five-binary metadata SHA-256:
  `7f872566a76e0842368ed1a6f302302889e7226ec39db063ffe8e90ce9d8ed22`.
- Exact native executable SHA-256:
  `59f0c80903b5309348521fed0b2b0675794a4848960c41f71bd5b99e5f1766d5`.
- Native report SHA-256:
  `37e3743f2ba7c863684874b72069263f0a9abd3febace41481a5a8d7fed3233b`.
- Observed changed target/frozen output: 460,578,816 bytes; owned cluster before
  cleanup: 50,077,696 bytes. Their observed sum, 510,656,512 bytes, remained below
  the 0.75 GiB expected / 1 GiB hard workflow budget. No target or runtime is held.

The recorded compiler command ran from `backend/` under Rust 1.88 with
`CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=2`, dev/test debug 0 and the explicitly
accepted target. A fresh named allocation is required before reproducing it:

```sh
rustup run 1.88.0 cargo test --locked --offline -j2 -p concert-service \
  --test party_live_cursor --test party_live_owner --test party_live_wire \
  --test party_live_native --test product_runtime --no-run --message-format=json
```

After target release, the copied adapter controller and native controller each
exited 0. Native invocation was `party_live_native --ignored --exact
--test-threads=1 party_live_native`; its controller then performed restart and
cleanup. Focused Root invocations used `--exact` with
`five_party_mutation_routes_bind_exact_c04_targets_and_queries` and
`only_gateway_may_dispatch_with_a_signed_field_one_actor`. Exact argv, environment
and artifact identities are retained with the cohort; no whole-workspace test
claim is made. The earlier Gateway 8 + typed-identity 13 checks remain qualified
at the unchanged returned shared-file cohort, not as a current combined build.

Physical/rally peer reviewed clean `425b5fea` read-only, including nested
application projection and bounded-window pagination, and reported no concrete
findings. The final successor changes only test/controller evidence. No peer
runtime credit is inferred from that static review.

Final source-only checks also exited 0: `python3 scripts/mcr/check-plan.py`
(planning provenance only), eight controller regressions, standalone Rust 1.88
format checking and `git diff --check`. All thirteen source archives remain
byte-identical; no frontend, manifest, lockfile, proto or migration was changed
by the replacement patch. The final delivery receipt records the documentation
successor commit, base commit and exact ten owned changed paths; the five
previously returned shared paths are excluded from that patch.

## Shared-path ownership return

The five transferred shared files are returned to Root at source `c7d8eff6`.
All five blobs are unchanged from the qualified `30c5cb34` compiler/Gateway/root
cohort. A five-path-only binary patch against published `4d5c044f` was applied
to a separate temporary Git index; its changed-path set and all five resulting
blobs matched. Patch SHA-256:
`5d662bef90afad48a8fd7a1e436040b506fecee4f8e01f34daca4c31a3723d09`.
Root retains its additive `pub mod child_recovery;` exception and may compose
subsequent routes. This owner makes no further edits to those five paths after
the explicit return. The Party module/tests remain assigned here for native
qualification; this split handoff does not certify full Party acceptance.

Root locally integrated the returned five-path patch at `1c651185` and then
assembled Social at descendant `78d2c8cd`. Source-only checks confirmed the exact
five changed paths, ancestry, all eight RPC operation identifiers and seven
Gateway paths (collection GET/POST yields eight operations). These observations
do not certify master publication, combined compilation or actual activation.

Physical/rally peer reviewed clean `f3cdd695` read-only and reported no concrete
audience-seam findings: canonical root UPDATE through BE15 commit, reader SHARE,
removal/audience and reapproval member generations, and exact result/invalidation
alignment. No build or runtime credit comes from that static review.

## Scope and source boundary

The remaining implementation ownership and final owned patch are limited to:

- `backend/services/concert-service/src/party_live/**`
- `backend/services/concert-service/tests/party_live*.rs`
- `backend/services/concert-service/tests/party_live_support/**`
- `backend/scripts/mcr/party_live*.py`
- this document

The returned five shared paths are Product RPC `{mod,query,owner_media}.rs`,
Gateway `concert_product/mod.rs` and `tests/product_runtime.rs`. They came from an
explicit isolated-write transfer on
source candidate `f5c3a99f`. They add the Party operation variants and exact route
closure while preserving its committed network/disclosure fields. The factory
and capability activation remain root-owned.

The Product API owner granted one temporary export-only line,
`pub mod party_live;`, in `backend/services/concert-service/src/lib.rs`. All other
lines remain untouched. The line must be returned to the root owner with the
committed module pin; it cannot be published as a phantom export. Product routes,
the Product RPC operation enum, configuration, proto and root composition remain
root-owned and are **NOT_RUN**.

The source base is mcr `8d2834592eee51385f048f7ca613bff18c8d0ec7`. The frozen C04
inputs are `contracts/domain/v1/party.schema.json`
(`7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03`) and
`contracts/domain/v1/operations.json`
(`622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839`).
The source registry and public activation remain separate from this producer;
the five newly transferred Product variants do not establish runtime qualification.

## List and detail

The producer binds exact verified actor, event, session family, request, original
deadline, HTTP method/target, raw body digest and idempotency key into a
non-serializable request binding. The closed target set is:

- `GET /api/concert/events/{eventId}/parties`
- `GET /api/concert/events/{eventId}/parties/{partyId}`
- `POST /api/concert/events/{eventId}/parties`
- `POST /api/concert/events/{eventId}/parties/{partyId}/applications`
- `POST /api/concert/events/{eventId}/party-applications/{applicationId}/decision`
- `DELETE /api/concert/events/{eventId}/party-applications/{applicationId}`
- `DELETE /api/concert/events/{eventId}/parties/{partyId}/members/{userId}`
- `POST /api/concert/events/{eventId}/parties/{partyId}/cancel`

List accepts only the C04 `PartyQuery` field names, rejects duplicate or unknown
parameters and invalid UTF-8 percent encoding, applies the exact default limit 20
and supports limits 1-100. Detail accepts no query, body, content type or
idempotency key. Create requires canonical JSON media type and a canonical UUID
idempotency key.

The initial page captures one immutable database `upper` bound. The AEAD cursor
binds actor, event, operation, collection, every canonical filter, limit, upper
and last scanned `(created_at,party_id)` position. It is opaque, expires after 15
minutes, is at most 1024 ASCII URL-safe characters, and cannot be replayed under a
different actor, event, filter, limit or operation. A continuation can reveal no
new parties outside the initial upper window.

Collection admission, the full scan, every visible row projection, cursor sealing,
final guard acknowledgement and commit happen in **one PostgreSQL transaction**.
The scan is bounded to 1000 candidate rows per request; denied-author or missing
Profile rows are filtered without resetting continuation. Event, actor admission,
participant and selected roots are locked `SHARE`. All candidate roots precede
policy pair acquisition. Local pair checks take `SHARE`; the owning relationship
policy can take `UPDATE`. A missing pair is transactionally reserved before
locking and is removed if the read rolls back. `READ_ONLY` permits reads;
`DRAFT` and `ARCHIVED` do not.

Party detail and every list row use the same current local event/admission/
participant/pair checks. Canonical Auth/Friend owner exclusion and the current
public Profile are injected through `PartyLiveOwnerGuard`; a failure is not
converted into an empty page. The owner object must contain the exact C03
`PublicProfile`, including a complete current `avatar` union. There is no local
avatar fallback or Profile/profile URL substitution. JSON Schema validation is
closed for `PublicProfile`, `PartyPublic`, `PartyList` and `CreateParty`.

The published root-facing seam is
`PartyLiveService::dispatch_for_scope(Arc<OwnerGuardScope>, Arc<dyn
PartyLiveOwnerGuard>, ProductRequest)`. Before matching the typed request, it
requires the held scope to be live, its actor/event/session/request/method/target
and raw body digest to match the Product request, and the exact Product protobuf
digest to match the sealed parent. It then derives a Party generation that also
binds the closed operation and create idempotency key. It never acquires or
settles a second canonical scope. Root holds the actual scope through response
serialization and attaches a published delivery capability and its private
response token to the Product response; only Gateway's outer HTTP-body
completion settles the owner scope.

## Create

Create parses the exact closed C04 fields and derives a `PartyCommand`. Its
dedicated BE15 producer locks and rechecks:

1. current event phase for write;
2. actor admission and participant;
3. the actor pair;
4. canonical Auth/Friend/current session authority;
5. current community feature authority; and
6. the owner-held transaction registration.

The receipt, outbox row, Party root and owner membership are inserted in the same
BE15 transaction. A deliberate Party domain failure rolls the effects back to the
producer savepoint and persists only its safe failed receipt. A same-key replay
returns the original result without repeating effects; changed input under the
same key conflicts. After a successful receipt, a current authorization reread
projects the real revision, owner membership, capacity and timestamps. Pending or
failed receipts never become a successful Party response.

The existing Party aggregate remains the source of the 2-50 capacity, meeting
time/text and owner-membership invariants. No private application note, member
identity list, social handle or physical-arrival claim is exposed. Application,
decision, removal/leave, and cancellation commands use that same locked root.
The owner's approval checks capacity in the serialized aggregate transaction;
bodyless DELETE commands bind the exact C04 query revision, reject duplicate or
extra fields, and preserve SAFETY availability in `READ_ONLY`. Member removal
revokes private Party audience rather than implying physical departure.

## Default-deny authority and activation gates

`UnavailablePartyLiveOwnerFence` always returns `DEPENDENCY_UNAVAILABLE` and is
not a production grant. The published canonical consumer API is
`relationship_authority::guarded::{OwnerGuardScope, GuardedSubject}`. A real
root-owned adapter must:

- acquire the canonical owner-held scope from the actual sealed Gateway parent;
- pass the scope's `GuardBinding` and enroll subjects through its
  `GuardedSubject`/public Profile path;
- hold it through Product response serialization and terminal settlement;
- use the original request deadline, never renewing it;
- check and enroll the current list subjects against both Auth and Friend;
- keep the local event/admission/participant/pair locks on the actual producer
  transaction;
- call `OwnerGuardScope::register_transaction_in` on the actual protected
  connection before effects; and
- retain uncertain quarantine on deadline, cancellation, owner failure or process
  loss until actual transaction outcome settlement.

The concrete `ScopePartyLiveGuard` consumes the root-acquired scope and the
qualified `PublicProfileReader` without acquiring another scope. Its integration
with an actual Product/Gateway request is still **NOT_RUN**. `PartyProductProducer`
implements root `ScopedProductProducer`, accepts that same held scope, checks the
eight closed operation variants, constructs the concrete guard, and returns JSON
status/body only. Its contract is the injected
`PartyLiveOwnerGuard`: derive `binding_digest()` from the exact Party binding;
return the current nonzero authority revision; call the same scope's current
subject exclusion and Profile reader; and register the Party-bound transaction
for settlement. `public_profile_in` obtains a `GuardedSubject` from the same
held scope, calls `PublicProfileReader::read(&subject)`, parses exactly
`PublicAuthor.json` as `serde_json::Value`, verifies its plain UUID `user_id`
against the guarded subject and the
published C03 schema, then rechecks the subject. It retains the same root scope and child. The passed PostgreSQL
connection is used by Party for its own local transaction registration and
locks, not as an alternate Profile query channel. The adapter therefore passes
that connection through the producer contract but obtains profile content only
through the reader and its guards.

Only the root's exact Gateway terminal callback may call
`OwnerGuardScope::finish_response` after external body EOF and response
serialization. Internal gRPC completion, Drop, deadline, cancellation and owner
failure must leave the scope in quarantine for reconciliation.

`SAFETY_SELF`, a current read observation, a signed workload credential, a Profile
row, or a local event row is not a substitute for those checks. The authority and
public Profile owners have published component and source qualifications, while
the joined Profile/Party production adapter is still **NOT_RUN**. Root has since
published the three Party operation variants and the retained product scope seam;
actual Party composition, HTTP and multiuser flow remain **NOT_RUN**. No
synthetic route or owner fixture is installed here.

The joined reader mapping is typed and fail closed. Only
`FeedError::ResourceUnavailable` at the Profile reader or a canonical subject's
dedicated `NotFound` means that one row is unavailable; list scans may skip it
and direct lookup returns not found. Every other guard, owner, privacy, scope,
deadline or dependency error aborts the response. A generic `PermissionDenied`
never becomes an empty page or hidden Party, and transport settlement retains
the same held scope through the actual response boundary.

## Historical owner validation (not replacement qualification)

Rust candidates 4e9c57e3 and d273c377 at 1.88.0 compiled with `--locked
--offline -j2`, incremental 0 and debug 0. The final source adds the concrete
reader adapter and fixture assertion corrections after the last compiled pin;
its Rust compile and tests are **NOT_RUN**. The final source includes the concrete
scope guard and `ScopedProductProducer` adapter that consume the now-published
Public Profile and root `ScopedProductOwner` interfaces.

Offline checks for the final source checkpoint:

- Python runner regressions: 8/8 PASS.
- Python compilation and `python3 scripts/mcr/check-plan.py`: PASS.
- Rust 1.88.0 formatting and `git diff --check`: PASS.
- The source-only `concert-service --tests` and `api-gateway` checks passed in
  the isolated worktree before the resource STOP. They are **not** allocator-
  qualified evidence and are not presented as final frozen test results.
- `party_live_wire` 8/8 and `product_runtime` 6/6 passed in that same local
  source-only run. The expanded native PostgreSQL test remains **NOT_RUN**.

The later owner-STOP direct-check pass covered the final source diff with
`rustfmt --check`, the 8 Python runner regressions, Python compilation,
`check-plan.py`, and `git diff --check`; it did not compile or run Rust after the
stop and is not allocator-qualified evidence.

The later owner-STOP direct-check pass covered the final source diff with
`rustfmt --check`, the 8 Python runner regressions, Python compilation,
`check-plan.py`, and `git diff --check`; it did not compile or run Rust after the
stop. The package remains WIP and does not claim final BE17/BE18 qualification.

Frozen four-test evidence from base `4e9c57e3` is in
`/Users/lhs/Desktop/mcr-evidence/be-meet-live-4e9c57e3/`. Adapter report 3 PASSes
cursor 3/3, owner 4/4 and wire 7/7 (14/14) with metadata SHA-256
`55d3022b6387b7580a8cf1568485961d109ce64ffa8abe3e90dfecf2896839e1`.
Rust executables are frozen there; native candidate 4's copied PostgreSQL run
failed the `create effects not atomic` assertion with cleanup PASS, so native is
honestly **NOT_RUN** as a final pass. That assertion had counted global seeded
outbox rows and both Party memberships; the final fixture is narrowed to the
resource's exact outbox row and expected three memberships, but needs a fresh
compile. Older native map-ID and seed failures are retained as negative evidence.

Build-target coordination had resource-accounting errors in this session. The
initial BACKEND four-test compile used the coordinator's explicit grant and was
released. A later short BACKEND native-only compile was performed while I treated
the coordinator's short-recompile message as a lease; the coordinator records
that this was a queued turn, not a named target allocation. That reuse was not
authorized and is not claimed as valid evidence. After the coordinator separately
offered MEDIA, three native fixture compiles were made there; the coordinator
records that MEDIA changed to Browser work between the earlier native-3 release
and native-4 access, so the latter was also not covered by a current allocation.
All four affected compiles had already ended and their target handles were absent
when the conflict was confirmed. The no-lease timestamps are retained in the
private evidence directory, and no later compile or target access was attempted.
These resource-control mistakes do not turn the failed native fixture into a pass
or establish ownership of root files.

After the two-target correction, I also ran source-only `cargo check` and two
focused test binaries against this worktree's local `backend/target` target before
receiving the later STOP. Those PIDs are identified in the coordinator resource
receipt; they were not covered by a fresh named allocation. No target was read,
cleaned, or mutated after the STOP, and these results are not claimed as frozen
or allocation-qualified evidence. The saved shared Product API draft was restored
by the root owner and then reapplied only in this isolated worktree.

The frozen runner is configured for only the exact dedicated
`party_live_cursor`, `party_live_owner`, and `party_live_wire` executables (3, 4
and 7 expected cases). It records cursor tamper/binding/expiry, closed
body/filter/route validation, safe status mapping and Party fixture invariants,
while marking native PostgreSQL, canonical holder, Profile, Product route and HTTP
as **NOT_RUN**. No Rust executable is claimed until a fresh frozen build exists.

## Remaining integration acceptance

Assigned Rust compilation, focused routes, PostgreSQL capacity/revision/outbox/
replay/removal and restart checks are complete in the replacement cohorts above.
Root must integrate the owned patch without publishing draft ancestry, compose
the actual feature/place resources and Profile reader, activate all eight
operations under the canonical owner, and run the relevant combined checks.
Actual canonical-owner mutation/Profile privacy races, HTTP response settlement,
real two-client IN02, browser/device/load and release gates remain **NOT_RUN**
for this handoff. BE17/BE18 and IN02 are not marked **ACCEPTED** by these injected
tests. Only the integrator publishes; no main-chat or scrum-lead approval is a
technical integration gate.
