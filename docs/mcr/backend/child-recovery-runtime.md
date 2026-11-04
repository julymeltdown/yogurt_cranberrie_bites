# Durable finished-root child recovery

Status: **IN_PROGRESS**. Production source authored; Rust build and native
component execution **NOT_RUN** on this revision. CR01–CR06 and actual production
main activation are **NOT_RUN**. The persistent owner-authority goal stays active.

## Assigned ownership and source

Root explicitly delegated the `PgScopeLedger::register` capacity predicate,
bounded enumeration in `scope_ledger/children.rs`, new `child_recovery.rs`,
new tests/controllers and this document to BE207. Only the single
`pub mod child_recovery;` declaration changes `product_rpc/mod.rs`; Party213 was
notified before editing it. Root retains main wiring, SDK enums and ScopeLifecycle.
Root's concurrent frozen source must not be changed by this handoff.

Base: HTTP fixture `265794581fb7b414b2ac8b3ec48771260944e5ca`, coherently merged
with Root SOCIAL_READ `25a8742638d47f940b44df638d7cca968ad96af1`. All existing
SOCIAL_READ full native identity, time-independent validation, closed admission,
encryption and dispatch semantics remain intact. No migration, manifest,
lockfile, protocol or generated artifact changes are part of this patch.

## Behavior

Previously only a live `OwnerGuardScope::finish_response` invoked child recovery.
Neither settlement resolver enumerated durable child intents. A lost Root
registry therefore stranded even read-only finished scopes. Durable admission
also counted only non-finished scopes, admitting new work after restart while
published finished scopes still had unresolved owner/child work.

The configured worker reads only pending children joined to genuine durable
`ROOT_FINISHED` scopes for its ledger's exact instance. It retains the original
keyring and actual typed `ChildIntentSettler`; there is no URL, GID, SQL, identity
or fallback-kind supplied by a caller. Encrypted full references are authenticated
and validated against the original immutable binding before native dispatch.
Expiry is irrelevant to settlement and cannot restore content access.

Each tick selects at most the configured batch size. One constant-memory cursor
orders `(scope UUID, kind, intent UUID)` globally. Every attempted candidate
advances it, including decrypt failure, timeout, unavailable owner or failed ACK.
The scan wraps at its end. This avoids both per-child and per-root starvation;
unknown work stays pending. An absolute system budget bounds the entire batch,
and each child gets the smaller remaining budget and its per-child timeout.
Database transactions have the existing lock/statement limits and are committed
before the owner/native call. A successful native call is followed by the existing
exact immutable ACK method. A lost ACK remains pending and is retried. Concurrent
workers rely on actual native idempotence and existing locked SQL transitions.

Admission counts every unfinished scope and every published finished scope
without its actual separate `delivery_completed_at` receipt, even after every
child ACK. Empty process memory, expiry, missing producer transaction rows and
process death cannot retire it. Only already-finished, never-published NoWork
with no serialization, transaction or child intent can retire without that
receipt. Child recovery never writes a root phase or delivery receipt.

## Startup ABI for Root

```rust
let settings = ChildRecoverySettings::from_env()?;
let worker = ChildRecoveryWorker::new(configured_ledger.clone(), settings)?;
// Attach worker.run() to the existing service supervisor and join it on shutdown.
```

`ChildRecoverySettings::new(batch_size: u16, per_child_timeout: Duration,
batch_budget: Duration, interval: Duration) -> Result<Self, Status>` validates
all bounds; `Default` supplies 16 / 500 ms / 2 s / 1 s respectively. `from_env`
accepts only unsigned decimal values and rejects malformed or out-of-range
configuration. Public `interval()` exposes the configured cycle interval.

| Environment variable | Default | Allowed |
| --- | --- | --- |
| `CONCERT_CHILD_RECOVERY_BATCH_SIZE` | 16 | 1–202 |
| `CONCERT_CHILD_RECOVERY_CHILD_TIMEOUT_MS` | 500 | 50–2000, no greater than batch budget |
| `CONCERT_CHILD_RECOVERY_BATCH_BUDGET_MS` | 2000 | 50–10000 |
| `CONCERT_CHILD_RECOVERY_INTERVAL_MS` | 1000 | 100–60000 |

`ChildRecoveryWorker::new(PgScopeLedger, ChildRecoverySettings)` returns a checked
worker. `tick(&mut self)` returns `ChildRecoveryCounts`: selected, attempted,
confirmed completed, pending and timed-out attempts, plus scan-wrapped. Completed
means an ACK was confirmed, not a uniquely new release and never completed
HTTP delivery. Logs contain counters or a fixed scan-unavailable diagnostic only.
`run(self) -> Result<(), Status>` retries dependency failures on the next bounded
cycle. Global shutdown denies new cycles and drains the current bounded cycle.

## Authored verification and remaining acceptance

`tests/child_recovery_runtime.rs` uses actual PG16, encrypted ledger intents and
the real `PublicProfileReader` LocalPublic native settlement adapter. Its public
constructor validates the selected source-bound release. Native transactions
hold real advisory locks and full immutable database identities; successful
recovery uses real exact rollback/`pg_xact_status`, not a positive service double.
Negative unavailable and hanging adapters only inject failures. Test cases cover:

- Seventeen native children across sixteen published finished roots; one unknown
  first candidate, eventual progress for later candidates, real writer exclusion,
  native release, no fabricated delivery receipt, capacity retained across a
  reconstructed ledger both before and after all child ACKs.
- Actual ACTIVE/SEALED children after expiry; no worker selection, phase promotion
  or ACK, and actual writers still excluded. Explicit owned-fixture native
  teardown is separate from worker settlement and does not acknowledge them.
- Omitted old recovery key, retained-key restoration, one bounded hanging child,
  later native progress and concurrent idempotent retries.

`tests/owner_authority_support/qualify_child_recovery.py` requires an exact clean
source checkpoint and copied executable, starts a fresh marker/inode-owned PG16
cluster, migrates the actual Concert schema and checks zero prepared work before
cleanup. These are native-adapter/SQL component tests. They deliberately use
fixture ledger lifecycle setup and same-process adapter reconstruction; they do
not qualify actual canonical acquisition, HTTP terminal handling, physical Root
restart or main activation. The full CR01–CR06 contract remains binding.

Source-level checks: `rustfmt +1.88.0` (owned Rust paths, children skipped), Python
AST parse, `git diff --check`, and `python3 scripts/mcr/check-plan.py` all exit 0.
Planning PASS is not application acceptance. Build/native receipts and exact
handoff ancestry must be added after execution.

The native controller also pins tracked release, contract and `frontend/public`
resource bytes consumed by the public constructor, following Public's native
controller approach. A source-bound constructor does not replace a complete
qualification checkpoint. Static peer review found no actionable defect in the
authorized production slice; that review did not execute native checks.
