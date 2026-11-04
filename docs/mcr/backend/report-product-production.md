# OPS-PRODUCT-01 — original USER report production leaf

Status: IN_PROGRESS. Base: `cb944f7210f6c55fdf1d0e897c96ea7ef0ba8325`, freshly
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
revoked target and unavailable routes/media, exact acquired-scope identity,
serialization distinct from terminal completion, and zero prepared work after
completion. These are internal native capability checks, not public Gateway,
physical-device, multiuser IN03 or whole product acceptance.

Rust/SQL/native execution is NOT_RUN pending a fresh finite named resource offer.
No old target/runtime lease or cache cleanup is reused. All failures, exact source
pins, commands/exit codes, output growth and private cleanup evidence must be
preserved during that validation before delivery is qualified.
