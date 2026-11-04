# SOCIAL-DELIVERY-01 production delivery composition

Status: CONTRACT_READY; implementation and focused native qualification delivered.
Root combined production startup and positive fanout qualification remain open.
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

Claim uses PostgreSQL clock, 30-second lease, at most four concurrent jobs,
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

The original source-first candidate claimed no compile/native result. Allocation from backend SL is required
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

The claim batch is four, preserving headroom in AppConfig's actual eight-slot
pool for the publisher's membership check and independent expiry/HTTP work.
No connection-limit configuration is changed. This bounds concurrency rather
than holding eight source transactions while their publishers request the same
exhausted pool. The five-family test spans two actual claim rounds.


## Qualified focused delivery round

Tested source: `34d53a519b631183d723ec37732fbaa2e787dda6`.
Own implementation ancestry: `291215b7` (new supervisor/tests/document),
`1547a2ca` (actual World event scope), `34d53a51` (four-job pool headroom).
Root's exact five-line World accessor source
`fde7b682108371fcde1f25079b850d1b07b39618` was imported as `0446e22d`;
Root already owns that dependency and must not duplicate its publication.
The nine previously qualified Social paths and all SQL remain unchanged.

The first unused offer returned before acceptance because the fresh 16 GiB
start gate was unmet; no compiler, target access or PostgreSQL workload began.
Fresh allocation `SOCIAL-DELIVERY-01-MEDIA-202611041310Z` was actually accepted
at **2026-11-04 22:11:55 KST**, with 17,267,355,648 free bytes and zero target
handles/compiler processes. Its fixed cutoffs were 22:21:55 KST for compilation
and freeze, 22:28:55 KST for copied work and 22:31:55 KST for full release.
The native resource prerequisite was the actual QA08 server release at
22:09:43 KST. This was capacity scheduling, not technical approval.

Exact build command, using Rust/Cargo 1.88:

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  -p concert-service --test social_delivery --no-run --message-format=json
```

Development/test debug was 0, incremental compilation 0, both Rust flag
overrides unset. The first build exited **101** in 6.14 seconds: Cargo marked
this checkout's domain/application artifacts Fresh, but they lacked modules
explicitly exported by the pinned source, producing 40 compilation errors.
That failed attempt and its Cargo JSON remain intact.

Root and the allocator explicitly granted one mtime-only refresh each for this
checkout's `concert-domain/src/lib.rs` and `concert-application/src/lib.rs`.
Pre/post SHA256, mode, UID/GID, inode/device, mtime and ctime are retained; bytes,
mode, ownership and inode were unchanged. The before byte count is derived from
the pinned identical source because pre-touch stat size was not separately
captured; the after size is actual stat. This is documented without inventing
missing metadata. No content change, staging, cache/fingerprint/dependency
removal, new target, flags or other timestamp refresh occurred.

The second identical build exited **0** in 36.61 seconds. One executable,
Cargo JSON, all tracked backend/contract sources, selected SQL prerequisites,
external runner and resource/input metadata were frozen; copy/freeze took
**0.58 seconds**, below the 30-second cap. The target was unconditionally
released at **22:16:54 KST**, with fresh zero handles and compiler processes,
before any copied test. There was no later target access.

One owned Unix-only PostgreSQL 16 cluster and one private database executed
**7 native checks before restart and 7 after restart: all 14 passed**, zero
failures. Both invocations exited **0**, each taking 0.83 seconds. The complete
copied workflow exited **0** in 5.32 seconds. The physical restart used distinct
postmaster PIDs and preserved `19|19|24|36` counts for receipts, outbox rows,
policy signals and summed generations. Prepared transactions were explicitly
**0** before restart, before final stop and in finally. Cluster cleanup passed;
fresh process inspection found zero owned runtime processes, with the owned
root and private PKI directory absent.

Full release was recorded at **22:17:30 KST**, within the original deadline.
Peak whole allocation was **1,864,577,024 bytes**, including historical retained
outputs, 180,535,296 bytes of known prior target growth, 40,943,616 bytes of new
target growth, source/copies/PKI/PostgreSQL/log/TMP and receipts. The original
4 GiB whole, 512 MiB PostgreSQL, process limits and 8 GiB continuous floor were
preserved without reset or extension. No application service or TCP listener was added.

Private evidence bundle: `mcr-social-delivery-s145ks6_`.

| Evidence | SHA256 |
| --- | --- |
| `native-report.json` | `6d3c8eef31588b3f79a5c3b390b9bf6cde1083dfc4582bd5bcb0e1a9584d3189` |
| `all-artifacts.json` | `669d85993f8b18228c761cab2d2627e98fef3aaf9e74aedc5b8a84638513391f` |
| `full-source.tar.gz` | `f0f5c2fb753ee23c04c72103164673dea3c3700862b7da68ba2a8756b9bbeb29` |
| `target-release.json` | `b9a8e305f149d17feaef467a5a88559c132f58c5f98e828ac83a0cd043245afc` |
| `full-release.json` | `2d5dde806abf93903641adfa4858abe023a56e75aa2dd0132e4f6fdf9894bb28` |
| `crate-root-mtime-refresh.json` | `0632561c3ce5d62e80ea5271a27b3fdbf92592c678eab0cb14ddcc3c21dbd7f3` |
| `runtime-inputs.json` | `624d80a926256f1d211966afe33e34ab5e8163cac66ec70fadfe885baa6b96dc` |
| copied `delivery-tests` | `f071cdc08a94155a2cda2b0cd2ca891071e7e1d84292bb5c0a8654cae5792f71` |

This qualifies selective queue scope and required-leg refusal/retry, database
expiry, readiness gating and cancellation under explicitly synthetic storage
and publisher fixtures. The actual Notification client exercised transport
refusal after real source/projection resolution. The bound-event production SQL
predicate selected a current-event block and left a foreign exhausted lease
unchanged. Neither assertion certifies actual World application or positive
Notification delivery. Current complete migrations, Root production startup,
canonical source events, real providers, public HTTP/WS/cache and IN03 remain
open. The historical aligned goal remains truthfully BLOCKED and is not reset
or falsely completed. Root is the designated publishing developer; its combined
checks and remote delivery proof remain pending until actually observed.
