# REPORT-STARTUP-01 — explicit USER report startup leaf

Status: the original startup leaf and Root mounting are published, with their
retained qualification below. BE36-OWN-REPORTS-01 extends only the owned overlay
to the original caller-owned Report status read; current validation NOT_RUN.
Historical startup scope and evidence below remain preserved.

Base: `beffe4975d5d702bbdf1d3d96458bd7846c7b70a`. Existing isolated worktree:
`/Users/lhs/Desktop/mcr-worktrees/be-operations-runtime`; new branch
`codex/mcr-be-report-startup`. The prior `codex/mcr-be-OPS-PRODUCT-01` branch and
all failed and qualified runtime artifacts remain preserved. No new worktree,
dependency, lockfile, SQL, contract, registry, shared parser or frontend edit.

Exclusive authored paths:

- `backend/services/concert-service/src/config/report_runtime.rs`
- `backend/services/concert-service/tests/report_runtime.rs`
- `docs/mcr/backend/report-main-startup.md`

Root retains `config/mod.rs`, main, public dispatch registration, shared private
key parsing, SQL, system resolver mounting and serial publication. This leaf is
not exported or mounted until Root composes it. The test target includes the
owned source by path with aliases for its unchanged library dependencies; it
does not edit an ungranted module export.

The current goal API was inspected: the aligned historical BE36/BE38 Operations
objective remains BLOCKED with obsolete base/worktree metadata. It was not reset,
falsely resumed or completed. The latest human no-review instruction supersedes
old review clauses; all developer implementation, validation, documentation,
committed handoff, required fixes and remote-delivery duties remain. No king/SL
inspection, acknowledgment or approval is a delivery gate.

## Explicit settings before database composition

`ReportStartupSettings::load(grpc, shared_canonical)` and
`ReportStartupSettings::from_lookup(read, grpc, shared_canonical)` return
`Result<Option<ReportStartupSettings>>`.

| Input | Required behavior |
| --- | --- |
| `CONCERT_REPORT_MODE` | Absent or exactly `off`: return None and read no report secrets or unrelated feature configuration. Exactly `user`: require guarded gRPC and the planned shared canonical path. Every other spelling fails. |
| `CONCERT_REPORT_CURSOR_KEY_HEX` | Explicit nonzero 32-byte lowercase hexadecimal key, independent from every active and retained Report receipt key. |
| `CONCERT_REPORT_RECEIPT_KEYS_JSON` | Closed JSON array, first entry active followed by at most 31 retained entries. Each entry has exactly `id` and `secret_hex`; duplicate fields and unknown fields fail. |

Key JSON is at most 16 KiB. All receipt secrets are exactly 32 bytes, nonzero,
lowercase hexadecimal and distinct; IDs use the existing SigningKey constraints
(1–32 ASCII alphanumeric, underscore or hyphen), with no repeated ID. The real
`FingerprintKeys` preserves every validated retained key. There is no local,
fixture, generated, hard-coded or other-feature fallback. Private settings have
no Debug or Serialize implementation; parse/lookup errors omit input values and
reader error payloads. The injectable parser does not mutate process environment.

Root must parse these settings before opening the database or listeners. In
`user` mode the boolean prerequisites describe that startup path; they are not
an owner grant. Actual composition still requires the typed existing canonical
owners. Missing inputs reject startup rather than silently disabling an explicit
opt-in.

## Shared owner overlay

```text
settings.compose_shared(pool, &existing_canonical_owners, base_product_owners)
    -> Result<ProductOwners>
```

Root must supply the same Concert pool, core/fence and base Product composition
already used by Participant and other live features. The leaf creates the actual
`OperationsRuntime::new(pool, OperationsOwners::default(), managed_keys, cursor)`
and actual `ReportProduct::new(runtime)`, wrapped in `ScopedProductOwner` using
`owners.fence.clone()` and `owners.core.delivery()`.

The internal overlay selects only `Operation::Report(ReportOperation::Create)`.
Every other already-admitted operation delegates the original VerifiedProduct
reference and original request unchanged to the supplied base owner. No second
acquisition, registry, ledger, Auth service, delivery publisher construction,
read settler, listener or background task is introduced. The public core API
does not expose private pool/delivery internals for comparing an unrelated base;
same-origin composition is Root's caller obligation, and actual scoped delivery
continues to enforce canonical scope Arc identity.

Default Operations owners keep roles and external target/media evidence absent,
review unavailable and no successful replacement providers. This overlay adds
no admin, moderator, report-read or READY-media operation. The retained built-in
USER/no-media evidence path enrolls actual Auth/Friend policy in the same held
scope and requires actual event target evidence; it does not grant operator
authority. Canonical SafetyInteraction intentionally allows a suspended target
while retaining reporter/session eligibility, as qualified by the preceding
report-product native tests.

## Exact native-settlement dependency

At this base `config::native_resolver::NativeResolverUnion::new(pool, instance)`
constructs the actual `OperationsNativeResolver` with the same pool and instance.
Its closed routing selects Operations only for the original Product Dispatch
parent, `createConcertReport`, POST and exact event `/reports` path with no query.
The Operations adapter verifies actual native transaction outcome and immutable
registered witness; a generic core or abort-only resolver is not COMMIT proof.

The existing `ParticipantRuntimeSettings::compose_shared` already constructs
this union and passes it to `owners.core.settlement_service`. Root must retain
and mount that actual service with system-only Auth/Friend caller guards, once,
alongside the same canonical Product delivery service. No additional Report
native adapter needs registration at this base, and this mutation creates no
child-read work requiring another read settler. This leaf returns ProductOwners
only; it does not pretend that constructing CanonicalRuntimeCore alone registers
any native resolver. Switching to a private/abort-only settlement path would
invalidate this dependency and must fail the real Root flow qualification.

## Provenance and developer validation

Configuration conventions were adapted from these read-only base files; no
business logic was copied:

| Base source | SHA-256 | Adaptation |
| --- | --- | --- |
| `src/config/intro_runtime.rs` | `00b05a2f58ad51ca8106eeaf9260de338b890ae68408a970c52aaa0daef13bc3` | Explicit mode, closed managed receipt versions and independent cursor material; simplified to Report-only inputs. |
| `src/config/inbox_runtime.rs` | `d6508faf8b0fd6df546c1f59d35499984eadd8a6334e83ffcbb07b1aaf1e2d05` | Existing canonical fence/delivery overlay; dispatch narrowed to Report Create. |
| `tests/product_feature_admission_support/fixture.rs` | `c6548495e045456d4a39a112d321fe451bd1dec98a932d5f45e75df9f8ad1002` | Unchanged signed Product RPC recorder/helper, transport-only refusal evidence. |

Paths above are relative to `backend/services/concert-service/`, at the task base.
Read-only resolver source SHA-256:
`228b5792aa3082c828a395448f4ffd56e695eba04681a3b370b754715bec4bd0`;
shared Participant assembly source SHA-256:
`f977dbc6e9d3228eb44f9e4fb8f4840bcbd14078ca18f683ce91477fe7fb9ffa`.

Seven authored tests cover independent off mode; exact mode and listener/core
prerequisites; missing/failed private inputs and redaction; strict cursor material;
closed receipt key shapes/IDs/duplicates; bounded retention and cursor separation;
and original signed report dispatch versus unchanged Feed/Age/Social delegation.
All recording owners refuse; those tests cannot certify canonical authority or
database success.

New Rust files were formatted; `git diff --check` and the planning provenance
check passed (exit 0, all 13 source archive checksums unchanged). The earlier
report-product five admission/four native passing cases remain their own pinned
qualification; they do not certify this new startup leaf or a public main path.
Root export/main mounting, combined service execution, public Gateway behavior,
whole BE36/BE38/BE39 and IN03 remain separate obligations. The named Root developer
publishes the validated committed handoff; this owner verifies remote delivery.

## Fresh bounded build and copied execution

Validated code source: `834aaa6b1bba047a3b01783ecde3f32b3079d4eb`, parent
`beffe4975d5d702bbdf1d3d96458bd7846c7b70a`. The evidence successor changes only
this document; no Rust changed after the frozen execution.

Fresh offer `REPORT-STARTUP-BACKEND-202611041342Z` was accepted at
2026-11-04 22:44:41.424446 KST (13:44:41.424446 UTC), with actual free space
30,783,242,240 bytes, above the 16 GiB start floor. Historical carry was fully
charged at 2,324,127,744 bytes with no deletion or reset. Hard new-output limit:
268,435,456 bytes; hard whole-output limit: 2,684,354,560 bytes.

Exact build, from the assigned `backend/` directory:

```sh
env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
  RUSTUP_TOOLCHAIN=1.88.0 CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
  /Users/lhs/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin/cargo \
  test --locked --offline -p concert-service --test report_runtime \
  --no-run --message-format=json -j 2
```

The controller selected the same toolchain's rustc/rustdoc and PATH. No profile
optimization override, cache deletion or build.rs refresh was needed. Exit 0,
2.808 seconds. Actual positive target output: 146,296,832 bytes.

Exactly one Cargo JSON test artifact was selected, copied and frozen in 0.338
seconds outside the target, alongside 2,381 complete tracked backend/contract/
planning readset hashes, unchanged helper hashes, runtime hashes and Cargo JSON:
`/private/tmp/mcr-report-startup-8upc2hu4/pin.json`.
Pin SHA-256: `d4dcd44dc2458a753192721144dd120a4a651106cdb1b625de518b97bd1245cf`.
Executable size: 7,731,648 bytes, below the 96 MiB copy limit; executable SHA-256:
`a0a59539f5c29503ca08ece00adba46e93d403e7cae9d35b26290a3d128e7b87`.

Unconditional zero-handle compiler/target release was recorded at
22:46:22.964812 KST, before every copied check. No target access followed.
The independent source/runtime/binary pin and 8 GiB live-space guards passed
before launching the copied executable:

```text
/private/tmp/mcr-report-startup-8upc2hu4/report_runtime --nocapture --test-threads=1
exit 0; 7 passed; 0 failed; 0.390 seconds
```

All seven checks fit the 30-second whole-binary bound, hence each individual
check also fit its bound. Copied child PID 73143 was reaped and confirmed absent.
No PostgreSQL, browser or native workflow was started. New whole-output peak:
160,186,368 bytes, including a conservative 1 MiB source/Git/proof allowance;
historical-inclusive peak: 2,484,314,112 bytes. Both output caps held. Completion
free space: 30,827,905,024 bytes; no time, space or cap cutoff fired.

Full release: 22:47:09.142026 KST, 46.177 seconds after target release, below
90 seconds and before every original cutoff (compile/freeze 22:52:11.424446,
work stop 22:53:41.424446, hard release 22:54:41.424446 KST).
No resources remain held. Commands, exit codes, full pin inventory and release
receipts are in `/private/tmp/mcr-report-startup-8upc2hu4`.
Copied log SHA-256:
`66361eace60f58b987ddd85ce002497d03a6b2952125cd5c438cec1c164cabeb`.

These results qualify the settings parser, compiled actual constructor interface
and typed dispatch overlay. Recording refusals are not successful owner proof;
executing the exported startup in the actual composed service remains Root's
implementation validation duty. No public-route or multiuser acceptance follows
from this bounded leaf qualification.

## Root startup mount and bounded combined qualification

After the exact three-path validated handoff, Root exports `config::report_runtime` and loads its settings before connecting the database. The shared Intro/Participant composition retains its existing native resolver union and guarded settlement/delivery services. The Report overlay attaches after the existing Inbox, Feed and Social overlays with that same core/fence and pool; off mode preserves the prior producer. Only USER/no-media Report create is activated by explicit `CONCERT_REPORT_MODE=user` with its managed inputs. No privileged role, review/media provider, new listener, background worker or child-read settler is added.

The owner's seven copied tests qualify its unchanged leaf. Root subsequently compiled the actual non-test Concert, Gateway, Post and Notification mains from `fb00cf5337df98e4ffdfca4893b6bdc35d19a2f0`: four locked/offline Rust 1.88 builds exited 0. All 3,726 frozen inputs matched before publication. Six actual-main pre-database refusal checks passed, including USER Report without shared composition and invalid Report mode. See `product-four-main-build.json` for exact artifact hashes, commands, exit codes and receipts.

The first copied check stopped at a harness omission (`CONCERT_METRICS_ADDR is required`); its failed evidence and five NOT_RUN cases are retained. A fresh copied-only allocation supplied the required metrics address, reused the exact frozen binaries without a rebuild, and passed all six checks. Compiler/target release preceded all copied execution; final copied-only FULL RELEASE was 2026-11-04 14:02:59.408684 UTC, 3.335 seconds after actual ACCEPT. No PostgreSQL was provisioned. Public Gateway Report, positive production stack and full multiuser behavior remain NOT_RUN.


## BE36-OWN-REPORTS-01 functional extension

On published base `7d5d91bd1121b1a95b2302ca02e9224019a5deaa`, explicit
`CONCERT_REPORT_MODE=user` retains the same managed configuration and shared
canonical core/fence/delivery. The overlay now dispatches both closed typed
Report operations: USER/no-media create and the frozen bodyless
`listOwnConcertReports` GET `/api/concert/events/{eventId}/me/reports`. Other
operations retain the base owner. No listener, child settler, role/media provider,
positive authority default or second scope is added.

The owned overlay transport check now includes the original signed Own query
and verifies both Report calls use the existing Report owner while unrelated
frames delegate unchanged. Actual private read authorization, caller isolation,
cursor binding and held Auth revocation are covered by the two new native
Report cases. Both passed in the current functional qualification on
`34c3dce69e9c1a7744b147ac615a6dcf6d28a401`, descending from implementation
`e8968df79c957679dce483542c201d98740963f8` and the published base above.

The updated seven configuration/transport checks were rebuilt and rerun from
the exact frozen copy, exit 0, 0.403 seconds, including both typed Report routes
and unchanged delegation of unrelated operations. The same round passed seven
signed admission and nine actual native cases. Recording transport refusals
remain dispatch evidence; the native cases supply the actual owner/settlement
evidence. No Rust changed after execution.

Frozen runtime executable: 7,754,976 bytes, SHA-256
`bff09e0a097e332940207247bd89b299e5395b53f9153ce4a67c3098c2e81605`.
Pin: `/private/tmp/mcr-report-product-frozen-6rfc9b_j/pin.json`, SHA-256
`6858a8aca6c7cb21ad02e37ebcd42222f98b2dd4581f293ea9f5b85ae1e059e3`.
Runtime log SHA-256:
`8c78358c32387cbed8836b26085f68e19b1f693ec5950b1f24902341ac586143`.
The copied command was `report_runtime --nocapture --test-threads=1`, after
unconditional zero-handle target release at `2026-11-04T16:54:11.509878Z`.
All private PostgreSQL resources were removed, prepared transactions were zero,
and FULL release completed at `16:54:25.872622Z`. No target reentry followed.

See `report-product-production.md` for exact build/provenance, retained earlier
cached-script failure, bounded resource accounting, current 23-case results and
native restart/cleanup evidence. Direct committed handoff uses Root's canonical
publisher receipt without a consumer proof or king/SL acknowledgment gate.
Positive composed-service/public Gateway, IN03 and whole BE36/BE38 acceptance
remain open; performance qualification is deferred.
