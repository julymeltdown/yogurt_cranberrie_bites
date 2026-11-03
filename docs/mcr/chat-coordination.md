# Active chat ownership

Updated 2026-11-03. The user authorized additional visible Codex chats, then
clarified that a fixed backend/frontend count is unnecessary. Four visible chats
are available; the authenticated Auth RPC has a clean committed handoff under
parent review. FE10 and the independent authority review completed bounded
review with actual COMPLETE goals and stopped checkouts. Each assigned writer has
its own worktree and branch; every shell command
and edit must explicitly target that worktree because app-level project context
can still point to the integration checkout.

| Task | Visible chat | Chat ID | Worktree / branch |
|---|---|---|---|
| BE04 | `mcr(be) BE04 canonical identity adapter` | `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `/Users/lhs/Desktop/mcr-worktrees/be-identity` / `codex/mcr-be-BE04` |
| BE04 Auth RPC | `mcr(be) BE04 authenticated Auth admission RPC` | `01a1a00c-2690-7bc0-94fc-52609655ea83` | `/Users/lhs/Desktop/mcr-worktrees/be-auth-admission-rpc` / `codex/mcr-be-auth-admission-rpc` |
| FE10 | `mcr(fe) FE10 signup and email verification` | `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` | `/Users/lhs/Desktop/mcr-worktrees/fe-signup` / `codex/mcr-fe-FE10` |
| QA | `mcr(qa) admission and event authority review` | `01a1a00c-c94e-74a2-a306-1ff686372881` | `/Users/lhs/Desktop/mcr-worktrees/qa-admission-contract` / `codex/mcr-qa-admission-contract` |

The original four increments started at `376cf21`; FE11 starts at `b39d5e5`
after the same chat completed FE08. All use `gpt-6.1-sol` with `high` reasoning
and no token budget. Actual goal tools confirmed FE08 **COMPLETE**, the new FE11
goal **COMPLETE**, BE15 **COMPLETE**, FE16 **COMPLETE**, and the original BE04 goal
**BLOCKED** on central prerequisites. The completed BE15 chat was then reused at
`5863f91` for a new Auth-owner read goal, now tool-confirmed **COMPLETE** after parent review. The old
BE15/FE16/FE08 checkouts have no writer. Dispatch alone is not proof. Each bounded goal covers implementation, relevant tests,
documentation, committed handoff and concrete integration-review fixes. An
increment goal can complete after delivery while unqualified task gates remain
open; do not silently turn that into whole-task or release acceptance.

The standing language instruction is **English only in every chat**: replies,
progress, questions, reviews, handoffs, final answers, new titles and engineering
prose, even when the user writes in Korean. Preserve exact historical source
text and identifiers. Every continuation carries this instruction and retains
an aligned unfinished goal rather than creating incompatible parallel goals.

## Exact implementation ownership

Paths below are repository-relative. Read access does not grant write access.
All central changes outside the listed exceptions remain with the integrator.

| Task | Writable paths and bounded composition |
|---|---|
| BE04 | `backend/crates/concert-application/src/identity/**`, application `tests/identity.rs`; `backend/services/concert-service/src/adapters/out/identity/**`, service `tests/identity.rs`; `docs/mcr/backend/identity.md`. Exact composition exceptions: application `src/lib.rs` identity export and service `src/adapters/out/mod.rs` identity export only. |
| BE15 | Application `src/commands/**` and `tests/commands.rs`; service `src/adapters/out/repository/{receipt,outbox}.rs`, `src/workers/outbox.rs`, `tests/command_receipts.rs`, exclusively reserved `migrations/0002_command_receipts_outbox.sql`; `backend/scripts/mcr/command_receipts_smoke.py`, `test_command_receipts_smoke.py`; `docs/mcr/backend/command-receipts.md`. No implicit ownership of surrounding repository/composition files. |
| BE04 Auth owner | Application `auth-application/src/concert_identity.rs`, `tests/concert_identity.rs`; Auth service persistence `concert_identity.rs`, `tests/concert_identity_owner.rs`; backend smoke scripts `auth_identity_smoke.py`, `test_auth_identity_smoke.py`; `docs/mcr/backend/auth-identity-owner.md`. Exact application/persistence `mod` exports only. Parent owns migration `0013`, all manifests/locks/proto, method policies, RPC/state/config/Gateway/platform composition. No writes to the stopped BE15 checkout. |
| FE11 | Auth UI `ResetPassword.tsx`, `Callback.tsx`, `recovery.css`; auth model `recovery.ts`, `recovery-contract.ts`, `callback.ts`; new auth tests `recovery.test.ts`, `callback.test.ts`, `recovery-browser-smoke.mjs`, fixtures `recovery-*`; new app `__tests__/recovery.test.tsx`; `docs/mcr/frontend/password-recovery.md`. Exact composition exceptions: `PublicBoundary.tsx` dispatch for existing two routes only; `session.ts` recovery methods/cleanup through its single canonical session/client; `cookie-policy.ts` minimal typed recovery-confirm coordination. No other existing test, root routes, shared utility or dependency ownership. |
| FE16 | `frontend/src/world/network/WorldTransport.ts`, `mock.ts`, `websocket.ts`, `__tests__/**`, optional feature-local `transport/**` helpers; `docs/mcr/frontend/world-transport.md`. Excludes FE17 `replication.ts`, world runtime/scenes/entities and application composition. |

BE04 prepares pure identity/authority ports and deterministic adapter tests.
Canonical Auth session checks, explicit provider binding and suspension authority
need shared owning-service contracts. The original writer is stopped with a
blocked goal; the Auth-owner increment is separately assigned below. Profile existence does not prove session,
suspension or binding. The production Concert policy stays unavailable until
real authenticated authority is qualified. Auth/Profile/Gateway, platform,
proto, manifest and credential changes remain integrator-owned.

BE15 pins C04 candidate hashes as design input; it does not claim C04 frozen or
expose a new public command wire. Aggregate mutation, safe receipt and outbox
must share one database transaction; replay reauthorizes, retired keys never
rerun and lease completion is fenced. It must qualify crash/retry/deduplication
with a real owned PostgreSQL cluster. Production producers and worker startup
remain disabled. Integrator prerequisites `f74fb66` and `852d414` supplied retained
dependency/module exports, integrated with the implementation as `d76215c` and
`5874c81`. They were not published alone with missing modules. Combined compilation,
layer checks and real PostgreSQL verification now pass; earlier metadata checks
alone were never a compilation claim.

FE08's reviewed public entry is integrated and its goal complete. Its former
`fe-public` checkout is stopped. FE11 now implements anonymous reset request/confirm
and honest callback error/return presentation on its separate recovery checkout.
Source reset mail emits `BASE#token=...`; deployment base/mail delivery remain
unqualified. Secrets stay transient, fragment material is removed before rendered
actions, reset confirmation clears/fences local access and uses existing cohort
coordination. OAuth success/session import remains unavailable pending a reviewed
handoff; no provider is enabled. Production legal publication remains unavailable.

FE16 delivered frozen C05 consumers, with shared mock/native transport semantics,
strict frame validation, injected ticket acquisition and owned Chrome WebSocket
fixtures. Tickets stay out of URLs, storage and logs. No production lifecycle
wiring, independent auth store, replication controller or new wire contract is
delegated. Its reviewed goal is complete and writes stopped. Mock/native parity
does not prove a real BE10/IN01 world. BE15 likewise completed its bounded internal
foundation after parent PostgreSQL/restart/cleanup review; production producers,
Auth policy and runtime startup remain unqualified. See the integration evidence
for [BE15](evidence/command-receipts-integration.md) and
[FE16](evidence/world-transport-integration.md).

Build/evidence roots are separate: `/Users/lhs/Desktop/mcr-build/be04`, `be15`
and `fe16`; FE08 records its own evidence root. Rust uses retained 1.88.0 and two
build jobs per lane. No worker starts source-default databases or changes global
services. Stagger expensive checks and avoid unnecessary duplicate build trees.

## Integration, review and queued capacity

The main chat owns shared contracts, manifests/locks, root routers, service
registration, infrastructure, review and serial integration. Only it pushes
`master`. Workers commit exact assigned paths and report base, hashes, paths,
commands/exit codes, limitations and proposed status. The integrator checks every
changed path and cross-directory import, returns concrete findings to the same
owner, then runs the affected combined checks before publishing.

The previous BE05 and FE09 increments are integrated and reviewed:
[Concert foundation](evidence/concert-core-integration.md) and
[session/sign-in](evidence/session-sign-in-integration.md). Their old worktrees
`be-concert-core` and `fe-session` are preserved with no writer. Earlier stopped
worktrees are also preserved; no subagent writes current chat worktrees. FE09
remains MOCK_READY despite its delivered increment goal completing.

FE11 and the Auth-owner read are integrated with parent candidate-bound evidence,
and both actual bounded goals are COMPLETE. Their clean checkouts are stopped.
The original BE04 goal remains BLOCKED; the parent owns authenticated composition
and the selected [Gateway fresh reads and signed admission](identity-admission-decision.md)
direction. Binding timestamps are not generations. New assignments must transfer
ownership to separate worktrees and establish actual new goals.
The [capacity assessment](parallel-capacity.md) records why another writer is not
forced. Scale from ready disjoint work and review capacity, not a chat quota.

No decorative left accent lines, colored left borders or equivalent inset/pseudo
bars are permitted. Only the user-selected revision-3 character source pack and
reviewed derivatives are authorized; registered source sheets are not a qualified
production avatar atlas. These constraints apply to every new chat.

## Current continuation ownership

FE10's bounded goal is tool-confirmed **COMPLETE**, with all three commits
integrated and parent evidence retained. Its checkout is stopped. The Auth RPC
goal remains **ACTIVE** through parent integration review of clean `9bf3233`;
the worker has stopped source/build writes and transferred the retained target
to the parent. FE10 starts
at published `e725ae4`; the Auth RPC starts at integrator prerequisite
`a44f8fa711b1ac73ce97d9e18d13dfc2eeee2382`, on top of `e725ae4`. The prerequisite
is on its isolated branch and is not published as a functioning RPC: the method
still returns fixed unavailable until the real handler is reviewed. Parent ran
103 platform authority, two live guard, two Auth policy and thirteen retained
Gateway verification-transport tests. Parent retains shared proto/platform and
will integrate the prerequisite with the worker's completed real handler.

- FE10 owns new auth UI `SignUp.tsx`, `VerifyEmail.tsx`, `enrollment.css`; model
  `enrollment.ts`, `enrollment-contract.ts`; auth tests `enrollment.test.ts`,
  `enrollment-browser-smoke.mjs`, fixtures `enrollment-preview.{html,tsx}`; new
  app `__tests__/enrollment.test.tsx`; `docs/mcr/frontend/signup-verification.md`.
  Exact existing exceptions are `PublicBoundary.tsx` dispatch/imports for the
  existing two routes and minimal canonical enrollment methods/coordination in
  `session.ts`. No cookie-policy/shared utility/manifest/contract ownership.
- Auth RPC owns new service gRPC `concert_admission.rs`, service test
  `concert_admission_rpc.rs`, backend smoke/helper scripts
  `auth_admission_rpc_smoke.py`, `test_auth_admission_rpc_smoke.py`, and
  `docs/mcr/backend/auth-admission-rpc.md`. Exact existing exceptions are one
  AppState owner-port field and real adapter injection in config, one gRPC module
  export, replacement of only the new server stub, and new-method-only policy
  tests. No proto/platform/Gateway/Concert/frontend ownership.

Production signup remains unavailable without approved eligibility, consent and
publication authority; synthetic policy fixtures may qualify the forms. Existing
account verification/resend are separate source-backed operations. No invented
challenge expiry, age policy, recorded-checkbox consent or delivery proof.
The RPC increment must qualify the actual generated Auth server behind production
guard/deadline layers and real owned PostgreSQL. Neither increment enables
production Concert authority or accepts an integration/release gate.


The completed FE16 chat was reused for an independent review on a fresh QA
checkout, base `5c02a31`, with only
`docs/mcr/qa/admission-contract-review.md` writable. Review commit `e0b37cc`
was integrated as `e103da2`; no actionable defect was found against the pinned
event correction and Auth prerequisites. Sixteen focused QA regressions and
source-pin checks passed. Its actual bounded goal is COMPLETE and checkout
stopped. The real RPC handler was outside that review; parent review remains
required. No new chats were created to satisfy a numeric quota.
