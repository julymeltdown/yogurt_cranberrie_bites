# Domain consumer prerequisites

Status: implemented transport/capability prerequisites for C04 consumer qualification;
C04 remains REVIEW_CANDIDATE. No party/gift feature, live route or full task acceptance.

The canonical FE01 client, also used by FE09 `auth.request`, accepts explicit
`errorFamily: 'domain'` plus `domainOperation`. The candidate operation registry
binds method, path parameters, closed typed query, success status and error
status/reason. `idempotencyKey` must be a canonical UUID for registered commands;
the client sends exactly `Idempotency-Key`. Raw arbitrary headers remain forbidden.
Registered enum `state` filters work; unqualified/Auth state and secret query
fields remain rejected before dispatch. No new client, persistence, automatic
command replay, polling or key generation is introduced.

`scripts/mcr/generate-domain-transport.py` projects metadata directly from the
106-operation candidate and referenced scalar schemas. `--check` detects drift;
unsupported scalar assertions fail generation. This is transport qualification,
not the full C07 SDK or response/context decoder. Feature decoders must validate
body schemas, actor/event/resource/receipt bindings, projection privacy and
PENDING semantics. An allowed 202 reaches the synchronous decoder with status;
it does not establish command success. Uncertain command failures stay uncertain.

The validated C03 bootstrap now retains all seven immutable capability decisions
and reasons in the existing scope-bound shell projection. `worldAvailable` remains
a compatibility projection and must agree with `capabilities.world.available`.
READ_ONLY/ARCHIVED cannot advertise normal mutations as available. Capabilities
do not grant resource ownership, eligibility, admin rights or authorization.
In C04, SAFETY cancellation may remain available in READ_ONLY: a mutation decision
disabled for EVENT_READ_ONLY must not become a blanket prohibition on safety
actions. Apply the reviewed operation policy and current server authorization;
unknown eligibility and other disable reasons fail closed.

Independent review R4/R5 remain open: receipt recovery after 404/retention,
requested command/key/final status/target binding, and operation-context predicates
for discovery/detail/own activity. No read-subset or command freeze is claimed.
Shared query-key expansion and actual feature composition remain separate work.

Validation uses retained Node 24.22.2 / pnpm 11.7.0. The focused canonical HTTP,
Auth contract/session/domain-request and shell tests, strict production build,
lint and generator drift check are recorded in the integration evidence. One
initial build failed because the new negative test harness explicitly omitted
optional fields under `exactOptionalPropertyTypes`; the harness was corrected
without relaxing product types. Synthetic injected fetch is not live service proof.
