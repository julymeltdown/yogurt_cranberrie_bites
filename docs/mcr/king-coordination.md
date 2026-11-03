# MCR delivery and resource ownership

The human's 2026-11-04 instruction supersedes the former single-PM/report-only
coordination model. Two equal kings handle separate delivery lanes, while SLs
allocate and recycle their own workers. The existing 112 required tasks and 61
required IA entries remain in scope.

| Role | Chat | Integration ownership |
| --- | --- | --- |
| Backend king | `01a19e5c-e9c6-7973-a53b-a6d19141620f` | `backend/`, shared HTTP/proto/realtime/asset contracts, infrastructure and cross-lane contract decisions; `/Users/lhs/Desktop/mcr-worktrees/delivery-checkpoints` |
| Frontend king | `01a1a385-9f32-73c2-8715-ff54bd1e0497` | `frontend/`, selected-art derivatives, visual/FE/art documentation, frontend root/shared composition; `/Users/lhs/Desktop/mcr-worktrees/king-frontend` |

Both kings may integrate qualified handoffs and publish to `julymeltdown/mcr`
`master` without a second general review. They use separate branches/checkouts,
explicit staged paths and non-force pushes. Before publishing, reconcile current
remote history; a non-fast-forward rejection preserves both sides' commits and
requires incorporating the peer's changes with affected validation. Verify
remote ancestry. Cross-lane contract changes keep one explicit writer. A worker
does not gain shared-path ownership merely by needing it.

## SL dispatch lanes

| SL | Chat | Ready-work allocation |
| --- | --- | --- |
| World / Map | `01a1a1fe-c276-7eb3-a36e-2f383c0e81b8` | World scene and Map presentation; coordinate frontend root/shared paths with Frontend king |
| Identity / social UI | `01a1a1fe-ca12-7cc3-9a1d-62bbb2689498` | Landing, sign-in presentation, Customize and Profile |
| Community UI | `01a1a1fe-c5f1-7e60-87c2-43785b07f941` | Feed, Meet and Giveaway; Frontend king transfers this lane if actual provider availability prevents dispatch |
| Wall / operations | `01a1a21d-39ec-7912-84cb-910058e7a110` | Wall and operations frontend, relevant QA; backend dependencies go to Backend king |

SLs inspect actual turns, persistent goals and clean/dirty worktrees before
reassignment. Each writer receives an exact task ID, exclusive branch/worktree,
disjoint paths, implementation and validation criteria, committed handoff, and
next ready task. A historical active flag is not proof of current execution.
Do not reset models, incompatible goals or stalled processes to manufacture
activity. Preserve WIP before any supported transfer. A blocked feature should
not stop unrelated ready work. Native backend build slots remain separately
leased by Backend king; frontend work does not need those slots.

The canonical account-deletion frontend handoff `52c2b357`, `be4ccf9a` is assigned
to Frontend king for integration; Backend king must not integrate it twice.
Its former writer is stopped with its bounded goal complete. Existing backend
Auth journal, Friend journal and World invalidation workers remain under Backend
king. Future worker allocations belong in their lane records rather than a
second central approval queue.

## Exact visual target

The latest user-supplied PNG is preserved byte for byte as
[reference-target-2026-11-04.png](design/reference-target-2026-11-04.png), SHA-256
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
It covers landing, sign-in, World, Map, Customize, Feed, Meet, Giveaway, Wall and
Profile. The human rejected the current frontend and requires matching components
and design, not an inspired palette/frame restyle. The earlier JPG and its audit
are retained as historical evidence; prior partial-adoption checkpoints are not
visual acceptance.

Match visible layout, proportions, typography, row density, imagery, icons,
controls and composition. Keep working DOM/Phaser components and real interaction
states; a screenshot placed over a nonfunctional page does not satisfy the task.
Use the selected character sources and preserve originals. Functional authority,
privacy and transactional invariants remain; reference numbers, sample identities
and decorative controls are not permission to invent live facts or capabilities.
No decorative left accent lines. Native comparisons at 360/390/430px must expose
remaining differences honestly, and the default application must consume the
delivered components and approved resources.

Parallel design-gap, resource-state and asset-readiness checks can accompany
implementation. They identify concrete work, not another general review gate.
Every assigned development chat retains a concrete unbudgeted persistent goal
through its implementation, relevant validation, documentation, committed delivery
and concrete fixes. Keep English communication and the user's model preferences.
