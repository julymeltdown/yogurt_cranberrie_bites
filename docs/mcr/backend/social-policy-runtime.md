# Social policy factory and recovery

Status: **Corrected full factory/recovery cohort PASS; public WS/FE acceptance open** (2026-11-05 KST).

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


## First allocated current-schema round (not qualified)

The 2026-11-05 KST 02:29 round froze source `b895c200` after retaining an initial
compiler exit 101: the retained module harness needed its existing library World
re-export. The next Cargo invocation exited 0 and froze exactly three executables.
The target was released at 02:31:05 KST with zero readers and no reentry.

Nine pure checks and actual SQLx application of **all 45 current Concert migrations**
passed. The first eight-case kernel run exited 101 (seven passed): the retained
unrelated `setConcertSettings` storage fixture lacked migration 0021's operations
deadline. Harness-only diagnostics within the original copied-test clock retained
that failure and excluded it solely to find additional issues. Delivery had six
passes and three failures: their explicit foreign `closeConcertEvent` storage
fixtures lacked migration 0018's lifecycle deadline. The factory's membership
refusal case passed; its held-baseline writer used UPDATE on a generation row
not yet visible outside the held transaction, so it returned no row rather than
blocking. No schema or native-witness guard was disabled or amended.

A final clearly labeled **DIAGNOSTIC_ONLY** run excluded those known fixture cases.
It passed seven kernel checks, the membership factory case, an actual pending
expiry seed and fresh-owner publication after physical PostgreSQL restart.
Postmasters changed `53688 -> 54603`; persisted counts were `4|4|22|2|43`, including
exactly two unpublished non-outbox signals. Prepared transactions were zero at
all three final restart/cleanup boundaries. This does not qualify the complete
source or replace the preserved failed cohorts.

All four private clusters were removed. Cumulative copied-native execution was
30.8242 seconds within the original 360-second allowance. FULL release occurred
at 02:32:55 KST, 109.914 seconds after target release; independent guard and all
owned runtime processes exited. Historical artifacts and positive target growth
**27,181,056 bytes** remain charged; no target reentry occurred. Native fixtures
are now corrected in source (explicit local operations/lifecycle storage deadlines
and a contending generation UPSERT); rebuilding/requalifying requires a fresh
allocation. Status remains **IN_PROGRESS; final cohort NOT_RUN**.

Evidence is retained under `/private/tmp/mcr-social-recovery-so50cci0`:

| Artifact | SHA256 |
| --- | --- |
| `all-artifacts.json` | `66a09826bc4505db205953f5a816a766b40175b2dbaa69dff132a9cdaa86f4e5` |
| `native-report.json` | `f5f0d3274b817831557ddcf01998976b0ea46df2943b504bedc7dcbc1392a56d` |
| `diagnostic-native-report.json` | `0367e4450dba9dfb2cd2dbb9f2e60aea6d987c7fea5c8fe2a3698fbc2d5ab8c1` |
| `diagnostic-factory-native-report.json` | `65d192367a85a05a7663b9de5762fcd921d0a64eceb9ba16c0fbfa195ab028b4` |
| `diagnostic-restart-native-report.json` | `0bce6975906d0b9cadaef2b9f5c6ff11c2ab616a7cd1009dd182cbd0983041d5` |
| `target-release.json` | `0fb3a2e929c9ead719062c61e29156618c1df877e86b88cf23b1ae602413d174` |
| `full-release.json` | `16f2ad84593a4dd695cbae5dd97a81ed9eb1ca4084fe3cf3c2be78ec94897a83` |


## Corrected full cohort, 2026-11-05 KST

Source `46defd2346e2e73641c552b408623ce357500c15` rebases the unpublished own
changes onto `cf1ca629a33ece977f8b5b1b6faf6152b35e5dbd`. The earlier failed cohort
and diagnostics above remain historical evidence. Their failures were fixed in
source, then the complete current focused inventory ran without the diagnostic
exclusions that had skipped failing delivery or factory cases.

Offer `SOCIAL-SHARING-MEDIA-202611041753Z` admitted at 02:54:31.276213 KST.
One locked offline Rust 1.88 `cargo test --no-run` built the five integration
executables (`social_live`, `social_delivery`, `social_policy_runtime`,
`bootstrap_features`, `sharing_deletion_runtime`) and actual non-test Concert
binary, exit 0. Dev/test debug and incremental were zero; both Rust flags and
wrappers were unset; contracts-build opt-level=2, jobs=2. All six copied binaries,
full tracked backend/contract source readset, actual complete Auth/Concert SQL,
tool hashes and argv were frozen outside the shared target. Target release at
02:55:47.579558 KST recorded compiler/handles/readers zero and no reentry.

The corrected Social round passed all 8 pure live tests, 1 pure factory lifetime
test, actual full SQLx migration initialization, 8 focused kernel cases, all 9
delivery cases, both real factory cases, pending-source seed and fresh-owner
recovery after a physical PostgreSQL restart. The rebased full Concert ledger has
47 migrations, each matching the frozen SHA384 set. Actual postmaster PIDs changed
77494 to 78647; persisted counts were `25|25|50|2|88` (command receipts, outbox,
policy signals, exact unpublished independent signals, policy revision sum).
Prepared transaction counts were zero before restart, after recovery and teardown.

This was followed sequentially by the actual Sharing factory/Auth worker round
in `sharing-deletion-runtime.md`. Four C03 component cases also passed. The total
copied cohort exited 0 in 33.475 seconds within the original cumulative clock.
No World/public listener or real frontend acceptance is inferred.

All PG roots were removed; runtime processes were empty; the independent guardian
exited 0. FULL release at 02:58:36.864049 KST was 169.284 seconds after target
release. The peak whole job was 4,918,493,184 bytes including previous retained
work, checkout, shared Git, positive target growth, copies and PG. New positive
target growth was 65,683,456 bytes; nonshrinking historical target carry is now
339,120,128 bytes. No old failures, target files or shared caches were removed.

Before native startup, a speculative extra 205 reservation caused two conservative
capacity refusals with no PG children. 205 then confirmed actual reservation zero
and its proposal was 1.5 GiB. The owner voluntarily narrowed unused future growth
to 536,870,912 bytes, with stricter effective total new-growth ceiling 881,147,904
bytes; no historical carry or original clocks changed. Fresh free bytes were
20,639,944,704, leaving 9,902,526,464 after the original OTHER 8 GiB envelope,
an additional conservative possible 205 1.5 GiB and own remaining 512 MiB.
The original whole/new upper bounds remained 8/2 GiB. FULL releases all outstanding
future reservation; retained evidence remains charged as already consumed bytes.

Retained evidence root: `/private/tmp/mcr-social-sharing-f_ut5nyi`.

| File | SHA256 |
| --- | --- |
| `all-artifacts.json` | `e517ea4a4ab81132ea3a65f2f272eaab0b7d4049e9b6953967f1fd20874143ab` |
| `social-report.json` | `17b862bc0fc39d05847dd22d3a37312a69f43c4b7f7a70f0fde02122dc257f85` |
| `features-report.json` | `b4f4b2839951b4479cd87a7ea7432c75615907f22e808770beb058f68135da61` |
| `sharing-report.json` | `653d3019eb4883568422ff40bb73d0e8b3e03887e62f7c3babdfc02e3540c188` |
| `cohort-report.json` | `40ae1de9b6653edc1a2bb9c64ae79af34ffcbdd0955ae9114a129970ca0fe209` |
| `target-release.json` | `a45d7ab5c3fd481701557622ce74778bdeeca51ec419d74ea8a3cd12ea11e71a` |
| `full-release.json` | `0d7c7f3c0fece0b9a08e0e2f76c723588d333ace7c678c4a70a3cd7b00bf2090` |
| `narrowed-native-reservation.json` | `b8d35dc5f28744fa8fa3a998e51551165033a61c0d5f41c64ca93f20ad52e6a7` |
