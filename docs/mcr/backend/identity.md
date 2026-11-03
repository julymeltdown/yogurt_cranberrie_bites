# BE04 canonical identity adapter increment

Status: **IN_PROGRESS** overall; pure application and deterministic adapter/consumer behavior **MOCK_READY**. Real guarded Auth and Profile owner reads and the joined HTTP owner chain are now **INTEGRATED** as bounded prerequisites. Email-only participation through a deployed Concert route, signed Gateway-to-Concert admission, event restriction authority and whole-task acceptance remain **NOT_RUN**. This increment does not certify BE04 acceptance.

Base: `376cf2127b5ae5b81fa8743293ec9cc0079f6bfd`, task branch `codex/mcr-be-BE04`, isolated worktree `be-identity`. Only the integrator publishes reviewed work. Commit ancestry is supplied in the handoff and external receipt; this document does not embed its own commit hash.

## Source and contract provenance

This is newly authored Rust, not copied rendering, a second identity store, or an upstream code extraction. It uses the imported foundation recorded by C01/BE01: `julymeltdown/port-rust` revision `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, original prefix `boilerplate-grpc-rust/`, now `backend/`. The imported source was inspected at the base above; source hashes accompany the external evidence. The upstream commit object is not claimed to have been independently fetched here. No Bigyeting code or `docs/source/` content changed.

Read contracts: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` BE04; `docs/mcr/core-contract.md` C03; `backend/docs/api/mcr-core.openapi.json` PublicProfile; BE05 typed IDs, clock, policy and repository ports. C03 permits only user UUID, display name (1–40 characters), optional bio (up to 160) and an authorized Concert avatar in PublicProfile.

Owner implementation inspected:

- `backend/proto/auth/v1/auth.proto`: `GetSigningKeys {}` returns `jwks_json`; `CheckSession {user_id, session_id}` returns `active, roles`. Auth `LoginUseCase::session_status` checks active canonical user, live refresh family and current roles. The role owner uses `ROLE_USER` and `ROLE_ADMIN`; participation here requires current `ROLE_USER`, never a client role or an admin-only assertion.
- `backend/crates/auth-domain/src/user.rs`: status is PendingVerification, Active or Deleted. No suspension state is present. `CheckSession` does not attest provider claim ownership or event eligibility.
- `backend/proto/profile/v1/profile.proto` and Profile GetProfile use case/server: `GetProfile {user_id}` returns the owner row, including legacy URL, username, counts, visibility and lifecycle. It does not verify the actor's current session or suspension. ProfileAuthorAuthority explicitly proves existence only.
- Auth/Profile method policies, `platform/src/grpc_auth.rs`, `edge.rs`, `grpc_method_policy.rs` and `actor_delegation/`: Concert is not a registered workload or approved caller. Gateway-only actor delegation and Post-only child forwarding cannot be reused by impersonating those services.

## Implemented boundaries

`concert-application/src/identity/` contains neutral ports, validated session/credential/reference values, canonical request context, separately bound session/eligibility/profile/provider evidence, a resolver and a per-request ParticipationPolicy adapter. Only typed Auth UUID and event ID identify a participant. No X account, provider identifier, profile content or browser assertion can replace them.

Each invocation authenticates first, checks the current session and `ROLE_USER`, then checks eligibility. Evidence must match actor, event, session and operation; its evaluation must occur at or after this invocation's request time and remain valid when used. Stale, future, expired and cross-context replies fail closed. Earlier evidence is checked again after later awaits to catch expiry. Provider lookup additionally binds the exact provider and opaque claim reference; unattached/revoked or foreign claims cannot authorize. This is a read of owner-confirmed binding, not account linking or a claim persistence endpoint. No provider binding is required for ordinary email-only participation.

`IdentityPolicy` binds a credential to one exact context and invokes fresh authority checks before existing ConcertCore repository dispatch. Denial maps to `PolicyError::Denied`; dependency failure maps to `Unavailable`. The richer resolver preserves `Unauthenticated`, `Denied` and `Unavailable` separately. The inherited core policy error only represents denial/unavailability, so a future transport must authenticate first and preserve 401 vs authenticated 403; it must not reinterpret every denial as a refresh trigger.

BasicProfile is internal allowlisted content with no serialization implementation: canonical UUID, display name and optional bio. It validates C03 lengths and excludes controls. It is **not** a new PublicProfile wire DTO: the later producer still must authorize and join C06 Concert AvatarState and relevant relationship/read policy. No legacy avatar URL, default image, exact age, raw proof, social handle, email, counts or private settings is included. Debug output redacts credentials, sessions, claim references, profile content and role values.

`concert-service/src/adapters/out/identity/` maps typed owner-boundary records to those ports, validates canonical identifiers and profile/lifecycle data, and keeps failure categories separate. Owner records are internal testable boundary values, not invented RPC payloads. They require independently verified proof metadata; copying input context into an owner reply does not qualify an implementation. A bearer JWT alone does not establish event/operation delegation. An internal caller credential rejection, timeout or malformed dependency reply becomes unavailable authority. Authenticated permission rejection remains denial. Only explicit access credential rejection at authentication becomes `Unauthenticated`.

No Concert transport client is registered. The original BE05 production
`UnavailablePolicy` and composition remain unchanged. `UnavailableOwner` is
another explicit fail-closed placeholder, never an accepting demo identity.

The separately integrated Auth/Profile clients and owner servers now supply:

- One current Auth account/session/provider-fact snapshot through
  `ReadConcertAdmission`, behind the generated server, registered Gateway
  credential, exact actor/body/replay proof and original deadline.
- One current self-only minimal Profile content read through
  `ReadConcertSelfProfile`, with the same workload/actor/body proof, bounded
  response and cancellation/deadline semantics.
- Gateway-local strict JWT user/session/access-expiry extraction and a
  request-bound reader that checks Auth, rechecks it after the Profile await and
  excludes private wire fields.
- A joined native HTTP test that reaches both guarded generated servers and
  three isolated PostgreSQL databases from the production Router/timeout path.

Those integrations are prerequisites, not a signed grant. The reader's
observation has no constructor, Clone, serialization or conversion to
admission. `ProfileState::Active` is content lifecycle, not account or event
eligibility. Production still fails closed until a Concert verifier and current
local event policy are actually composed. See the
[joined HTTP owner evidence](../backend/http-owner-chain.md),
[Auth admission evidence](../evidence/auth-admission-rpc-integration.md),
[owner client evidence](../evidence/identity-owner-clients-integration.md) and
[selected admission direction](../identity-admission-decision.md).

## Required shared prerequisites

The integrator received the early source-grounded proposal and confirmed central ownership. Required before production qualification:

1. Concert-owned event restriction/role/policy-revision authority with current
   event scoping; no inference from Profile lifecycle or Auth roles. This remains
   BE37/BE38/BE39 and is required for operations whose policy needs it.
2. Exact provider/reference/context binding through the current owner RPC is
   integrated. Real provider login and revocation/relink qualification remain
   separate; ordinary email-only participation must not require an X row.
3. The Gateway fresh-owner-read to signed Concert-admission contract selected in
   the [admission decision](../identity-admission-decision.md). The bounded owner
   reads are integrated; the portable assertion and Concert verifier are not.
4. Concert workload/caller identity and isolated credentials/TLS; explicit
   method policies, edge budgets, propagated deadlines, bounded admission/retry
   behavior and producer/consumer negative tests. Adding Concert to generic
   allowlists alone is insufficient.
5. Centrally reviewed existing-version Cargo dependencies/generated contract artifacts and service configuration if real RPC clients are assigned. No dependency or lock resolution changed here.
6. Real isolated end-to-end admission evidence: signed request, email-only
   identity, current-session revocation/deactivation/deletion, event restriction,
   malformed and foreign proofs, provider wrong-user/replay/expiry, dependency
   outage and authenticated denial. Owner-only real evidence exists; no browser,
   authenticated Concert request or product endpoint has passed this gate.

## Owned paths and validation

Owned additions: application `src/identity/mod.rs`, application `tests/identity.rs`, service `src/adapters/out/identity/mod.rs`, service `tests/identity.rs`, this document. Exact allowed composition edits append only `pub mod identity;` to application `src/lib.rs` and service `src/adapters/out/mod.rs`. No other tracked path changes are part of this increment.

Evidence root: `/Users/lhs/Desktop/mcr-build/be04/`, fresh BE04 target directory. Rust `+1.88.0`, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `--locked`; retained dependency versions. Final commands and exit codes are recorded in `checks.json` and the corresponding logs:

- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -p concert-domain --test values`.
- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -p concert-application --tests`.
- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -p concert-service --test identity --test config --test health`.
- Rustfmt check of the four newly authored Rust files; `python3 scripts/mcr/check-plan.py`; `git diff --check`; exact assignment path and cross-directory import inspection.

Meaningful deterministic cases cover email-only authorization without provider/profile dependence, current roles, ended/unknown session, suspension/unknown eligibility, wrong actor/event/session/operation at every owner boundary, stale/future/expired proofs and expiry during awaits, exact provider/reference match, malformed/cross-user/deleted/unknown/oversized profiles, safe diagnostics, owner timeouts/outages/authenticated denial and zero repository dispatch after denial/unavailability. Email-only consumer success reaches the repository only after all required mocked authorities approve. These are mock/consumer proofs; no real multiuser or interservice PASS is implied.

Remaining status: overall goal stays active for shared production prerequisites, real service qualification and concrete integration-review fixes. A clean committed patch is a review handoff, not accepted production participation.

## Proposed shared contracts for integrator decision

This section began as a proposal grounded in source at `376cf21` and the
inspected C04 policy. The integrator later selected the Gateway-read path in
[the admission decision](../identity-admission-decision.md). The selected path
and implemented owner RPCs supersede the earlier Concert-to-owner exchange
suggestion below; remaining alternatives stay historical design evidence. The
existing mock proof records do not establish any production guarantee.

### Suspension ownership and the unresolved scope decision

Auth owns global canonical account/session admission. Concert should own event suspension keyed by `(event_id,user_id)`, event roles and event policy revision; Profile remains content/lifecycle only. An Auth-global ban and a Concert-event ban are separate owner records, not a copied participant identity or inferred Profile state. Compose current Auth account/session admission, any approved Auth-global restriction, and fresh Concert-event eligibility; denial by either scope denies the operation, an unavailable required owner returns unavailable. A healthy, authoritative owner may define absence of an active ban as eligible; timeout, absent implementation, unknown schema/state or missing policy coverage cannot mean eligible.

**Decision required:** `operations.json` routes `moderateConcertUser` under `/api/concert/events/{eventId}/admin/users/{userId}/moderation`, while `policy.json` `moderate_user` explicitly requires revocation of sessions, sockets, tickets and private grants. An event ban alone does not revoke an Auth-wide refresh family. The integrator must define whether this action deliberately invokes Auth-wide revocation, or introduce and approve distinct event/global actions and session semantics. Do not silently weaken revocation, turn an event moderator into a global administrator, restore consent on UNSUSPEND, or report a partially failed owner action as applied. BE38/BE39 must enforce current event moderator/operator authority and an immutable reason/expected revision/receipt. Event read-only state, age restrictions, venue admission and global suspension remain distinct policies.

The integrator resolved this event/global separation in
[the admission decision](../identity-admission-decision.md): event moderation,
roles, restriction revisions and grants stay event-local; Auth-wide restriction,
if introduced, is a separate Auth operation. Event release never revives an old
grant. The earlier proposal of a global suspension field remains optional future
scope: unchanged Auth account states and a healthy authoritative no-restriction
result do not become a fake `Suspended` enum. The event restriction store,
Concert-local role/revision reads, operator checks and revocation propagation
remain BE37/BE38/BE39 work, not BE04. Their absence keeps production Concert
authority unavailable for operations requiring them.

### Narrow internal RPC proposal (historical)

Prefer new, separately guarded services in `backend/proto/authority/v1/authority.proto`, leaving generic Auth/Profile method allowlists and the Post child protocol unchanged. Proposed names/fields below require central review; none exists today:

| Proposed method | Minimal request | Minimal owner result |
|---|---|---|
| `AuthConcertAuthorityService.ResolveIdentity` | Context: canonical actor `user_id`, `session_id`, `event_id`, semantic `purpose`; optional command `operation_id`. All bound to verified Concert parent/child authority. | Canonical user/session, current `session_active`, current roles, global eligibility `ELIGIBLE/SUSPENDED/UNKNOWN`, global-policy revision, owner `evaluated_at`, bounded `valid_until`. No email or reason. |
| `AuthConcertAuthorityService.CheckProviderBinding` | Same context plus existing Auth provider enum and opaque `identity_reference` (Auth `user_identities.id`, never provider subject/email). | Exact user/provider/reference, `attached`, owner binding revision, `evaluated_at`, `valid_until`; false/missing/foreign references are indistinguishable denials. |
| `ProfileConcertAuthorityService.GetBasicProfile` | Same context, own canonical target only for this increment. | `user_id`, display name from nickname, optional bio, explicit lifecycle and evaluation bounds. No URL/avatar/username/counts/visibility/email. The later producer joins an authorized C06 avatar for C03. |

Proposed context field layout, if approved: `user_id=1`, `session_id=2`, `event_id=3`, semantic `purpose=4`, optional command `operation_id=5`; root/child request `context=1` gives actor field path `[1,1]`. Purpose names map explicitly to the exact approved Concert operation. The inherited delegation `op` UUID is a command/idempotency identity, **not** the semantic purpose. UUIDs must be canonical and non-nil at the wire boundary; missing/unknown purposes deny. Response binding derives from verified parent/child extensions plus actual owner reads, never from copying unverified request fields. Return owner time/revision only when the owner actually evaluates the state; clamp validity by parent/access expiry, request deadline and relevant suspension/provider expiry, then recheck after downstream awaits.

Auth already has `user_identities.id`, `provider_type`, `provider_subject`, `email_verified` and `deleted_at`. `linked_providers` filters `deleted_at is null` but proves only a provider set, not an exact binding. `find_by_provider_subject` currently lacks the deletion filter and exposes subject/email; do not reuse it as a qualified minimal binding read. Add a dedicated owner query constrained by canonical user, exact row reference/provider, `deleted_at is null`, active account/session and applicable verification state. An owner binding revision/epoch must change on unlink/relink/deletion so old authority cannot revive a transferred or replaced claim. Provider names remain EMAIL/GOOGLE/APPLE/X; enum presence is not availability, and EMAIL must not require an X row. Auth provider binding does not verify an age-provider proof, Instagram handle, reveal consent or audience: BE26/BE29 retain those separate owners and one-use callback contracts.

### Gateway fresh reads → signed Concert admission (selected)

The selected first path is recorded in
`docs/mcr/identity-admission-decision.md`. Gateway verifies
the browser JWT, performs fresh request-bound Auth and minimal Profile reads,
binds them to canonical `sub`/`sid`/operation/event/access expiry, rechecks the
borrowed context after every owner await, and signs a short-lived assertion.
Concert authenticates Gateway, verifies the exact assertion and method/body,
checks current local event policy and revision, then constructs one sealed
per-request identity policy. It does not call back to Auth/Profile from that
same admitted root. The implemented Gateway reader is the read/join portion;
it still produces a private observation, not a grant.

Raw JWT/session/authority values stay out of URLs, logs and public DTOs. The
signature covers actor/session, event, semantic purpose, exact method/body,
request/command correlation, the owner results used, and bounded validity.
Concert's check must remain live through policy creation and must fail if its
access, request stop or local policy is unavailable or expired. Gateway
credential rejection is dependency unavailability; invalid browser credentials
are 401, authenticated denial 403 and missing required authority 503. No retry,
cache or invented success fallback may convert a failed read.

Email-only participation requires no provider or X relation. An optional exact
provider row/provider/generation check remains email-independent and fails
closed when requested. Foreign/deleted/unlinked claims cannot attach.

The following earlier Concert-to-owner exchange proposal remains historical
design evidence and is not the selected first increment:

The superseded proposal used Gateway to establish a signed root, then Concert
to exchange that root for narrowly scoped Auth/Profile child authority. A
`GatewayConcertAuthorizationExchangeService` would bind presenter, audience,
method/body digest, root digest, operation and expiry. That architecture is
retained only for a future explicit decision; it would require new methods,
policies, issuer code, owner guards, revocation and real transport evidence.
Existing Post child v2 is not a generic Concert switch.

### Remaining change map and qualification

All paths remain integrator/owner assignments. This is a change map, not a
transferred writable set. The selected first path adds a portable assertion and
Concert verifier; it does not add a second owner-read edge.

- Assertion contract/generation: a new versioned, domain-separated signed
  protobuf message containing actor/session/event/purpose, exact method/body
  digest, request/command correlation, owner-result digests, issue/expiry and
  fresh nonce. Update `backend/crates/contracts-build/src/lib.rs`, the selected
  service build inputs and central manifests/lockfile only after review.
- Platform: register `Workload::Concert`, exact Gateway-to-Concert and
  Concert-to-owner policies as needed, `GatewayConcert` edge budgets, and a
  dedicated verifier/issuer module with replay, tamper, expiry and deadline
  tests. A verified caller credential alone never creates event authority.
- Gateway: bind the existing strict HTTP context and checked Auth/Profile
  observations to the exact request, then sign only after final context and
  event-purpose rechecks. Keep raw claim values private. No owner observation
  becomes a grant before the final await recheck.
- Concert: authenticate actual Gateway transport, verify signature and exact
  method/body/actor/session/event/purpose/operation, then evaluate current local
  event policy/revision. Only the composed result becomes `IdentityPolicy`;
  absent verifier/config remains `UnavailablePolicy`.
- Event authority: restriction storage, roles, policy revision and operator
  semantics remain BE37/BE38/BE39. BE04 may consume reviewed event-policy ports
  but must not invent a second identity store or allow an event moderator to
  revoke unrelated Auth families.
- Isolation/deadlines: central Concert credentials, caller registry/TLS,
  addresses, readiness and service composition. Every nested call uses
  `min(remaining parent budget, edge budget)`. Identity reads do not retry after
  ambiguous dispatch. Aggregate/per-edge values require load-backed approval.

Required negative qualification: unregistered/spoofed workload; missing/wrong
signature/kid/audience/method/body/purpose; foreign user/session/event/command;
caller-supplied owner timestamps; assertion replay/expiry during signing or
use; current inactive/deleted/revoked/expired account/session; event A restriction
without crossing to B; provider wrong-user/replay/expiry; email-only/no X;
malformed lifecycle/private-field canaries; timeout/outage/credential failure
versus 401/403; zero repository effects on refusal; real two-user producer and
consumer tests. Production stays unavailable until the approved assertion,
current event policy and those real flows qualify.
