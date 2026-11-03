# Joined HTTP identity owner chain

Status: INTEGRATED bounded verification; BE04 remains IN_PROGRESS.

The test uses the production Axum timeout middleware, strict HTTP access
extractor and configured `ConcertIdentityReader`. The HTTP request is dispatched
in process through `Router::oneshot`; its Auth and Profile calls cross actual
loopback Tonic transports into guarded generated services and three isolated
PostgreSQL databases. Neither successful owner is replaced with a fake reply.
The private probe route is test-only and is not registered in the product router.

The implementation began in the assigned `be-http-real-owners` checkout at
`1f6dceb`. After repeated fixture assertion errors, the worker explicitly stopped
and transferred source/build ownership to the integrator. Final source is worker
commit `ca26683`, integrated as `e6c629d`. Only the new Rust integration test and
its explicitly assigned Python harness changed. Retained production identity
code is unchanged. Later master changes to Concert-only dependencies and the
receipt harness are outside this Auth/Profile test's dependency behavior.

The final test makes nine sequential controls in one Rust scenario: a healthy
email login; revoked-session refusal; deleted Profile refusal; inactive-account
refusal; a new login and healthy positive control after account restoration;
missing Profile; Profile outage; a delayed Profile; and Auth outage. A forwarding
wrapper counts the actual guarded Profile dispatches. Refused Auth and Auth
outage produce no additional Profile dispatch. The wrapper delays but never
fabricates the owner's result.

The account-status trigger intentionally revokes existing families. Restoring
ACTIVE alone cannot restore that session. The corrected fixture obtains a new
real LoginEmail token rather than undoing revocation. Earlier attempts failed
because they reused that revoked token and changed unrelated assertions.

For the deadline case, Profile is delayed 700ms under the original 200ms HTTP
stop. Exactly one Profile dispatch occurs, and refusal arrives between 150ms
and 700ms. Both the checked access context and outer HTTP timer enforce that
same stop: the private probe returns 401 when the access check wins, or 504 when
the outer timer wins. These are test-route projections, not a newly frozen
product error mapping. A successful late owner response is never accepted.

Rust 1.88.0 locked/offline compilation with two jobs and debug information
disabled exits 0. The fresh native scenario exits 0, with Auth/Profile/Gateway
migration receipts 13/7/17, respectively. PostgreSQL TCP listening is disabled;
the owned cluster is stopped and its exact directory removed. The
[sanitized receipt](../evidence/http-owner-chain/receipt.json) pins both delivered
source files, the binary, raw receipt and native log. The original unsuccessful
attempts remain in the private build evidence directory.

The boundary checker permits this exact assigned harness only when invoked with
`--allow-harness backend/scripts/mcr/concert_http_owner_chain_smoke.py`.
It grants no other script, manifest, contract or opposite-lane ownership.

This does not qualify signed Gateway-to-Concert admission, local event policy,
provider-binding cases through this HTTP route, public Concert endpoints, TLS,
browser/device flows, revocation propagation or release readiness. Previous
owner/component evidence remains separately scoped. Shared Rust build ownership
was transferred to BE08 after the passing run and cleanup.
