# Parallel chat capacity assessment

Date: 2026-11-03. This is an execution decision based on the current task graph,
integrated source and available chat tools, not a fixed staffing requirement.
The user explicitly authorized visible chat creation and clarified that two
backend plus three frontend chats need not be forced.

The desktop tools support creation, messaging, renaming and compact status
tracking of separate user-visible chats. Two existing chats were reassigned
after reviewed delivery; two additional project chats were successfully created.
All four acknowledged actual active persistent goals. The requested model is
`gpt-6.1-sol` with `high` reasoning. Exact IDs, worktrees, branches and ownership
are in the [coordination register](chat-coordination.md).

## Four useful assignments now

| Assignment | Why it can progress independently | Qualification boundary |
|---|---|---|
| BE04 canonical identity | The Concert foundation and frozen core contract exist. Typed authority ports, safe minimum projections and fail-closed adapter tests can progress. | BE03 is integrated, not fully accepted. Suspension/provider proof and authenticated Concert service edges still need owning-service contracts and real verification; production policy stays unavailable. |
| BE15 receipts/outbox | Its graph prerequisite BE05 is accepted. Transactional receipts, bounded claims, crash recovery and idempotent consumer effects can be tested against a fresh owned PostgreSQL cluster without BE04 production identity. | C04 remains a pinned review candidate. The assignment is an internal foundation with injected authority/producer seams, no invented public API or enabled runtime worker. |
| FE08 public entry/documents | FE05 is accepted and the delivered FE07 shell is MOCK_READY. Landing/redirect behavior and safe document presentation can use existing session ports and deterministic fixtures. | Real world lifetime remains unqualified. Approved publication content/version/retention policy and a production document contract are missing, so production documents remain unavailable. |
| FE16 mock/native transport | Its graph prerequisites C05 and FE01 are accepted. A real browser WebSocket fixture can qualify transport parity, validation and cleanup before the Concert realtime server exists. | No FE17 replication ownership, production lifecycle registration, avatar dependency or real two-user world claim. |

The worktrees start at `376cf21`. This is two backend and two frontend writers,
plus the main integrator. FE08/BE04 are explicitly bounded implementations while
their remaining semantic prerequisites are resolved; the table does not waive
task acceptance dependencies. Recheck the graph and these boundaries before
advancing to real feature composition.

## Why not add a fifth immediately

FE11 password reset/callback is a plausible next lane after the delivered FE09
increment, but shared session-model and application registration work needs a
single clear owner. Its checkout is prepared without an active writer. It can
be assigned when it adds independent throughput or when a current chat finishes.

FE10 signup needs consent/legal publication decisions. Avatar/rendering lanes
need a reviewed C06 revision and production-ready derivatives from the user's
selected sheets. Additional social/party/gift lanes would depend on unresolved
C04 policy and identity/relationship authority. Creating those chats now would
not remove the shared prerequisites. A dedicated review chat may be useful at a
concrete handoff; it need not remain idle between candidates.

## Shared changes and resource limits

Each writer has a separate worktree and exact file list, including within the
same backend/frontend lane. Only BE15 owns migration `0002`; BE04 cannot add an
uncoordinated migration. Root routers, contract inputs, service registration,
manifests, lockfiles and CI remain integrator-owned. Tiny explicitly delegated
module exports are reconciled during integration. No worker pushes master.

The first BE15 central dependency/export patch is isolated as `f74fb66` on
`codex/mcr-be15-composition`, to be cherry-picked by that worker and integrated
with its implementation. Its locked offline metadata check and lock comparison
preserve every resolved version. It cannot compile independently until the new
modules arrive and must not be published alone.

The local capacity check found 48 GiB RAM, 16 logical CPUs and about 38 GiB free
on the data volume. Four chats are feasible; these facts do not establish safe
capacity for arbitrary simultaneous Rust builds. Use two Rust jobs per lane,
retained toolchains, separate evidence/target paths, bounded owned services and
staggered heavy checks. Preserve existing qualified binaries and unrelated data.

Every assignment carries English-only communication, a persistent bounded goal,
the no-left-accent UI rule and the selected-art-only rule. Continue useful work
without waiting for matching backend/frontend completion, but reduce or reassign
idle/conflicting writers. Completion requires a committed handoff and review
fixes, not simply a chat reporting that its implementation is done.
