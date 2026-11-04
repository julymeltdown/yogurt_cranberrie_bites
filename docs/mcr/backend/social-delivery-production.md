# SOCIAL-DELIVERY-01 production delivery composition

Status: IN_PROGRESS; source-first candidate, compilation/native checks NOT_RUN.
Base: published `cb944f7210f6c55fdf1d0e897c96ea7ef0ba8325`.
Owner: Social implementation chat; publisher/startup composition: Root.

Only three new files are assigned: `src/config/social_delivery.rs`,
`tests/social_delivery.rs`, and this document. Existing qualified Social paths,
all migrations, common outbox, config exports, main and World remain untouched.
The old Social branch/history and ignored Python caches are preserved. This
isolated worktree is not one of the seven Root RW5 physical migration inputs.

## Root interface

```rust
SocialDeliveryRuntime::new(
    pool: PgPool,
    sink: SocialInvalidationSink,
    notification: Arc<NotificationDeliveryClient>,
) -> Self
.with_world(
    receiver: tokio::sync::oneshot::Receiver<Arc<WorldInvalidationConsumer>>,
) -> Self;
// async run(self) -> anyhow::Result<()>; observes platform global shutdown.
```

The Notification type is the actual mTLS outbox transport in
`adapters::out::notification::delivery`, not the held inbox reader in
`notification_live`. Root supplies its canonical endpoint, credential and TLS
and sends its actual single-World consumer only after World readiness.
Construction creates no listener, registry, provider or seed data. Root alone
exports the new config module and supervises the returned future. A closed
World sender fails; cancellation while awaiting readiness returns without claims.
No receiver means block/unblock are excluded. Once configured, a pending
receiver prevents every claim; it cannot be bypassed by the one-batch API.

## Delivery and queue ownership

Selective claim binds immutable successful/retired receipt, exact outbox tuple
and the sealed source snapshot. It includes only set/remove handle, set/publish/
close Intro and create/decide/cancel/revoke Social request. After the actual
World receiver resolves, it additionally includes block/unblock. The block/unblock claim, exhaustion and delivery checks bind the actual
resolved consumer event UUID; foreign-event block jobs retain state and attempts.
Unknown,
canonical, maintenance, close-event and all other families keep their state,
leases and attempt budget. There is no global claimant or blanket World sink.

All included operations require the actual Social sink. Four request operations
also require `PgProjectionOwner<SocialProjectionRouting>` and the actual durable
Notification transport, including legitimate current-policy suppression. Block
and unblock additionally require actual World application after Social withdrawal.
Any required-leg failure prevents global acknowledgment. Partial successes are
safe to retry under the original immutable identity; no synthetic baseline or
successful fallback is installed. Final success/failure uses the unchanged
repository's live lease-token/attempt fencing and bounded retries.

Claim uses PostgreSQL clock, 30-second lease, at most eight concurrent jobs,
at most eight total attempts, SKIP LOCKED and the retained 2..128-second retry
backoff. Exhaustion touches only included sealed families. Classification plus
all delivery legs are bounded to ten seconds, with five-second claim and final
settlement limits. SQL transactions have two-second statement, 500ms lock and
three-second idle limits. Source locks are released before IO. Outbox ticks
are 500ms with skipped missed ticks. Shutdown drops SQL transactions and aborts
owned batch tasks; ambiguous effects retry, never become successful cancellation.

The independent one-second expiry loop emits at most four actual DB-clock
request/Intro/grant/claim/birthday deadline changes and drains at most eight
sealed deadline signals per round, with a 30-second round bound. It does not
acknowledge command jobs. Unexpected storage/expiry/publisher errors propagate
to Root supervision with a fixed diagnostic, no private values. Per-command
retryable delivery errors use normal queue backoff instead.

## Validation and remaining limits

The focused test compiles the exact owned source by path until Root owns its
public export. Seven ignored native cases require the existing marker/inode/
UID/socket/system-identifier guarded Social fixture and valid private Concert
TLS material. They cover five pure-Social mutation families; unsupported jobs
and exhausted foreign leases; all four request projections followed by actual
Notification transport refusal; Social publisher retry; actual database expiry;
World receiver readiness/closure/shutdown; and cancellation of in-flight tasks.
A recording publisher is explicitly synthetic. Test command/authority setup is
retained-storage fixture setup, not an acquired OwnerGuardScope or runtime seed.
Failed/leased fixture jobs are explicitly retired only after assertions for
isolation; that teardown is never counted as delivery success.

No compile/native result is yet claimed. Allocation from backend SL is required
before Cargo, PostgreSQL or shared target access. Source and runtime SQL/fixture/
contract hashes, exact commands, exit codes, resource release and limitations
will be recorded after execution. No implicit resource extension or cache cleanup.

Positive real Notification delivery, actual World consumer application, complete
current migration composition, startup supervision and public HTTP/WS/cache/IN03
remain open for Root's combined runtime qualification. Prior Social 10-pure/
80-native results belong to their exact earlier dependency/source snapshot.


The World event accessor is an explicitly supplied Root-authored dependency:
original commit `fde7b682`, only five lines in World invalidation's public module.
It is imported without authoring existing World paths. The worker's block scope
uses its actual bridge event, with a bound UUID in SQL at all three stages.
The foreign-job case executes the exact production claim predicate to show a
current-event block is selected and a foreign exhausted lease remains untouched;
this is not actual World-application evidence. All seven test cases remain.
