# Receipt harness pins after event-scope review

The parent combined helper run at `9dd8bc6` found one failure among 97 cases:
the BE15 harness correctly refused contract drift after reviewed event-moderation
correction `5c02a31`. That central contract update had not refreshed the harness's
exact operations/policy hashes. This is an integrator-owned follow-up, separate
from the Auth RPC worker's unchanged nine-path implementation.

The parent compared parsed before/after inputs. All 106 operation IDs, methods,
paths and command flags are identical; the only operation change is the explicit
`PATH_EVENT_ONLY` scope on `moderateConcertUser`. The common schema, receipt
shape, idempotency/fingerprint policy and every other operation rule are
unchanged. Fresh event guards and the moderation rule were deliberately clarified
in `5c02a31`, with independent review `e0b37cc`. See the
[comparison receipt](receipt-contract-pin-refresh.json).

The harness now pins those reviewed candidate hashes and still refuses unknown
drift before database work. A new negative regression checks that refusal. The
full helper suite and contract checks are rerun with the combined RPC candidate;
their final results are retained in the Auth RPC integration evidence. The
initial helper failure is preserved there as well.

No runtime Rust, SQL, producer or outbox behavior changes. Earlier BE15 native
evidence remains bound to its historical source/contract candidate; this refresh
does not claim a new native command run, C04 freeze, live moderation or task
acceptance. Production producers and Concert authority remain unavailable.
