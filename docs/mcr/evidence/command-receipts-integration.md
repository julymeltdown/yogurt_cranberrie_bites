# BE15 internal command foundation integration review

Date: 2026-11-03. Assignment base `376cf21`; worker implementation `11a7448`
and review fix `780c265`. Integrated as `7cbb97e` and `c2a2c30`, with separate
integrator dependency/export commits `d76215c` and `5874c81`. The strict layer
assertion was updated at `16383f7` for retained UUID/SHA2/HMAC edges only.
Parent verification used clean integrated source `5863f91`, including BE04.

The parent inspected every changed path and the implementation, migration,
outbox lease/worker, canonical input and regression tests. The exact eleven
worker paths match the final worker bytes: [source inventory](command-receipts/hashes.json).
No frontend, public router, runtime startup or cross-lane implementation was added.

Three concrete review findings were corrected before acceptance of this increment:

- An unkeyed request digest could expose low-entropy pickup secrets to offline
  guessing. C04 now keeps fingerprints internal, and the implementation uses
  injected versioned HMAC keys, retained-key replay verification and constant-time
  MAC verification. Rotation preserves the original receipt; a missing key fails
  closed. [Contract review](receipt-fingerprint-review.md) records the rationale.
- Receipt expiry was evaluated before awaited stored-target authorization. Both
  replay and status lookup now read database time after that await. A controlled
  delayed-authorization regression crosses the expiry boundary.
- A successful result and its invalidation could share a UUID but disagree on
  revision. Full reference equality is now required; mismatch rolls back receipt,
  aggregate mutation and enqueue together.

## Parent verification

All commands exited 0 unless explicitly listed as expected negative cases in the
[native receipt](command-receipts/native.json). Rust uses 1.88.0, locked/offline,
two jobs and the existing parent target; PostgreSQL is 16.14.

- Explicit application/service targets pass **33 tests**: eight command, eleven
  application identity, four existing policy, two config, one health and seven
  service identity. [Test log](command-receipts/tests.log).
- All backend smoke helper suites pass **82 tests**, including strict layer and
  retained lockfile-resolution checks. [Python log](command-receipts/python.log).
- Built the actual Concert executable and command-receipt test binary from the
  integrated checkout. A new marker/inode-owned PostgreSQL fixture passes **17
  real database tests**, concurrent duplicate/conflict and revocation cases,
  rollback, retained-key rotation/loss, expiry, bounded admission, fenced leases,
  retry exhaustion and a durable deduplicating test consumer.
- The harness proves migration failure rollback, forward recovery, apply/reapply,
  damaged-ledger refusal and persistence across an actual PostgreSQL restart.
  Every source/contract hash in the receipt matches; its recorded tree is clean.
  The fixture is removed and its absence independently checked:
  [cleanup record](command-receipts/cleanup.json).
- Combined frontend tests/build/lint, boundary/origin guards and planning checks
  also pass. The final C04 pins match the revised contract review.

No further actionable finding remains in this bounded internal foundation. Its
worker goal was confirmed **COMPLETE** through the actual goal tool. Overall BE15
remains **IN_PROGRESS**: real production producers, canonical Auth policy,
public request mapping, secret configuration, worker startup, load qualification
and downstream integrations are **NOT_RUN**. Delivery remains at least once;
the test consumer's deduplicated effect is not external exactly-once acceptance.
