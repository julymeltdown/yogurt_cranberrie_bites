# REPORT-STARTUP-01 — explicit USER report startup leaf

Status: IN_PROGRESS; source and pure configuration tests authored, Rust execution
NOT_RUN pending a fresh allocated build round. This is a new three-file task,
not renewed ownership of the preceding qualified nine report-product paths.

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
check passed (exit 0, all 13 source archive checksums unchanged). Build and copied test
execution are NOT_RUN until a fresh finite resource offer is accepted. The earlier
report-product five admission/four native passing cases remain their own pinned
qualification; they do not certify this new startup leaf or a public main path.
Root export/main mounting, combined service execution, public Gateway behavior,
whole BE36/BE38/BE39 and IN03 remain separate obligations. The named Root developer
publishes the validated committed handoff; this owner verifies remote delivery.
