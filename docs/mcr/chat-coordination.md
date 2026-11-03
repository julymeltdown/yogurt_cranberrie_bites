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
The user's standing language instruction is **English only in every chat**, even
when the user writes in Korean. It covers replies, progress, questions, reviews,
handoffs and final answers. Both development chats received this instruction
directly; all future assignments must carry it. Preserve source text and
identifiers where exact fidelity is needed.
The first handoffs started from `2d9198a684ec363c476ae75835ddf4289e47762c` and
were reviewed and integrated through `360adfd`; see the
[integration review](evidence/core-foundation-integration.md). Current backend
work starts from its handoff `0fb22af06fbc139a8f9b6740d8cf780eaa55557a`;
frontend starts from `d76cbb5ca7ff419cf8c4e6ac1d587e4c69c42644`. Both read current
integration working agreements in addition to branch-local instructions. No
subagent continues writing either worktree. The table records the last observed
chat titles; the new task scopes below supersede the titles' former scope.

Backend now owns only `backend/scripts/mcr/dev_stack.py`,
`backend/scripts/mcr/test_dev_stack.py`, `infra/mcr/backend/**`, and
`docs/mcr/backend/local-development.md`. The infrastructure subtree is an explicit
bounded delegation by the integrator. Its BE02 work audits image/protobuf/SQLx
paths and builds an owned foreground local-development stack. Runtime/migration
helpers are read-only inputs; changes require a concrete request to the parent.
Docker execution remains NOT_RUN when its daemon is unavailable; no global
Docker restart is authorized. Auth hardening and real user journeys follow.

Frontend now owns only `frontend/src/shared/query/keys.ts`,
`frontend/src/shared/query/privacy.ts`, `frontend/src/shared/query/__tests__/**`,
and `docs/mcr/frontend/query-privacy.md`. Its FE02 task uses actual TanStack
Query with canonical user/event/generation keys, cancellation plus eviction,
observer-visible cleanup, private draft ownership and late-result suppression
after logout/account/event or policy changes. FE01 is a read-only input; report
concrete required changes to the parent. No backend, shared contract, dependency,
router or style edits are assigned. Mock cache evidence does not qualify real
cookie rotation or cross-tab behavior.

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
The stopped C04 writer's eight untested schema drafts remain under
`/Users/lhs/Desktop/mcr-worktrees/contracts-C04/contracts/domain/v1/` for the
integrator to continue. The integrator also owns the unaccepted C05 realtime
drafts. These drafts are not a frozen contract or an accepted task.
