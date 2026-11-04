# BE42 PrivateData core deletion

Status: **IN_PROGRESS**. The bounded core leaf is implemented here. Whole
PrivateData deletion remains **INCOMPLETE** until every owner category in the
[field inventory](account-private-data-field-inventory.md) has a real owner
step and the Auth-to-Concert transport is qualified.

## Receipt and authority

The public entry is `PrivateDataCoreDeletion::apply` using the shared
`WorldDeletionCommand` with `WorldDeletionTarget::PrivateData`. The command must
pass through the mandatory `DeletionAuthority`, which supplies a nonzero source
binding and exact original deadline. The deadline must be future and no more
than five seconds. Re-verification occurs before commit; a changed source or
bound command rolls back the transaction.

The core receipt binds the user, saga, actor, nanosecond retention timestamp,
target command digest and verified source digest. Its fence revision is one,
and the receipt cannot be rewritten to a different command. The durable cursor
is a UUID lower bound over `(user_id,event_id)`, while scan, neutralized and
removed counters advance monotonically. Each transaction handles at most 128
event ids, and repeated application resumes from the committed cursor.

## Lock and transaction order

One shared PostgreSQL advisory lock, keyed by the canonical Auth user, is taken
first. It precedes event, admission, participant, avatar and authority locks.
The participant preference writer, avatar writer and prepared Profile
coordinator use the same lock before their existing row locks. This prevents a
normal writer from passing a pre-fence check and committing after the erase.

The core receipt, permanent fence, participant neutralization, avatar removal
and transaction-local controlled bypass commit together. A transaction-local
setting is accepted only after authority verification and the user-first lock;
the resulting trigger prevents every other direct personal write and delete.

## Dispositions

Existing participant personal settings become `PARADE` with low power, reduced
motion and sound all false. The participant key stays because it is the
canonical aggregate reference. Existing avatar selections are removed. Other
personal values and their owning services are listed in the inventory and must
not be inferred from core success.

## Validation boundary

The source tests cover command binding, deadline/source failure, redaction and
lock precedence. The ignored native test is intended for a marker-owned fresh
PostgreSQL database and covers multiple events, non-target user preservation,
idempotent replay, the permanent write fence and receipt completion. Rust 1.88
locked/offline compilation and the native run require a fresh named allocation.

This leaf does not provide canonical Auth transport, production route wiring,
foreign service database writes, or full BE42/IN02 acceptance.
