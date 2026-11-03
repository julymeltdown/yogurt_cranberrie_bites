# Signed Concert bootstrap authority and Gateway client review

Status: **Scoped boundary review PASS; candidate verified, awaiting integrator disposition.** This is a bounded architecture and transport implementation review of the signed Concert bootstrap authority and Gateway client (commits `34d4bb6` and `61c3e1f`), not project-wide BE04/BE06 or live Gateway routing acceptance.

## Candidate and scope

Review checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-bootstrap-boundary`, branch `codex/mcr-qa-bootstrap-boundary`, base `4d6fc85b710f69a73b6942374308941d8f7a7c38`.
Evaluated commits:
- `34d4bb614325b915569489b59e6deefba88a002f`: Define signed Concert bootstrap RPC with bounded Gateway authority
- `61c3e1fb2057584bf798442359be03a2390c5588`: Dispatch bounded signed bootstrap after current Auth and Profile reads

This report (`/Users/lhs/Desktop/mcr-worktrees/qa-bootstrap-boundary/docs/mcr/qa/bootstrap-boundary-review.md`) is the reviewer's sole tracked write. No application source, proto definitions, manifests, or build scripts were modified by the reviewer. Main checkout was accessed read-only. No git push was performed. Per review instructions, Cargo compilation and PostgreSQL execution were strictly prohibited in this review worktree to preserve shared native compilation target ownership for the BE06 bootstrap implementer and integrator.

Inspected paths across the candidate:
- Platform: `backend/crates/platform/src/grpc_auth.rs`, `backend/crates/contracts-build/src/lib.rs`
- Protocol: `backend/proto/concert/v1/concert.proto`
- Gateway: `backend/services/api-gateway/src/adapters/out/concert_client.rs`,
  `backend/services/api-gateway/src/adapters/out/mod.rs`,
  `backend/services/api-gateway/tests/concert_identity_composition.rs`,
  `backend/services/api-gateway/tests/support/concert_bootstrap_client.rs`,
  `backend/services/api-gateway/build.rs`
- Concert Service: `backend/services/concert-service/src/rpc.rs`,
  `backend/services/concert-service/tests/bootstrap_authority.rs`,
  `backend/services/concert-service/build.rs`,
  `backend/services/concert-service/Cargo.toml`
- Documentation: `docs/mcr/backend/bootstrap-admission-contract.md`

---

## Security and contract audit

Read the BE04/BE06 specification, the frozen C03 bootstrap contract, and the implementation in `34d4bb6` and `61c3e1f`:

1. **Workload and caller policy (Default Deny)**:
   - Added `Workload::Concert` to `platform::grpc_auth::Workload` (total 7 workloads, stable wire identifier `"concert"`).
   - In `concert_service::rpc::POLICY`:
     - `service: "concert.v1.ConcertService"`
     - `workload: Some(Workload::Concert)`
     - `methods: &[("Bootstrap", &[Workload::Gateway])]`
     - `actor_fields: &[("Bootstrap", &[&[1]])]`
   - Sibling callers (e.g. `Workload::Post`), unauthenticated callers, invalid service tokens, or non-Gateway workloads are rejected at the transport guard (`VerifiedCaller(Workload::Gateway)` required).
2. **Actor and payload delegation binding**:
   - The Ed25519 signature generated via `ActorDelegationSigner` signs the exact protobuf body, RPC method (`/concert.v1.ConcertService/Bootstrap`), actor ID, and audience (`Workload::Concert`).
   - `authorize_bootstrap` strictly validates:
     - `actor == user_id` and `proof.actor() == user_id`.
     - `http_method == "GET"` and `http_target == format!("/api/concert/events/{event_id}/bootstrap")`.
     - Non-nil, canonical UUID formatting for `user_id`, `event_id`, `session_family_id`, and `request_id`.
     - Public profile limits: non-empty trimmed display name <= 40 chars with no control characters; optional bio <= 160 chars with no control characters.
     - Tampered bodies or mismatched actor/target values are rejected immediately before reaching the producer.
3. **Observation age, budgets, and deadline composition**:
   - `BOOTSTRAP_BUDGET` is set to Duration 2 seconds.
   - Gateway bounds assertion expiry:
     `expires = original.expires_micros().min(context.access_expires_micros()).min(now + remaining).min(auth_at + 2s).min(profile_at + 2s)`.
   - `authorize_bootstrap` enforces:
     - `observed <= now` and `expires > observed`.
     - `expires.saturating_sub(observed) <= BOOTSTRAP_BUDGET (2s)`.
   - The original received deadline is preserved via `AuthorityRpcDeadline::within_original`.
   - `VerifiedBootstrap::check()` evaluates remaining deadline against the original ceiling and is required after every await and before commit. Later awaits cannot reset or extend an expired budget.
4. **Stale reply and response shape/binding validation**:
   - `GrpcConcertClient::bootstrap` verifies the response:
     - Rejects any response without `cache-control: no-store`.
     - Enforces response size limit (non-empty and <= 4MB).
     - Validates returned C03 response binding and top-level shape: `contract_version == "0.1.0"`, `event.event_id == requested_event`, `participant.event_id == requested_event`, `participant.user_id == actor`, `participant.profile.user_id == actor`. (Note: this is binding and shape validation against the expected actor/event, not full deep C03 schema validation, which remains with the client/contract layer).
     - Mismatched actor, mismatched event, or cacheable replies throw `BootstrapError::InvalidResponse`.
     - Checks identity and deadline before and after gRPC dispatch, preventing commit of stale replies.
5. **No retries and privacy metadata hygiene**:
   - Configured `EdgeBudget` uses `RetryClass::Never` and `FailurePolicy::FailRequest`.
   - Private tokens, emails, provider subjects, and sensitive profile metadata are excluded from `BootstrapRequest`.
   - `VerifiedBootstrap` implements custom `Debug` redacting internal contents (`"VerifiedBootstrap([REDACTED])"`).
6. **Stack overflow remediation in nested futures**:
   - Nested client futures in `GrpcConcertClient` and `concert_bootstrap_client.rs` are boxed (`Box::pin(...)`), resolving the nested future stack-size overflow without altering global stack limits.

---

## Command execution vs inspected evidence attribution

To maintain strict attribution between reviewer-executed checks and parent/worker-executed evidence:

1. **Reviewer-executed commands in this worktree**:
   - `python3 scripts/mcr/check-plan.py`: exit 0, PASS (118 tasks, 64 IA entries, planning provenance intact).
   - Read-only source code inspection of all candidate files and tests listed above.
   - `git log` and `git show` diff inspections across commits `34d4bb6` and `61c3e1f`.
   - **No Cargo or PostgreSQL commands were executed by the reviewer**, adhering strictly to the shared target restriction.
2. **Source-inspected test suites (NOT rerun by reviewer; parent-verified historical evidence)**:
   - **Concert Service Authority Tests (`backend/services/concert-service/tests/bootstrap_authority.rs`)**:
     - Inspected source code for `authentic_current_bound_request_reaches_the_producer` (1 call dispatched).
     - Inspected source code for `invalid_workload_signature_binding_or_budget_never_reaches_the_producer` covering 7 rejection scenarios (unsigned, credential, sibling, audience, actor, tampered, no-deadline).
     - Inspected source code for `signed_but_invalid_owner_assertions_are_refused` covering 9 rejection scenarios (event, target, method, name, bio, expired, future, stale, family).
     - Observed evidence source: documented pass in commit `34d4bb6` / `docs/mcr/backend/bootstrap-admission-contract.md` (`cargo test -p concert-service --test bootstrap_authority` reported 3 tests PASS).
   - **Gateway Client Composition Tests (`backend/services/api-gateway/tests/support/concert_bootstrap_client.rs`)**:
     - Inspected source code for `bootstrap_fresh_owner_reads_produce_one_signed_bound_concert_request` (verifies Auth -> Profile -> Concert chain, 1 call each, no private leaks).
     - Inspected source code for `bootstrap_auth_refusal_suppresses_both_profile_and_concert` (auth refusal halts chain before profile/concert).
     - Inspected source code for `bootstrap_rejects_foreign_or_cacheable_results_and_preserves_original_deadline` (foreign user ID rejection, cacheable header rejection, and original deadline timeout expiration with zero automatic replays).
     - Observed evidence source: documented pass in commit `61c3e1f` / `docs/mcr/backend/bootstrap-admission-contract.md` (`cargo test -p api-gateway --test concert_identity_composition` reported 11/11 tests PASS following future boxing).

---

## Boundaries and limitations

1. This review covers the internal RPC contract, transport authorization guard, and Gateway client adapter in commits `34d4bb6` and `61c3e1f`.
2. The concrete transactional producer (BE06 database tables, migration 0004, and participant/avatar creation under PostgreSQL transaction locks) is not part of this candidate.
3. Public HTTP router wiring from Gateway to external browsers remains to be composed; these tests certify internal service-to-service communication.
4. Goal remains ACTIVE through integrator review disposition.

