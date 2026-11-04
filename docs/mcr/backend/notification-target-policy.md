# NB03-L notification target local policy

Status: **CONTRACT_READY** local component handoff. Clean source `fa2fbdef26fd087c0810a5c318ea221e9522b202` compiled successfully; all six actual PostgreSQL component cases and seven committed-row writer probes passed. This is partial NB03, not inbox, public HTTP or global notification acceptance. Root integration/review fixes remain assigned.

## Assignment and boundary

Base: `cde817b2ade8caabcf8c3a96508121053997873e`. Isolated branch `codex/mcr-be-NB03-local-target-policy`, checkout `/Users/lhs/Desktop/mcr-worktrees/be-notification-target-policy`. Former BE27 writer stopped at clean `b0418437`; that checkout stays parked and untouched. Root retains module export, runtime/provider composition, wire contracts, native child recovery, and retention through delivery. No existing source, manifest, lockfile, proto, migration, frontend, other lane, or `docs/source/` file is changed.

Exactly eleven new writable paths:

- `backend/services/concert-service/src/notification_target_policy/{mod,local,party,gift,social,operations}.rs`.
- `backend/services/concert-service/tests/notification_target_policy.rs`.
- `backend/services/concert-service/tests/notification_target_policy_support/{mod,fixture}.rs`.
- `backend/scripts/mcr/notification_target_policy_smoke.py`.
- `docs/mcr/backend/notification-target-policy.md`.

At assignment, actual goal inspection reported an unfinished incompatible historical BE27 BLOCKED objective, which was reported honestly and left unchanged. After Root published BE27, a fresh remote/readback verification confirmed `master=6724e5932151918cf3fed334cdbeafdcc91fc50f`, all thirteen mapped delivery commits in its ancestry, all seventeen BE27 postimages byte-identical to `b0418437`, and fresh combined Intro qualification PASS. Only then was the BE27 goal legitimately completed and an explicit unbudgeted NB03-L goal created. That NB03-L goal remains active through committed handoff and concrete review fixes; no incompatible goal was silently reset or replaced.

## Frozen private API and caller obligations

```rust
pub async fn observe_local_target(
    connection: &mut sqlx::PgConnection,
    context: concert_application::notification::Context,
    candidate: concert_application::notification::Candidate,
    now: chrono::DateTime<chrono::Utc>,
) -> concert_domain::notification::Result<concert_domain::notification::Visibility>
```

The component takes no pool, starts no transaction, commits/releases nothing and returns no target/source details. It rejects an autocommit/untracked connection using SQLx's tracked transaction depth. `Context::check(now)` and actual wall clock preserve the original <=5-second context; a whole-call timeout bounds work to its original remaining lifetime, existing statement/lock limits are only narrowed, and DB-clock/wall-clock expiry is rechecked before any successful observation. A timeout or SQL failure is Unavailable; caller must roll back an aborted transaction rather than treating a partial observation as authority.

Existing event, exact target, current feature, admission, participant and normalized pair rows are held FOR SHARE on the caller's connection. Nonlocking application/reservation parent lookup is only a hint: root lock precedes a second locked child read and full aggregate/index identity validation. Domain Party/Giveaway validators are reused read-only. No insert-on-read reserves missing policy, no RLS/authority GUC is installed, no fake grant/role/provider is created, and no public DTO is produced. Candidate values remain unchanged.

**Root must additionally bind the exact private RootReadView and original Candidate identity, canonical Auth family/current Friend/source-block authorization, all actual target owner dependencies, and native registered transaction identity. Retain this actual transaction and canonical exclusions through child completion and Root EOF.** A local Visible observation cannot implement RootTargetSnapshotOwner alone, grant a public projection, authorize a mutation/reveal/pickup, or bypass operation/role checks. Root must discard expired observations and preserve the same original deadline; component checks do not guarantee a later EOF on their own.

## Owning policy decisions

| Target | Current local policy | State/time behavior |
| --- | --- | --- |
| PARTY | Exact event/root, validated aggregate/index, enabled feature; actor/owner/source current local admission/participant/pair | Owner's public status projection includes OPEN/CLOSED/CANCELLED/COMPLETED. Active party membership/physical meeting audience is separate. |
| APPLICATION | Exact event/child/parent plus matching aggregate record; caller is current party owner or applicant; actor/owner/applicant/source local constraints | All four valid application states remain private status records under the owning read policy. No note/profile returned. |
| GIVEAWAY | Exact event/root and validated aggregate/index; current feature and actor/owner/source local constraints | DRAFT is Hidden. OPEN/PAUSED/CLOSED/CANCELLED retain public status. Stock is not a read grant. |
| RESERVATION | Exact event/root/child/aggregate identity, current feature; caller is giveaway owner or recipient; all relevant local subjects | Valid active and terminal/expired status records remain readable under the owner's metadata policy. No pickup code, meeting, location or lazy mutation. Expiry must have valid stored shape and does not renew any grant. |
| SOCIAL_REQUEST | Exact event/request; caller is requester or recipient; valid revisions/expiry/state and 1–2 locked binding rows; current feature and both subjects/source constraints | Existing getConcertSocialRequest explicitly projects expired/revoked/rejected/cancelled metadata. Local Visible permits only that same metadata target; it grants no reciprocal handle access or consent. |
| NOTICE | Exact event/notice, valid revision and nonfuture confirmation, current actor/source local constraints | Only PUBLISHED is Visible; locked DRAFT/WITHDRAWN is Hidden. No title/text/source URI selected. |
| REPORT | **Unavailable for every caller** | Singular admin target needs an actual operation-bound EventRoleFence. Reporter own-status list is not singular report authorization. Root confirmed no participant Report detail route. This required product policy remains open, not cut. |
| POST | **Unavailable** | Remote Post owner must provide its actual policy/fence. Never Hidden/Stale or positive local substitution. |
| No target | **Unavailable** | Candidate has no kind/operation to establish an original no-target owning policy. No blanket Visible fallback. |

All supported local reads reject DRAFT/ARCHIVED events and explicit SUSPENDED admissions as Hidden with the existing negative row held. OPEN/LIVE/READ_ONLY reads remain eligible. Existing local block flags or disabled feature rows yield Hidden; this never replaces canonical Auth/Friend authorization. Missing event/target/admission/participant/pair/settings/binding rows are Unavailable: a missing row cannot be held by FOR SHARE. Corrupt/mismatched revisions, aggregate/index disagreement, SQL/transport failure and elapsed work are Unavailable, never converted to Hidden/Stale. No owning policy currently permits Stale, so none is returned.

No separate content-hide boolean/tombstone is invented: current target handlers have no such positive local projection. Moderation suspension is checked through actual local admission state; operation-bound content moderation owner policy remains Root/actual target-owner composition, never inferred from saga history, absence of a moderation record, or this local result.

## Validation design and honest limits

Integration test uses `#[path]` to include only the new module; no lib export is added. Six exact ignored native cases cover all six supported policy branches, preserved Candidate identity and caller native transaction, wrong event/actor, missing target/source/pair/participant, unavailable Post/Report/None, explicit locked denial states, draft/withdrawn content, expired/terminal metadata, original context expiry/overlong lifetime, autocommit rejection, actual SQL lock timeout and a terminated owned DB connection.

Seven writer probes are designed to verify actual `pg_blocking_pids` against the observer PID while the component has already returned, then verify each writer resumes only after the caller rolls back: existing committed event, party, giveaway, application, reservation, admission and participant rows. Actual write SQL executes in a separate transaction and rolls back. A timer alone is not blocking proof.

Fixture data is synthetic persistence metadata, never canonical authority or successful owner commands. The fixture commits only unprotected core/community rows under the existing schema policy. **Protected settings, notice, social request/binding and pair rows stay inside the test caller transaction and roll back; no authority GUC, RLS/trigger bypass or schema repair is used.** Their cases prove SQL branch/negative policy only. Their actual committed-owner revocation interleavings, including pair fences, remain NOT_RUN. The qualifier requires zero protected rows committed and zero prepared transactions, checks actual central migration receipt checksums, rechecks frozen source/binary hashes, and finally destroys its marker/inode-owned socket-only PG16 cluster. No actual owner service, inbox callback, child lease or Root EOF is impersonated.

Source-only checks executed (all exit 0):

- `rustfmt +1.88.0 --edition 2021 --check` on the nine new Rust files: parsing/formatting only.
- Python `compile()` on `backend/scripts/mcr/notification_target_policy_smoke.py`: syntax only, no PG import/execution.
- `python3 scripts/mcr/check-plan.py`: PASS, planning provenance only; original archives untouched.

Initial proposed build (historical; final allocated command below also pins contracts-build optimization):

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 test -p concert-service --test notification_target_policy --no-run --locked --offline -j2 --message-format=json
```

Only that one test artifact is requested, no extra binary/library test inventory. Pin clean committed source/full input inventory, Cargo JSON, actual manifest path and baked fixture path, and freeze/hash executable outside the shared target. Unconditionally verify zero target handles and RELEASE before running the copied executable/private PG, with no target reread. A later allocated component run will use:

```sh
python3 backend/scripts/mcr/notification_target_policy_smoke.py --checkpoint /absolute/private/checkpoint.json --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report /absolute/private/new-report.json
```

Compilation, all six SQL cases, all seven writer probes, central migration receipts and cleanup: **PASS**, exact evidence below. Full inbox/public HTTP/global notification acceptance: **NOT_RUN**, requires Root's actual composition and separate qualification.

## Provenance and dependencies

All new policy logic is newly authored from read-only owning code at the exact base below. No old UI/source pack or service implementation is copied into runtime. Test fixture ownership validation and Python qualifier structure are adapted from the named backend helpers; domain validators/SQLx/Tokio/Chrono/UUID/Serde use already retained dependencies without manifest/lock changes. Original specification U10, O04, BE33, BE36 and IN03 define authorized target links, stale handling and reporter/moderator privacy; they do not authorize a synthetic positive fallback.

Source revision for every following entry: `cde817b2ade8caabcf8c3a96508121053997873e`.

| Source path | SHA-256 | Use |
| --- | --- | --- |
| `backend/crates/concert-application/src/notification/mod.rs` | `07b0e37d6a503cda623660bab63143c75bb5cf48ab811f1d49ecd417276a1b98` | Context::check and Candidate; unchanged exact <=5 second original context |
| `backend/crates/concert-domain/src/notification/mod.rs` | `24df36fa3380af8e47a48ca420bf789757eb2b39fd56013d49f902e72e06c0e7` | Visibility/NotificationError; no new states or public values |
| `backend/services/notification-service/src/concert/authority/inbox.rs` | `2b13e811c3fb6ff75daed9731bb03b5de83ee11dd063736392f8afb0437053a1` | RootTargetSnapshotOwner contract; component is not its implementation |
| `backend/services/concert-service/src/community/mod.rs` | `245eeeba24c9840f29b56f59e2f6afee0b5ccb7785ad86afd88639b48892014b` | PgCommunity::party and root load/validation |
| `backend/services/concert-service/src/community/catalog.rs` | `055320b9379ab33c67d9f540362eeea793ef8ba9a4e65e1d4ddda37b5d2dcd84` | applications owner/applicant audience and public party metadata |
| `backend/services/concert-service/src/party_live/wire.rs` | `02f12929bd65d3bf812ed0fadb8f188caf687b243498143d322360661c09493d` | application_from_receipt, public_party, load_application; exact aggregate/child identity |
| `backend/services/concert-service/src/community/giveaway.rs` | `43149c2e219aacbaf6f4c7e6afc04a11136eaecb27847aba1844a16af3d37433` | detail/reservation current owner/recipient policy; retained terminal status |
| `backend/services/concert-service/src/gift_live/resources.rs` | `48aa4f5b31cb4219fc1493588a932458ea80247e980347c11ece4ebe43238b4b` | current feature and event phase fences |
| `backend/services/concert-service/src/social_live/producer.rs` | `374b491eb8d96332d766247ab5709469afebcf9393c072badba2b11313b9c925` | detail/request_value; terminal/expired metadata remains readable, never reveal |
| `backend/services/concert-service/src/adapters/out/repository/event_operations.rs` | `3f84369f75e180bae61ed0a3a87d25f92ae13df0d83e2e51f7a6b874099f93d4` | public_notice only PUBLISHED and current authority |
| `backend/services/concert-service/src/adapters/out/repository/report.rs` | `13175731ac5b92b9fd3a8767cb38cf0dfe0fed50b3c9cfd6153b134d988a5ebb` | own status list versus privileged detail; no reporter singular grant |
| `backend/services/concert-service/tests/gift_live_support/owned.rs` | `cb8e8ea1045f4530d2e705c7f21401f48c7de3b4bfd0b7a03e51ff2fc7a37a13` | fixture.rs marker/inode/socket ownership validation adapted; no policy provider copied |
| `backend/scripts/mcr/migration_smoke.py` | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` | smoke runner uses unchanged owned Cluster, execute, receipt checks |
| `backend/services/concert-service/tests/age_runtime_support/qualify.py` | `5d960e413cb54eff1ebaaafdacf0e64f231c70022098221b5fe2798226b23c68` | smoke runner pattern adapted: clean source/binary pins, finally cleanup |
| `frontend/src/features/inbox/model/targets.ts` | `a29acab3f1b6e2681c4ee678b4221f147d8949f17a0bb404990f86286c4e4a3c` | read-only compatibility evidence: REPORT has no participant detail route |


## Targeted source review

A read-only peer review of `edfe6cd2` found one concrete fixture bug: notice confirmation used `clock_timestamp()` while the actual immediate 0021 CHECK requires `confirmed_at <= statement_timestamp()`. The fixture now uses the owning constraint's `statement_timestamp()`. This repair changes no policy, authority or scope. Review performed no builds, PG, target or network activity and reported no other actionable findings in the frozen local-component scope. Compilation/runtime remain NOT_RUN until the named offer.


## First allocated compilation and exact repair

Under `NB03-L-TARGET-POLICY-BACKEND-202611040854Z`, clean `f59d56c96a4f5ffe44528bb58de8308ee71dcfc0` failed compilation (exit 101, 40.37 seconds): the retained SQLx feature set exposes no `sqlx::migrate!` macro. Only the owned fixture was affected. The repair uses the existing `migration_runtime::prepare_database` with Startup mode and the actual `env!("CARGO_MANIFEST_DIR")/migrations` path; no manifest/lock/feature change or schema repair. The fixture still migrates the central owner directory and the qualifier still requires exact ledger checksums. Failed Cargo JSON/stderr, full source inventory and 919.63 MiB source archive are preserved in private `/tmp/mcr-nb03-local-f59d56c9`. Fresh lsof found zero target handles before unconditional release; no PG process or copied test ran. Retry requires a fresh named offer, preserving the same original time/byte baseline and failure evidence.


## Final executed component evidence

Tested source `fa2fbdef26fd087c0810a5c318ea221e9522b202`, with final documentation-only commit following it. All eleven assigned NEW paths are preserved; no existing source/export/migration/manifest/lock/contract/frontend file changed. Runtime source inventory is rechecked after final documentation and remains byte-identical to the tested source.

Allocation history stays explicit:

1. `NB03-L-TARGET-POLICY-BACKEND-202611040854Z`: compile failed 101 on missing SQLx macro. Full source archive, Cargo JSON/stderr, source inventory and zero-handle release preserved.
2. `NB03-L-TARGET-POLICY-REPAIR-BACKEND-202611040858Z`: fresh preflight found only 14.10 seconds of original target time left and failed its required >=20-second assertion. My orchestration incorrectly continued to send ACCEPT/start the old runner; its unchanged original deadline guard immediately terminated Cargo (-15, 0.002 seconds), without an actual new compilation. This was an orchestration error. Fresh zero-handle release followed; no PG or copied test started. The failed preflight/abort were reported rather than reclassified as a successful repair. No original time/byte baseline was silently renewed.
3. Distinct explicit extension `NB03-L-TARGET-POLICY-EXTENSION-BACKEND-202611040900Z`: <=60 seconds target build/freeze, <=180 seconds copied including >=60 seconds cleanup reserve, <=240 seconds additional total. Original byte baseline and whole hard 4 GiB cap were preserved, including every prior archive/log/source delta/target growth. Before ACCEPT, the owned workflow guard was changed to this explicitly authorized clock, compiled for syntax and checked to fail **before Popen** on failed preflight/remaining time. Guard SHA-256 `c6373ac9d4f22d10232f1584fb23951dd185d4cda9d86b57f606a0a8083ef311`; actual ACCEPT epoch `1793782836.0470378`, target deadline `1793782896.0470378`, whole deadline `1793783076.0470378`. Fresh free space 28.784 GiB, handles 13063/368640, all carried evidence 965352702 bytes. The preflight passed before dependent work proceeded.

Final actual build, exit 0 in 0.92 seconds, from `backend/`:

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 --config profile.dev.package.contracts-build.opt-level=2 --config profile.test.package.contracts-build.opt-level=2 test -p concert-service --test notification_target_policy --no-run --locked --offline -j2 --message-format=json
```

Exactly one test executable was newly compiled. Actual Cargo manifest/target source paths and the absolute baked fixture manifest literal were checked. The full live migration SHA-384 inventory was pinned before release. Frozen executable, original full archive plus exact source delta, full input inventory and Cargo JSON stayed outside the shared target. Fresh `lsof -nP -F pn` found zero target handles; target RELEASE at epoch `1793782861.767679` preceded all copied tests/PG. No target path was read afterward. Cumulative target growth was 14397440 bytes; all outside evidence 979858728 bytes, within the 1 GiB source/executable subcap. Final free space was 28.757 GiB. No broad/narrow clean, install, dependency feature or target change was performed.

Actual copied run, exit 0 in 2.55 seconds:

```sh
python3 backend/scripts/mcr/notification_target_policy_smoke.py --checkpoint /tmp/mcr-nb03-local-f59d56c9/checkpoint.json --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report /tmp/mcr-nb03-local-f59d56c9/native.json
```

One private marker/inode-owned socket-only PostgreSQL 16.14 (Homebrew), one database. The compiled fixture's baked manifest directory was used by the retained migration runtime to read central migrations; actual 37 SQLx ledger receipts matched the frozen live SHA-384 inventory exactly. These are runtime-source migrations, not statically embedded SQL, and the executable path alone was not treated as migration-byte proof. No Auth/Friend/service/RPC/HTTP/browser process was started or impersonated.

| Exact executed case | Result |
| --- | --- |
| `local_targets_use_actual_current_owner_rows` | PASS, actual same caller backend/XID retained and candidate unchanged |
| `missing_foreign_and_unsupported_targets_fail_unavailable` | PASS, missing/foreign rows, source/pair dependency, Post/Report/None all fail closed |
| `explicit_local_restrictions_hide_without_dependency_downgrade` | PASS, private audience, suspended/local block, feature stop, withdrawn/draft and closed event |
| `terminal_metadata_never_creates_a_private_action_grant` | PASS, explicit owning expired/revoked metadata policy only; no reveal/pickup output |
| `retained_row_exclusions_block_actual_writers` | PASS, all seven actual writers blocked on observer PID after component return, resumed after caller rollback |
| `original_deadline_and_dependency_failures_remain_errors` | PASS, untracked TX, overlong/expired context, real lock wait bounded by 150 ms original deadline, actual killed DB dependency |

Writer exclusion proof covers committed **event, Party root, Giveaway root, Application child, Reservation child, admission and participant** rows. It does not claim committed notice/social/settings/pair revocation interleavings. All protected fixtures rolled back, with the actual final combined row count **0**. Prepared transactions: **0**. Entire disposable cluster cleanup: **PASS**, owned root absent. Whole workflow RELEASE was sent immediately; no build or PG resource remains held.

Private immutable evidence directory: `/tmp/mcr-nb03-local-f59d56c9`. Large binaries/archives and private fixture data are not committed.

| Evidence | SHA-256 |
| --- | --- |
| Full source archive at `f59d56c96a4f5ffe44528bb58de8308ee71dcfc0` | `35487e1b586c797ec5143f81cd59a601f96de05a4ef9eadca0c7d8589a35806d` |
| `repair-fa2.patch`, exact binary Git delta to final tested source | `1691c0863c3ab607a041ac08294425191d02ee76b54080e0fcc711db0602d45f` |
| `source-manifest-fa2.json` | `9d9ff1c631f70ef70db733223d65596cba92b99c519b9c70131b881c7d263c07` |
| `cargo3.jsonl` | `70b07f96312b1058f18a12b74117e92430c658dfbed323d93656bdf8e9709017` |
| Copied `notification_target_policy`, 14100560 bytes | `292d355142d4cc92c00e338856f6405f8750846e572b769fa679c837a6d02851` |
| `checkpoint.json` | `8729910916cd104cbc6f871b9885536e2b1028271cfb916a80cb4d2383d29a6d` |
| `native.json` | `e0a29a6d620fb30bf6effffe5e37ec4afa2f07eb6cdb6c99cca1621d0c2c0b8a` |
| Extension preflight | `3e2c55ec771a56e52d80d5269775d98d7038da23244d94044af3bc0bebe5e391` |
| `target-release3.json` | `3a4dc20caeb5af2d6dbc9a7acd8926dad145a579cb958b316cc30444e008a721` |

## Handoff ancestry and remaining Root work

Own commits in order from the exact published base: `edfe6cd2b2ae39a35228410e6f9c978dc40a8301` implementation/harness/provenance, `f59d56c96a4f5ffe44528bb58de8308ee71dcfc0` reviewed notice timestamp fixture fix, `fa2fbdef26fd087c0810a5c318ea221e9522b202` retained migration-runtime fix, followed by this final documentation-only commit. No prerequisite or other-lane commit is bundled.

Root may serially integrate this validated component without a main/SL technical review gate. Root alone owns export/composition and must retain actual operation-bound target/source/canonical/native/RootReadView/EOF exclusions. Report needs a future typed operation/role-capability policy; remote Post needs its actual owner; None needs an actual owning policy. Protected owner commit/revocation interleavings and full actual inbox/public HTTP/global notification acceptance remain **NOT_RUN**. This component does not turn on the absent RootTargetSnapshotOwner, manufacture a positive provider, or close whole BE33/NB03 acceptance.
