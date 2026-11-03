# Profile query lifetime correction

Status: **IN_PROGRESS**, production patch pending independent native PostgreSQL
regression and parent review. Base `c6404c33e2d284b776652b416d0b96ed45856a28`, branch
`codex/mcr-be-profile-query-bound`. Only the Profile persistence and RPC
`concert_identity.rs` files and this document are changed. No application enum,
wire, dependency, lockfile, migration, shared test or public endpoint is changed.

## Failure and correction

The independent actual Gateway/Profile/PG chain reproduced an abandoned lock
wait twice: cancelling the Gateway restored its 256 permits, while the database
query still waited beyond 3155 ms and occupied the sole pool connection. SQLx
0.8.6 dropping a future queues transaction/pool cleanup; it does not send a
PostgreSQL CancelRequest. A local timeout alone cannot bound that server work.

The RPC now scopes its existing captured signed/original stop over owner execution.
The repository captures one stop: the shortest active task deadline and retained
GatewayProfile local ceiling (800 ms minus the existing 100 ms margin). Direct
owner calls also use that retained ceiling. Pool acquisition, setup, content read,
decode, commit and final return share an absolute `timeout_at` and remaining-time
checks; no stage obtains another full budget.

On the acquired transaction, a separate parameterized
`select set_config('statement_timeout', $1, true)` finishes **before** the original
parameterized content SELECT is prepared/executed. The value is remaining whole
milliseconds sampled after acquisition; expired/submillisecond budgets refuse
instead of configuring zero (which disables PostgreSQL's timeout). It is never a
pool-wide/session setting or a function hidden in the content SELECT, which could
wait on relation locks before evaluating configuration. The original content SQL,
subject binding, deletion fence, transfer limits and statement observation remain
unchanged. Successful reads commit; errors/cancellation drop the transaction and
queue rollback, retaining the server-side timeout while its query unwinds.

PostgreSQL applies the relative statement timeout from command arrival, including
Parse/Bind/Execute work; it does not promise instantaneous cancellation at the
client's clock tick. Local absolute fences still reject late results. Independent
native evidence must prove bounded lock-wait cleanup and same-pool reuse while
the external lock remains held, plus transaction-local setting restoration.

Error contracts remain unchanged: native cancellation or submillisecond refusal
before the actual authority deadline may return fixed `DependencyUnavailable` /
`UNAVAILABLE`. At or after the captured authority stop, the RPC post/final fence
returns `DEADLINE_EXCEEDED`. No diagnostic-text parsing, invented cancellation
authority, new portable category or sleep to force a status is introduced.

## Provenance and validation

Retained source foundation is port-rust
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, imported under `backend/`.
The transaction-local setup pattern follows existing Auth
`backend/services/auth-service/src/adapters/out/persistence/concert_identity.rs` at the exact base, SHA-256
`cc7d95999d1bba494e2a58d8ee48138ac89665cd7f8bef22cbcf3692038cc651`; it is provenance, not runtime proof.
PostgreSQL documents [statement_timeout](https://www.postgresql.org/docs/17/runtime-config-client.html#GUC-STATEMENT-TIMEOUT)
and [transaction-local set_config](https://www.postgresql.org/docs/17/functions-admin.html#FUNCTIONS-ADMIN-SET).
SQLx source inspected: retained 0.8.6 `sqlx-postgres/src/transaction.rs`,
`sqlx-core/src/transaction.rs` and `sqlx-core/src/pool/connection.rs`.

Focused commands use Rust 1.88.0, `--locked --offline -j 2`, the coordinated shared
target, and both dev/test debug settings zero. Runtime results and exact command
exits are recorded below and in the external hashed handoff. No native harness is
duplicated; the independent integration worker owns the actual PostgreSQL gate.

- Guarded RPC controlled-owner regression: 12 passed, 6 native cases explicitly
  filtered; exit 0. This is not database acceptance.
- Application owner validation regression: 10 passed, none filtered; exit 0.
- Retained native owner test compilation: exit 0; no database execution claimed.
- Profile library/binary compilation: exit 0.
- Exact three-path boundary, original content SQL, retained-input hashes, Rustfmt,
  whitespace and planning checks: all pass, exit 0.

Executed Cargo commands (all `--locked --offline -j 2`; each exit 0):

```sh
cargo +1.88.0 test --locked --offline -j 2 -p profile-service --test concert_identity_rpc -- --skip native_
cargo +1.88.0 test --locked --offline -j 2 -p profile-application --test concert_identity
cargo +1.88.0 test --locked --offline -j 2 -p profile-service --test concert_identity_owner --no-run
cargo +1.88.0 check --locked --offline -j 2 -p profile-service --lib --bins
```

Static commands: `python3 scripts/mcr/check-plan.py`,
`python3 scripts/mcr/check-boundaries.py --base c6404c33e2d284b776652b416d0b96ed45856a28 --lane backend --allow-doc docs/mcr/backend/profile-query-deadline.md`,
`git diff --check`, and Rust 1.88 Rustfmt `--check` on both edited Rust files.
External evidence is in `mcr-build/profile-query-bound/`; the immutable handoff
receipt binds exact commit/base/path hashes, command exits, logs and fresh target
release. No failed compile/test attempt occurred in this patch.

Independent native cancellation, shortest-deadline cleanup, single-pool reuse and
setting restoration remain **NOT_RUN for this patch** until the integration owner
runs the exact corrected source. Whole BE04, public composition and production
TLS/configuration remain outside this correction. Parent alone integrates/pushes;
the goal remains ACTIVE through that concrete regression/review gate.
