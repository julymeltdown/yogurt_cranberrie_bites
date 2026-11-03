# Active chat ownership

Updated 2026-11-03. The user authorized additional visible Codex chats, then
clarified that a fixed backend/frontend count is unnecessary. Four independent
assignments are active. Each has its own worktree and branch; every shell command
and edit must explicitly target that worktree because app-level project context
can still point to the integration checkout.

| Task | Visible chat | Chat ID | Worktree / branch |
|---|---|---|---|
| BE04 | `mcr(be) BE04 canonical identity adapter` | `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `/Users/lhs/Desktop/mcr-worktrees/be-identity` / `codex/mcr-be-BE04` |
| BE15 | `mcr(be) BE15 command receipts and outbox` | `01a1a00c-2690-7bc0-94fc-52609655ea83` | `/Users/lhs/Desktop/mcr-worktrees/be-commands` / `codex/mcr-be-BE15` |
| FE08 | `mcr(fe) FE08 public entry and documents` | `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` | `/Users/lhs/Desktop/mcr-worktrees/fe-public` / `codex/mcr-fe-FE08` |
| FE16 | `mcr(fe) FE16 world transport` | `01a1a00c-c94e-74a2-a306-1ff686372881` | `/Users/lhs/Desktop/mcr-worktrees/fe-world-transport` / `codex/mcr-fe-FE16` |

All four assignments start at `376cf2127b5ae5b81fa8743293ec9cc0079f6bfd`,
use requested model `gpt-6.1-sol` with `high` reasoning, and have confirmed actual
**ACTIVE persistent goals**, with no requested token budget. Dispatch alone was
not counted as proof. Each goal covers bounded implementation, relevant tests,
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
| FE08 | `frontend/src/features/public/**`; exact app `PublicBoundary.tsx`, public replacement in `FoundationApp.tsx`, new `__tests__/public-entry.test.tsx`; `docs/mcr/frontend/public-entry.md`. Separately committed compatibility allowance: app `__tests__/shell.test.tsx` obsolete home assertions and app `__tests__/browser-smoke.mjs` obsolete landing selector only. |
| FE16 | `frontend/src/world/network/WorldTransport.ts`, `mock.ts`, `websocket.ts`, `__tests__/**`, optional feature-local `transport/**` helpers; `docs/mcr/frontend/world-transport.md`. Excludes FE17 `replication.ts`, world runtime/scenes/entities and application composition. |

BE04 prepares pure identity/authority ports and deterministic adapter tests.
Canonical Auth session checks, explicit provider binding and suspension authority
need shared owning-service contracts. Profile existence does not prove session,
suspension or binding. The production Concert policy stays unavailable until
real authenticated authority is qualified. Auth/Profile/Gateway, platform,
proto, manifest and credential changes remain integrator-owned.

BE15 pins C04 candidate hashes as design input; it does not claim C04 frozen or
expose a new public command wire. Aggregate mutation, safe receipt and outbox
must share one database transaction; replay reauthorizes, retired keys never
rerun and lease completion is fenced. It must qualify crash/retry/deduplication
with a real owned PostgreSQL cluster. Production producers and worker startup
remain disabled. Integrator prerequisite `f74fb66` on isolated branch
`codex/mcr-be15-composition` supplies exact dependency/module exports for the
worker to cherry-pick; it requires the worker's modules and must not be published
alone. Locked offline metadata and unchanged resolved versions were verified;
that is not a standalone compilation claim.

FE08 builds newly authored public entry and a safe versioned-document presenter.
Production publication is unavailable until approved legal/safety/contact copy
and its authoritative contract exist. Synthetic documents are test-only. A ready
canonical session redirects home to `/world`; anonymous entry loads no world
runtime or socket. Root routing/main/shared auth are outside this assignment.

FE16 consumes frozen C05 read-only, with shared mock/native transport semantics,
strict frame validation, injected ticket acquisition and owned Chrome WebSocket
fixtures. Tickets stay out of URLs, storage and logs. No production lifecycle
wiring, independent auth store, replication controller or new wire contract is
delegated. Mock/native parity does not prove a real BE10/IN01 world.

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

The prepared `/Users/lhs/Desktop/mcr-worktrees/fe-recovery` /
`codex/mcr-fe-FE11` has **no chat and no active writer**. Password reset/callback
work is queued until its shared auth integration can be assigned usefully.
The [capacity assessment](parallel-capacity.md) explains why four lanes are
useful now and why a fifth is not forced. Scale from ready disjoint work and
review capacity, not a fixed chat quota.

No decorative left accent lines, colored left borders or equivalent inset/pseudo
bars are permitted. Only the user-selected revision-3 character source pack and
reviewed derivatives are authorized; registered source sheets are not a qualified
production avatar atlas. These constraints apply to every new chat.
