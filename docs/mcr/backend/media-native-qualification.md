# BE24 final producer native qualification

2026-11-04 KST — **BE24-NATIVE-FINAL scoped qualification PASS**.
The frozen final producer candidate passed its whole nonignored native suite
(17 passed,0 failed), then its existing restart-only test passed after a real
owned PostgreSQL16 stop/start. Schema, exact source-query timeout/expiry guards
and owned cleanup passed. Full BE24 remains **IN_PROGRESS**: production authority
is not composed and HTTP/upload journeys, root composition and integrated gates
remain unqualified. Earlier failed-candidate evidence is preserved unchanged.

## Scope, source and ownership

- Exact base and native execution HEAD:
  `0d0846c1f4e6ff30e02f231bd922dda15089a627`.
- Isolated qualification branch: `codex/mcr-qa-media-native-final`.
- Checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-media-native-final`.
- Only changed tracked path: `docs/mcr/backend/media-native-qualification.md`.
  The documentation handoff commit directly follows the exact base; its final
  SHA is recorded in the Main handoff rather than relabeling the execution HEAD.
- Producer checkout remains owned separately at
  `/Users/lhs/Desktop/mcr-worktrees/be-media-upload`, also at the exact candidate.
  Source ownership was not transferred.

The existing `backend/scripts/mcr/media_upload_smoke.py`, its
`migration_smoke.py` helper, migrations, tests, fixtures, contracts, source,
original artifacts and prior reports were read-only. No Cargo, build, target,
service composition, production/default-root or shared-file writes occurred.
No new test implementation or dependency was added. Main integrates producer
source separately and alone publishes this documentation handoff.

The required specification, project assessment, execution plan and existing
[BE24 source/limits document](media-upload.md) informed this qualification.
This record adds execution evidence without amending their product acceptance.

## Artifact admission and stability

Original build receipt:
`/Users/lhs/Desktop/mcr-worktrees/be-media-upload/backend/services/concert-service/tests/media_support/artifact-receipt-final.json`.
SHA256 `83d6a7c35cb6ea7241493782cf400143e4158c1180d390c295c9ad00ca4ffb1a`.
It records source `0d0846c1f4e6ff30e02f231bd922dda15089a627`, exit0 from
Cargo+1.88.0 locked/offline test compilation, and eight exact selected source
pins. That compilation was already complete; no compilation was repeated here.

Original private executable:
`/Users/lhs/Desktop/mcr-worktrees/be-media-upload/backend/services/concert-service/tests/media_support/artifacts/media-tests-fixture-0d0846c`.
SHA256 `a071365d21dc5d78af9bbd2324a88221ac1ffdff84b6e25ec8777946b1ff4630`;
size17,572,544 bytes; mode0500. The unchanged harness admitted its exact receipt
path, executable hash, selected source hashes and source ancestry before creating
any cluster. Both checkout HEADs equaled the candidate; QA was clean and the
producer had only the declared untracked final receipt, with no tracked changes.

An independent before/after snapshot rehashed both original files, all1,325
tracked files under `backend/` and `contracts/` in both checkouts, and the five
PostgreSQL executables. No differences occurred. This snapshot establishes
stability and matching checkout inputs; it does not expand the build receipt's
eight selected source hashes into a claim of exhaustive compiler provenance.

| Selected build and execution source | SHA256 |
| --- | --- |
| `backend/Cargo.lock` | `208bd6ae3e1657ee5cc6128241bf7fbc39c122efc467efa8deba30b98f2a1dc4` |
| `backend/crates/concert-application/src/media/mod.rs` | `cedd1fbd677c28bb4cc2ff64638935238cf0fc8bcf6967e5df9387cdd9333618` |
| `backend/services/concert-service/Cargo.toml` | `4d5267c08a16fc58db65372526368388eca81949c275beb322c6c99c49eaf14b` |
| `backend/services/concert-service/src/adapters/out/media/body.rs` | `3c50d22df399c712aed08ed9ad5dcca4ecc74840e9bbb0aa07ef1aa566cab8df` |
| `backend/services/concert-service/src/adapters/out/media/codec.rs` | `6888a021fcee24ea8a4a8e27121b199496a95b9618f4f69f56a7bda90e4de052` |
| `backend/services/concert-service/src/adapters/out/media/mod.rs` | `e579de7fe0496c8fe3e54ab0fb29af957d8a9e94306055d3bb0ba82c9e3717d0` |
| `backend/services/concert-service/tests/media.rs` | `579a106e949bb217dbf001648775e94ac91b3a572bde994d374df00afc77f2d1` |
| `backend/services/concert-service/tests/media_support/mod.rs` | `9056f421f97966fcec6bca87390cd510e784018132bde19de36feb0e06fdf14f` |

## Native command and resource boundary

PostgreSQL tools were verified executable, version16.14 Homebrew, resolving under
`/opt/homebrew/Cellar/postgresql@16/16.14/bin/` (`postgres`, `initdb`, `pg_ctl`,
`psql`, `createdb`). Their SHA256 values are retained in the private before
snapshot and remained unchanged after execution.

The unchanged ownership helper creates a new mode0700 temporary root, random
credentials and private marker, checks UID/inode/token and postmaster identity,
and refuses existing data roots. It enables fsync, disables TCP listeners, uses
only a namespaced private Unix socket (port5432 is its socket filename), and
does not inherit HOME/PG/database/provider credentials. The Rust fixture also
checks root/marker privacy, inode, owner token, exact `mcr_concert_be_media`
database/socket and actual data directory before connecting. No existing or
foreign database was used. Stop/remove are restricted to that owned root.

From the clean candidate checkout:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/media_upload_smoke.py \
  --repository-test /Users/lhs/Desktop/mcr-worktrees/be-media-upload/backend/services/concert-service/tests/media_support/artifacts/media-tests-fixture-0d0846c \
  --artifact-receipt /Users/lhs/Desktop/mcr-worktrees/be-media-upload/backend/services/concert-service/tests/media_support/artifact-receipt-final.json \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /private/tmp/mcr-be24-native-final-0d0846c-20261104/native-report.json
```

Actual exit0. Before/after admission boundaries:
`2026-11-03T18:32:35.052800+00:00` through
`2026-11-03T18:33:08.267501+00:00` (2026-11-04 KST); these bound the run,
not precise subprocess start/end timestamps. Observed server version:
`16.14 (Homebrew)`.

## Observed results

| Check | Actual result |
| --- | --- |
| Receipt/source admission | PASS; exact candidate and artifact |
| Five source migrations, each `psql -X --no-password -v ON_ERROR_STOP=1 -1 -f` | All exit0;21 media columns verified |
| Timeout clamp: unlimited, shorter caller, max integer | All exit0;1500/5000,100/200,1500/5000 ms |
| Expired PENDING/UPLOADED/READY mutation SQL | All exit0; zero rows changed |
| Expired private-byte read and intent projection | Exit0; no bytes/intent returned |
| Whole nonignored producer/codec set (`--test-threads=1`) | Exit0;17 passed,0 failed,1 restart-only ignored;0 filtered |
| Actual owned PostgreSQL stop then start | Completed between the two test processes |
| `--ignored --exact durable_restart_verify --test-threads=1` | Exit0;1 passed,0 failed,0 ignored;17 filtered |
| Restart persistence | PASS |
| Owned cleanup | PASS; helper checked root absent after stop/removal |
| Real production authority | NOT_COMPOSED |
| HTTP E2E and SQLx runtime migration ledger | NOT_RUN |

All14 reported command entries exited0. The first suite includes actual database
publication/replay/concurrent finalize, private staging and reads, purpose-bound
transactional linkage, stale/revoked/unavailable policy, protected-read revocation,
orphan/owner erasure, quota/state guards, shorter SQL deadline, MIME/digest/foreign
ID rejection, size/dimension/output limits, metadata/animation handling and
streaming poison/overrun checks. The delayed-authority read/link/finalize and
expiry-during-sanitization regressions all passed on this compiled final candidate.

`durable_restart_seed` committed READY media before stop. After start, the
separately invoked `durable_restart_verify` read its sanitized4×3 image, checked
absence of the private metadata marker, and verified exactly three SUCCEEDED
command receipts before fixture cleanup. The harness verifies startup against
the actual owned data directory and rejects an existing postmaster. Its report
does not record before/after PID or start timestamps; the restart conclusion is
the executed stop/start path plus the durable verification, not invented PID data.

No producer failure surfaced in this run, so no source correction or rerun was
needed. Earlier SQL-timeout and restrictive-FK fixture cleanup failures in
`media-upload.md` and their original JSON evidence remain historical failures.
The current successful result does not rewrite those reports or relax retention.

## Evidence pins

Private evidence directory (mode0700):
`/private/tmp/mcr-be24-native-final-0d0846c-20261104`.
All three JSON evidence files are0600 and contain no live credentials or gate
configuration. The harness checks exact generated password/URL/token values
against test diagnostics before retaining redacted output. The owned cluster was
removed; no live account, service or credential inputs were provisioned.

| Evidence file | SHA256 |
| --- | --- |
| `native-report.json` | `0521fad7e3b1b1d14626f3f874fa3b4f7c4441f21a8bbae654784d122b0b4de9` |
| `binding-before.json` | `d99ae22c07661bbd5f9149f2b4ebd6a1de9697e50ba176a8781c501338f80888` |
| `binding-after.json` | `5ddb37fce15eca558764854145ebc1d243f93db93bd848434ec6acbd32c459bd` |

| Execution input | SHA256 |
| --- | --- |
| `backend/scripts/mcr/media_upload_smoke.py` | `3e2b2b66cb2b8a7e806f95eea9954575ce3fdc99fa0fb7068d8c9227d4c0d613` |
| `backend/scripts/mcr/migration_smoke.py` | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` |
| `contracts/domain/v1/feed.schema.json` | `9c8d991df0bb707d7a7a2d43d4b132e7fadb878a52633e1c1f338afc9407f8ed` |
| `contracts/domain/v1/operations.json` | `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` |
| `contracts/domain/v1/policy.json` | `508d22af964b718cdcbd5a25b32b9a1e3f7b651a5b8c25c87d56f1f6b68d0ebe` |

Migration paths are under `backend/services/concert-service/migrations/`:

| Exact source migration | SHA256 |
| --- | --- |
| `0001_concert_core.sql` | `9978e75e569c1ba25cb191ddf5b02bbdffcd5e64f3385173f05a885370d9215a` |
| `0002_command_receipts_outbox.sql` | `5f5760dfe6384cb2ab5162bcc27fbc0ed75bdca6bb85229012f6389269ee442f` |
| `0003_places_map_releases.sql` | `b2d4ab07a2addece8ddb4a3e413da2a32605d00ea34082a36f5940d1ff162e7a` |
| `0004_bootstrap_admission.sql` | `cb1c0093eeaef79dbe783052447c34c78aad5c6daebdc2f6f3dc7f4b17112ac9` |
| `0012_media.sql` | `aae18779fda409d5a9de1844336c3e58d3705197346ce94b9b5e0a926e119224` |

The report carries these exact pins and full sanitized test output. The independent
post-run audit checked them against unchanged files in the qualification checkout.

## Additional checks and acceptance limits

`python3 scripts/mcr/check-plan.py`: exit0, including all13 source archive hashes,
planning exporter regressions and task/IA coverage. This is planning provenance.
`git diff --check` and the explicitly staged document check: exit0. No unrelated
tests, build or extra review gate is introduced by this report.

The policy fixture is a synthetic transaction-locked test adapter backed by
`be24_test_policy`; it is not canonical Auth or real BE37 remote revocation
fencing. Production still defaults to unavailable authority. The passing tests
do not establish real HTTP initialize/PUT/finalize/read routes, authenticated
browser uploads, purpose-specific Post/Giveaway/Report journeys, cross-service
Post publication atomicity, root composition or an enabled cleanup scheduler.
The harness applies source SQL directly and does not qualify the full SQLx runtime
ledger or integrated service grants/startup. It also does not qualify devices,
load/RSS ceilings, global deletion/backups, IN02, IN03, fullBE24 or release.

Main should integrate the coherent producer separately, retain these source pins,
and qualify those real owner/transport/composition paths on the integrated
candidate. This successful bounded native result leaves their actual acceptance
states unchanged.
