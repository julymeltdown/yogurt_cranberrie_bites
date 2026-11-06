# Task 17 — BE43 boundary and abuse hardening

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-hardening`
Branch: `codex/mcr-exec-w2-hardening`
Base: `113e99474` (merge(infra): seven-service compose stack and lifecycle smoke)
Delivered commit: `800c830a6` (feat(mcr/BE43): harden body, rate, ticket, upload and WS boundaries)
Plan row: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` BE43 (P4, BE-CORE); current ordering in
`docs/mcr/functional-delivery-plan.md` row BE43 (`IN_PROGRESS / 3`).
Register files were NOT edited in this task (explicit instruction: "No register edits").

## Status

Implementation and negative tests complete on this branch. Everything below was executed
in this worktree on 2026-11-06 with `cargo +1.88.0` and
`CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w2-hard`.

## Delivered boundaries

| Boundary | Behavior | Test |
| --- | --- | --- |
| Public login body | Body over axum's 2 MiB public JSON bound -> 413 before Auth dispatch | `browser_auth_http::login_body_beyond_the_public_bound_is_413_before_auth_dispatch` |
| Login throttle | Refused admission -> 429 + `Retry-After: <window seconds>`; neutral envelope, no credential/account echo; Auth never called | `browser_auth_http::throttled_login_is_429_with_the_admission_window_as_retry_after` |
| Ticket issue body | C05 4096-byte bound -> 413 `PAYLOAD_TOO_LARGE` before the identity/world owner reads | `concert_identity_composition::world_dispatch::http_ingress::oversized_ticket_body_is_413_before_any_owner_read` |
| Avatar upload | First byte over 1 MiB -> 413 (was 400); exact limit still accepted | `profile_routes::tests::avatar_field_reader_accepts_limit_and_rejects_one_byte_over` |
| Reveal / product representations | `no-store`, `no-cache`, `nosniff`; `ETag`/`Last-Modified` stripped | `concert_product::social_route_tests::reveal_and_every_product_representation_are_never_storeable` |
| WS replayed/unknown ticket | Closed 4401 `AUTH_REQUIRED`, fixed reason only | `world_ws::unknown_ticket_and_replay_never_receive_a_world_snapshot` |
| WS oversize frame / fragment aggregate | Closed 1009 `MESSAGE_TOO_BIG`, fixed reason only, no frame echo | `world_ws::native_oversize_and_fragment_aggregate_close_with_1009` |
| Error taxonomy | 429 carries `Retry-After` on every 429; 413 has its own status and no retry hint | `unit::gateway_errors::*` |

No blanket ban or global block was added. Limits stay per route/class; the accepted 1 MiB
avatar and the first successful login prove legitimate traffic still succeeds.

Production code changed:

- `GatewayError` gained `PayloadTooLarge` (413) and a `TooManyRequests { message, retry_after_seconds }`
  that emits `Retry-After`; all rate-limit refusals now report the admission window
  (`InMemoryRateLimiter::window_seconds`).
- Avatar upload overflow returns 413 instead of 400.
- No global middleware or route was disabled.

## Commands and results

```
cargo +1.88.0 check -p api-gateway -p concert-service --all-targets --jobs 3   -> exit 0
cargo +1.88.0 test -p api-gateway --lib -- avatar_field_reader                 -> 1 passed; exit 0
cargo +1.88.0 test -p api-gateway --lib -- reveal_and_every_product_representation
                                                                              -> 1 passed; exit 0
cargo +1.88.0 test -p api-gateway --test browser_auth_http -- login            -> 3 passed; exit 0
cargo +1.88.0 test -p api-gateway --test unit -- gateway_error                 -> 20 passed; exit 0
cargo +1.88.0 test -p api-gateway --test concert_identity_composition -- oversized_ticket_body
                                                                              -> 1 passed; exit 0
cargo +1.88.0 test -p concert-service --test world_ws -- unknown_ticket_and_replay
                                                                              -> 1 passed; exit 0
cargo +1.88.0 test -p concert-service --test world_ws -- native_oversize       -> 1 passed; exit 0
rustfmt +1.88.0 --edition 2021 --check <changed files>                        -> clean
```

## Limitations

- `integration::gateway_profile_route_matrix::profile_routes_cover_lookup_search_update_delete_and_avatar_upload`
  needs a container runtime (`/var/run/docker.sock` absent here): NOT_RUN in this environment.
  Its oversized-avatar assertion was updated to 413; runnable coverage of the upload bound is
  the lib test above.
- The composed Gateway's ticket POST path requires the live request-journal/admission stack in
  this harness (it answers 503 without it), so the 4096-byte ticket bound is exercised on the
  C05 router (`concert_world::router_with_boundary`), the component that owns the bound.
- Repo-wide `cargo fmt --check` reports 87 pre-existing unformatted files outside this change;
  every file touched here is rustfmt-clean.
