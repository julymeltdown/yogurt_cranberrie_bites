# Active chat ownership

Updated 2026-11-03 after the user supplied two existing chats and explicitly
authorized stopping their former tasks and assigning MCR development. After
reviewed BE02/FE02 delivery and read-only C05 review, both chats received new
assignments in fresh worktrees. Their app-level directory remains the integration checkout,
so every shell command and edit must target the assigned worktree explicitly.

| Role | Visible chat | Chat ID | Worktree / branch |
|---|---|---|---|
| Backend | `mcr(be) BE03 browser auth adapter` | `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `/Users/lhs/Desktop/mcr-worktrees/be-auth` / `codex/mcr-be-BE03` |
| Frontend | `mcr(fe) FE07 persistent world shell` | `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` | `/Users/lhs/Desktop/mcr-worktrees/fe-shell` / `codex/mcr-fe-FE07` |

Both assignments were dispatched with model `gpt-6.1-sol` and reasoning `high`.
The user's standing language instruction is **English only in every chat**, even
when the user writes in Korean. It covers replies, progress, questions, reviews,
handoffs and final answers. Both development chats received this instruction
directly; all future assignments must carry it. Preserve source text and
identifiers where exact fidelity is needed.
Both current worktrees start at `69a5624cd153a216f2398cebabf062400dce23eb`.
The user also explicitly requires persistent goals in every development chat.
Both chats have received instructions to inspect/create their own goals and
continue their current BE03/FE07 assignments through implementation, meaningful
verification, documentation, committed handoff, and integration-review fixes.
Dispatch alone does not establish that a goal was created; actual goal status
must be confirmed. Future assignments carry this requirement without resetting
aligned active goals or changing file ownership. No token budget was requested.
Both chats subsequently confirmed their BE03/FE07 goals active. The frontend
delivered `d7c720c` / `8ba0081` for review and retains ownership of scoped review
fixes; no next task or completion has yet been assigned.
The former worktrees are preserved but have no current writer. Earlier delivery
ancestry and checks are in the [foundation review](evidence/core-foundation-integration.md)
and [privacy/tooling review](evidence/privacy-local-stack-integration.md).
Both read current integration agreements; no subagent writes their worktrees.

Backend owns `backend/services/api-gateway/src/adapters/in/http/**` (explicitly
including HTTP router composition), `backend/services/api-gateway/src/config/**`
only for needed auth/origin configuration, `backend/services/api-gateway/tests/**`,
and necessary Auth producer adaptations under `backend/services/auth-service/src/**`
and `backend/services/auth-service/tests/**`. Tool/doc paths are
`backend/scripts/mcr/browser_auth_smoke.py`, `test_browser_auth_smoke.py` in that
same directory, `infra/mcr/backend/verify_local_auth.py`, and
`docs/mcr/backend/browser-auth.md`. BE03's first increment implements frozen C03
email/core routes, Origin/intent/no-store protections, honest provider capabilities
and real isolated auth qualification. Native helpers are read-only. New binaries
use `/Users/lhs/Desktop/mcr-build/be03`, preserving the qualified shared artifacts.

Frontend owns `frontend/src/app/**`, `frontend/src/main.tsx`, and
`docs/mcr/frontend/world-shell.md`. App router/entrypoint composition is an explicit
bounded delegation. FE07 builds the six-tab persistent shell, nested routes/back,
safe return targets, lazy public/admin boundaries and injected auth/bootstrap/world
lifecycle ports. Deterministic lifecycle fixtures must not become production fake
identity or world data. No shared UI/styles, assets, contracts, dependencies or
backend edits are assigned. Local app CSS is allowed; shared changes require a
concrete handoff. Actual Phaser/socket/device behavior remains later qualification.

The main chat reviews diffs, verifies file ownership and failure behavior,
integrates commits serially, updates evidence, and pushes `master`. Workers
commit only assigned paths on their task branches; they do not push or merge.
No decorative left accent lines, colored left borders or substitute left bars
are permitted. User source pack revision 3 is authoritative for future artwork
consumers; source-sheet registration is not a production atlas qualification.

Local transfer records are preserved outside Git at
`/Users/lhs/Desktop/mcr-build/handoff-be-runtime.md`,
`/Users/lhs/Desktop/mcr-build/handoff-fe-transport.md`, and
`/Users/lhs/Desktop/mcr-build/handoff-contracts-C04.md`.
The integrator resumed the stopped C04 writer's preserved drafts in
`/Users/lhs/Desktop/mcr-worktrees/contracts-C04`, advanced that worktree to
`c654507`, and integrated the qualified review candidate `b615e35`. It now has
schema/operation/policy registers, fixtures and an offline validator; the
[remaining C04 requirements](domain-contract.md) still prevent a freeze.
C05 realtime v1 is now
frozen after [both independent reviews](qa/realtime-contract-review.md); the
integrator retains ownership of shared contract changes and C04 continuation.
