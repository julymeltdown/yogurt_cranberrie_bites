# BE26 combined integration checkpoint

The scoped age-verification implementation is integrated from author
`0e97c855a0795e278a2e53564a1806649563dc6d` at candidate
`34a11e4f3e4aa6d817bedda4ff7cb0ca1ac11a07`. The author stopped with a clean
checkout and an actually completed bounded goal; the subsequent goal read was
null. This is **IN_PROGRESS** for BE26 as a whole, not public verification or
IN03 acceptance. See [author evidence](age-verification.md).

Integration verified all eight assigned new paths, their exact author bytes,
1,453 author backend/contract pins, frozen receipts and binary hashes. No
frontend, central registration, contract or migration file was changed. Existing
author peer findings were already fixed; no second general code review was added.

Fresh combined verification used retained Rust 1.88.0, locked/offline Cargo,
one build job, no incremental compilation and debug information disabled.
The released media build target was reused exclusively, preserving previous
immutable binaries. At least 8 GiB free space was required. The actual command
arguments, source pins and command logs are retained in the private receipt;
[the portable summary](age-verification-combined.json) records their hashes.

- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 1 -p concert-application --test age_verification`: exit 0, 10 tests passed.
- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 1 -p concert-service --test age_claims --no-run --message-format=json`: exit 0; exact emitted binary copied and hashed before target reuse.
- Existing `age_claims_support/native_smoke.py` ran that binary on a newly owned PostgreSQL 16.14 cluster, with TCP listeners disabled. All 13 central SQL prerequisites plus the local age schema applied successfully. Producer run: 16 passed, zero failed, one restart-only test intentionally held until the restart.
- Actual PostgreSQL stop/start changed both process ID and start time. The deferred restart test then passed. All 16 schema/test command exit codes were zero; owned cluster cleanup passed and its directory is absent.

All 1,461 combined backend/contract source pins remained unchanged through
qualification. The native receipt binds the same clean candidate HEAD as the
build receipt. Prior author failures and historical receipts remain preserved.
Only the existing bounded suite was rerun to qualify the combined dependency
state; this does not broaden the author evidence to real provider or Auth.

Real independently verified provider integration, canonical Auth/session/event
fences, root module exports, runtime schema registration/grants, HTTP recovery
semantics, retention/deletion/backup orchestration and IN03 remain open. An
own-status response after SQL commit is **not** a BE25 ClaimReader lease. The
BE27 claim-reading bridge must retain real owner/claim authority through
projection; timestamps or context identifiers cannot replace that guarantee.
