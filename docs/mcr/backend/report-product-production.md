# OPS-PRODUCT-01 — original USER report production leaf

Status: OPS-PRODUCT-01 and REPORT-STARTUP-01 are published with retained developer
validation. OPS-ORIGINAL-CONTINUATION-02 is IN_PROGRESS; its new native cases are
NOT_RUN pending a bounded resource allocation. This does not accept whole BE36/BE38.
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

Status: source authored; native compilation/execution NOT_RUN. The existing four
native regressions stay in the same binary. A fresh finite resource allocation,
exact executable/source freeze, developer-owned execution and cleanup, committed
handoff to the named Root publisher and fresh remote proof remain required.
These checks do not establish moderator/evidence routes, operator authority,
public Gateway delivery, multiuser IN03 or whole BE36/BE38 acceptance.
