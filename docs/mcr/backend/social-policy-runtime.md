# Social policy factory and recovery

Status: **IN_PROGRESS; native validation NOT_RUN** (2026-11-05 KST).

The delivery base is `30a9c07bb1c0b0a7d1bd76d6c85dfc072bcd3694`.
Root explicitly delegated the complete Social-only
`backend/services/concert-service/src/config/social_policy_runtime.rs`, its new
`tests/social_policy_runtime.rs`, this document, and the exact expiry caller /
regression slices to the Social implementation owner. Root retains public WS,
main, core/router and publication ownership. No frozen source, migration,
manifest, lockfile, external contract or existing World policy implementation is
modified.

`SocialWorldPolicy::compose(pool)` returns the incumbent `transport`, `sink` and
`publisher: Arc<dyn SocialPolicyPublisher>`. The exposed Arc is the exact value
cloned into the sink. Root supplies `Some(policy.publisher.clone())` to
Auth207's qualified `LiveDeletionOwners.sharing_publisher`; this is a typed
composition surface, not evidence of a completed production join. Auth207's
source checkpoint is `932c867c5df00887487f08c8841a0a7d3906d0d9`; its qualified
cohort intentionally supplied no Sharing publisher.

Construction preserves one SocketTransport registry. The sealed publisher is
created before attaching the held-baseline owner, so its registry-only handle
cannot cycle through the baseline back into itself. It retains the same durable
membership coverage query, the 800-connection bound and closed reason-only
publication mapping. Uncovered active membership makes publication retryable.
An event with no serving membership can complete through its durable generation;
reconnecting streams still require the held-baseline path. This change creates
no listener, task, secondary registry, authority or success fallback.

Prepared checks in `tests/social_policy_runtime.rs` cover the Arc lifetime without
IO, actual SQL membership coverage refusal through both factory and supplied-Arc
sinks, durable retry after an explicitly marked fixture disconnect, fresh-factory
baselines, a held baseline excluding a concurrent generation writer, and
isolation from another event's membership. Native checks use actual PostgreSQL
and the production sealed publisher/registry, but their stored event, identity,
authority and membership rows are explicit fixtures. They do not certify actual
Auth/admission, a socket disconnect, public frame delivery or browser cache state.

The associated expiry caller consumes `PendingPublicationRound`: a retryable
publisher stays pending while later bounded rounds reach independent tuples;
source/storage errors still escape the supervisor. Its new regression runs the
actual supervisor across refusal and retry without manufacturing an ACK or
retiring the foreign job. Existing seven delivery regressions are retained.

Qualification command (requires the allocated target and approved runtime lease):

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  -p concert-service --test social_live --test social_delivery \
  --test social_policy_runtime --no-run --message-format=json
python3 backend/services/concert-service/tests/social_live_support/native.py \
  --policy-recovery --artifacts /absolute/copied/all-artifacts.json \
  --report /absolute/copied/native-report.json
```

Use Rust 1.88, dev/test debug=0, incremental=0, both rustflags variables unset.
The runner requires three copied executables and current committed source hashes;
actual full Concert migration SHA384 receipts must match the SQLx ledger. All
Cargo readers must stop and the target lease must be released before copied
checks start. No allocation has been consumed by this prepared source notice.
Real full-stack/WS/FE acceptance remains open.


Current-schema fixture boundary: raw Intro insertion is prohibited by migration
0049's actual native-witness requirement. Recovery fixtures therefore use only
an actual future request deadline and do not forge an Intro or disable its guard.
The focused eight kernel cases exclude the historical selected-schema Intro
fixture test and include an actual two-second publisher-timeout/deferred/retry
case preserving exact signal identities. The legacy selected-schema runner keeps
its historical 15-case inventory separate. Full migration-ledger qualification
does not turn these storage fixtures into canonical admission or provider proof.
