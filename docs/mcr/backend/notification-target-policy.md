# NB03-L notification target local policy

Status: **IN_PROGRESS**, source prepared; Cargo compilation and PostgreSQL execution **NOT_RUN** pending a named allocation from 21d and explicit ACCEPT. This is a partial NB03 component, not inbox, public HTTP or global notification acceptance.

## Assignment and boundary

Base: `cde817b2ade8caabcf8c3a96508121053997873e`. Isolated branch `codex/mcr-be-NB03-local-target-policy`, checkout `/Users/lhs/Desktop/mcr-worktrees/be-notification-target-policy`. Former BE27 writer stopped at clean `b0418437`; that checkout stays parked and untouched. Root retains module export, runtime/provider composition, wire contracts, native child recovery, and retention through delivery. No existing source, manifest, lockfile, proto, migration, frontend, other lane, or `docs/source/` file is changed.

Exactly eleven new writable paths:

- `backend/services/concert-service/src/notification_target_policy/{mod,local,party,gift,social,operations}.rs`.
- `backend/services/concert-service/tests/notification_target_policy.rs`.
- `backend/services/concert-service/tests/notification_target_policy_support/{mod,fixture}.rs`.
- `backend/scripts/mcr/notification_target_policy_smoke.py`.
- `docs/mcr/backend/notification-target-policy.md`.

Actual persistent goal inspection still reports the unfinished historical BE27 goal as BLOCKED, with an incompatible old objective. It was reported to Root, neither replaced nor falsely completed. NB03-L remains assigned through implementation, meaningful validation, documentation, clean committed handoff and concrete review fixes; its completion must not be inferred from that stale metadata.

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

Pending named allocation, exact proposed build:

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo +1.88.0 test -p concert-service --test notification_target_policy --no-run --locked --offline -j2 --message-format=json
```

Only that one test artifact is requested, no extra binary/library test inventory. Pin clean committed source/full input inventory, Cargo JSON, actual manifest path and baked fixture path, and freeze/hash executable outside the shared target. Unconditionally verify zero target handles and RELEASE before running the copied executable/private PG, with no target reread. A later allocated component run will use:

```sh
python3 backend/scripts/mcr/notification_target_policy_smoke.py --checkpoint /absolute/private/checkpoint.json --pg-bin-dir /opt/homebrew/opt/postgresql@16/bin --report /absolute/private/new-report.json
```

Compilation, all six SQL cases, all seven writer probes, central migration receipts and cleanup: **NOT_RUN** until actual execution. Full inbox/public HTTP/global notification acceptance: **NOT_RUN**, requires Root's actual composition and separate qualification. Resource sizes/deadlines and target identity will follow 21d's actual named offer, not a self-created allocation.

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
