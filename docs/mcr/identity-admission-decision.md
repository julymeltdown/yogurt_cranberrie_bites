# Identity admission integration direction

Date: 2026-11-03. Integrator decision for the next BE04 increment, based on
the [identity handoff](backend/identity.md) and current owning source. This selects
implementation boundaries; it does not freeze a new wire, enable production
policy, or claim that the outstanding authority contracts already exist.

## Ownership and scope

Auth owns canonical account UUID, current account eligibility, login-session
family and exact live provider binding. Profile owns allowlisted content and its
lifecycle. Concert owns event-local participation, roles, restriction state and
admission/revocation epochs. Do not copy identity into a parallel store or infer
an authenticated session, suspension or provider consent from Profile existence.
Email-only admission has no X/provider prerequisite.

An event-path moderation action applies only to that event: it must fence that
event's membership, sockets, tickets and private grants. It cannot revoke unrelated
Auth families or convert a local operator into a global operator. Unsuspension
does not revive old grants or tickets. The ambiguous C04 suspension wording is
clarified in the [event moderation candidate correction](evidence/event-moderation-scope.md).
Its public producer still requires actual owner/epoch/invalidation qualification.
Global account restriction, if introduced, remains a separately authorized Auth
operation; no silent `ROLE_ADMIN` mapping to event/global operator is permitted.

## Selected first path

Gateway performs fresh authenticated owner reads and produces a narrowly signed,
request-bound admission to Concert. Concert verifies the Gateway workload,
signature and actor/request binding, then checks its current local event policy
and revision before dispatching its transaction. This avoids introducing new
Concert-to-Auth/Profile edges and a child-exchange protocol in the first increment.
Existing Post exchange policy is not widened as a shortcut.

Gateway verifies the browser JWT subject, session family and access expiry itself;
browser-provided actor/event/role claims are not authority. Its owner read is
uncached for this admission. Only necessary Profile content is joined, and all
earlier evidence is rechecked after slower awaits. The eventual signed admission
must cover actor/session, event, purpose, method/target, canonical body digest,
request/command correlation, evaluated owner results and their validity bounds.
Bearer/refresh tokens, provider subjects, email and private Profile fields never
cross into that admission. This is a Gateway assertion after owner reads, not an
Auth-signed transferable capability.

One original wall/monotonic request budget spans owner/channel waits and signing.
No later edge starts a fresh full budget. Effective expiry is bounded by that
original deadline, browser access expiry and every owner/local evidence limit.
Use the retained `platform::actor_delegation::authority_deadline` semantics when
composing the RPC path. No numeric revocation SLO is invented here. A completed
read cannot guarantee instantaneous later revocation: the real gate needs an
explicit bound plus fresh checks/epoch fencing and socket revalidation or a
qualified invalidation protocol, failing closed on a gap.

## Current source evidence and missing pieces

- `auth/v1/auth.proto` CheckSession accepts user/family and returns active/current
  roles. Auth LoginService checks ACTIVE account, a live family and roles using
  separate awaits. It has no event restriction or exact provider-reference result.
  Auth's current UserStatus is PendingVerification/Active/Deleted; no new global
  suspension state is assumed from that closed model.
- `auth-service/.../persistence/user_repository.rs` linked_providers filters
  deleted identities but returns only a provider set. find_by_provider_subject
  returns raw subject/email and lacks the deleted filter. Neither is the exact
  minimal current claim read needed here. Identity-row ID/relink immutability and
  verified binding semantics must be established from owning writes and tested.
  Reconnaissance confirms relink preserves row ID and changes linked_at; that
  transaction-stable timestamp is insufficient. Parent migration `0013` assigns
  a fresh bounded owner sequence epoch on every identity INSERT/UPDATE, ignoring
  caller-supplied generations. No fake revision is synthesized from context.
- `gateway-domain/src/high_risk.rs` recognizes root `/admin/...` and two account
  actions. Nested Concert event administration is not covered. The existing
  high-risk guard caches positive session answers; it is not a per-invocation
  admission read. Its existing Auth client starts a fresh edge timeout; that call
  must not be reused unchanged as a multi-await original-deadline proof.
- Concert is absent from the retained Workload enum and has no product RPC adapter.
  Adding its name to an allowlist alone does not establish actor/request authority.

The Auth application/persistence read and authenticated internal RPC are now
integrated with candidate-bound native evidence. Owner read `c300a23` supplies
the minimal snapshot; prerequisites `a44f8fa`/`e79fd177`, implementation `9bf3233`
and test correction `5c59f1f` are integrated and reviewed together. The final
combined candidate `97a698a` passes eleven controlled generated-server cases,
five real RPC and 22 retained owner PostgreSQL cases, migration/reapply/restart
and cleanup. See [RPC integration evidence](evidence/auth-admission-rpc-integration.md).
These are Auth-owned facts, not public admission or an accepting Concert adapter.

The next Gateway prerequisite must retain authenticated JWT access expiry as well
as canonical subject/family. The existing `JwtVerifier::verify_access_token`
checks the token through configured signing keys but returns legacy
`VerifiedAccessToken` with only subject, roles and optional session ID; that
projection does not retain signed expiry. It cannot serve unchanged as the
multi-await original-deadline context. Preserve existing routes while introducing
and testing a narrow strict context from the verified decoder: mandatory
canonical user/family, actual signed expiry, original monotonic stop, redacted
nonportable state and rechecks after waits. No browser-supplied actor/role may
construct it. This prerequisite alone grants no owner result or Concert authority.

Parent retains proto/platform/Gateway/Concert contracts and composition. The
next exact assignment must use a fresh worktree/goal, keep the completed Auth
checkout stopped and preserve [ownership boundaries](chat-coordination.md).

Required qualification includes foreign actor/family/reference, deleted/unlinked/
relinked claims, expired/revoked account/session, owner outage, role/epoch changes,
slow owner awaits, cross-event moderation, queued expiry and private-field canaries.
Real email-only Auth/Profile/Concert admission and complete revocation remain
NOT_RUN. The existing production `UnavailablePolicy` stays in place until the
authenticated path and these gates pass.
