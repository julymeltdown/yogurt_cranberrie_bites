# OPS-PRODUCT-01 — original USER report production leaf

Status: OPS-PRODUCT-01 and REPORT-STARTUP-01 are published with retained developer
validation. OPS-ORIGINAL-CONTINUATION-02 is delivered by the canonical publisher
receipt for master `7d5d91bd1121b1a95b2302ca02e9224019a5deaa`. BE36-OWN-REPORTS-01
is IN_PROGRESS, functional validation NOT_RUN. This does not accept whole BE36/BE38.
Original OPS-PRODUCT-01 base: `cb944f7210f6c55fdf1d0e897c96ea7ef0ba8325`, freshly
fetched from `julymeltdown/mcr` master. Isolated existing worktree:
`/Users/lhs/Desktop/mcr-worktrees/be-operations-runtime`, new task branch
`codex/mcr-be-OPS-PRODUCT-01`. The prior Operations branch and all frozen evidence
remain preserved. This assignment implements one original BE36 capability;
it does not accept whole BE36/BE38/BE39 or activate public Gateway routing.

The actual goal API reports the aligned historical BE36/BE38 objective BLOCKED,
with obsolete old-base/worktree metadata. It has not been reset, falsely resumed
or completed. The latest human no-review policy supersedes old goal/checkout
review clauses. The developer owns validation, fixes, documentation and committed
handoff; the named Root publishing developer owns serial integration, relevant
combined checks and remote delivery. No king/SL technical review or acknowledgment
is required. Waiting for resources or publication is distinct from a product or
ownership blocker. No model/reasoning override or new worktree/chat is introduced.

## Exact authored ownership

- New `product_rpc/report_admission.rs`: single typed `ReportOperation::Create`,
  exact original POST `/api/concert/events/{eventId}/reports` operation
  `createConcertReport`, keyed original JSON body and strict C04 input limits.
- `product_rpc/mod.rs`: only the Report variant, contract ID, route dispatch,
  request validation and mutation classification. Existing Age/Intro/Inbox/
  Social/Feed/Party/media behavior remains untouched.
- `product_rpc/query.rs`: only the typed report admission call.
- `product_rpc/owner_media.rs`: explicit unavailable arm for Report.
- New `operations_live/report_product.rs` and its one-line module export.
- New `tests/report_product_admission.rs`, `tests/report_product_native.rs` and
  this document. Existing helper files are read-only.

Root retains main/router/config dispatch, callback/outbox integration and
publication. No SQL, proto/contract/registry, manifest, lockfile, CI, frontend,
World or Social source is changed. Previous migration 0047 and transaction
witness/settlement implementation are consumed from the published base.

## Leaf API and behavior

`operations_live::report_product::ReportProduct::new(Arc<OperationsRuntime>)`
implements the existing `ScopedProductProducer`. Root supplies its existing
runtime and already-acquired canonical `Arc<OwnerGuardScope>` with the original
`VerifiedProduct` and full Product frame. The adapter validates its exact route,
compares the full GuardBinding derived from that original verified parent/context
with the held scope, and delegates the same frame and scope to the existing
OperationsRuntime. It constructs no second acquisition, authority fence,
registry, ledger or delivery capability. Root's existing `ScopedProductOwner`
and delivery service retain serialization and actual terminal ownership.

Admission decodes original JSON bytes directly into a closed struct, preserving
duplicate-key rejection. Canonical nonzero UUIDs, exact JSON content type,
idempotency key, 64 KiB transport bound, required fields, reason enum, 2,000 Unicode
characters and retained domain control-character/media bounds are enforced.
It never rewrites signed bytes. Only USER with zero media is currently available;
other valid targets/media are explicitly unavailable. No report read, moderator
queue, settings, role, notice or other privileged route is added.

The existing runtime retains all business behavior: actual current Auth/Friend
actor and target policy, event admission, server report gates/quota/deduplication,
idempotent command receipt, transaction witness, private audit/outbox and safe
response. Member admission never grants an operator role. This path needs no Age,
operator, reviewed-notice or READY-media provider.

## Source provenance and validation boundaries

No business logic is copied. Admission follows the frozen
`contracts/generated/mcr-domain.openapi.json` `Domain_admin_CreateReport` and
`contracts/domain/v1/operations.json` original operation. Existing domain
CreateReport validation is reused. Existing dependencies and resolutions suffice.

The new native fixture reads unchanged `operations_live_support/{capture,owners,
environment}.rs` and `product_feature_admission_support` at the base. Its private
Concert database identity checks adapt the former environment checks into the
owned new test file, requiring only `mcr_concert_report_product` on the marked
private Unix-socket cluster. This allows the real published NativeResolverUnion's
dedicated-database rule. No ambient/live PostgreSQL URL is accepted. No child
producer is invoked; the fixture's child settler refuses unexpected child work.

Five admission cases exercise actual signed Product RPC decoding and explicitly
do not claim a recording owner proves authority. Four ignored native cases use
real Auth/Friend services and PostgreSQL, CanonicalRuntimeCore, NativeResolverUnion,
the actual ReportProduct/OperationsRuntime and Root ProductRpcService/scoped
owner/delivery service. They target original report/receipt/outbox/replay,
current Auth revocation blocked until terminal, foreign parent and rebound body,
missing event target and unavailable routes/media, suspended-target safety policy,
exact acquired-scope identity,
serialization distinct from terminal completion, and zero prepared work after
completion. These are internal native capability checks, not public Gateway,
physical-device, multiuser IN03 or whole product acceptance.

## First bounded validation and retained failure

The initial source `8501b058ee14efe83dd69a5528064cb9c8faa391` failed the exact
two-target Rust 1.88 locked/offline build with E0277 in the new native fixture's
boxed gRPC-body error conversion (exit 101, 43.798 seconds). The assigned fixture
fix is `3f79e686aec286de622e6e16809e10f45517ac4e`. Its exact two-target rebuild
passed (exit 0, 2.467 seconds). Both Rustflags variables were unset; jobs 2,
debug info 0 and incremental compilation disabled. No cache cleanup or build.rs
mtime refresh was needed.

The finite offer `OPS-PRODUCT-01-BACKEND-2G-202611041240Z` was accepted at
2026-11-04 12:40:33.162306 UTC, with its original 600-second clock unchanged.
Its explicit capacity-only amendment raised the whole cap to 2,415,919,104 bytes,
including the conservative historical carry of 1,341,550,592 bytes. The two exact
executables, Cargo JSON, 2,366 source hashes and runtime/controller hashes are
frozen at `/private/tmp/mcr-report-product-frozen-m63cqxzb/pin.json`.
The shared BACKEND target was unconditionally released with zero handles at
12:43:26.297942 UTC, before every copied test, and was not accessed afterward.

At that frozen source the five admission cases passed (exit 0). Three of four
native cases passed; the fourth failed (exit 101) because the fixture incorrectly
expected a suspended report target to be denied. The published canonical
`OwnerGuardScope::check_subject` explicitly exempts SafetySelf/SafetyInteraction
from subject eligibility/block denial, while retaining actor/session eligibility.
Operations' existing USER evidence path uses SafetyInteraction and additionally
requires an actual event-admission row. The fixture correction tests that actual
safety policy positively and separately requires a missing event target to be
denied. No production owner policy or report business logic is changed.

All original failure logs remain in
`/private/tmp/mcr-report-product-validation-roc6wfaw` and the frozen directory.
The failed native case panicked before its terminal call, leaving two prepared
transactions before owned teardown. The single private PostgreSQL 16.14 cluster
`/private/tmp/mcr-pg-fn68xa86` was stopped and its exact root removed; copied
children were reaped. Teardown does not certify native settlement. The whole
measured peak was 2,170,953,728 bytes, including historical carry, under the
amended cap; no time/free-space/cap cutoff fired. Resources were fully released
well before the original release deadline. The native report SHA-256 is
`502851b8dfc6bad0e7747590938267fd66fa4b601c2114a820c109c7313e782a`.

That failed run does not qualify the native capability. The corrected fixture
was committed as `7f0d50592022c6764173b1d6747aa0182598ae68` and qualified only
by the distinct fresh follow-up below. No old lease was reused or failed evidence
rewritten. No public Gateway or whole-task acceptance is claimed.

## Qualified corrected-source follow-up

Fresh offer `OPS-PRODUCT-01-FIX-BACKEND-202611041310Z` was accepted at
2026-11-04 22:11:32.946388 KST (13:11:32.946388 UTC), with free space
17,271,046,144 bytes, exceeding the 16 GiB starting floor. Its whole-output
allowance was explicitly amended to 2,684,354,560 bytes without changing source,
target, scope or the original ten-minute clock. All retained historical output
remained conservatively charged at 2,082,742,272 bytes; no artifacts or caches
were deleted. Fresh controller/baseline, new target output, copies, private
database cluster, logs and temporary files were charged in addition.

Validated code source: `7f0d50592022c6764173b1d6747aa0182598ae68`.
Source ancestry after the published base: `50fec38b2e41a77c9e431e039affc802a720857a`
→ `8501b058ee14efe83dd69a5528064cb9c8faa391`
→ `3f79e686aec286de622e6e16809e10f45517ac4e`
→ `7f0d50592022c6764173b1d6747aa0182598ae68`.
The evidence successor changes only this document; no Rust or SQL changed after
the successful frozen-source execution. No dependencies or lockfiles changed.

Exact build command, from the assigned worktree's `backend/` directory:

```sh
env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
  RUSTUP_TOOLCHAIN=1.88.0 CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  /Users/lhs/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin/cargo \
  test --locked --offline -p concert-service \
  --test report_product_admission --test report_product_native \
  --no-run --message-format=json -j 2
```

The controller also selected the same toolchain's rustc/rustdoc and prepended its
binary directory to PATH. Build passed, exit 0, 5.091 seconds; no mtime exception
was used. Exact Cargo JSON, commands, toolchain/runtime/actual fixed-path PKI
executable hashes and resource receipts are retained in
`/private/tmp/mcr-report-product-retry-0oyd18ln`.

Exactly two executables were copied in 0.830 seconds and frozen with 2,366
backend/contract source hashes, including all consumed fixture helpers and SQL
migrations, plus runtime/controller pins:
`/private/tmp/mcr-report-product-frozen-wdi33fse/pin.json`.
Pin SHA-256: `eee0260226b77aa8053ea38fe2252d0b986a0ac63c49aa17afc309f03fc7fe19`.

| Frozen executable | Bytes | SHA-256 |
| --- | ---: | --- |
| report_product_admission | 7,625,264 | `0580919f55a471268cbac0bb75a08891ff9cd6a82550d2056880a679bae2b80c` |
| report_product_native | 44,866,448 | `d6f8d18920f2084360c286c2e2eaa77c478138d5938b1df8f493aaab6cb8ff1a` |

The target was unconditionally released with zero handles at
22:12:51.596814 KST, before every copied test. It was not accessed afterward.
The actual copied commands and results were:

```text
/private/tmp/mcr-report-product-frozen-wdi33fse/report_product_admission --nocapture --test-threads=1
exit 0; 5 passed; 0.500 seconds
/private/tmp/mcr-report-product-frozen-wdi33fse/report_product_native --ignored --nocapture --test-threads=1
exit 0; 4 passed; 2.525 seconds
```

The five admission cases prove the exact C04 route, typed limits, original signed
frame and refusal boundaries. The four native cases prove actual original
USER/no-media report/receipt/outbox/replay through ProductRpcService and the same
canonical scope; current actor revocation waiting until actual Root terminal and
then denying replay; foreign actor/session, expired parent and post-admission body
rebinding refusal; and the deliberate suspended-target safety behavior, missing
event-target denial and unavailable privileged/read/media routes. Actual terminal
completion is distinct from response serialization; ordinary membership never
grants an operator role.

One owned PostgreSQL 16.14 cluster served exactly the private Auth/Friend/Concert
databases. Migration ledgers matched all pinned sources (18/18/40). A new
postmaster preserved witness, receipt and outbox digests and the migration ledgers;
all retained native witnesses resolved COMMITTED. Actual prepared transaction
count before owned teardown was **0**. All captured PostgreSQL descendant PIDs
were absent after stop, copied children were reaped, and exact owned root
`/private/tmp/mcr-pg-l907wiu4` was absent. Teardown itself is not settlement proof;
the successful native terminal assertions and observed zero count supply that
evidence.

Measured whole peak: 2,412,408,832 bytes, including all conservative historical
carry and 183,427,072 bytes of new positive target output, below the explicitly
amended cap. Free space at completion: 18,588,078,080 bytes. No time, free-space
or cap stop fired. Full release was recorded at 22:16:59.552661 KST, before the
original work stop 22:20:32.946388 and hard release 22:21:32.946388 KST.

Native report:
`/private/tmp/mcr-report-product-frozen-wdi33fse/native-report.json`, SHA-256
`c7bf2bb456a2b127e1119c28bab09956e13bf88c63c2709c3f4adeb03c330385`.
Resource report SHA-256:
`3a7255dbfaea3cb60a302aa3eaad18a2c3c3c4289f1249be0635b986c7410c96`.
Admission log SHA-256:
`f576ae48e2d3cd4efd65a0f96917c10b921e0a81619fc439b4ca01b66bef6f14`.
Native log SHA-256:
`6fa6f53e7a3005765de64b0585bfa5fb40c3d32af6f6aff5653fb56fab3844d1`.

This qualifies only the original report product leaf and its real internal
owner/delivery flow. Public Gateway dispatch, Root startup composition and
publication remain the named Root publishing developer's scope. Broad
BE36/BE38/BE39, media, moderator access, public multiuser IN03 and product
acceptance remain open. Direct committed handoff needs no king/SL review or
acknowledgment; the implementation owner must verify actual remote delivery.

## OPS-ORIGINAL-CONTINUATION-02 — original BE36 real mutation boundaries

Base: `bcbcf3edf4eba23a3011358018142574077d1426`, fetched from delivery master.
Same isolated worktree, branch `codex/mcr-be-ops-original-continuation-02`.
Only the assigned native test and this document change. All production code,
helper modules, migrations, contracts, routers and startup composition remain
read-only. The aligned persistent goal still reports BLOCKED; this bounded work
does not reset or complete that broad historical goal.

Original BE36 requires duplicate/spam handling, idempotent server mutations and
reporter privacy. Three additional ignored native cases exercise those behaviors
through the actual signed Product RPC, original canonical Auth/Friend authority,
real OperationsRuntime/repository and authenticated Root delivery terminal:

- Two concurrent distinct keys for the same reporter/target must produce one
  successful report, one safe failed duplicate receipt, one outbox item and one
  audit entry. Replaying the failed key must return the same safe receipt without
  more effects. Concurrent deliveries settle separately before checking that all
  three owner databases have zero prepared work.
- Ten distinct admitted real Auth targets receive ten real signed reports. The
  eleventh must return 429 without another report, receipt, outbox item, audit or
  budget charge. Existing same-key replay remains available at quota. A changed,
  freshly signed body under that key must return 409 without additional effects.
- An event-specific test-only constraint in the identity-checked disposable
  Concert database rejects the outbox insert after report/audit mutation. The
  actual error must be safe 503, with report/audit/receipt/budget fully rolled
  back. Removing that constraint and retrying the same key must succeed once.
  No fault is added to production code or migration history.

Initial status: source authored; native compilation/execution NOT_RUN. The existing four
native regressions stay in the same binary. A fresh finite resource allocation,
exact executable/source freeze, developer-owned execution and cleanup, committed
handoff to the named Root publisher and fresh remote proof remain required.
These checks do not establish moderator/evidence routes, operator authority,
public Gateway delivery, multiuser IN03 or whole BE36/BE38 acceptance.

The first bounded build at source `9211e8a04d82d91d66de77756a2a68da506de21d`
failed before test compilation (exit 101, 1.616 seconds): a newer cached Concert
build-script executable in the shared MEDIA target requested
`authority/v1/deletion_source.proto` from this older checkout. The pinned local
build script did not reference that file. Fresh published master
`fa6f3f72addedd8a8d9b9486a4e2ed8be56e0ec6` contains both the newer reference and
the tracked proto. The developer refreshed only this task branch onto that
published revision; no shared source, target cache or peer worktree was repaired
or deleted. The failed source and evidence remain preserved.

Attempt evidence: `/private/tmp/mcr-report-original-85d8fxhs`. Actual ACCEPT:
`2026-11-04T15:05:05.219515Z`; target release with zero handles:
`2026-11-04T15:05:09.003812Z`; FULL release:
`2026-11-04T15:05:09.047179Z`. No executable was frozen and no PostgreSQL/native
or browser work started. The independent guardian was reaped. Positive target
output was 86,495,232 bytes; new peak was 92,999,680 bytes. Full current common
Git (1,085,927,424 bytes) was charged in addition to all retained historical
output/failed-proof carry, giving a whole peak of 3,663,241,216 bytes below the
explicit 4 GiB ceiling. A fresh allocation is required before any target reentry.

## Continuation qualification and retained resource failure

The refreshed source is `ba05b079fe510d29c4e386422c62c8fecb24f1ae`, with the
two-file implementation/documentation patch `fa88de7a17b0c62bf050e89e7fe0523e9250910f`
on published base `fa6f3f72addedd8a8d9b9486a4e2ed8be56e0ec6`. No production code
change was necessary: the new native cases confirm the existing server invariants.

Offer `OPS-ORIGINAL-RETRY-MEDIA-202611041515Z` accepted at
`2026-11-04T15:15:29.291354Z`. Its explicit byte-identical timestamp refresh of
the own-worktree Concert, Notification and Gateway build scripts is recorded in
`/private/tmp/mcr-report-original-retry-fl6hk8qc/byte-identical-build-script-refresh.json`:
SHA-256, size, mode, UID/GID, inode/device and before/after times. No content or
permission changed, and no target cache was deleted.

The exact single-target command passed, exit 0, 45.524 seconds:

```text
cargo +1.88.0 test --locked --offline -p concert-service --test report_product_native --no-run --message-format=json -j 2
```

Both Rustflags variables were unset; dev/test debug info 0, incremental disabled,
and explicit dev/test package overrides used optimization level 2 for
`api-contracts` and `contracts-build`. The complete actual argument vector and
Cargo JSON are retained in that retry directory.

This allocation nevertheless **FAILED its storage limits**. A final linker burst
produced 629,411,840 positive target bytes; after the executable freeze, observed
NEW output was 681,553,920 bytes, above the 512 MiB allowance. The controller
lacked a final pre-freeze limit check and copied the executable before its next
observation. Qualification was stopped during source hashing, before any copied
test executable or PostgreSQL fixture ran. Target release with zero handles was
`2026-11-04T15:16:17.587916Z`; actual FULL, after the guardian was reaped, was
`2026-11-04T15:16:17.909376Z`. No target reentry followed. Preserve the raw report
and `corrected-resource-report.json`: the latter retains the previously recorded
carry floor despite a lower current Git size, giving whole peak 4,346,187,776
bytes, above 4 GiB. The later offer does not retroactively approve this failure.

The complete frozen artifact was retained outside the released target:

- Pin: `/private/tmp/mcr-report-product-frozen-knmywgk9/pin.json`, SHA-256
  `0a63e63761380c0a27be037ed2e01908a555e917efb0818beaa25f2de6857007`.
- One executable: 44,952,048 bytes, SHA-256
  `2780f9ca9f1cb9333d15e697ba2a6d0e157b2221c17a5a15887d816ffea10582`.
- 2,392 source/helper/SQL/proto/contract pins, seven runtime binary pins and
  retained Cargo JSON were verified before subsequent execution. Supplemental
  tool/controller/config pins are retained outside the repository.

Fresh offer `OPS-ORIGINAL-COPIED-NATIVE-202611041523Z` authorized only the existing
copy, with no Cargo, target access, rebuild or further executable copy. ACCEPT:
`2026-11-04T15:23:32.613438Z`; work stop `15:25:32.613438Z`; hard FULL
`15:26:32.613438Z`. Independent guardian preceded children. Previous failures,
full Git and their historical peaks stayed charged: monotonic carry
4,348,346,368 bytes, NEW allowance 128 MiB, explicit whole allowance 5 GiB.

All **seven native cases PASS**, exit 0, 6.161 seconds including controller
invocation (test harness 5.54 seconds). This includes all three new mutation
boundary cases and the four retained authority/delivery regressions. No startup,
admission, browser or public Gateway suite was rerun. Private PostgreSQL 16.14
served exactly the owned Auth/Friend/Concert databases; migration ledgers matched
18/18/42 source files. A new postmaster preserved witness, receipt and outbox
digests, all native witnesses resolved COMMITTED and ledgers stayed unchanged.
Prepared transactions before teardown were **0**. All six captured PostgreSQL
PIDs were absent, and exact owned root `/private/tmp/mcr-pg-7resbv1n` was absent.
Teardown itself does not certify settlement; the actual terminal checks and
observed zero prepared count supply that proof.

Actual FULL: `2026-11-04T15:23:41.555939Z`, after guardian reap, 8.943 seconds
after ACCEPT. NEW peak 90,992,640 bytes was below 128 MiB; whole peak
4,439,339,008 bytes was below 5 GiB. Maximum observed owned processes: 20/48.
Free space at completion: 26,635,055,104 bytes. No resource remains held.

Native report: `/private/tmp/mcr-report-product-frozen-knmywgk9/native-report.json`,
SHA-256 `9192eb1a947a14efd7620b9eb66dfaa9c128594dd64074cd44852a4655e674c7`.
Copied-only resource report:
`/private/tmp/mcr-report-native-only-ljs9slpc/resource-report.json`, SHA-256
`1762bc3174fce27841546d6721e76fbbd27eee85a376bbf49d9addd1d7c275e2`.
Native log SHA-256:
`cb9f0e77478669ac14e5ddf58af0ebb0b5cd27a49beb476ea30a9821458dbd93`.

Developer validation is complete for this bounded continuation. Direct committed
handoff routes to the named Root publisher without a king/SL review or ACK gate;
the developer must still verify fresh remote delivery. The broad historical goal
remains truthfully BLOCKED, not reset or falsely completed. Moderator/evidence
routes, operator authority, public Gateway delivery, IN03 and whole BE36/BE38
remain outside this scoped qualification.

## BE36-OWN-REPORTS-01 — safe caller-owned report status

Functional follow-up on published base
`7d5d91bd1121b1a95b2302ca02e9224019a5deaa`, same isolated worktree, branch
`codex/mcr-be-BE36-own-reports-01`. The preceding continuation is delivered by
Root's canonical publisher receipt, with source/delivery mappings
`fa88de7a` → `205d2d3c`, `ba05b079` → `5c6133a0`, `5f2bd7cb` → `d06cc139` and
both final postimages exact. The latest direct human packaging override cancels
the obsolete per-consumer readback/ACK, proof-only commit and new-goal requirements.
No duplicate proof was produced. The broader existing goal stays truthfully
unfinished; performance qualification is deferred under the latest direction.

The original frozen `listOwnConcertReports` endpoint is
GET `/api/concert/events/{eventId}/me/reports`, SELF, READ, no idempotency key,
`admin.OwnReportQuery` → `admin.ReportList`, 200, no-store. The actual retained
OperationsRuntime and PgReports::own already provide current canonical member
authorization, caller-scoped rows, safe status projection, authenticated cursor
binding and native read settlement. This slice exposes that existing behavior;
it requires no operator role, target/media provider or new authority lifecycle.

All authored changes stay within the eight assigned files: typed Own admission,
the existing leaf and startup overlay, their three test files and two Report
documents. Root's generic Report route/query wrappers already call these owned
methods. No central router, registry, proto, SQL, manifest, lockfile, helper,
frontend or production main change is needed.

Admission preserves the original bodyless signed frame and exact event path.
Its closed raw query accepts only canonical `limit` 1–100, Report `state`, and
bounded opaque `cursor`, with duplicate/unknown fields and ambiguous encoding
rejected. Cursor authenticity, caller/event/filter/limit binding and current
authority remain server-runtime decisions. The leaf checks the selected exact
Report operation against the original held GuardBinding and delegates the same
context/frame/scope. Startup uses its existing shared core/fence/delivery and
routes both typed Report operations; unrelated operations still delegate.

New signed admission cases cover original-frame fidelity and rejected payloads,
private filters, privileged routes and malformed query shapes. The recording
owner still refuses and supplies no authority evidence. The existing startup
overlay case now includes Own dispatch. Two new real native cases create actual
reports for two real callers, verify private paginated own status and cross-user/
filter/limit cursor rejection without command/audit/outbox side effects, and
prove an actual Auth revocation writer waits for the read's authenticated Root
terminal before a subsequent read is denied. Seven earlier native regressions
remain in that binary.

Status: implementation authored; compilation, native execution and current
startup/admission regression checks NOT_RUN. Relevant functional validation,
documentation, committed direct Root handoff and developer-owned fixes remain
required. Moderator detail/evidence, operator routes, public Gateway/IN03 and
whole BE36/BE38 acceptance are not established by this slice.
