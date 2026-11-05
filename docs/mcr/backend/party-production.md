# Party production ports

Status: IN_PROGRESS. PARTY-PRODUCTION-PORTS-01 implements the production prerequisite for BE16/BE17/BE18 and IN02. No whole-task acceptance is claimed. The latest main and two test-executable builds passed at source `60478c66d1e3feedf35ad49ddc483f254cc49d4d`, with backend source unchanged from `0c1219093ce7e38ff4bd3dd7491f261ac7ba4a55`. The five corrected copied route tests passed in the separately allocated round below; fresh native/public qualification remains NOT_RUN. Earlier checkpoint statements below are historical; the latest build record follows at the end.

## Ownership and source

Exclusive worktree: `mcr-worktrees/be-world-profile-producer`; branch `codex/mcr-be-party-production-ports`; public base `f339f6240db94353e990fed722da0feb598ae9d2` contains published Party native settlement `2592d540ceb293ecc65ca65a387e88d516d8ffa4`. The earlier settlement goal is complete and its original evidence remains retained; this is new production behavior.

Root transferred `config/party_runtime.rs`, `config/party_resources.rs`, the sole `party_runtime` module export, and `party-shared-composition.md` from `af5ec20f22c15ac2d5546f25efec0473f3e27215`. Runtime/resource formatting is adjusted locally. That historical source's previous compilation did not activate Party. Existing dependencies remain unchanged: anyhow, async-trait, chrono, concert-application/domain, JSON Schema, platform, Prost, Serde, SHA-256, SQLx, Tonic and UUID. No manifest, lockfile, public contract, source archive or frontend change.

The additional exact transfer covers a new `config/party_composition.rs` and export; `party_live/settlement.rs` only six registered Party command-status READ support; new `0062_party_command_status.sql`; Party runtime tests/scripts/docs; and two Party-only main hunks. Wall 202 and Operations 205 received direct pre-edit notification and confirmed no conflicting shared hunks. Root remains the named publishing developer. No review, approval or acknowledgment stage is required.

## Actual composition

Main loads `PartyStartupSettings::load(runtime.is_some(), intro_settings.is_some())` before database/listeners and composes its overlay after Social and before Reports. It passes the same existing pool, `shared.owners` and base Product owner. The existing Participant shared assembly already constructs `NativeResolverUnion` from the same pool and instance as its canonical core. The Party addition selects the existing eight Party operation/method/target tuples and only the six Party command-status queries. Other domains and Wall status queries retain their owners. There is no second listener, canonical identity, registry or lifecycle.

Default/off remains off and reads no keys. Live requires guarded gRPC/shared canonical assembly and explicit independent nonzero retained receipt/cursor keys. Production requests use actual Auth/Friend scope, current local admission/participant and BE37 policy, canonical PublicProfile reader, event feature/phase locks, and BE09 physical-place selection. No permissive resource or profile substitute is installed.

Status uses the same acquired parent and `PgCommandRepository.lookup`, scoped to the authenticated actor/event/registered operation/key. The immutable original command witness supplies an internal target hint; current feature, actor, root ownership and subject-pair authorization are rechecked on the actual read connection. Removed applicants cannot recover their prior application through status. Status returns only the frozen common CommandReceipt projection, never an application note, aggregate, original request/body, profile claim or owner token. The actual status transactions register their own READ witnesses; an old command receipt never certifies the status transaction committed.

Migration 0062 forward-replaces only the existing Party assertion to admit a closed six-operation status GET/empty-body/READ tuple. Published 0057 stays byte-identical. Exact confirmed native/parent/owner checks, original deadline and immutable witnesses remain. Existing deferred READ/LOOKUP effect prohibitions remain installed for aggregate, application, membership, receipt and outbox writes, including effects before the witness.

## Validation plan and limits

Source formatting and `git diff --check`: exit 0. New route tests cover eight actual Party tuples, swapped operation/event/parent, malformed query, and exact six-command status selection without foreign-domain capture. Imported configuration/resource tests cover default/off, invalid opt-in/dependencies, key rotation/reuse/duplicate/unknown fields, feature phases and safety actions. Cargo/native execution requires a fresh bounded resource allocation; no old lease is reused.

The retained seven-service foundation is pinned at `18076c951a7283354c869caaf64d40491ca52d75`, with producer qualification and per-binary source/SHA receipts. Its unchanged mains may be reused only within fresh admitted bounds; changed Concert must be rebuilt from this actual main. That cohort is historical foundation evidence, not Party production proof. New Auth consent behavior must not be silently mixed into the older cohort.

Fresh qualification must exercise three actual Gateway signup/mail/verify/login principals, strict canonical C03, real Party negative/resource/status/replay/privacy behavior, actual PostgreSQL restart and cleanup. Current-authorized successful status, revocation, foreign target and six-operation boundaries need meaningful native coverage. Synthetic component fixtures, if used, are labeled and never establish physical approval or an actual positive public journey.

WAITING_INPUT: the current quality-v6 map's four place records are CONCEPT_ONLY/UNREVIEWED. Root confirmed `venue-provenance.md` has no qualified physical publication/operator verification. Production Party creation must reject these through BE09. No input is promoted, invented or relabeled to force a positive journey. Positive physical create/apply/approval remains unaccepted until a real eligible reviewed place/resource is supplied. Physical rally/meetup and whole IN02 remain separate gates.

Original scope remains open: specification M04/BE16 requires edit meeting and close; the current public Party adapter exposes eight operations and omits these. Frozen C04 already defines `updateConcertParty` as PATCH `/api/concert/events/{eventId}/parties/{partyId}` and `transferConcertParty` as POST `/api/concert/events/{eventId}/parties/{partyId}/owner`. The existing registry and headless `PartyCommand` agree. `PartyAction::Update` accepts CLOSED and invalidates meeting/audience revisions when meeting coordinates or time change. This is missing adapter wiring, not a missing contract. Exact shared and Party-leaf ownership was requested from the publishing owner; this checkpoint does not silently cut those requirements.

## Delivery

Source commits `ba8e037a7ade7109832a4f77f47eaefc53e5059d` (production composition) and `e6ebb82e86b753bbfd768f7404b17a36372558f1` (resource visibility fix); no publication claimed.

The first fresh build allocation accepted at 2026-11-04T17:35:37.304226Z and fully released at 17:42:12.807514Z (395.5 seconds within the 480-second total cap). Compilation of the library test executable and three route tests succeeded after the visibility fix. Their executables were frozen outside the target; tests are NOT_RUN. The actual non-test Concert main build failed because its distinct Cargo build-script output omitted the required generated account-deletion module. All four actual source build scripts and the source proto exist; the next main-only build must refresh the exact authorized four build-script mtimes immediately before that profile. The retry stopped at the original 300-second deadline. No clock or cap was extended.

The independent observer also raced its own `du` reader during a repeated zero-handle check; that controller refusal is retained separately. Admission had actually established zero external handles. A later peer Cargo/PG job began around 17:40:32 before our target release. Its owner acknowledged the conflict, excluded that run, stopped and reaped its processes. We did not kill a peer process or claim a clean combined run. Fresh global Cargo/rustc/target handles zero were verified at 17:42:12.806885Z, then the target was handed directly to Reports 205. No reentry, PG runtime or public success is claimed.

Private receipts and attempts: `/tmp/mcr-party-native-settlement-01/production/`; target-release SHA256 `0bba1c36c233848c0ce0de2bd897a213d0f0a2744e0052ed658c8bea346fe0e2`. Retained historical target counters remain UNKNOWN and centrally charged, not zero. Fresh public copied-native admission is NOT_READY until the actual main is built. The future controller uses Root's supported R7 pool sizes (Auth/Friend 3, Profile/Notification 2, Post 3, Gateway 2 + journal 2 + background 1, Concert unchanged 8) and an owned PG64/prepared128/auxiliary-off functional cluster; this makes no maintenance, replication or load claim.

The forward status assertion additionally checks the same transaction's pinned BE37 validity at deferred READ commit, because a read has no aggregate write to fire the existing community deadline trigger. Developer owns implementation, relevant validation and fixes, documentation and committed handoff. Root publishes the validated scope normally with combined checks and a canonical receipt. Required unfinished work remains active.

Source follow-up after `78a0ba761103929198bf7554ac964307be36f8de`: status routing and migration 0062 decode C04 form fields exactly once before selecting the closed Party operation/key. The raw signed HTTP target and its digest remain unchanged. Tests include encoded field names/operation bytes, decoded duplicates, encoded separators, malformed escapes and foreign operations. These new cases and the SQL remain NOT_RUN. The runtime harness also removes a stale World address lookup: it reserves only Concert gRPC and metrics, and must not look up an unreserved World listener before deleting World settings. AST parsing, formatting and diff whitespace checks pass; no new compilation or native lease is claimed.

The prepared runtime journey now checks the native SQL selector against all six operations, both query orders, encoded fields and malformed/foreign cases. Three real actors query all six absent command receipts and an encoded create query before and after a physical PostgreSQL restart. It requires an exact count of committed READ witnesses linked to ROOT_FINISHED scopes and confirmed Auth/Friend registrations, without exporting identities or native tokens. After restart it performs bounded seven-service readiness before public replay. This remains prepared, NOT_RUN evidence; neither missing-receipt responses nor parser tests certify a successful command-status projection or revocation.

## Frozen lifecycle completion follow-up

The publishing owner subsequently transferred exact Update/Transfer-only Concert Product parser/admission/media arms, Gateway Party route/receipt hunks, and necessary Party live validation/guard/native/status support. Gift 267 and Reports 205 were notified before edits. New forward migration 0064 widens the retained native/status allowlists for these two existing operations, preserves 0057/0062 unchanged, and requires an actual current BE37 pin at deferred status READ commit. Registration must occur before policy work, so the deferred check rejects a transaction that never performed current policy authorization as well as an expired one.

Source now wires PATCH party edits with OPEN/CLOSED/COMPLETED states and POST party/owner transfer through the existing `PartyCommand`. It validates the frozen closed JSON before owner acquisition, binds method/target/body/key, requires current owner and relevant subject authorization, selects the actual BE09 physical place for an edit, preserves optimistic revisions/capacity/meeting/audience invalidation, and retains the normal safe receipt/outbox path. Status selects eight Party mutations. Transfer status requires the original actor to remain an active member and current actor/owner policy; update status requires current ownership. Generic media owners cannot consume these routes.

Four focused route/body tests are prepared, including all ten Party tuples, exact methods and events, closed update/transfer fields, duplicate fields, invalid state/capacity/revision and foreign status operations. Existing Gateway Party receipt-ownership tests now include PATCH and owner transfer. Current changes are source-only: compilation, native execution and public lifecycle success are NOT_RUN. Both Concert and Gateway actual mains now require fresh source-bound builds. The launcher labels those two inputs `PARTY_PRODUCTION_COMPILED` (build evidence only), while the other five retain their real historical foundation qualifications. A historical Gateway binary cannot qualify these new routes.

Root also assigned Party private-input withdrawal and Meeting Beacon/Rally revocation under BE42, including narrow user-first deletion-fence exclusions and new migration 0068 for command/source-bound owner journals. That work is separate unfinished implementation within this coherent scope. Party owns one combined PARTY_AND_GIFT_INPUT category adapter only after its own and Gift 267's actual subreceipts are terminal; aggregate coordinator paths remain with 00c. Neither a Party-only result nor an unconfigured Gift port may certify the category complete.

## Successful public journey gate

`backend/scripts/mcr/party_production_positive.py` now authors the required successful three-user probe. It creates no process, database seed, identity, operator grant or reviewed venue. A separately allocated caller must supply the real qualified Gateway/Auth/C03 stack and an actually published eligible place. The probe retains bodies, application notes, tokens and receipt identities in RAM; only status labels and bounded outcome summaries enter the report.

The probe covers create and exact retry; changed-body idempotency conflict; foreign-actor status refusal; private application text exclusion from public views and third-actor collections; applicant cancel/reapply; owner approval; capacity rejection without oversubscription; safety cancellation while full; actual meeting edit, ownership transfer and former-owner refusal; member removal and receipt revocation; closing, closed-party application refusal and cancellation. All eight successful mutation receipt shapes are checked for exact actor/event/operation/key/resource and a closed safe projection. The same in-memory object compares the owned database's actual postmaster start timestamp across restart before checking durable Party and receipt replay.

This probe is NOT_RUN and is deliberately not mounted in the current negative-only catalog round. Current quality-v6 venue inputs remain CONCEPT_ONLY/UNREVIEWED. A physical input flag or a fixture cannot approve them. The later positive round needs its own usable resource allocation and actual published input.

Authoring the probe exposed two additional frozen functional interfaces absent from the current source: `listConcertPartyApplications` (GET party/applications) and `getConcertPartyActivity` (GET me/parties). Neither has a Concert parser/producer nor a Gateway route in this source. The owner collection and current-membership assertions remain in the probe; they are not omitted to obtain a narrower pass. The exact implementation ownership/source dependency or shared-hook grant was requested from the named publisher. The existing five-executable build round can validate its assigned lifecycle/cleanup scope but cannot certify this missing original functional surface or whole BE16/IN02.


## Application collection and private activity follow-up

Status: IN_PROGRESS; compile/native/HTTP/restart qualification NOT_RUN. Root explicitly assigned the two frozen read operations to this Party implementation owner, separately from the immutable five-EXE source at `9fcd837d846571f5951fb1bc56cb90532bf53abd`. Publication stays with the named publishing developer; there is no king/SL review gate.

`listConcertPartyApplications` is the bodyless GET on the existing Party applications path. It locks the current Party root and requires the current authenticated owner, then projects each permitted applicant through the qualified PublicProfile reader. Its closed form query admits only limit, state and an opaque cursor. `getConcertPartyActivity` is the bodyless GET on `/me/parties`; it uses only the authenticated actor and returns owned Parties, that actor's memberships and that actor's applications as three independent cursor pages. It accepts no requested user identity. Current ownership, membership, admission, participant, pair policy and profile authorization are rechecked; removed members cannot recover prior private application notes through activity.

Candidate discovery precedes policy/pair locks, and all selected Party roots are locked in UUID order before projection. Each collection scans at most 1000 candidates and emits at most 100 records. Cursors authenticate actor, event, operation, collection, relevant Party, state filter, limit and an immutable initial upper bound, with a 15-minute expiry. They reuse the retained cursor key under a separate private-cursor associated-data domain. No new secret, listener, identity, dependency or schema contract is introduced.

New migration `0082_party_application_activity_native.sql` forward-replaces the native assertion from `0064_party_update_transfer.sql` at the stated base revision. It preserves actual native XID/backend/system identity, confirmed parent registrations, exact request/reference digests and original deadline checks, admits the two operations only as empty-body READ witnesses, and requires a current non-null BE37 authority pin at READ commit. Prior migrations and business tables are unchanged. The generic owner guard and native resolver remain unchanged; the latter already delegates to the closed Party operation parser. Other-domain ProductRPC and Gateway behavior is preserved.

Prepared component tests cover decoded query duplication, foreign projection parameters, body/content/idempotency rejection, operation swaps, and cursor actor/event/operation/collection/Party/state/limit binding, tampering, wrong keys and expiry. Rust syntax parsing and patch applicability alone do not certify their behavior. Fresh allocated compilation, actual canonical Auth/Friend native settlement and HTTP current-owner/transfer/removal/privacy/pagination/restart checks remain required. The full three-user positive probe still requires a genuinely reviewed published physical place; synthetic fixture approval cannot satisfy that input gate.


The same follow-up consumes the exact unchanged migration-directory API hunks from `9a3c008b6914146d1c38d5de29010dc91ac2d4eb` (parent `2ae0d393c28d9f42a9b33af572e56b2f7857f70d`) under Root's explicit selective grant. Only `config/mod.rs` migration-directory function/helper/inline tests and pool propagation, the main Migrate/Verify callsite, and the participant-cancellation test caller Result adaptation are imported. Other exports, composition, CLI inspection behavior and feature mounts remain untouched. No dependency, contract or migration from that supplier branch is imported.

`party_production_runtime.py` now requires that API in the actual Concert binary source and materializes the exact source-revision SQL package in its owned temporary cluster root. Each CLI/catalog command and serving startup receives `CONCERT_MIGRATION_DIRECTORY`; package inventory, identity and SHA-384 bytes are checked before and after command execution and at later input boundaries. The actual SQLx ledger must still match the pinned source manifest. Old binaries cannot gain the new override and are rejected as unsupported by this harness. Package checks alone certify no schema, native transaction or product behavior. The supplier API and Party launcher changes remain source-only until fresh actual-main and owned PostgreSQL qualification.


## Fresh five-executable build — 2026-11-04 19:19 UTC

Source: `b8272c06da4423432ec8e872b0607aac4db22897`, parent `9fcd837d846571f5951fb1bc56cb90532bf53abd`. The publishing developer explicitly released the earlier follow-up-after-freeze timing condition because the previous offer closed unaccepted and produced no executable. This coherent source includes both private reads, the unchanged selectively imported migration-directory API, lifecycle and cleanup. The declined offer and failed older attempt remain retained; neither was reused.

Allocation `PARTY-FIVE-EXE-MEDIA-20261104T1902Z` actually accepted at epoch `1793819985.5969238`. Rust 1.88, locked/offline, two jobs, dev/test debug zero, incremental zero and `contracts-build` optimization 2 were used. Both commands exited 0, in this order, against the assigned backend manifest:

```text
cargo build --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 --config profile.test.package.contracts-build.opt-level=2 -p concert-service -p api-gateway --bin concert-service --bin api-gateway --message-format=json-render-diagnostics
cargo test --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 --config profile.test.package.contracts-build.opt-level=2 -p concert-service --lib --test party_runtime_routes --test party_deletion_native --no-run --message-format=json-render-diagnostics
```

The exact authorized byte-identical build-script/source mtime refresh and before/after identities are retained in `round2/build-script-mtime-main.json`. Actual mains built first. Cargo's later integration-test build also emitted the normal Concert executable; the frozen Concert copy is that later artifact from the same source, not a claimed preserved hash of the earlier overwritten main.

Exactly five copies were frozen outside the shared target, combined 254,178,800 bytes:

| Executable | Bytes | SHA-256 |
| --- | ---: | --- |
| api-gateway | 57,877,408 | `832b0e2c44c83450858a38bf05d68db9f618dbfdf64ca4e08724f8f61703dfee` |
| concert-service | 70,435,888 | `39e14a1c0a0bb069b6247433dc5586cceeb948435b45ce99f3d3fb835e860f08` |
| party_deletion_native | 14,853,216 | `458d5943269ebf1f79a6e698e65fa5f3e032928334f349e059c37a407f789ec9` |
| party_runtime_routes | 13,866,768 | `98b540e7aaae74f4fa0f4b87521e5fa3ecd2c684526b10489aeafbd91d96a7e5` |
| concert-lib-tests | 97,145,520 | `fe9317f469637dce9c10626b4d0b92f0aee28b1957d2d4b599431fd58e3b4a88` |

The independent guardian reported no limit breach and all owned processes reaped. Conservative peaks: NEW 564,834,304 bytes, WHOLE 7,007,285,248 bytes, positive target growth 312,631,296 bytes, RSS 4,591,452,160 bytes, 10 processes; minimum free 14,151,966,720 bytes. Historical carry remains charged. Literal target release occurred at epoch `1793820153.377543`, with compilers and target handles zero. It went directly to successor 207 before documentation; no target reentry. FULL release epoch `1793820278.4583948` was 125.081 seconds later, within the 180-second reserve and original absolute deadline. Workflow and remaining NEW reservation are zero.

Private evidence stays in the existing production evidence directory: `round2/lease.json`, `source.json`, both compiler JSONL/stderr pairs, `compile.json`, `compile-guardian.json`, `binaries.json`, `target-release.json` and `full-release.json`. Binary receipt SHA-256: `2ca340b6d71fcf909272a93f0c3c32139d83885261ea78b31dd0ed7c2f2b5429`; target release SHA-256: `33136e8f65cf625418a91359283b9c5d0a4d5d566918b445ee0ee3469fc33e17`; FULL SHA-256: `b8c41cc78c70d082c57da272dd0e7dbc2c81a38873ca2dcc3ef9f142b609a0dc`.

This was BUILD/COPY ONLY. Component tests, owned PostgreSQL migrations, native deletion, seven-service HTTP/C03/status/privacy/restart and the successful physical-place-dependent three-user journey remain NOT_RUN. Compilation does not establish those gates. The next bounded copied-unit/SQL-custody round is requested separately; the actual-main production round and eligible reviewed place remain required. Publication stays with the named publishing developer after developer-owned validation and fixes, without king/SL review or acknowledgment.


Party launcher follow-up: the historical shared `world_in01_live.QualificationGuard.start` still requires 16 GiB. The Party-owned launcher now uses an explicit bounded subclass, leaving that shared helper unchanged. Its required CLI inputs carry the actual offer's start floor (never below 11 GiB), inclusive NEW cap (at most 2 GiB) and all other outstanding growth. Admission additionally requires 9 GiB after every reservation; the existing 8 GiB live floor, original absolute deadlines, WHOLE/process checks and cleanup remain. It also measures output plus the owned cluster's positive growth against NEW. A separate independent guardian still owns complete history/RSS/process accounting. This is a harness-only change: frozen Rust binaries remain bound to `b8272c06da4423432ec8e872b0607aac4db22897`. Syntax parsing passes; the new runtime guard and production round are NOT_RUN until freshly allocated.

The Party launcher's temporary `Cluster` subclass also supplies the existing constructor's explicit fresh `report.parent/pg` root. Existing paths are refused by that constructor; marker/inode validation, start/stop and cleanup remain inherited unchanged. Keeping PG beneath the round's private output makes its bytes and daemonized postmaster visible to the independent guardian without searching or attaching a peer's temporary cluster. Shared cluster/helper files stay unchanged. This source-only containment adjustment remains NOT_RUN.


## Copied unit/custody round — 2026-11-05 04:33 KST

Allocation `PARTY-UNIT-CUSTODY-COPIED-20261104T1932Z` accepted epoch `1793820802.933722`; original work/FULL cutoffs `1793820892.933722`/`1793820922.933722`. Rust binaries stayed immutable at `b8272c06da4423432ec8e872b0607aac4db22897`; Python helpers were separately pinned at `a51c8303242b69eab3632cd02c709dbd060baf04`. All 2,273 backend hashes and five binary hashes were checked before children. Admission free 14,459,232,256 bytes; after all conservative eligible reservations, 11,976,204,288 bytes. No target, Cargo, new executable/copy, PG, service or browser access.

Actual result: **21 passed, 2 failed out of 23 declared cases**. Configuration 4, resource-policy 2, portable migration-directory 4, private query/cursor 2, three of five route tests and all six SQL-package custody tests passed. The first controller selected zero resource tests because their actual module is nested under `config::party_runtime::resources`; it refused that result. The developer corrected the filter, retained the first four passing cases, then ran the remaining cases without resetting clocks/caps or repeating already passing cases. Both actual route failures remain preserved. A subsequent guarded attempt ran only the six previously unexecuted Python cases.

Exception: after the first guardian finished, the developer ran a metadata-only `concert-lib-tests --list` diagnostic outside the guardian (24.7 ms; zero cases executed). This was a controller mistake, recorded separately; it is not claimed as a guarded diagnostic. All actual test-case execution was guarded. No resource-clean overall qualification or full-task acceptance is claimed.

The two failures have distinct causes. The lifecycle body parser called the frozen `Domain_party_UpdateParty` and `Domain_party_TransferOwnership` schemas, but its local closed schema-name allowlist omitted both, making valid requests fail before schema validation. The fix admits exactly those two already frozen schemas; it changes no contracts or permissive validation rule. The route test also expected `AbortOnly` for a signed event/target mismatch, although canonical `OwnerBinding::validate` correctly returns `InvalidArgument` before selecting any adapter. The corrected assertion requires that exact refusal and leaves shared binding/resolver code unchanged. Fresh compilation and execution of these fixes remain NOT_RUN.

Literal FULL release epoch `1793820872.536777` preceded the original work cutoff; it went directly to allocator 21d, responsible technical publisher and successor QA06 owner 1ab7 before documentation. Workflow/remaining NEW are zero; owned children reaped. Observed peaks across attempts: NEW 491,520 bytes, WHOLE 7,007,776,768 bytes, RSS 169,443,328 bytes, four processes, minimum free 14,155,132,928 bytes. Private receipt `unit-full-release.json` SHA-256 `736be45a89110fd2537055d3d24c75de533715530d392100ccf21ec1afd366f4`; failures, stdout/stderr, source/lease and guardian attempts remain in the existing evidence directory. The final controller's exit zero describes completion of the six remaining custody cases, not an overall test pass; the combined receipt truthfully remains FAIL.

Production HTTP/native/restart and physical-place-dependent positive gates remain unfinished. The copied-unit allocation grants no automatic seven-service continuation or target reentry. The implementation owner owns the two fixes and their new source-bound build/validation; publication stays with the named publishing developer without king/SL review.


## Three-executable follow-up build — 2026-11-05 05:45 KST

Frozen source `0c1219093ce7e38ff4bd3dd7491f261ac7ba4a55` contains the two route/parser fixes above. Allocation `PARTY-SETTLEMENT-THREE-EXE-MEDIA-20261104T2000Z` accepted at epoch `1793825114.718864`, after Root's literal FULL release. With the same Rust 1.88 locked/offline/two-job/debug-zero settings, the actual Concert main build exited **0**. The subsequent `cargo test -p concert-service --test party_runtime_routes --test party_deletion_native --no-run` exited **101**: inherited Mural Tools B protocol/repository/runtime imports could not find `concert_application::mural::tools_b` or `concert_domain::mural::tools_b`, followed by missing `PgMuralTools` methods. This is a failed combined build, not a route-test result. No private binaries were copied; the five earlier binaries retain their original source attribution. The corrected five route cases and all new production/native/restart checks remain NOT_RUN.

The independent guardian and all owned compiler children reaped. The frozen HEAD remained clean and all 2,279 inventoried source hashes matched at release. Fresh compiler and target-handle checks were zero. Literal target release epoch `1793825296.558784` went directly to the named successor Gift267 and allocator before documentation; no target reentry. FULL epoch `1793825316.627695` followed 20.069 seconds later, within the original limits. Workflow and remaining NEW reservation are zero.

Conservative peaks: NEW 110,116,864 bytes, WHOLE 7,117,893,632 bytes, target growth 109,711,360 bytes, RSS 2,150,088,704 bytes, ten processes; minimum free at final custody 13,663,748,096 bytes. Evidence remains under the existing private `production/round3/`: frozen source/lease/admission, both compiler JSONL/stderr pairs, build-script mtime custody, compile/controller/guardian results and literal releases. Target receipt SHA-256 `808c2fc68c83c96f80fe11ca77db9f5f7060d647ddc47a6917b1192ef4df370e`; FULL SHA-256 `cadad87ac268d65b7fe0f66f9bf193d4268739618aaea26d11fa0dbef2f9360d`. No tests, PostgreSQL, application services or browser ran. The responsible publisher has the precise inherited dependency failure; its remedy and any build retry require their own source disposition and fresh allocation.


## Interrupted three-executable retry — accounting exception

Allocation `PARTY-IN02-THREE-EXE-MEDIA-20261104T2130Z` accepted at epoch
`1793827857.437202` against unchanged backend `0c1219093` and documentation HEAD
`3ba672435`. The actual main build exited 0; the integration-test no-run build
exited 101. No executable copies were produced. The failure was two Auth service
non-exhaustive matches: Cargo reused an Auth application artifact whose diagnostic
source points into another checkout and contains consent/policy variants absent
from this frozen source. No Auth behavior is changed to accommodate that artifact.
A future allocated attempt will refresh the exact byte-identical own Auth inputs.

This round is FAILED / UNQUALIFIED. Its accepted 10 GiB WHOLE accounting omitted
the entire used shared-target baseline (19,471,908,864 bytes at admission), using
historical Party carry plus growth instead. The preserved guards therefore do
not establish resource compliance. The accepted cap and clock are not amended.
The guardian exited at epoch `1793827938.6120682` with no remaining owned children;
that exit is not a target or FULL release. Session interruption delayed the
actual supplemental TARGET release to `1793834091.330573` and FULL release to
`1793834102.259836`, 5,404.823 seconds after the original FULL cutoff. Fresh
reconciliation found no compiler processes or target handles. Workflow and future
NEW reservations are zero, with no target reentry.

Raw inputs, compiler output, guard results and supplemental release receipts stay
in the existing private `production/round4` evidence directory. Target receipt
SHA-256: `2f7f990ce570c834685a1ca68cd4e6ff4dffafe3f1b2559bd5a2bf81d882e022`;
FULL receipt SHA-256:
`a92a84879722ab9d77527533fc776569d45259733aa7a3c465c77d2765ea1a2b`.
The separate unaccepted retained-WHOLE 2140 offer was explicitly declined before
any child or target access so Gift could proceed. A later attempt requires a new
finite offer and both corrected guards charging the full used target plus the
retained outside inode union and a nonshrinking prior maximum. Tests, native SQL,
real services, restart and physical positive acceptance remain unfinished.


## Qualified retained-target build round5 — 2026-11-04 23:28 UTC

Allocation `PARTY-ROUND5-RETAINED-WHOLE-20261104T2328Z` accepted at epoch1793834887.602565. Actual source60478c66d1e3feedf35ad49ddc483f254cc49d4d retained backend0c1219093ce7e38ff4bd3dd7491f261ac7ba4a55; no BE39, Report or notification side packet was applied. The private controller and independent observer counted the entire used target plus the outside inode union and nonshrinking prior maximum under the40GiB WHOLE cap. NEW remained bounded1GiB (target768MiB/outside256MiB), with40ALL/8GiB RSS and original600/660/840-second compile/work/FULL clocks. Exact original inputs were refreshed only by the authorized hash/inode/mode/size-preserving mtime operation before each Cargo profile, including this checkout's auth-application lib/consent input; no Auth semantic change was made.

Rust1.88.0 locked/offline j2 debug0 incremental0 with contracts-build opt2: actual `cargo build -p concert-service --bin concert-service` exit0; `cargo test -p concert-service --test party_runtime_routes --test party_deletion_native --no-run` exit0. Total86.264seconds. These commands compiled and copied executables only. The five corrected route tests, deletion tests, services, PostgreSQL, restart, actual canonical native effects and whole IN02 journey were NOT_RUN.

Exactly three private binaries,99,156,112bytes total:

| Executable | Bytes | SHA256 |
| --- | ---: | --- |
| Concert main | 70,435,920 | ff364da78776db5c25bd6ce592b782dbd0fa62e849963b6f4b785b163e6b057d |
| party_runtime_routes | 13,867,280 | 11e7230b292cb653d93352b65e6d9c7e82ff04cb05b1b66e3668d9377dac9602 |
| party_deletion_native | 14,852,912 | be452efd39779c3ac638e469880a91f276922e5d03fe052c1bae0cfc6dede012 |

Observed peaks: WHOLE24,405,528,576bytes; NEW147,243,008bytes; RSS3,136,323,584bytes;9ALL; minimum free17,133,686,784bytes. Both controllers completed without a reported bound breach and all owned children were reaped. Source inventory2279files and clean source HEAD were checked at release. Actual TARGET release1793834994.0424309, then FULL1793835005.141267; these are literal observations, not inferred from Cargo exit. Both receipts went directly to allocator21d and successorWorld20f before later documentation. No target reentry is authorized.

Private immutable receipts are under `/private/tmp/mcr-party-native-settlement-01/production/round5/`: binaries.json SHA2561bcff484e30ca822fd17d2453237df3664e49433e3dc1fbb1686fce12c05352e; target-release.json52dc67a9b6ae05cac3c5747b85b9be777b706e2684416dc4bbb7a7308eaa3a87; full-release.jsone8d17ab006f3d1b02cc96577ff77bc61c30f78d11e787d399f2991dcd635168e. `build-provenance-derivation.json` links the launcher-compatible build markers to those actual compiler/copy/release records. New private `production-inputs-60478.json` pins the fresh Concert/deletion copies and source60478 helpers while retaining the unchanged Gateway and five explicitly historical foundation binaries. The old b827 descriptor remains preserved. Input preparation grants no runtime qualification or execution allocation.

Next: a fresh finite copied-unit offer for the five corrected route cases; separately rebase the proposed native resource accounting onto the complete retained WHOLE/outside-union carry before seeking a new native offer. The physical positive remains dependent on a genuinely reviewed published place. BE39 moderation is a separate private source packet and inherits none of this build evidence. Persistent original goal metadata remains historically BLOCKED and is not reset or falsely completed; required validation/publication remains unfinished.


## Corrected copied route tests — 2026-11-05 00:14 UTC

Status: PASS_COMPONENT_5_ONLY. Allocation `PARTY-COPIED-ROUTE5-UNIT01-20261105T0004Z` ran only the existing immutable `party_runtime_routes` copy built in round5. Binary SHA-256 remains `11e7230b292cb653d93352b65e6d9c7e82ff04cb05b1b66e3668d9377dac9602`, source `60478c66d1e3feedf35ad49ddc483f254cc49d4d`, unchanged backend tree from `0c1219093ce7e38ff4bd3dd7491f261ac7ba4a55`. Checkout HEAD was clean at `207acd1ec07084fff3886b27a264c45c864b4361` before and after execution. The separate private BE39 packet was not applied or exercised. No dependency, application source, migration, fixture or public contract changed.

Command: `/private/tmp/mcr-party-native-settlement-01/production/round5/party_runtime_routes --test-threads=1`; exit 0; 5 passed, 0 failed, 0 ignored, 0 filtered out. Exact cases:

- `frozen_update_and_transfer_bodies_bind_exact_methods_and_closed_fields`
- `malformed_and_foreign_routes_never_select_party`
- `party_native_routes_bind_all_ten_actual_operation_method_target_tuples`
- `private_party_collections_close_queries_and_native_operation_binding`
- `status_selects_only_eight_party_mutations_and_binds_raw_query`

These component results cover the corrected ten-operation method/target selection, closed update/transfer bodies, private collection query/native binding and the eight-mutation status selector. They supersede the earlier corrected-route NOT_RUN statement only. They do not establish actual database authorization, native settlement, public HTTP behavior, restart durability, physical eligibility or whole BE16/IN02 acceptance. The earlier twelve component passes were not rerun. Deletion and all thirteen BE39 tests remain NOT_RUN in this round.

The actual predecessor was Gift parse SOURCE01 FULL epoch `1793837651.853777`, receipt SHA-256 `6de77faff312dcfe0b83259ae90e09617b071738745a5a59883f2b505be961fa`. Its RSS-limit failure did not prevent resource succession or receive technical PASS credit. Party ACCEPT epoch `1793837686.004983`; fixed WORK stop `1793837746.004983`, FULL stop `1793837806.004983`. Actual Party FULL was `1793837712.758538`, after the controller exited and explicit owned-PID absence checks. The literal release went directly to allocator21d before this documentation, freeing the SourceC successor without a review, acknowledgment or documentation gate.

Both independent guards were live with complete initial process/RSS/WHOLE/NEW/free-space samples before EXE start `1793837695.7805882`: outer initial `1793837695.136677`, inner initial `1793837695.6884632`. The outer guard collected five samples, the inner four. Peak process count 5/6; observed RSS maximum 120,143,872/536,870,912 bytes; charged NEW maximum 237,568/16,777,216 bytes, including the entire preexisting control directory plus positive Git/checkout growth. Private TMP and all proofs stayed in the owned directory. Charged WHOLE retained the conservative nonshrinking 36 GiB prior carry under the 40 GiB ceiling, combined with the saved qualified full-target maximum and fresh outside/history/full Python/full Rust inode union. The 36 GiB is conservative retained accounting, not a newly measured target size. No target path was opened, statted or scanned. Fresh OTHER was conservatively 5 GiB; initial free 16,341,966,848 bytes met the 11 GiB start and 9 GiB after all remaining NEW plus OTHER requirements. Lowest sampled free was 16,338,698,240 bytes, above the 8 GiB live floor. No guard breach, cleanup failure or remaining owned child/thread/process was reported. At FULL the workflow and remaining NEW reservations were zero. There was no Cargo, new binary copy, PostgreSQL, service, listener or browser execution.

Private evidence directory: `/private/tmp/mcr-party-native-settlement-01/production/route5-unit/`. Preserved original controls were superseded before ACCEPT by the same-scope v2 dual-guard controls; actual proposal SHA-256 `431004295a564a3429c63c9a5254f613ca10351ad9d4da83f951da725a34b9e7`, runner `9eba9901338e3e4cce09f13cfba1430e312b9c956725f1b713625babf43f6e75`, outer guardian `92b4ed0f76a43f8907d9926d665bb0c18a964e7a67977049ef63e5876f64775b`, shared guard `ce6a9231834bfb4020a634cd6d82c1f544df5de82c46773f1839209543bf0005`, footprint `175b7ef40da9a4793d3576639c8fefca591ef3311fceb531ad05ea44196c4974`. `tests.json` SHA-256 `6d5566ffa48919220f555970d1c9e9d5fed9a2b135d7dcd5530b7fd212ba11b1`; `guardian.json` `2f3cf81d34d2e8009951d8f80c1eae99f612f46569f41cdab779dc5b8032189d`; `FULL.json` `e9c34ca48cc8dc959251ff62be353d89b99f6ce492cac7ace4885283644c2e6c`. The lease, ready records, heartbeat, raw stdout/stderr and original frozen controls remain preserved alongside them.

The next technical gate is a separately allocated actual native/service/restart round with refreshed complete retained-WHOLE accounting and current resource admission. A successful physical Party create/apply/approval journey still requires genuinely reviewed published place input. The original persistent goal remains historically BLOCKED and is not falsely completed by these five component passes. Publication follows the existing named delivery owner; no king/SL validation or approval stage is introduced.
