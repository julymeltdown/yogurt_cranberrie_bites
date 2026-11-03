# MCR delivery and resource ownership

The human's 2026-11-04 instruction supersedes the former single-PM/report-only
coordination model. Two equal kings handle separate delivery lanes, while SLs
allocate and recycle their own workers. The existing 112 required tasks and 61
required IA entries remain in scope.

| Role | Chat | Delivery responsibility |
| --- | --- | --- |
| Backend king | `01a19e5c-e9c6-7973-a53b-a6d19141620f` | Backend and project priorities, major milestones, staffing escalation and periodic publication tracking. The `delivery-checkpoints` checkout is for management documentation, not application implementation. |
| Frontend king | `01a1a385-9f32-73c2-8715-ff54bd1e0497` | Frontend/art priorities, reference-design delivery, major dependencies and staffing through frontend SLs. Frontend root/shared implementation belongs to its assigned developer. |

The user's latest instruction removes per-commit review and publishing queues
for both kings and SLs. Every assigned implementation owner may validate, commit
and publish its exclusive scope directly to `julymeltdown/mcr` `master`.
Use separate branches/checkouts, explicit staged paths and non-force pushes.
Reconcile remote history, preserve peer commits, run affected checks and verify
remote ancestry. Report delivered SHA, paths, evidence and remaining limits.
Cross-lane contract changes keep one explicit writer; needing a shared file does
not grant ownership. Escalate actual scope conflicts, not routine publication.

The human's latest 2026-11-04 instruction assigns all implementation validation
to developers: code, queries, tests, runtime evidence and fixes. Implementation
owners choose and run the necessary checks, resolve technical interfaces directly
with relevant implementation owners and publish their exclusive validated scope.
Kings and SLs must not inspect, review or validate implementation or queries, rerun
tests, or impose a separate QA, review or approval stage. Kings track project
priorities, staffing, major milestones and periodic commit/push progress. SLs track
the broad feature picture, dependencies and staffing. Required product scope,
validation evidence and truthful persistent goals remain unchanged.

A king receiving a review or verification request must decline: "I don't perform reviews. The responsible developer owns validation; please resolve this with the implementation owner." SLs likewise do not review code, queries or tests. No work waits for king or SL review. Staffing, priority, ownership conflicts and missing product decisions may still be escalated.

## SL dispatch lanes

| SL | Chat | Ready-work allocation |
| --- | --- | --- |
| World / Map | `01a1a1fe-c276-7eb3-a36e-2f383c0e81b8` | World scene and Map staffing/dependencies; implementation owners coordinate frontend root/shared paths directly. |
| Identity / social UI | `01a1a1fe-ca12-7cc3-9a1d-62bbb2689498` | Landing, sign-in presentation, Customize and Profile |
| Community UI | `01a1a1fe-c5f1-7e60-87c2-43785b07f941` | Community feature staffing and dependencies, including Feed, Giveaway and the transferred frontend Wall backlog; coordinate Meet dependencies with Identity / social UI. |
| Backend staffing / resources | `01a1a21d-39ec-7912-84cb-910058e7a110` | Backend ready-work assignments, idle-chat lifecycle, build-target queues and routine ownership coordination. The former frontend Wall responsibility has moved to Community UI. |

SLs confirm actual turn/goal state, ownership and preservation of work with the
developers before reassignment. This is staffing coordination, not code, query
or test review. Reuse completed chats for related ready work; archive genuinely
terminal chats with no ready next task after preserving commits, unfinished files
and required backlog scope.
Do not create additional chats just to maintain a quota. Each writer receives an exact task ID, exclusive branch/worktree,
disjoint paths, implementation and validation criteria, committed handoff, and
next ready task. A historical active flag is not proof of current execution.
Do not reset models, incompatible goals or stalled processes to manufacture
activity. Preserve WIP before any supported transfer. A blocked feature should
not stop unrelated ready work. Backend staffing / resources SL manages the two
existing backend build-target queues and actual handoffs; frontend work does not
need those slots. Routine allocations do not wait for a king.

Earlier handoff instructions assigning integration to a king are historical.
Assigned implementation owners handle integration and publication directly.
Technical contracts and interface decisions belong to those owners; kings
resolve product priorities or unresolved staffing conflicts. Current allocations
belong in their lane records rather than a second central approval queue.

## Current backend feature ownership

The following assignments reuse existing chats and checkouts. They are exclusive
implementation scopes, not approval roles. Detailed path grants travel with each
assignment; narrow existing root exports preserve the other owner's additions.

| Feature owner | Chat | Writable checkout and scope |
| --- | --- | --- |
| Spatial release / canonical avatars | `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `be-world-profile-producer`: the selected-avatar increment is published; the reused chat now owns the explicitly assigned backend spatial-release/bootstrap increment and coordinates with art, World and root developers. No frontend edits. |
| World remote invalidation | `01a1a20f-8d37-71c3-a429-8a55bb6d6c49` | `be-world-invalidation`: World runtime/effects and the assigned remote-invalidation increment; consumes owner replay clients and coordinates startup wiring with the root developer. |
| Event closure backlog | `01a1a267-37a0-7723-a4e9-a3a6f5b27f52` | `be-lifecycle-resume`: bounded closure delivery is published and the terminal chat is archived. Remaining operator/retention/root requirements stay on the backend SL backlog. |
| Live participant | `01a1a209-80da-7e53-a295-b64cfe09c1d7` | `be-participant-producer`: live participant/profile/avatar/preferences orchestration; consumes canonical identity and Profile-owned mutations. Root and Profile implementations retain separate owners. |
| Auth/Friend authority stores | `01a1a1fe-16e1-7ee3-a9dc-8b5f037cc24c` | `be-owner-authority-stores`: reused after replay-client delivery for the exact Auth/Friend store/RPC/migration scope transferred by the canonical-authority developer. The previous checkout and published replay work are preserved. |
| Production owner media | `01a1a213-03de-72d3-9ecf-2f72b574cf82` | `be-media-upload`: existing media producer plus new media runtime/authority; exact tests/harness. Root exports, configuration, Gateway and proto belong to the composer. |
| Production event Feed reads | `01a1a21a-a088-7c31-83e3-19de119ba873` | `be-world-recipient-producer`: Concert Feed adapters, Post event-content read authority/RPC and necessary Post read-authority proto extension; no root/media/World edits. Former Friend RPC work is published and preserved. |
| Product API composition | `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` | `be-media-combined`: new Concert product RPC/config/proto, Gateway product routes/client, public notice-read wiring and narrow root registrations. Consumes the media/Feed implementations; does not approve their commits. Its former main-checkout ownership is revoked. |
| Canonical owner authority | `01a1a207-f012-7343-a24b-12b9c544973f` | `be-intro-age-policy`: authority lease/platform/Concert and cross-producer orchestration; the transferred Auth/Friend store scope has its own exclusive developer above. |
| Canonical public-profile projection | `01a1a205-3f89-78c3-aa67-719c4bb40b8d` | `be-operations-resume`: Profile-owned public projection/RPC/proto and Concert public-profile adapter; coordinates shared authorization registration with the authority owner, not World-specific display authority. |
| Profile self-update | `01a1a1fd-6b5d-75b0-8d42-01ade79b9393` | `be-world-projection-context`: explicitly assigned Profile self-update producer, new protocol/migration and dedicated evidence. Existing shared Profile registrations remain with their assigned writer; participant owns its calling orchestration. |

All checkout names above are under `/Users/lhs/Desktop/mcr-worktrees/`. A chat's
old app cwd is not write permission. The original `/Users/lhs/Desktop/mcr`
checkout and `delivery-checkpoints` are not assigned to any worker above.

Media/Feed scope requires genuine canonical authority and actual HTTP journeys.
Their library producers and inherited Post routes do not establish event API
readiness. Feed reads, Feed writes, owner media, linked/public media and unrelated
People/introduction/sharing/inbox capabilities must not be conflated. Missing
cross-owner authority is an implementation dependency, never permission to ship
fixture authority or a permissive capability flag. These increments do not cut
remaining project scope.

Build targets remain single-writer resources. The current named owners may pass
an explicitly released target directly to the assigned next worker after frozen
artifact copying, handle cleanup and a fresh 8 GiB free-space check. They report
the transfer to the backend staffing / resources SL; no separate king approval
is required. The SL's current queue determines the next owner, not older fixed
worker pairings in handoff notes. A worker uses copied executables for its own
socket-only PostgreSQL cluster; distinct owned native clusters may run alongside
another worker's compilation. No native test reads a mutable shared executable.
An assigned target includes the developer's scoped native qualification; no
additional king/SL approval or validation gate is introduced.

Stale old turns were interrupted with branches/WIP preserved. The BE25 chat
is now reused for canonical owner authority; the completed bounded moderation
chat is reused for public-profile projection after its prior handoff was already
integrated. BE27 introduction, BE33 notifications, BE34–35 mural, beacon-photo
and lifecycle chats have preserved backlog requirements; archival does not mark
those features complete. Existing
historical goal states are reported honestly when the goal API cannot replace an
unfinished objective; that administrative limitation does not prevent the
authorized new assignment.

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

Implementation owners perform their visual comparisons and asset/runtime
validation as part of delivery. Leads use the reported gaps to allocate work;
they do not repeat those checks or create another review gate.
Every assigned development chat retains a concrete unbudgeted persistent goal
through its implementation, relevant validation, documentation, committed delivery
and concrete fixes. Keep English communication and the user's model preferences.
