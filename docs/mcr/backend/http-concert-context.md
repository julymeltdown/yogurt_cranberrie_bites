# HTTP Concert access context

Status: **IN_PROGRESS** pending integrator review. This bounded increment captures
the strict Concert access context at an actual Axum request boundary. It adds no
public route, admission decision, owner read or participation grant. Whole BE04
remains active.

Base: `60ca208d23e67b2372f6a27db7e7cecc4ff7cdc6`. Worktree
`be-http-concert-context`, branch `codex/mcr-be-http-concert-context`.

## Implementation

`concert_access::HttpConcertAccess` is a nonserialized Axum extractor. It requires
the `RequestDeadline` inserted by the existing HTTP timeout middleware, extracts the
ordinary bearer token, and calls the configured `JwtVerifier` with that same absolute
stop. Missing deadlines return `504`; malformed, expired, non-canonical, missing-session,
or otherwise rejected tokens return a fixed `401` without token or verifier diagnostics.

Unknown-key handling uses the retained `SigningKeyRefresher`: it may await a configured
issuer refresh, then performs strict verification again against the same original stop.
The extractor never constructs a deadline, reads trusted roles as owner authority, or
exposes the raw token. Its `check`, `remaining`, `user_id`, and `session_family` methods
recheck the private context. Handlers still owe current Auth account/session/binding,
Profile and event checks before any decision.

The only change to `http/mod.rs` is the module export. `tests/unit.rs` registers the new
test module. The tests build actual Axum routers and use middleware to inject the absolute
deadline; they do not grant access by calling the verifier directly.

## Validation

Rust 1.88.0, locked/offline dependencies, jobs 2, `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0`, shared target `/Users/lhs/Desktop/mcr-build/backend`:

```sh
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline \
  -p api-gateway --test unit http_concert_access -- --nocapture
```

Exit **0**: **5 passed**, 0 failed, 209 filtered. The cases cover successful strict
context capture after middleware dispatch delay; exact identity and session-family
preservation; no handler dispatch for a missing or expired trusted deadline; malformed
and missing-session refusals; and an unknown-key refresh that stalls beyond the original
request budget without extending or dispatching. A handler delay consumes the original
120 ms budget rather than starting a fresh one.

`git diff --check`, Rustfmt, planning and backend boundary checks are run before the
committed handoff. Their exact exits and the shared-target release record accompany the
handoff outside Git. This does **not** qualify unknown-key refresh success with a live
issuer, production TLS/HTTP listener, current Auth/Profile owners, event authority,
multiuser behavior or a public Concert endpoint.
