# IN04 managed Operations issuer

Owner: backend205. Root1fc owns launcher composition, the shared runtime window
and normal serialized publication. This is a new private qualification helper,
not a product API, alternate role authority or another stack launcher.

The assigned paths are `backend/scripts/mcr/in04_wall_runtime/roles.py`, its
`test_roles.py`, and this document. Preparation preserves the frozen
`codex/mcr-be-operations-public` checkout at
`ebb3e40ada2b9e14d16bc973af999c8dac9702a4`; the committed packet contains only
these three new paths. No Report, service-policy, shared manifest, migration,
contract or launcher changes belong to this packet.

## Integration

Import `ManagedRoles` after adding the existing `backend/scripts/mcr` directory
to the helper import path. In Root's existing DevStack environment callback,
after its actual owned PostgreSQL cluster has completed migration/restarts and
before Concert starts:

```python
roles = ManagedRoles(
    stack, work_deadline,
    expected_system_identifier=actual_postgres_system_identifier,
    window_end_epoch=root_absolute_work_expiry,
)
concert_environment.update(roles.environment())
```

`stack` must be the actual `dev_stack.DevStack`; its cluster must be the actual
`migration_smoke.Cluster`; `work_deadline` must be the existing
`in03_stack_runtime.WorkDeadline`. Expiry must fit inside that work deadline.
The helper verifies the existing root inode, owner marker, private permissions,
postmaster PID, actual PostgreSQL system identifier, data directory, Unix socket,
database owner, disabled TCP and database server clock. It creates neither a
cluster nor a service. It executes only bounded read-only SELECTs against this
cluster, using the cluster's private connection environment.

Before Concert starts it creates a fresh private issuer directory, Ed25519
private key and public key, then atomically publishes the ordinary mode0600 file
containing `[]`. The returned environment has exactly the live-mode setting,
public issuer-key JSON and publication path. Private key bytes, password and
bearer are never returned or logged.

After three real HTTP Auth enrollments/logins and event admissions:

```python
records = [
    {"user_id": user_id, "credentials_path": credentials_path}
    for user_id, credentials_path, _access_token_in_ram in principals
]
receipt = roles.publish_operator(actual_event_id, records, principals[2][0])
```

The sanitized records contain no bearer or password. Each credential path is
exactly the owned cluster root's `in01-principal-{ordinal}.json`, ordinals1–3.
The current enrollment supplier is
`backend/scripts/mcr/session_deletion_runtime/enrollment.py`, commit
`dee64fd1ac868f4dffbea49c0e75ca8e98727f3a`, SHA256
`c4a328cdbb220202ef90502324f9873da5012e045b4f1e8b22d6ba9843760dc9`.
Its closed credential schema is `user_id`, `email`, `password`, `login_path`
(`/api/auth/login`), and `access_token` (`MEMORY_ONLY`). Its user ID comes from
the actual successful login; there is no persisted `login_user_id` claim.

The helper independently requires all three distinct canonical users to have an
actual ACTIVE, nondeleted Auth account, matching verified nondeleted identity,
successful login audit and actual ACTIVE admission to the exact event. Those
read-only checks are not fabricated principal seeding. Only the third user may
receive the single initial OPERATOR assignment, at expected revision0. There
are no role-table SQL writes, positive role fixtures or bearer inputs.

The signature is Ed25519 over `MCR-OPERATIONS-ASSIGNMENT-V1` followed by one NUL
and compact UTF-8 JSON in the existing Rust `AssignmentStatement` field order:
version, statement_id, issuer, event_id, actor_id, expected_revision, role,
issued_at, expires_at. Public keys are32-byte lowercase hex; signatures are64-byte
lowercase hex. `issued_at` is the actual database clock and `expires_at` does not
exceed Root's pinned absolute window. The helper verifies its signature using
the actual OpenSSL binary before atomically replacing the publication file;
it checks custody and the database clock again after signing. Keys and tool
identity are pinned. Replaying the exact binding preserves statement/file
identity; changed binding or publication contents fails closed.

The retained production `OperationsPublication` worker must apply this input
using the SAME configured `OperationsAssignments` and PostgreSQL pool. A helper
receipt says `applied: NOT_OBSERVED`. Root's real OwnAccess/settings/Admin journey
must observe the actual application; signing alone does not establish it.
Initial `[]` is not revocation of an existing durable grant. This helper supports
one fresh expected-revision0 grant only, not rotation, renewal or revocation.
Root owns the existing cluster's cleanup, including its private issuer files.

## Validation and limits

The test module runs actual OpenSSL signing and independent signature checks,
including modified payload/signature negatives, alongside controlled read-only
SQL answers. Ownership checks use actual private files/root markers/postmaster
metadata, with no PostgreSQL process. It covers initial publication/privacy,
exact third actor/event/revision/server-time/expiry, atomic replay, wrong system
identifier, already-started Concert, deadline and post-sign expiry, missing Auth
or admission, credential binding, private file/symlink/hardlink failures, marker,
postmaster, directory and stack replacement, key tampering and wire-policy
negatives. Simulated SQL responses do not establish real enrollment or admission.

Execution status: **PASS** for the source suite on2026-11-04UTC, under
`IN04-MANAGED-ROLES-SOURCE-20261104T2035Z`: the allocated command
`python3.14 -B -Werror -m unittest -v test_roles` exited0,15 tests passed in0.405s.
OpenSSL3.6.3 performed actual Ed25519 key generation, signing and verification;
SQL responses were simulated. Both controller and independently pinned guardian
were active before test children. Peak5 processes,95,158,272 RSS bytes and151,552
new allocated bytes; initial nonshrinking carry12,451,090,432 bytes fit the14GiB
whole ceiling. Actual FULL at epoch1793824596.574358 verified test/guardian PIDs
absent, controller exit0 and owned TMP absent, before documentation/commit.

Private source/proof packet:
`/private/tmp/mcr-in04-managed-roles-source-k4wgzbcp/control/`, with `pins.json`,
`lease.json`, `tests.stderr`, `result.json` and `full.json`. Original frozen test
pins record roles.py SHA256
`6687d85b83735c03b0bd8f0b6ecdeee6adbbc3c417b2452e87d656e1bde87781`
and test_roles.py SHA256
`7c194b31d6deeccc692aec961747b3660ff17d0b269290920909618a5662af37`.
The initial qualified source bytes are preserved in commit
`1576c74c121f1daa61c30ab2b53d21ec01337099`; the following correction has its own run.
Real same-window role application, OwnAccess, settings and Admin: **NOT_RUN**;
Root owns the shared-window execution and backend205 owns helper fixes.

No new dependencies are installed. Runtime dependencies are existing Python
standard-library helpers and Root's explicitly supplied executable OpenSSL3
binary. No cargo target, PostgreSQL cohort, browser or service is started by the
source test command.


The follow-up clamps expiry after UTC microsecond conversion to avoid an IEEE
floating-point multiplication rounding edge. Its deterministic fractional-epoch
regression and refreshed16-case suite passed under the separate finite
`IN04-ROLE-EXPIRY-FIX-SOURCE-20261104T2045Z` allocation. The same command exited0;
16 tests passed in0.430s. Acceptance epoch1793824986.830849, actual FULL
1793825005.060270 before documentation/commit. Independent controller/guardian,
private TMP and test children were reaped/removed; no PostgreSQL process, service,
Cargo target or browser ran. Peak5 processes,90,734,592 RSS bytes,3,002,368 new
allocated bytes (including concurrent Git growth); nonshrinking carry
13,311,029,248 bytes fit the14GiB whole ceiling. The original round's clock was
not reused. Proof is in the same private packet's `control-expiry/` directory.

Existing consumer provenance at the frozen EBB source revision:
`backend/services/concert-service/src/operations_live/public_access/issuer.rs`
SHA256 `29c1f99658aba1a136ff3adc395fd9c9bc3531fdb21b61d9001d7634ac890da8`;
`publication.rs` in that directory SHA256
`d1060768387342dd77984185e7693d9e67507ff073357b65df803a1db4430003`.
The helper is newly authored; these consumers define its serialization and
publication interface, with no copied rendering code or new dependency.

Separate native-role qualification on2026-11-04 passed all3
`operations_public_roles_native` cases at EBB plus actual PostgreSQL16.14 restart
persistence on the same owned Auth/Friend/Concert cohort. Its retained immutable
pin is `/private/tmp/mcr-operations-roles-frozen-yxptu0xt/pin.json`; native receipt
is `native-report.json` alongside it, with log SHA256
`df9fe1c8ce4914952974fe0f6866c4a9f64af970778078f5804887e065b00077`.
This result verifies native role invariants; it does not execute this publisher
or the production HTTP/Admin flow. Those real same-window outcomes remain open.
