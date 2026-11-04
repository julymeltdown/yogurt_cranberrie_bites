# Hourly SL task progress

The user's 2026-11-04 instruction requires each SL to determine the individual
progress of its assigned tasks every hour. Four active chat heartbeats implement
that cadence. The existing project coordination heartbeat remains separate.

| Lane | SL chat | Automation |
| --- | --- | --- |
| Backend staffing and resources | `01a1a21d-39ec-7912-84cb-910058e7a110` | `mcr-backend-hourly-task-progress` |
| World and Map | `01a1a1fe-c276-7eb3-a36e-2f383c0e81b8` | `mcr-world-and-map-hourly-task-progress` |
| Identity and Social | `01a1a1fe-ca12-7cc3-9a1d-62bbb2689498` | `mcr-identity-and-social-hourly-task-progress` |
| Community and Wall | `01a1a1fe-c5f1-7e60-87c2-43785b07f941` | `mcr-community-and-wall-hourly-task-progress` |

Each SL maintains one row per original task assigned to its lane, including
waiting tasks. A row records:

- Original task ID and actual implementation owner.
- Current task-level disposition and the timestamp/source of that report.
- Concrete work delivered since the previous check.
- Remaining original acceptance criteria.
- Blocker or dependency, its owner and the next action.
- Latest committed work and whether it reached the delivery repository.

Use compact live chat snapshots and existing developer reports. Update the
assigned lane record when information changes and send material completion,
staffing or dependency changes to the coordinating king. Do not notify on an
unchanged snapshot. Keep communication in English and preserve each chat's model
and reasoning settings.

The [central original-task register](task-progress-2026-11-04.json) supplies the
recorded baseline. Deduplicate original task IDs across helpers and workers;
supporting commits or completed bounded goals do not add completed tasks. Missing
current evidence means unreconciled status, not NOT_STARTED. Do not invent a
percentage, silently discard prior acceptance, or count a component result as a
complete live product flow. All 112 required tasks and 61 IA entries remain.

Developers own code, query, test, runtime and visual validation, technical
interfaces, fixes and direct non-force publication. SLs track the feature picture,
dependencies and staffing. Kings track major milestones and project priorities.
Neither coordination role performs technical reviews or imposes approval gates.
If asked for review, reply: "I don't perform reviews. The responsible developer
owns validation; please resolve this with the implementation owner."

Continue accepted development between checks. Reuse a genuinely finished worker
for a ready disjoint scope or preserve its work and archive it. An observation
timeout does not establish that a live worker stopped. Every new development
assignment retains concrete original task IDs, an isolated checkout, exact file
ownership, developer-owned validation/documentation/commit/push criteria and a
truthful persistent goal without an invented token budget. Confirm actual
acceptance; sending an assignment alone does not establish active development.
