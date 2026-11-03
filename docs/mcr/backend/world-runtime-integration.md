# World runtime integration — 2026-11-04

The owned runtime series `a4487b9`, `c73d57b`, `c529741` is integrated as
`c63bff4`, `83d2788`, `9a77d238ce4367882cd1fa77799c223a4fcf68f4`, after
`6893410890cf5759e9f1c0f84d855104f6cb1c74`. Already-published prerequisite
merges/dependency copies were excluded. All 17 delivered files match the final
author commit byte for byte; central world module exports were preserved.

Integration verified all 1,479 author input hashes, five immutable executable
hashes and final receipt SHA-256
`06e2f0319e116c36b8d54fe1d243687f7f646bd1c54e077fe2bf43cca1b98e49`.
The author's no-run build and 97 focused checks passed: 28 simulation,
11 replication, 15 recovery, 24 runtime, 16 actual WebSocket and three explicitly
executed isolated PostgreSQL bridge checks. Native binaries and test results
remain bound to that author's source; fresh combined Rust validation is
**NOT_RUN**. The released existing target now belongs exclusively to the
Auth-only owner-read producer. Integration `git diff --check` passes (exit 0).

Production World startup remains unavailable until qualified owner producers
are supplied. Auth uses the existing dedicated ReadCurrent protocol; the old
ReadWorldSession proposal in historical author notes is superseded. Profile,
recipient authorization/revocation, the successful database room-switch race,
approved art and real two-account/device/load gates remain required work.
The 256-byte native write-buffer probe proves its specific failure branch,
not production saturation or fifty-user capacity. No IN01 or task acceptance
is inferred from these fixtures.

The runtime author retains its isolated checkout for source-only switch/bridge
work, with no Cargo lease. See [the source handoff](world-runtime.md) for exact
commands, evidence locations, source provenance and remaining limitations.

## Sealed Auth, successful switch and lease correction follow-through

Author `537a440` is integrated as `83aa315`; `a4308b7` as `e15bac3`.
Their ten distinct owned paths match the final author source exactly. Integration
verified 1,494 tested-source pins and three frozen executable hashes. The final
receipt is `7ec01ee2e32ec1d337a2c49d39587b3b5293743bdc5963a12f0f104d9030692d`.
Six combined contract inputs are intentionally newer from closure amendment
`911f156`, qualified separately by 98 contract tests; no fresh combined native
run is claimed for that amended closure contract.

The author passed 62 focused cases: runtime 27, actual WebSocket 16, library 14,
explicit PostgreSQL bridge 4, and one real signed-Auth/owning-Auth-and-Concert
composition. Previously ignored relevant database cases ran explicitly. This
includes delayed successful room switching, one-use admission without a prior
membership, fresh Auth before renewal, revocation/suspension/outage denial,
original credential expiry, and cleanup. Profile/recipient fixtures remain
synthetic; the actual production root still fails closed without their owners.

Native validation exposed a concrete unit error in persistent membership
creation: `FRESH_AUTHORITY_MILLISECONDS` was applied with `Duration::seconds`.
The exact authorized line now uses `Duration::milliseconds`. Initial and switch
leases are bounded to five seconds; fresh Auth caps effective publication to
two seconds or less. Renewal can extend the bounded persistent lease only after
fresh authorization, without extending the original credential expiry. Failed
candidate evidence is preserved alongside the corrected passing regression.

The author explicitly released the shared build target after all handles
terminated. Integration verified the release hash, absence of all four owned
fixture roots and no retaining process. The source remains committed and the
broader runtime goal remains unfinished pending production Profile/recipient
and invalidation integration. No IN01, real two-user, device or load acceptance
is advanced by this handoff.
