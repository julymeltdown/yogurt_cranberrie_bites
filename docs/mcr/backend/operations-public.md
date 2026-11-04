# Operations public composition

Task: OPERATIONS-PUBLIC-01. Producer owner: backend205. Publisher: Root1fc.
Status: IN_PROGRESS. Runtime validation: NOT_RUN. No king or SL review gate.

## Registered additive access contract

The versioned `operations_access_v1` policy registers
`getOwnConcertOperationsAccess`, a bodyless SELF GET at
`/api/concert/events/{eventId}/me/operations-access`. Its closed response is
`admin.OwnOperationsAccessV1` with `server_time` and nullable `access`.
The access lease contains exactly actor/event, MODERATOR or OPERATOR, current
positive safe-integer role revision, actual request UUID as decision identity,
and UTC checked/expiry times. This informational read is never a bearer grant.
No active assignment returns null; missing configured authority returns 503.

`server_time` and non-null `checked_at` must be the SAME actual PostgreSQL clock
sample. Expiry is the earlier of assignment expiry and that sample plus 60
seconds. The producer must retain its current role exclusion fence through the
actual canonical Root HTTP terminal. A business-transaction SHARE lock alone is
insufficient. Production role assignment/withdrawal requires configured trusted
issuer material, durable exact revisions, expiry and revocation, not fixture SQL
or an environment role boolean.

The browser binds its full captured canonical PrivacyScope and qualifies the
full request RTT against a monotonic request-start clock. Its local deadline is
request start plus the bounded server TTL, never response receipt plus TTL. It
rejects unknown, backward, late, expired, aborted or scope-mismatched results and
synchronously retires the old decision before replacement or null. Every read
rechecks exact scope and expiry. Every privileged server operation independently
checks current authority; clients cannot infer an unobserved server withdrawal.

The existing registered `getConcertSettings` / `admin.EventSettings` response is
the OPERATOR-only authoritative producer of event revision and phase for FE39.
The access response deliberately contains no invented event facts. FE37 owns
Moderator consumers; FE38 and Root153 own Operator/clock/common adapters. They
consume an immutable registered producer handoff after actual qualification.

## Current evidence and limitations

The immutable role dependency packet is
`docs/mcr/backend/operations-role-dependency.json`. It identifies the five
role-only source/migration blobs and exact additive canonical lifecycle hooks.
It deliberately excludes unfinished Operations routing, Report changes and
application-main composition. Wall and Lifecycle consumers must use the SAME
publication/assignment owner installed before the single canonical registry.
`require_operator` and `require_moderator` retain the actual role exclusion
through Root terminal; callers still own exact operation, account, admission,
native transaction and source checks.

The four configured-issuer tests and three real-owner role exclusion/revocation
tests are authored but NOT_RUN. The role owner, publication worker and privileged
public routes have not compiled or qualified in a runtime round. The publication
poller source now serializes synchronization, avoids reapplying unchanged files,
withdraws readiness on changed/invalid input, and rereads file identity before
marking the actual publication current. These are source changes, not runtime
evidence.

O04 Report decision source now maps its actual Moderator/Operator action and
stores append-only private decision metadata in migration `0079`, tied to the
real command receipt and Report revision. It is not yet mounted or validated.
O05/O06 migration `0076` defines immutable actual owner-effect/native witness
journals, but has no qualified effect writer or World socket acknowledgment.
Social-only revocation cannot certify all private owners. Those scopes remain
IN_PROGRESS and their new runtime validation remains NOT_RUN.

The published client-foundation commit `bc75182e` contains delivery/readset
evidence; it does not register a role/clock producer. The previous USER/no-media
Report path remains separate and retains its real canonical owners.

Source checks (strict `/private/tmp/mcr-contracts-admission/bin/python -B`):

- `-W error -m unittest discover -s backend/scripts/mcr -p operations_public_contract_test.py`: 4 passed, exit 0.
- `scripts/mcr/validate-domain.py`: 175 schemas, 115 operations, 55 fixtures, exit 0; candidate state, not runtime acceptance.
- `scripts/mcr/generate-domain-transport.py --check`: exit 0.
- `backend/scripts/docs/generate-mcr-domain.py --check`: exit 0.
- `git diff --check`: exit 0.

These checks establish closed wire registration only. Production role issuance,
durable delivery fencing, restart recovery, actual authorized/denied/revoked
HTTP, and combined Operations composition are NOT_RUN. No privileged route is
claimed mounted by this contract-only source change.

Root has delegated the report evidence leaf and a precise bodyless content
operation, with additive typed-child work separately owned by backend267.
The byte operation is not yet frozen: actual Media report-purpose association,
READY ownership, canonical account locks and its EOF fence must be composed.
Reviewed-place release, notice source/legal review and FE41 operational
maintenance/application-version decision inputs remain real-owner integration
work; no successful stub or hardcoded decision may substitute for them.

## Source ownership

Root explicitly delegated only the additive domain admin schema/operations/
policy, exact regeneration postimages in the two domain OpenAPI artifacts and
`frontend/src/shared/api/domain-operations.generated.ts`. CORE contract ownership
remains backend207. The generator scripts are reused unchanged. No frontend
handwritten implementation, core schema, proto or source-pack content changed.
All `docs/source/` bytes remain unchanged.

The enrollment C07 owner temporarily owns both combined generated OpenAPI
projections. New DOMAIN source authoring remains here; this owner must not write
or deliver stale projections across that enrollment baseline. Canonical intake
regenerates the new source later. There is no consumer acknowledgment gate.

## IN04 dependency priority and current source boundary

The actual virtual Wall operator flow depends on the same canonical role owner,
current signed publication and Wall202's exact authorized admin producer. It has
no external age-provider, physical-place or Report-evidence prerequisite. The
standalone factory at `a41a9a1433f912d828bdc5e3682560c3133dfd49` and its
packet `317a7c358` remain the minimal source dependency; the original five role
blobs and migration 0061 are unchanged. Root1fc publishes the coherent main
composition, Wall202 implements its admin flow, and the IN04 developer owns the
real multiple-client journey.

The three authored role native cases now use that actual single-use factory,
one canonical core, the same returned role store/publication and private signed
managed files. Their existing cases cover no-assignment denial, invalid-file
withdrawal/recovery, actual Operator exclusion blocking revocation until Root
terminal, and old signed publication replay leaving a revoked assignment
revoked. They are still NOT_RUN. The OwnAccess read additionally acquires the
actual common account exclusion before local event/admission/role locks, denies
an existing account-deletion fence, and bounds the entire attempt by its
original disclosure deadline. These changes do not qualify the public route.

The two-executable Operations role allocation covers four issuer cases and the
three real-owner role cases, followed by owned PostgreSQL cleanup. It does not
certify OwnAccess HTTP, Wall commands, a production main or full IN04. Compiler
and zero-handle target custody must be handed to the actual next writer before
copied execution, PostgreSQL, documentation or publication.

## First bounded role round: compile failure, native not run

The exact two-test selection at source
`6888f07837293da07b044d4eb1cc5268f6dfbb11` started under
`OPERATIONS-ROLE-TWO-EXE-MEDIA-20261104T1837Z` at
2026-11-04T19:05:20.786661Z. The explicitly amended WHOLE ceiling was 14 GiB,
with historical carry 10,857,697,280 bytes and NEW 2 GiB. Fresh free space was
18,652,180,480 bytes; subtracting all proposed/outstanding new growth left
16,236,261,376 bytes. Both controllers enforced the 12 GiB start class for this
2.25 GiB aggregate, the 9 GiB residual and continuous 8 GiB floor.

`cargo test --locked --offline -p concert-service --test
operations_public_issuer --test operations_public_roles_native --no-run` with
Rust 1.88, two jobs, debug/incremental disabled and the retained contract profile
settings exited 101 after 51.17 seconds. Its actual compiler diagnostic is E0382:
the Operations publication worker moves `pool` into the supervised coroutine,
then shutdown attempts `pool.close()` outside it. No test executable was emitted
in the Cargo artifact inventory, no artifact was frozen, and issuer/native/HTTP
qualification is NOT_RUN. The exact original Operations worker hunk needs a
separate pool clone; the current main-file owner receives that concrete fix.

Cargo also attempted its implicit normal Concert binary prerequisite. This was
outside the intended no-main-build description, and is recorded rather than
counted as a passing two-artifact build. Cargo's documented integration-test
selection includes package binaries; changing to `cargo rustc --test` does not
establish their exclusion. See the [Cargo target selection documentation](https://doc.rust-lang.org/cargo/commands/cargo-rustc.html#target-selection).
A fresh allocation must account for that necessary compilation or use an
explicitly agreed alternative. No manifest was changed to bypass the failure.

Compiler/target-handle/guardian-reader-zero target release was
19:06:16.651329Z, directly sent to Wall202 before copied work. Its receipt SHA256
is `00b4a6f049808271bcdc217431ff1d72d43d243972170fcd8af6a10c5c004dd5`.
FULL release was 19:06:35.463363Z: controller exited, independent guardian reaped,
no PostgreSQL, service, owned root, workflow or NEW reservation remained. NEW peak
was 569,323,520 bytes, WHOLE peak 11,427,020,800, positive target growth
562,130,944 and outside peak 7,192,576. Maximum owned processes were seven and RSS
3,093,266,432 bytes. Evidence remains in
`/private/tmp/mcr-operations-roles-ckl8v4fp/`; all history remains charged. The
released target is not reentered and the spent round is not resumed.

### Source fixes after the failed round

The main-file owners explicitly transferred exactly two Operations worker-pool
hunks. Commit `c2cfdcd7b38ab3d1f6b454910985c9d8f1a0dd7a` clones
`operations_worker_pool` before the supervised coroutine and passes that clone
to `publication.run`, preserving the original outer shutdown `pool.close()`.
No other main-file line changes. Fresh compilation is NOT_RUN.

A second issue found while tracing the actual role writer is independent of the
compiler failure. Migration 0061's immutable guard permits INSERT only at role
revision one. The previous `INSERT ... ON CONFLICT DO UPDATE` writer would first
execute its BEFORE INSERT guard for a revision-two revocation, so it could fail
before reaching UPDATE. PostgreSQL documents both trigger paths for this form;
see [PostgreSQL 16 trigger behavior](https://www.postgresql.org/docs/16/trigger-definition.html).
The writer now uses explicit INSERT for the absent row and explicit guarded
UPDATE for the already locked existing row, within the same advisory lock,
transaction, verified statement and exact expected revision. It requires one
changed row. The signed statement/revision guards, journal, replay semantics and
historical migration bytes remain intact. The existing three real-owner native
cases already cover actual revocation and old replay; they remain NOT_RUN.
This source correction changes the store blob in the precise dependency packet;
consumers must use that new store revision instead of the earlier five-blob
snapshot. It does not widen role issuance or add a new SQL migration.

### Follow-up build passed; native fixture namespace failed

`OPERATIONS-ROLE-FOLLOWUP-MEDIA-20261104T1920Z` admitted at
2026-11-04T19:29:51.432762Z against
`0e6b4b6f911507db7ba6cc555f46e6a8d3c8eb2d`, after both the predecessor's target
and FULL releases. The specific 12 GiB start guard and 9 GiB after-all-growth
reserve passed; both controls pinned the future per-workflow class rule. The
512 MiB conservative peer-growth reservation covered the then-current 496 MiB
upper bound. Historical carry was 11,579,277,312 bytes, NEW 2 GiB and WHOLE 14 GiB.

The exact two selected integration-test executables compiled successfully, exit
0 in 52.47 seconds, including the explicitly allocated necessary implicit normal
Concert binary prerequisite. The worker-pool clone therefore compiled. Only the
two test executables were copied; no Concert main copy or run, Gateway or stack
was added. The frozen pin is
`/private/tmp/mcr-operations-roles-frozen-8krh1b53/pin.json`.

The four issuer cases passed, exit 0, with log SHA256
`2871f883e7469f212fa37ecdd8b5247e85f8423fe44d5d4793046cec588d2470`.
The three native cases all failed before constructing the role owner, each with
`Invalid Operations role pool or instance`. This is a concrete fixture naming
incompatibility: the retained fixture selected `mcr_canonical_concert`, while the
production owner deliberately accepts only `mcr_concert` or a bounded safe
`mcr_concert_*` name. It does not establish a factory or native-role success, and
the database restart stage was not reached. Native prepared transactions were
zero before teardown; owned PostgreSQL processes and the exact root were absent
afterward. The failing native log SHA256 is
`4a02066086048b064f34080a9e0bcac282411cf9a08ece3940c006761d10e82c`.

The fixture now additionally accepts only the exact isolated Concert test name
`mcr_concert_roles_native`. Historical Report fixtures retain their original
name. The private qualification runner must create that exact database and use
it for its Concert ledger and restart queries. All existing private-root,
socket, owner, token/inode, current-database, data-directory and PostgreSQL
identity checks remain. Production pool validation, the six minimal production
dependency postimages and migration 0061 are unchanged. This fixture correction
requires a fresh build/native continuation; the already passing issuer cases
need no unchanged repeat. Corrected native and restart qualification are NOT_RUN.

Compiler/handles/guardian-reader-zero target release was 19:30:52.916730Z, sent
directly to Gateway207 before copied tests or PostgreSQL. The receipt SHA256 is
`2806559f45ecfe4441a6b67798152f377723db215211d52e2c3ed39e09d2c0fe`.
Actual FULL release was 19:32:03.274103Z, guardian reaped, exact owned roots absent
and workflow/NEW reservations zero. NEW peak was 858,775,552 bytes, WHOLE peak
12,438,052,864, positive target growth 728,432,640, outside peak 130,342,912 and
PostgreSQL peak 88,432,640. Maximum processes were 15 and RSS 3,192,520,704 bytes.
Evidence is `/private/tmp/mcr-operations-role-followup-_es81skn/` and the adjacent
frozen native report. The spent turn is closed with no target reentry. Actual
OwnAccess HTTP/default App discovery and the assembled IN04 journey remain open.

### OwnAccess remains part of the assembled Admin delivery

The latest delivery direction explicitly retains genuine frontend role discovery
under original BE38, FE37/FE38/FE39 and IN04. Wall's server permission checks do
not close that requirement. The existing endpoint and producer are reused;
there is no parallel role producer, caller role header or bootstrap grant.

Root1fc owns the shared main/Product/Gateway/registry joins and publication.
Root153 owns central frontend composition and developer53d the assigned Admin
consumer. They received the exact immutable OwnAccess-only contract source
`3a089a3b6b98f01d301a69c02020d77519fa3a2b`: additive changes to
`contracts/domain/v1/admin.schema.json`, `operations.json` and `policy.json` only.
Those three hunks predate ReportEvidence and report-decision additions. Current
machine artifacts must be regenerated by their assigned writer from the merged
registry; the older generated files are not whole-file replacements.

Root reports source joins for the existing `public_access/protocol.rs` and
`runtime.rs` and the existing Operations overlay using the SAME published role
store, publication and canonical core as Wall. This is source disposition, not
build, native, public HTTP or default App proof. A frontend registry without
`getOwnConcertOperationsAccess` correctly remains unavailable and makes no
fallback request. Actual member `access:null`, actual signed role discovery,
current caller/event binding and assembled Admin entry still require their
responsible developers' genuine runtime qualification. ReportEvidence, age
providers and physical-place flows are not prerequisites for virtual Wall IN04.

### Current split of Operations and newly assigned public service policy

The latest human-authorized staffing direction routes the whole original BE38
public maintenance/minimum-client service-policy producer to existing Social16e
under a new durable assignment objective. Its old unfinished Social goal stays
intact; no reset, false completion, pause, new chat or substitute API goal is
required.205 retains signed Operations role assignments, publication and
OwnAccess. Whole Report work transfers separately to213 under the committed
Report STOP packet `142a8d648c73a014f78071897ad1cb7b499af531`.

205 has made NO public service-policy producer writes: neither
`operations_live/service_policy/` nor `0087_event_service_policy.sql` existed at
the handoff census. No existing Settings/application DTO, producer, routing,
wire, authority or schema hunk was changed for public maintenance/minimum-client
policy. There is no service-policy resource lease or runtime evidence to hand
over.205 stops any new work on that public-policy namespace and transfers only
the following concrete disjoint scope from Root's newly stated bounded grant:

- `backend/services/concert-service/src/operations_live/service_policy/**`.
- One additive `pub mod service_policy;` export in
  `backend/services/concert-service/src/operations_live/mod.rs`; no wholesale
  replacement or other Report/role exports. Coordinate the exact edit turn with
  the Report owner for this mixed file.
- New `backend/services/concert-service/migrations/0087_event_service_policy.sql`;
  preserve existing migrations and Root's allocation of the number.
- Dedicated new `backend/services/concert-service/tests/service_policy*.rs`,
  `backend/scripts/mcr/service_policy/**`,
  `docs/mcr/backend/service-policy.md` and
  `docs/mcr/backend/service-policy-assignment.md`. These exact names follow
  Root's final grant and supersede the earlier proposed script/doc names.

Root's final transfer names the new isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/be-service-policy-16e`, branch
`codex/mcr-be-BE38-service-policy`, base
`142a8d648c73a014f78071897ad1cb7b499af531`.16e directly accepted the actual
STOP/census and whole new objective; checkout creation is a receiving-owner
operation, not asserted as complete by this source packet. Root likewise names
the whole Report checkout `be-report-target-media-03de`, branch
`codex/mcr-be-BE36-report-target-media`, at the same142a base. Actual receiving
checkout/assignment evidence remains with those developers.205's checkout is
not shared and no resource/target lease transfers with either source base.

The new objective covers actual database-revisioned operator-managed
NORMAL/MAINTENANCE and validated minimum CLIENT release policy, canonical
operator authorization/revocation, CAS/idempotency/audit, server-clock expiry,
current publication/failure/recovery, typed public bootstrap/admission decisions
and server enforcement. It includes meaningful bounded developer-owned
validation and fixes, provenance/evidence/limitations and committed delivery to
Root as publisher. No helper or source-only result closes the feature.

Root retains central C03/C04/public contracts/registry, generated outputs,
main/config/root composition and any additional shared-file transfer. Existing
`admin/event.rs` Settings has features/TTL/cooldown only; it is not the public
service-policy DTO. Existing `PgEventOperations::settings` and operator
`getConcertSettings` remain read-only compatibility inputs, not grants for16e
to overwrite or public authority. Existing role publication/OwAccess files stay
with205; use the actual retained typed authority or obtain exact adapter hunks
directly before any edits. No synthetic role, cached bootstrap, local World
state, generic503 or feature boolean supplies this decision.

16e proposes the literal typed snapshot/early-denial ABI directly to Root and
Root153 before any transport writes. Auth/service eligibility/age/event admission
and READ_ONLY/SAFETY remain distinct axes. Once the actual public contract and
qualified producer are usable, hand off DIRECTLY to Root153 and World with the
frozen contract/source revisions, truthful closed states/version/expiry semantics,
responsible-owner qualification receipt and remaining limits. The completed
FE41 goal and unchanged client matrix stay preserved; this is not a new IN04
prerequisite or a manager review/acknowledgment stage.

Root's actual OwnAccess-only canonical source publication is
`a0cb4396d2fe5c333e7863aa5bf4c8b88a357ac9` (parent `7ba6de456`, preserving
`3a089a3b6b98f01d301a69c02020d77519fa3a2b` ancestry): the three additive domain
files above plus `backend/scripts/mcr/operations_public_contract_test.py`.
Root reports fresh strict4 cases PASS and FULL19:46:14.409788Z. This is the
publishing developer's source qualification, not205's independent test rerun.
Generated machine closure148, genuine endpoint/default App and IN04 remain
pending their owners; current147 absence correctly fails closed.


### Corrected native role qualification —2026-11-04

The fixture database-name correction `f4472dd7518889ebc8a02cd0ea2d0163dc526572`
is now qualified at the unchanged source
`ebb3e40ada2b9e14d16bc973af999c8dac9702a4`. The earlier failing native result
remains historical evidence; this is its fresh continuation, not an unchanged
repeat of the already passing issuer suite.

Under `OPERATIONS-ROLE-FIXTURE-MEDIA-20261104T2000Z`, exactly one
`operations_public_roles_native` test executable was built/frozen with locked,
offline Rust1.88, two jobs, debug0 and incremental0. The recorded Cargo command
and exit0 are in the private control packet's `commands.json`. All3 cases passed
with exit0: an admitted member without assignment remains unprivileged; an actual
prepared role exclusion blocks revocation until real Root terminal; exact old
signature replay cannot restore a revoked assignment. Log SHA256
`df9fe1c8ce4914952974fe0f6866c4a9f64af970778078f5804887e065b00077`.

The same owned PostgreSQL16.14 cohort then restarted. Auth18/Friend18/Concert49
migration ledgers and assignment, role-guard, statement, receipt and outbox
digests were unchanged; postmaster identity changed. Prepared transactions were0
before teardown, captured PostgreSQL PIDs were absent and the exact owned root
was removed. This establishes native role/restart behavior, not HTTP OwnAccess,
settings, default App/Admin discovery or the managed-issuer application's result.
Those assembled journeys remain NOT_RUN. No issuer rerun, HTTP flow, Concert main
copy/run, Gateway, browser or second stack was added to this round.

Actual acceptance was20:40:18.845552UTC. Target/compiler/handle/guardian-reader
release20:41:28.281032 was sent directly to Root1fc, BackendSL and Party before
copied qualification/cleanup/docs. Main's routing amendment made Root2050 the
actual next target owner, then Party/Gift; the frozen control's legacy Party
notification field does not grant Party custody before Root release. There was
no target reentry. Actual FULL20:42:15.706094 verified guardian reaped, owned PG
root/processes absent, workflow0 and new reservation0 before documentation.

Private controls/proof: `/private/tmp/mcr-operations-role-fixture-fix-wss3lx0b/`.
Immutable executable/source/runtime pin and native receipt:
`/private/tmp/mcr-operations-roles-frozen-yxptu0xt/{pin,native-report}.json`.
Peak NEW854,335,488 bytes, WHOLE13,309,038,592, target726,302,720, outside128,032,768,
PG88,903,680,16 processes and3,155,066,880 RSS bytes. Retained source/copy/proof
remains charged as historical carry. The working checkout stayed at EBB; this
is a separate documentation-only handoff. Developer205 retains validation fixes;
Root1fc handles normal publication without a king/SL review or ACK stage.


### ReportEvidence-only bridge ownership transfer —2026-11-05 KST

At Root1fc's explicit request,205 stops all future ReportEvidence-specific bridge
hunks listed here and transfers them to Report implementation owner213
(`01a1a213-03de-72d3-9ecf-2f72b574cf82`) in its isolated Report checkout. This is
writer scheduling, not technical review or an acknowledgment gate. Existing EBB
and frozen settings handoff15c618 remain unchanged.

Transferred future hunks, not whole files:

- `operations_live/public_access/mod.rs`: the NEW `report_evidence` module export
  and any exactly declared ReportEvidence-only producer-leaf export required by
  that owner. Existing exports, particularly Settings, retain their owner.
- `operations_live/public_access/protocol.rs`: one NEW ReportEvidence operation
  variant; its `id()` arm mapping to `readConcertReportEvidence`; exact GET
  `/api/concert/events/{event}/admin/reports/{report}/evidence/{media}/content`
  recognition and evidence-specific closed validation/tests. Existing OwnAccess
  and SettingsRead routes, IDs, payload constraints and tests retain205. The new
  variant's literal Rust name is for213/267 to resolve; it does not exist at EBB
  or15c618 and is not a previously frozen ABI.
- `config/operations_runtime.rs`: ReportEvidence-only imports, actual producer
  construction and scoped-owner mount; one evidence overlay field/initializer
  and exact ReportEvidence dispatch arm. Reuse the SAME actual pool, roles,
  publication, canonical fence and core delivery. Existing OwnAccess/Settings
  construction/dispatch and role lifecycle/issuer/publication semantics retain205.

Paths above are under `backend/services/concert-service/src/`. There is no grant
to rewrite a whole mixed file or make a second authority/role owner.
`public_access/runtime.rs` remains the OwnAccess producer: preserve its15c618
exact OwnAccess discriminator and all existing read/guard/SQL behavior. No hunk
in that producer is transferred. Root-owned shared `product_rpc/mod.rs`/query,
main/router composition, contract registries, generated files and infrastructure
remain with their named owners.267 retains the actual typed ReportEvidence
SDK/0080 dependency;213 resolves that interface directly with267.

The current unexported `public_access/report_evidence.rs` supplies request-bound
`EvidenceRoute`/`ProjectedEvidence` authority. It is not already a functioning
scoped product producer or proof of actual evidence delivery. This transfer
authorizes the listed assembly seams and does not certify implementation,
queries, byte delivery, audit, Root EOF or runtime acceptance.213 owns necessary
implementation/validation/fixes and its committed Root publishing handoff.205
retains role/publication/store/OwnAccess/Settings and the f447 fixture correction.

Literal STOP/transfer was sent directly to213 before this documentation commit.
No source bytes, frozen handoffs, targets or peer checkouts were changed; no test,
service or database was started. Root publication uses normal exact-path intake
without king/SL review or ACK.
