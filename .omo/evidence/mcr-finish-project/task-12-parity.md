# Task 12 - route parity and missing public command templates

Worktree: `/Users/lhs/Desktop/mcr-worktrees/exec-w2-routes`
Branch: `codex/mcr-exec-w2-routes`
Base: `5a5da504f`

## Changed paths

- `scripts/mcr/contracts/route-parity.py`
- `scripts/mcr/contracts/test_route_parity.py`
- `backend/services/api-gateway/src/adapters/in/http/concert_product/command_routes.rs`
- `backend/services/api-gateway/src/adapters/in/http/concert_product/mod.rs`
- `backend/services/api-gateway/tests/concert_product_http.rs`

## Gate

`python3 scripts/mcr/contracts/route-parity.py` compares:

- every `RegisteredOperation` in `backend/crates/concert-application/src/commands/registry.rs` (62)
- every non-GET command row in `contracts/domain/v1/operations.json` (63, adds `setConcertServicePolicy`)
- against every exact `.route("<path>", ...)` template and verb under
  `backend/services/api-gateway/src/adapters/in/http/**`

Contract `{eventId}`/`{partyId}` names are normalized to axum `:event_id`/`:party_id`
templates before comparison. Any missing method/template pair prints `MISSING` and exits 1.

## Mounted families

All missing templates now mount through the existing `concert_product::dispatch` proxy
(no gateway domain logic): party update/owner/rally, beacons create/update/revoke,
giveaways create/update/pickup-meeting, reservations cancel/issue-code/claim, feed
post/comment/reaction mutations, reports create, admin settings/phase/places/notices,
admin report decision, admin user/content moderation, admin closure, admin service policy.

`command_routes::owns` extends the existing receipt-ownership boundary so the Gateway
idempotency journal does not intercept these Concert-owned commands; `command_routes::protects`
keeps unmounted suffixes under those families inside the closed C04 404 envelope.

## Results

- `python3 scripts/mcr/contracts/route-parity.py` -> exit 0, `commands=63 missing=0`
- `python3 -m unittest scripts/mcr/contracts/test_route_parity.py -v` -> 4 tests OK
  (includes a deliberately removed template that must exit 1)
- `cargo +1.88.0 check -p api-gateway --all-targets` -> exit 0
- `cargo +1.88.0 test -p api-gateway --test concert_product_http` -> exit 0, 6 passed
  - `every_mounted_command_family_reaches_the_concert_boundary_behind_authentication`:
    all 32 mounted command shapes return the gateway 401 envelope unauthenticated, and
    with a valid bearer reach the Concert product boundary (503 DEPENDENCY_UNAVAILABLE
    because this harness has no producer), not the generic Gateway fallback.
  - `unknown_command_suffix_under_a_mounted_family_is_a_closed_concert_404`: an unmatched
    suffix under a mounted family returns the closed C04 404 envelope.

## Limitation

The Gateway transport proof stops at the Concert product boundary. An unknown-but-valid
event UUID becomes 404 only when the Concert product producer composes the matching
family owner and reports `RESOURCE_UNAVAILABLE`; the producer-side wiring for beacons,
giveaways/reservations, party update/transfer/rally, feed mutations and admin report
moderation is owned by tasks 13, 15, 16, 17 and 49. This task proves the templates and
the authenticated forwarding path only.

Commit: 4465e3f6f feat(gateway): mount every registered concert command route
