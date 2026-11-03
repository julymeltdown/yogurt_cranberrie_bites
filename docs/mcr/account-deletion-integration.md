# Canonical account deletion integration — 2026-11-04

Status: **INTEGRATED source; complete account-deletion flow NOT_RUN**.
The published eight-target contract now has its matching Auth saga producer and
frontend consumer. Real Concert cleanup owners, canonical browser transport/root
composition, reconnect/reveal denial and retained-field erasure remain required.
This record does not accept BE42, IN03 or IN05 as a whole.

## Delivered change

Auth creates and requires all eight canonical targets, and cannot complete a saga
with missing steps or successful steps lacking completion timestamps. Forward
Auth migration 0015 adds Concert steps and reopens legacy four-step completion
without discarding successful legacy work. Missing production cleanup owners
remain failed/retriable; typed fixture acknowledgments are not real erasure.

The frontend requires the same eight targets exactly once, validates their
identity and timestamps, and presents ordered English progress labels. Legacy
four-step completion, malformed responses and incomplete cleanup cannot sign the
user out as successfully deleted. Once-only DELETE and explicit manual status
recovery remain unchanged. The authorized safety fixture now uses the same
canonical inventory; its browser-smoke implementation is unchanged.

## Ancestry and scope

- Integration base: `7f05f596cb64ae7f415d0822addf4b3e1fe77a44`.
- FE author `3f2bb32b64c8fc0cf9f94bd04c89b681616e9de7`, based on contract
  `d3b7c382`, is delivered as `5050bd28`: ten assigned paths.
- BE author `5c89566281353e1661383c94a7401de241d5fa41`, based on `ef9422c4`,
  is integrated through `36b8bc9c`: sixteen assigned paths. Its qualified source
  is `389060b90703be7bc1e8e9645c4b0bd926fb0ed9`; only the results document
  changed afterward. The twelve worker commits retain their individual authors
  and cherry-pick provenance.
- Worker contract import `57e23860` was omitted because original `d3b7c382` is
  already delivered. All seven imported contract paths match exactly.
- All existing implementation preimages matched. All 26 delivered files match
  their respective final authors byte for byte. The 26 recorded frontend/backend
  evidence source hashes also match. No conflict resolution or broad review was
  needed. Auth migrations 0001–0014, source archives and manifests are unchanged.

## Validation

On combined source `36b8bc9c`, retained Node 24.22.2 passes all **378**
account/Auth/privacy tests in thirteen files and strict frontend TypeScript.
The Python deletion-runner safety checks pass **2/2**. Rust 1.88.0 compiles all
three Auth deletion/unit/durability test targets with locked offline dependencies,
two jobs, debug information disabled and incremental compilation disabled.
Cargo-JSON-selected binaries copied outside the build target pass **124** Auth
unit tests and **5** deletion-adapter tests. Each successful command exits zero.

The first combined build failed because the shared workspace build cache retained
the author's protobuf include root. Package-scoped cleanup of the 33 local
workspace packages removed 427 generated files (681.8 MiB), preserving external
dependencies and source. The unchanged build then passed in 22.61 seconds.
No new dependency installation or third build target was used. The temporary
frontend dependency link is removed, and the BACKEND build reservation is released.

Source-bound author evidence is retained without repeating the native suites:

- FE: account-specific and unchanged mixed safety browser checks pass at
  360/390/430 pixels. They are explicitly synthetic HTTP/owner fixtures.
- BE: 15 domain, 23 application, 124 unit and five adapter checks pass; isolated
  PostgreSQL 16.14 proves the actual 0014→0015 upgrade, rollback/reapply, retry,
  partial failure, restart persistence and all five inherited durability cases.
  All fifteen migration receipts match; owned database cleanup passes.
- The inherited two-saga test's batch limit was corrected from ten to twice the
  canonical target count. Its original assertions remain. The first failure
  report is preserved separately.
- Planning/source provenance and whitespace checks pass. These checks do not
  establish application acceptance or a live browser journey.

See [machine-readable evidence](evidence/account-deletion-integration-2026-11-04.json),
[Auth handoff](backend/account-deletion.md),
[frontend handoff](frontend/account-deletion-targets.md), and
[canonical contract](account-deletion-contract.md).

## Next owner

The existing FE36 chat has accepted an isolated implementation assignment for
canonical Auth deletion transport and default `/me/account` / `/account/deleted`
composition. It must retain manual recovery outside the private WorldShell and
preserve canonical cleanup, identity fencing and exact eight-step completion.
Its actual new unbudgeted goal is ACTIVE. Real Concert cleanup and the combined
runtime acceptance remain backend/integration work, not completed by that UI task.
