# Event authority and Auth admission prerequisite review

Date: 2026-11-03. Independent bounded QA review. **No actionable defect found in
the two reviewed changes.** This decision permits continued implementation and
integration review of these prerequisites; it does not accept C04 or certify an
operating admission/moderation flow.

## Scope and source identity

The QA checkout is `/Users/lhs/Desktop/mcr-worktrees/qa-admission-contract`, branch
`codex/mcr-qa-admission-contract`, clean starting base
`5c02a31f26d2822f274bb09743178295fdbd110f`. Its only assigned writable path is
this review document. The former FE16 implementation assignment remains stopped.

Two independent source increments were reviewed:

1. Event moderation correction
   `5c02a31f26d2822f274bb09743178295fdbd110f`: domain policy, schema, operation
   registry, generated OpenAPI, assembler guard, adversarial regression and
   associated evidence/decision documentation. This is a candidate contract
   clarification, not an accepting runtime producer.
2. Auth internal admission RPC prerequisites, read only in
   `/Users/lhs/Desktop/mcr-worktrees/be-admission-rpc-prereq`:
   `a44f8fa711b1ac73ce97d9e18d13dfc2eeee2382`, followed by test correction
   `e79fd177c44ca4c18621ce23560eb7007e3e48fe`. The latter was the clean checkout
   HEAD inspected. Their parent is
   `e725ae4e1c511b299619de8c0f5e343e3daf640e`. This branch is not presumed to
   contain the subsequent event-scope correction or the active worker's real
   RPC handler.

The prerequisite changed-path inventory is limited to:

- `backend/crates/platform/src/actor_delegation.rs`
- `backend/crates/platform/src/actor_delegation/read_authority.rs`
- `backend/proto/auth/v1/auth.proto`
- `backend/services/api-gateway/tests/verification_transport.rs`
- `backend/services/auth-service/src/adapters/in/grpc/method_policy.rs`
- `backend/services/auth-service/src/adapters/in/grpc/server.rs`
- `docs/mcr/backend/auth-admission-rpc-contract.md`

Review authority remains the implementation specification and current project
working agreements. `docs/mcr/identity-admission-decision.md`, project assessment,
execution plan and the Auth RPC composition contract provide integration context;
they do not silently accept omitted specification outcomes.

## Findings and disposition

There are no actionable P0–P3 findings against the pinned changes. No correction
patch is requested by this review. The following checks explain that bounded
conclusion and identify where a static declaration stops short of runtime proof.

### Event moderation correction

| Check | Source and conclusion |
|---|---|
| Exact event scope | `contracts/domain/v1/operations.json:5785` registers the event/user path with `authority_scope: PATH_EVENT_ONLY`; generated OpenAPI carries that same extension. No global account operation was added. |
| Fresh authority and atomic fence | `contracts/domain/v1/policy.json:5` requires fresh event roles/restrictions and same-transaction revision checks; `:920` requires the exact event role, current target/event-restriction revision and operator-peer protection. A global Auth role cannot stand in for event authority. |
| Suspension and release | `policy.json:927` advances the event admission/revocation epoch and invalidates that event's derived authority. Auth refresh families and other events explicitly remain unchanged. UNSUSPEND requires fresh authorization and restores no prior tickets, sessions, grants or consent. The structured `event_user_moderation` block at `:1177` agrees. |
| Schema meaning | `contracts/domain/v1/admin.schema.json:342` describes an event-local restriction; `:389` distinguishes event suspension from owning account deletion states. The request has no new global account/family controls. |
| Assembly drift guard | `backend/scripts/docs/generate-mcr-domain.py:131` rejects nonlocal scope, an unscoped path and an Auth-admin actor classification for `moderate_user`. `scripts/mcr/tests/test_domain_bundle.py:48` exercises all three mutations and the valid generated extension. This guards those assembly inputs, not live authorization or arbitrary future semantic policy edits. |
| Public/private separation | Existing common guards preserve canonical path binding, safe existence errors and no-store personalized/error responses. `UserReview` is an authorized admin projection, not a public account-status API. No handle, credential or new global lifecycle mutation was introduced. |

The current `UserReview` still requires `PublicProfile` even for deletion states.
This patch does not define the full terminal tombstone/redaction or global
deletion projection. Those are already listed before C04 freeze in
`docs/mcr/domain-contract.md:177`; the added state description must not be read
as closing them. Likewise, declarative epoch advancement does not demonstrate
server transactions, cross-event runtime isolation, stream/cache invalidation,
expiry enforcement or a revocation service-level guarantee.

### Auth internal RPC prerequisite

| Check | Source and conclusion at `e79fd177` |
|---|---|
| Narrow internal shape | `backend/proto/auth/v1/auth.proto:220` takes canonical actor, exact JWT session-family reference and optional exact owner binding. The response contains observation time and account/session/binding facts. It exports no bearer/refresh credential, provider subject, profile, event role or reusable signed capability. Binding absence supports ordinary email-only admission orchestration. |
| Caller/method/actor binding | Auth method policy at `backend/services/auth-service/src/adapters/in/grpc/method_policy.rs:10` permits only Gateway and binds protobuf field 1 to the signed actor. `platform/src/grpc_method_policy.rs:33` authenticates the registered caller before method policy; `:297` requires delegation before dispatch. No new workload or Concert caller allowlist was introduced. |
| Context construction order | `backend/crates/platform/src/actor_delegation.rs:658` clears any previous read context and captures start/original deadline before bounded body collection. Signature, issuer/caller, audience, full method and body digest checks precede actor-field binding and replay consumption at `:692–711`. Only then is `VerifiedReadAuthority` installed. |
| Encapsulation | `actor_delegation/read_authority.rs:8` keeps context fields private and its capture constructor restricted to the owning module. Public methods expose actor and bounded deadline, not the signed token. Debug output is redacted. This is trusted server request context, not a portable browser credential. |
| Original budget and expiry | `read_authority.rs:45` refuses a missing original deadline and takes the minimum of original stop, guard-entry ceiling and captured proof stop. `actor_delegation/authority_deadline.rs:13` and `:53` also retain current signed wall expiry and enforce monotonic expiry. Earlier buffering/dispatch waits cannot restart the handler resource ceiling. An already expired root proof tolerated by legacy skew receives no strict read context. |
| Corrected monotonic regression | `e79fd177` changes only the test setup at `read_authority.rs:125`: a short captured monotonic stop elapses while the signed wall interval stays live, then deadline derivation must fail. It does not change production expiry behavior. |
| Production registration | `backend/services/auth-service/src/main.rs:84` installs the caller-deadline layer around the generated Auth service registered through `guard()` at `:95`. Source tracing supports availability of the original task deadline at verification; qualification of the actual handler remains separate. |
| Honest implementation boundary | `backend/services/auth-service/src/adapters/in/grpc/server.rs:23` returns fixed `Unavailable`. The prerequisite does not yet parse canonical IDs/binding, enforce the handler's two-second ceiling, call the real owner, sanitize owner errors, or recheck late completion. These are explicit obligations of the assigned subsequent implementation, not delivered behavior of the stub. |

The existing verifier's generic actor-field parser accepts UUID spellings that
the actual handler must reject if noncanonical. Provider closure, positive owner
binding epoch, exact user/session/binding ownership, response validity and current
negative facts also require that implementation. The prerequisite does not claim
these checks are already implemented by proto comments or a method-policy test.

## Evidence inspected and independently rerun

Commands below ran from the isolated QA checkout with bytecode writes disabled.
No Rust builds, shared build-target writes, database services, containers,
browser profiling or application implementation edits were performed.

| Independent QA command/check | Result |
|---|---|
| `python3 -B -m unittest discover -s scripts/mcr/tests -p test_domain_bundle.py -v` | Exit 0; 16/16 tests pass, including all three event-scope mutations, unchanged core paths/definitions and deterministic assembly. |
| `PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/docs/generate-mcr-domain.py --check` | Exit 0; `MCR domain HTTP REVIEW_CANDIDATE checked`. |
| Python `hashlib.sha256` comparison of all six paths in `docs/mcr/evidence/event-moderation-scope.json` | Exit 0; all six exact source/generated/test hashes match. |
| Git source/ancestry and changed-path inspection in both clean checkouts | Exit 0; source pins and prerequisite path inventory above confirmed. |

The event correction retains its earlier full contract run in
`docs/mcr/evidence/event-moderation-scope.{md,json,log}`: exit 0 and 138 regressions
(21 asset, 23 core, 31 realtime, 47 domain, 16 bundle). This review checked its
source hashes and reran the focused affected suite; it did not repeat that full
run or turn its fixture examples into actual multiuser evidence.

Parent-supplied Rust logs were read from `/tmp/mcr-admission-rpc-prereq`. They
record successful test summaries; this reviewer did not rerun those commands.
The logs are temporary external evidence and do not themselves contain an
immutable Git source receipt. The exact contents inspected have these hashes:

| Log | Recorded result | SHA-256 |
|---|---|---|
| `authority.log` | 103 passed, 0 failed, 77 filtered | `181ff32a582d9669c2d557f90f70c8009e7b59bdab186f6bbe0a9a2efd3abe17` |
| `guard.log` | 2 live generic guard tests passed | `3063929063a7846972c96707e3c65207bd3f4b1152206dee2108579e401794b9` |
| `auth-policy.log` | 2 generated proto/actor-policy tests passed | `3db4032f8a684fafc5adaad8fc0253fcf0a8b376a09faa6e0436ded1176b62df` |
| `gateway-regression.log` | 13 verification transport tests passed | `e01421dcf12bcd661b724c0e5213261aa6839fcdb4a8ee590572804f65d273f1` |

The generic live guard tests do not qualify the new Auth method's real handler.
Its next review must cover production guard/deadline composition, denial before
owner dispatch, real isolated PostgreSQL owner reads, malformed canonical input,
exact session/binding checks, missing/elapsed original deadlines, actual proof
expiry, replay, owner failure and delayed success. Successful owner facts alone
do not qualify Gateway JWT validation, Profile eligibility, Concert-local roles
and revisions, signed admission, production revocation or multi-replica replay.

## Handoff and acceptance limits

The review assignment's persistent goal was created and its actual status checked
as ACTIVE. It remains active for the integrator's review and any required fixes;
a sent handoff is not evidence of accepted completion. Only this document is
committed on the QA branch, as a descendant of the stated clean QA base. The
handoff supplies its exact commit; the integrator alone may publish reviewed work
to master. No source archive, contract, implementation, dependency or build
configuration is changed by this review.

C04 remains **REVIEW_CANDIDATE / NOT_ACCEPTED**, with domain runtime
**NOT_IMPLEMENTED**. The RPC prerequisite remains an unavailable stub until the
separately assigned real implementation and its required checks are delivered.
This review grants neither whole-task acceptance nor release readiness.
