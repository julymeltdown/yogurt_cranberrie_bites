# BE04 canonical identity adapter increment

Status: **IN_PROGRESS** overall; pure application and deterministic adapter/consumer behavior **MOCK_READY** after the checks below. Real interservice identity, email-only participation, suspension and provider-binding acceptance are **NOT_RUN**. This increment does not certify BE04 acceptance.

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

No transport client is registered. The original BE05 production `UnavailablePolicy` and composition remain unchanged. `UnavailableOwner` is another explicit fail-closed placeholder, never an accepting demo identity.

## Required shared prerequisites

The integrator received the early source-grounded proposal and confirmed central ownership. Required before production qualification:

1. A real owning suspension/eligibility primitive, with current actor/session/context binding; no inference from Profile lifecycle or roles.
2. An owner-backed provider-to-canonical-user claim binding read, exact provider/reference/context/time binding and revocation behavior; no X prerequisite.
3. A reviewed Gateway-to-Concert actor/event/operation authority contract, and request/response proof mapping for exact Auth/Profile reads. Existing replies lack the complete proof model used here.
4. Concert workload/caller identity and isolated credentials/TLS; explicit method policies, edge budgets, propagated deadlines, bounded admission/retry behavior and producer/consumer negative tests. Adding Concert to generic allowlists alone is insufficient.
5. Centrally reviewed existing-version Cargo dependencies/generated contract artifacts and service configuration if real RPC clients are assigned. No dependency or lock resolution changed here.
6. Real isolated Auth/Profile/Concert service evidence: email-only identity, current-session revocation/deactivation/deletion, suspension, malformed and foreign proofs, provider wrong-user/replay/expiry, dependency outage and authenticated denial. This increment has no browser, database mutation, provider seed or product endpoint.

## Owned paths and validation

Owned additions: application `src/identity/mod.rs`, application `tests/identity.rs`, service `src/adapters/out/identity/mod.rs`, service `tests/identity.rs`, this document. Exact allowed composition edits append only `pub mod identity;` to application `src/lib.rs` and service `src/adapters/out/mod.rs`. No other tracked path changes are part of this increment.

Evidence root: `/Users/lhs/Desktop/mcr-build/be04/`, fresh BE04 target directory. Rust `+1.88.0`, `CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`, `--locked`; retained dependency versions. Final commands and exit codes are recorded in `checks.json` and the corresponding logs:

- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -p concert-domain --test values`.
- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -p concert-application --tests`.
- `cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked -p concert-service --test identity --test config --test health`.
- Rustfmt check of the four newly authored Rust files; `python3 scripts/mcr/check-plan.py`; `git diff --check`; exact assignment path and cross-directory import inspection.

Meaningful deterministic cases cover email-only authorization without provider/profile dependence, current roles, ended/unknown session, suspension/unknown eligibility, wrong actor/event/session/operation at every owner boundary, stale/future/expired proofs and expiry during awaits, exact provider/reference match, malformed/cross-user/deleted/unknown/oversized profiles, safe diagnostics, owner timeouts/outages/authenticated denial and zero repository dispatch after denial/unavailability. Email-only consumer success reaches the repository only after all required mocked authorities approve. These are mock/consumer proofs; no real multiuser or interservice PASS is implied.

Remaining status: overall goal stays active for shared production prerequisites, real service qualification and concrete integration-review fixes. A clean committed patch is a review handoff, not accepted production participation.
