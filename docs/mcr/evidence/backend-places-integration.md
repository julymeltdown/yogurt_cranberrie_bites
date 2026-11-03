# BE08 bounded integration

Reviewed 2026-11-04. Final worker candidate `caee1a7` is integrated through
`5f7eb1c`; the independent final review `4ca13ad7` is integrated as `4d6fc85`.
The six owned source/fix commits were integrated serially. The dependency
prerequisite `5bdd6f0` was not duplicated: main already contains its original
`a518149`. Worker base was `097651c`.

The implementation adds typed place/map-release values, event-bound catalog and
detail reads, immutable content pins, reviewed/current physical-place selection,
and PostgreSQL persistence. Selection may hold a shared row lock through the
caller's associated transaction, preventing closure from racing a committed
selection. Map JSON hashes use the agreed canonical encoding; tilemap paths and
provenance URIs now match C06 rather than an invented HTTP-only policy.

Parent verified every final handoff source hash against `caee1a7` and the merged
files. Pinned Rust 1.88.0 locked/offline, jobs=2/debug=0:
`cargo test --manifest-path backend/Cargo.toml --locked --offline -j 2 -p concert-domain -p concert-application --test place`
passed 9/9, exit 0. Existing unused-field warnings in test fixtures remain.

[Retained native report](places/native-a91ed08.json) SHA-256:
`978ddd07ba369956515298c87ebeb7b7dba7ae0c48c51073b0fead476b6d8486`.
[Final handoff](places/handoff-caee1a7.json) SHA-256:
`b0b1230ff2ac27c82f864a65376edf476744a61964f532659ff0f295d98f081d`.
Both were copied byte for byte and checked. The native PostgreSQL 16.14 report
is PASS, native exit 0, restart persistence PASS and cleanup PASS. It belongs to
`a91ed08`; repository, migration, native tests and harness are unchanged in the
final parser revision. It is not a new PostgreSQL run on the integrated commit.

The native harness applied direct SQL, not the application migration ledger.
Public HTTP/RPC place routes, production deployment, browser/device and load
checks remain unqualified. Status **INTEGRATED** applies to this bounded
implementation; whole BE08 and the integration gates remain open.
