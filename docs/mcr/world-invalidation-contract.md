# C05 owning-service invalidation replay, revision 1

Status: **CONTRACT_READY**. This is Main's internal transport decision for the
existing C05 authorization and replication requirements. Producer journals,
guarded RPC implementations, World consumer wiring and native runtime acceptance
are separate deliverables. No endpoint or production World serving is enabled
by adding these protobuf definitions.

The public WebSocket contract, HTTP APIs, task scope and access-token expiry do
not change. [C05](realtime-contract.md) still requires revalidation at most every
five seconds, earlier on known logout/block/suspension/closure, and removal of
blocked users before further presence/interaction delivery. The current tighter
effective Auth lease remains at most two seconds. No global transaction across
owning services or withdrawal of bytes already handed to transport is required.

## Ownership and authentication

The new services in
[`relationship/v1/relationship.proto`](../../backend/proto/relationship/v1/relationship.proto)
are `AuthConcertInvalidationService` and `FriendConcertInvalidationService`.
Each exposes `ReadHead` and `ReadAfter`. They share message definitions but have
independent journal epochs, committed cursors, endpoints and consumer checkpoints.
An Auth response cannot advance the Friend checkpoint or vice versa.

These are system-consumer RPCs. The existing service credential registry, TLS
configuration and method-policy guard must authenticate the caller as Concert
and restrict each method explicitly. The handler must independently require its
verified caller extension. Public clients, Gateway actor tokens alone, other
workloads and forged consumer IDs do not authorize a read. There are no actor
fields: a deleted or suspended person's current JWT is not needed to deliver
the signal that revokes their old World authority. Normal actor-authorized
`ReadCurrent` methods remain unchanged and are still required for revalidation.

`consumer_id` identifies the stable World owner; it is not proof of authority.
`request_id` is a fresh canonical nonzero UUID. Replies echo the complete query
exactly. The receiving client verifies origin, query, epoch, cursor bounds,
signal identity and allowed reason/scope before applying anything. Production
uses the existing TLS/service-credential mechanism with no development fallback.
No journal data, identifiers or diagnostics enter a public World payload.

## Durable producer semantics

The owner's relevant mutation and journal record commit in the same local
transaction. A failed or rolled-back mutation produces no committed signal.
Epochs and signal UUIDs are permanent; redelivery retains the same identity.
Cursor allocation must serialize inside the owning transaction. A bare SQL
sequence and `MAX` cannot establish committed order: a delayed earlier
transaction must never appear behind an already returned checkpoint.

Sequences are contiguous committed integers from one through 9007199254740991.
Zero represents the position before the first signal. Producers retain records;
this revision has no pruning or server-side acknowledgment endpoint. The journal
epoch is distinct from any existing relationship-observation epoch/revision.
Observation metadata cannot substitute for a replay cursor.

| Owner / reason | Required scope | Effect |
| --- | --- | --- |
| Auth: `AUTH_SESSION_CHANGED` | User and exact session-family UUID | Invalidate authority derived from that family. |
| Auth: `AUTH_USER_AUTHORITY_CHANGED`, `AUTH_CREDENTIALS_CHANGED`, `AUTH_USER_REMOVED` | User UUID | Invalidate that user's current authority across events/families. |
| Friend: `FRIEND_RELATIONSHIP_CHANGED` | Two distinct user UUIDs | Invalidate both directions of the pair's visibility/interaction projections. |
| Friend: `FRIEND_USER_REMOVED` | User UUID | Invalidate relationships/projections involving that user. |

Capture relevant actual mutators, including direct deletion SQL and credential
triggers, rather than relying on a single public handler. Auth's producer may
retain its local unprefixed closed kind names and map them at this adapter.
Friend captures block/unblock and deleted-user tombstones used by its current
relationship observation. If future owner policy changes affect C05 authority,
they need corresponding producer coverage before deployment. Ordinary Profile
display freshness does not create a new global invalidation requirement; its
existing bounded current reads remain. Concrete Profile lifecycle authority
must be handled where it actually controls eligibility.

Signals contain no token, refresh JTI/hash, email, profile text, handle or
permission grant. UUIDs must be canonical and nonzero. An unspecified/unknown
reason, absent scope, mismatched reason/scope, wrong serving owner, malformed
family or identical pair members is invalid, not an ignorable extension.

## Replay and bootstrap

`ReadHead` returns a committed head cursor. An empty journal has sequence zero.
`ReadAfter` requires a matching owner epoch, an `after` sequence not beyond that
owner's committed head, and an explicit limit from one through 256. The server
reads head and page from one consistent committed snapshot. Results must be the
exact contiguous range beginning at `after + 1`, bounded by the requested limit.
`next` is the last returned signal cursor, or the unchanged `after` when empty.
The response head can be later than `next`; that means another page is required.
An empty page is valid only when `after == next == head`. A client must never
jump to a head beyond the page it actually applied.

The following fail closed: wrong or changed epoch, a future cursor, duplicate or
missing sequence, duplicate signal identity with different content, invalid
scope/reason, query mismatch, malformed/truncated response, timeout, unavailable
owner, and loss of an established source. Return sanitized errors without raw
SQL or payloads. Invalid input uses `INVALID_ARGUMENT`; epoch/future-position
mismatch uses `FAILED_PRECONDITION`; unavailable dependencies, cancellation and
deadline use their corresponding gRPC status. Clients must not turn those
failures into successful catch-up or an empty page.

Each RPC uses one bounded monotonic operation deadline of at most one second,
including channel readiness and response validation; no hidden retry creates a
fresh budget. Request/response sizes are bounded, with the replay response capped
at 128 KiB. Producer SQL and response work must finish within the same budget.
The consumer schedules the next caught-up read within 250 ms, pages backlog
without admitting new publication, and closes readiness on failure. This polling
schedule is an implementation choice; it does not replace the C05 lease bounds.

A new consumer may establish a head baseline only after its old publications,
memberships and queued work are fenced/invalidated. It records each owner baseline,
performs current typed authorization reads, and consumes intervening signals
before becoming ready. It cannot jump over history while retaining old cached
authority. A restarted owner resumes its durable applied cursor, or performs
this full invalidating bootstrap. A reset/epoch mismatch follows the same safe
bootstrap; it never silently restamps the old cache as current.

## Applying and acknowledging locally

The World owner validates a signal, takes the existing native-handoff fence,
disables affected permits, cancels stale parent operations and invalidates
relevant view generations and pending payloads. Known blocked users disappear
in both directions before another presence/interaction handoff. An already
numbered frame cannot simply be dropped and followed by a sequence gap: use the
existing safe resync/revoke semantics. Signals grant no membership and extend no
credential, lease, consent or authority deadline. Re-observe using current typed
owning-service ports before resuming eligible publication.

Only after those local effects are effective may the consumer durably record
the applied owner cursor. Replayed/duplicate signals must be harmless. A crash
between effects and checkpoint repeats work; it cannot skip effects. Durable
dedup alone is insufficient if a new runtime would recover stale local authority.
The consumer must preserve the fence across its own membership/generation
recovery. No remote acknowledgment is necessary while the producer retains all
records; the durable local checkpoint is the acknowledgment boundary.

Concert-local changes use the existing transaction-bound command receipt/outbox
and the same World application fence. Existing outbox lease, retry and fanout
obligations remain. A World handler must not acknowledge unrelated feature work
or silently discard deliveries whose required consumers have not applied them.
This contract introduces no second Concert pipeline and no automatic worker
startup.

## Required runtime qualification

Use actual owner mutations while a World lease and queued presence are valid:
Auth logout/family revocation/suspension/deletion/credential change and Friend
block/unblock/deletion. Prove transactional signal capture, authenticated bounded
read, correct cursor paging, local fence/removal, and durable checkpoint ordering.
Exercise concurrent commit ordering, rollback, duplicate/reordered input,
consumer/producer restart, stale epoch, backlog, lost source, RPC cancellation,
and send-versus-invalidation races. Preserve unrelated users/events and exact
credential expiry. An unauthenticated or sibling-workload call must fail.

The contract's protobuf compatibility check proves definitions only. Source
journals, deterministic transport fixtures or narrow native tests do not by
themselves qualify the combined production World path or IN01.

## Definition validation

Retained vendored `protoc` 31.1 compiled the baseline at `18d4af02` and this
revision with `--descriptor_set_out` (exit 0). Existing Python protobuf 6.33.6
compared all three prior message descriptors and both prior service descriptors:
each remains byte-identical. The new definitions use unary bounded reads and
one exclusive user/family/pair scope, with no credential fields. Artifacts are
in `/private/tmp/mcr-invalidation-contract-20261104/{before,after}.pb`;
the [committed result](evidence/world-invalidation-contract-2026-11-04.json)
records descriptor/source hashes and the limits of this check. The descriptor
command uses the repository's `backend/proto` include root and
`relationship/v1/relationship.proto` input. No dependency was installed.

Planning/source provenance and `git diff --check` also pass (exit 0). Rust
generation/compilation and actual authenticated RPC behavior remain to be
qualified with the producer/consumer implementation; neither reserved Rust
build target was taken from its active owner for this contract-only change.
