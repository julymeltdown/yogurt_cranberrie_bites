# Community/meeting shared-adapter lock order and rollback qualification

Status: **IN_PROGRESS**. Recovery resumes on published producer base `707ea68`.
The earlier source reportedly failed compilation with SQLx “Executor is not
general enough” in `after_connect`/`raw_sql`; no successful native qualification
is inferred. This revision replaces that callback stream with prepared single
statements and explicit connection reborrows. Its compile retry awaits an actual
released Cargo lease; native PostgreSQL execution remains **NOT_RUN**.

## Ownership and provenance

- Sole QA writer: chat `01a1a232-b91d-78f1-a13a-1361788ba51c`.
- Current sole writable checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-community-policy-resume`.
- Current branch: `codex/mcr-qa-community-policy-resume`.
- Recovery base: `707ea68d585324430144c971724efbf5c73167d6`.
- Previous checkout `/Users/lhs/.codex/worktrees/d5a6/mcr` is read-only after
  verified terminal interruption; no commands or edits resume there.
- Recovery receipt: `/private/tmp/mcr-recovery-20261104-qa-ops/receipt.json`.
  Only the two assigned files were copied, with original hashes
  `81d907ab382ab5f251cbedbcda6e8331208153a16b395322ecb888537c16cc44`
  (test) and `486d57e788ba2f97ba15c6f26cdf486760513b0f103c7ae709de2b77aa698be2`
  (report). No dependency directories were copied.
- Original checkout base: `072ea86768ab3606854e81a67ab923c78c7ca71d`.
- Delivery-authorized master prerequisite sync:
  `efdb06ada44bd540ac95acb4d195df5f1bf986eb`, then corrected integrated
  producer base `64b25dc16fba66959213dc901b760b73cc7a02bc`.
- Source-only QA checkpoint after rebase:
  `51807a08f399a2a681b584f2d1aed292f0f0b356` (parent `64b25dc`);
  this checkpoint explicitly records native execution as NOT_RUN.
- Former writer base: `51783cf5651b68ca98faede29d442c4dccc24f6f`;
  inherited draft `a788a99730edc53363ef183bd524acacca745809` and repaired but
  uncompiled revision `66a6c5c43d990b4599b7894466f6c6432955f6d1`.
  The two assigned files were restored from `66a6c5c` and repaired here. Its
  native execution remained NOT_RUN; its former goal is not modified.
- Exact writable paths:
  `backend/services/concert-service/tests/qa_community_policy_lock_order.rs`
  and this report. No production, migration, contract, manifest, other QA,
  frontend, or preserved `docs/source/` files are authored by this handoff.

The test uses existing locked `sqlx`, Tokio, async-trait, chrono, serde_json,
UUID and anyhow dependencies. It reads the existing
`tests/support/meeting/mod.rs` fixture to publish an actual BE08 catalog with
explicitly synthetic test provenance. It uses actual participant rows, BE37
policy, party aggregates/membership, meeting adapters, command receipts and
outbox tables. Only the remote Auth/Friend owner proof is replaced by a labeled
synthetic authority; the feature/resource composition is an internal test
adapter, not a qualified production authority.

## What the repaired test proves

1. Create party, apply, approve and create a party-scoped beacon with actual
   `PgCommunity` and receipt-backed `BeaconCommands`, including real place
   selection. No invented production schema or manual effect staging is used.
2. Start an actual `PgCommunity` member-removal command. A scheduling wrapper
   pauses its first full BE37 authorization after the real producer has locked
   the party root. The wrapper delegates every authorization to BE37.
3. Call the actual meeting actor-only guard and start a meeting read on the
   same reader transaction. Establish root contention using exact backend PIDs
   and `pg_blocking_pids`, not a timing sleep as evidence.
4. Start an actual receipt-backed beacon command. It must also wait on that
   root before acquiring the shared BE15 budget. Release the writer's gate.
5. The writer must complete sorted relationship authorizations, acquire the
   BE15 budget and stage its real aggregate/member, receipt and outbox effects
   while both meeting operations remain blocked on the writer. Each wait is
   bounded. Completed policy subjects are checked against sorted owner/peer IDs.
6. A test-only `AFTER UPDATE` receipt trigger checks all actual staged effects:
   terminal receipt/key/target, exact correlated READY outbox/revision, removed
   membership/revision, and changed party aggregate/member count. Only after
   every check passes can it request a witness advisory lock held by the
   observer. Seeing the writer blocked by that observer proves nonzero staged
   effects existed in that transaction. A missing effect fails before witness.
7. Releasing the witness injects a PostgreSQL exception; the actual command
   repository rolls back. Verify the whole party JSON and member row match
   their committed pre-write values, zero receipt rows for the actual command
   key, zero removal outbox rows for its target, and no leaked receipt budget.
   The original reader reauthorizes and returns the intact beacon; the competing
   real beacon command succeeds after release.
8. Drop the test-only failure trigger. The same removal key succeeds, replay
   returns the same receipt and exactly one correlated outbox, and actual
   meeting access for the removed member is denied before outbox delivery.

The fixture uses a private socket-only PostgreSQL cluster. The connection guard
requires explicit fixture variables, root inode/device identity, private
non-symlink root/marker/data/socket paths, matching owner marker token/UID,
allowed fixture database/user/socket/port, exact server data directory and no
TCP listener. There is no generic `DATABASE_URL` or arbitrary local fallback.
The pool has acquisition, statement, lock and idle-transaction bounds; spawned
reader/writer futures have their own deadlines as well as coordinator bounds.

## Commands and results so far

- `rustup run 1.88.0 rustfmt --edition 2021 backend/services/concert-service/tests/qa_community_policy_lock_order.rs`: exit **0**.
- `python3 scripts/mcr/check-plan.py`: exit **0**, planning-only PASS;
  application runtime qualification is separate.
- Previous native compilation: **reported FAIL**, SQLx callback lifetime error;
  the original command/log/exit code was not recovered with the two-file source
  transfer. This is retained as a failure, not described as a passing build.
- Recovered callback repair compile retry: **NOT_RUN**, no Cargo lease used yet.
- Guarded PostgreSQL: **NOT_RUN** for this revision.

## Limits

This is bounded native adapter/transaction qualification with deterministic
synthetic remote owner proof and synthetic place provenance. It does not prove
public HTTP/Gateway composition, live Auth/Friend authority, real venue
verification, live multiuser behavior, IN02, whole BE task acceptance, capacity,
or release readiness. The immutable published `0008_meeting_beacons.sql` must
remain byte exact; corrected meeting deadline guards belong to forward `0016`.

## Reproduction runner (after copying the leased native binary)

Run from the checkout root, setting `QA_NATIVE_BINARY` to the copied executable's
absolute path. This reuses the repository's owned-cluster lifecycle without
altering its script. The report and generated credentials stay in private
runtime directories; cleanup stops/removes only the marker/inode-owned cluster.
The SQL scripts are applied to the fresh database directly; this runner does
not claim execution of the service's SQLx migration-ledger path.

```bash
python3 - "$QA_NATIVE_BINARY" <<'PY'
import hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path
root = Path.cwd()
sys.path.insert(0, str(root / 'backend/scripts/mcr'))
import migration_smoke as m
binary = Path(sys.argv[1]).resolve(strict=True)
assert binary.is_absolute() and os.access(binary, os.X_OK)
evidence = Path(tempfile.mkdtemp(prefix='mcr-qa-lock-order-', dir='/tmp')).resolve()
report = {'result': 'FAIL', 'cleanup': 'NOT_RUN', 'tests': [],
          'source_head': subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
          'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
          'migration_ledger': 'NOT_APPLIED', 'public_policy': 'NOT_RUN', 'IN02': 'NOT_RUN'}
source = root / 'backend/services/concert-service/tests/qa_community_policy_lock_order.rs'
report['qa_source_sha256'] = hashlib.sha256(source.read_bytes()).hexdigest()
names = ['0001_concert_core.sql', '0002_command_receipts_outbox.sql',
         '0003_places_map_releases.sql', '0006_relationship_policy.sql',
         '0007_community.sql', '0008_meeting_beacons.sql',
         '0009_receipt_domain_failures.sql', '0011_outbox_maintenance.sql',
         '0016_meeting_deadline_guards.sql']
cluster = None
try:
    tools = {name: m.executable(Path('/opt/homebrew/bin') / name) for name in m.PG_TOOLS}
    cluster = m.Cluster(tools)
    report['owned_directory'] = str(cluster.root)
    cluster.init(); cluster.start()
    db = 'mcr_community'
    cluster.sql('postgres', f'create database {db} template template0;')
    report['migration_sha256'] = {}
    for name in names:
        path = root / 'backend/services/concert-service/migrations' / name
        report['migration_sha256'][name] = hashlib.sha256(path.read_bytes()).hexdigest()
        cluster.sql(db, path.read_text())
    report['postgres_version'] = cluster.sql(db, 'show server_version;')
    environment = {**cluster.env, 'CONCERT_TEST_DATABASE_URL': cluster.url(db),
        'CONCERT_TEST_OWNER_ROOT': str(cluster.root),
        'CONCERT_TEST_OWNER_TOKEN': cluster.token,
        'CONCERT_TEST_OWNER_IDENTITY': f'{cluster.identity[0]}:{cluster.identity[1]}'}
    def run(phase, env, expected=0):
        result = subprocess.run([str(binary), '--test-threads=1', '--nocapture'],
            cwd=root, env=env, capture_output=True, text=True, timeout=45)
        output = result.stdout + result.stderr
        assert cluster.password not in output and cluster.url(db) not in output
        (evidence / f'{phase}.log').write_text(output)
        report['tests'].append({'phase': phase, 'exit_code': result.returncode,
            'expected_exit': expected, 'output_sha256': hashlib.sha256(output.encode()).hexdigest()})
        assert result.returncode == expected, f'{phase} failed; inspect private log'
        if expected == 0:
            assert '1 passed; 0 failed' in output
        else:
            assert 'fixture inode changed' in output
    run('wrong-inode-refused', {**environment, 'CONCERT_TEST_OWNER_IDENTITY': '0:0'}, 101)
    run('before-restart', environment)
    query = "select md5(string_agg(row_to_json(p)::text,',' order by event_id,party_id)) from concert_community_parties p;"
    before = cluster.sql(db, query)
    assert before
    pid = cluster.assert_postmaster()
    cluster.stop(); cluster.start()
    assert cluster.assert_postmaster() != pid and cluster.sql(db, query) == before
    report['restart_persistence'] = 'PASS'
    run('after-restart', environment)
    report['result'] = 'PASS'
except BaseException as error:
    report['failure'] = getattr(error, 'detail', type(error).__name__ + ': ' + str(error))
finally:
    try:
        if cluster is not None: cluster.cleanup()
        report['cleanup'] = 'PASS'
    except BaseException:
        report['cleanup'] = 'FAIL'; report['result'] = 'FAIL'
    (evidence / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'evidence': str(evidence), **report}, indent=2))
raise SystemExit(0 if report['result'] == 'PASS' else 1)
PY
```

## Pinned producer and contract inputs

The current published producer base is `707ea68d585324430144c971724efbf5c73167d6`.
Comparison against the earlier `64b25dc` baseline finds no byte changes in the
community, meeting, relationship, command-receipt or shared meeting-fixture
modules relevant to this test. No old-pin import barrier or dependency copies
are required. The table records current last-changing revisions and bytes.

| Path | Last-changing commit | SHA-256 |
| --- | --- | --- |
| `backend/services/concert-service/src/relationship/mod.rs` | `73c96f0a86503ae54a0fc3a77ba64478e9d1adad` | `66f0bb08a7878d50a8a6e7474cf7c56446bc463c78bb85d57bc56c32f3400d70` |
| `backend/services/concert-service/src/meeting/authority.rs` | `0c03780819d16d4f1fe145297f727279b24f092a` | `5c4ae886b1bda0de735f548cfd2f41d4fdb084207e61a89c8888ec7809637416` |
| `backend/services/concert-service/src/meeting/commands.rs` | `0c03780819d16d4f1fe145297f727279b24f092a` | `d57e17ab45927b4ad131039b9eaa391af44d06e382a2ab8742c035ab6ac2b93e` |
| `backend/services/concert-service/src/community/mod.rs` | `e0ac5e2ce8d903069f6119ad6363b2f8ce3682dd` | `0c9b4ec3d08e76ca763a325cc085fa1eb7a87c17375949e1f960578d4e2a396f` |
| `backend/services/concert-service/src/adapters/out/repository/receipt.rs` | `c2a2c3020121c7385680eaae7cfa6ba027fe5e01` | `01012f034b30aa5f53f43402feae9ca72048cbb526e727794dbed3dd5ca4355f` |
| `backend/services/concert-service/tests/support/meeting/mod.rs` | `1db0b8155c845c4d312e263c2d00d1a814501c79` | `171156836f7ef59bff64ead2ef7bc86ebb12a29b509d0880ed4bed67960890b4` |
| `contracts/domain/v1/party.schema.json` | `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d` | `7cbea6af623668a36d641e7498bf52f4869b9bc44594b46e7f4a3806cc907d03` |
| `contracts/domain/v1/beacon.schema.json` | `b615e35bb07fb5797e6d7445a93aa6e1eeab7152` | `68e9249c79644f7f3c137c7eddf3bd1977a33223de5e9dbd7d2c236e20dbf486` |
| `contracts/domain/v1/operations.json` | `911f15665379425afeb52c1401626f39f5298739` | `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839` |
| `contracts/domain/v1/policy.json` | `911f15665379425afeb52c1401626f39f5298739` | `81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb` |

Published migration `0008_meeting_beacons.sql` SHA-256 remains
`b8259eb000c15059ddcba60db64518562d2cede7d2d68c42de3df8a85b44c655`.
