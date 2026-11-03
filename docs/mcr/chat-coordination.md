# Active chat ownership

Updated 2026-11-03 after the user supplied two existing chats and explicitly
authorized stopping their former tasks and assigning MCR development. Both
accepted the assignments and were observed executing in the designated
worktrees. Their app-level working directory remains the integration checkout,
so every shell command and edit must target the assigned worktree explicitly.

| Role | Visible chat | Chat ID | Worktree / branch |
|---|---|---|---|
| Backend | `mcr(be) BE02 native runtime smoke` | `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `/Users/lhs/Desktop/mcr-worktrees/be-runtime` / `codex/mcr-be-runtime` |
| Frontend | `mcr(fe) FE01 HTTP transport` | `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c` | `/Users/lhs/Desktop/mcr-worktrees/fe-transport` / `codex/mcr-fe-transport` |

Both assignments were dispatched with model `gpt-6.1-sol` and reasoning `high`.
Both start from `2d9198a684ec363c476ae75835ddf4289e47762c`. They read the current
integration working agreements in addition to their branch-local instructions.
No subagent continues writing either worktree.

Backend owns only `backend/scripts/mcr/runtime_smoke.py`,
`backend/scripts/mcr/test_runtime_smoke.py`, and
`docs/mcr/backend/runtime-smoke.md`. Its bounded task is native six-service
lifecycle/readiness, dependency failure, restart/persistence and cleanup using
owned temporary PostgreSQL/Redis resources and existing pinned binaries.
Failed reports remain preserved; successful execution is independently reviewed
before the execution record advances. Auth hardening, production packaging and
real user journeys are separate work.

Frontend owns only `frontend/src/shared/api/http.ts`,
`frontend/src/shared/api/errors.ts`, `frontend/src/shared/api/__tests__/**`, and
`docs/mcr/frontend/http-transport.md`. It implements frozen C03 same-origin
HTTP rules, memory-only session ports, bounded read refresh/replay, cancellation,
identity-generation isolation and typed errors. It must not edit backend code,
shared contracts, dependencies, routers or styles. Mock transport evidence does
not qualify server cookie rotation or cross-tab behavior.

The main chat reviews diffs, verifies file ownership and failure behavior,
integrates commits serially, updates evidence, and pushes `master`. Workers
commit only assigned paths on their task branches; they do not push or merge.
No decorative left accent lines, colored left borders or substitute left bars
are permitted. The integrator owns the current CSS correction separately from
the transport patch.

Local transfer records are preserved outside Git at
`/Users/lhs/Desktop/mcr-build/handoff-be-runtime.md`,
`/Users/lhs/Desktop/mcr-build/handoff-fe-transport.md`, and
`/Users/lhs/Desktop/mcr-build/handoff-contracts-C04.md`.
The stopped C04 writer's eight untested schema drafts remain under
`/Users/lhs/Desktop/mcr-worktrees/contracts-C04/contracts/domain/v1/` for the
integrator to continue. The integrator also owns the unaccepted C05 realtime
drafts. These drafts are not a frozen contract or an accepted task.
