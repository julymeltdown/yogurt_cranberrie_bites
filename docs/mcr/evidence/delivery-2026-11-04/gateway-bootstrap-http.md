# Gateway bootstrap HTTP composition

Delivery added the actual `GET /api/concert/events/:event_id/bootstrap` ingress to the existing Axum router. The strict access extractor supplies identity; fresh signed Auth and Profile reads precede one signed Concert call. Canonical event UUIDs, GET-only, an empty query/body and the original deadline are enforced. No configured Concert endpoint leaves this route unavailable.

`GATEWAY_CONCERT_GRPC_ENDPOINT` is an explicit optional setting. Configured values require HTTP(S), a host and no credentials, path, query or fragment; production also applies the inherited nonlocal-host rule. A new negative test initially exposed that the inherited host check alone accepted a schemeless string. Strict URI validation fixes this; subsequent fragment coverage required explicit rejection because the HTTP URI parser drops fragments. The final configuration selection passes all 13 tests.

The outer response layer produces the closed C03 error envelope for routing, access, timeout and transport failures, applies no-store/no-cache/Vary Authorization, and discards inherited diagnostic bodies. The composition tests exercise public ingress, one fresh signed chain, denied Auth suppression, foreign/cacheable result rejection and late-result deadline fencing.

## Delivery execution

All Cargo commands used Rust 1.88.0, the sole `/Users/lhs/Desktop/mcr-build/backend` target, jobs 2, dev/test debug 0 and `--locked --offline`. Final commands and outcomes:

- `cargo test --manifest-path backend/Cargo.toml -j2 -p api-gateway --test unit config:: -- --test-threads=1`: exit 0, 13/13 PASS after the strict endpoint correction. The two earlier failing new-test attempts were retained in this account rather than presented as green.
- `cargo test --manifest-path backend/Cargo.toml -j2 -p api-gateway --test concert_identity_composition -- --test-threads=1`: exit 0, 15/15 PASS, including the three HTTP ingress scenarios.
- `cargo test --manifest-path backend/Cargo.toml -j2 -p api-gateway --lib config::hardening_tests -- --test-threads=1`: exit 101, 14 PASS and 2 FAIL. Both failures occurred while acquiring inherited Docker fixtures because `/var/run/docker.sock` was unavailable. Neither fixture is claimed as qualified; no Docker or unrelated data was changed.
- Rust 1.88 rustfmt on changed implementation/config/test files and `git diff --check`: exit 0.

The compile slot was then explicitly released to the critical admission correction. These checks qualify the bounded ingress and configuration patch. They do not prove approved-art startup, live owner configuration, the combined Concert parent tree, a real two-user flow, device behavior or release readiness. Shared canonical participant/mutation ports remain separately assigned.
