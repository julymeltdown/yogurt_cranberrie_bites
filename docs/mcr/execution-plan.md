# Parallel execution and delivery plan

The [revised functional delivery plan](functional-delivery-plan.md) is the current user-authorized amendment. Its 112-row register replaces outdated staffing, wave and status descriptions in earlier versions of this document. Preserve the original specification/source, all 112 task IDs, 61 IA entries and six excluded optional tasks. Read progress from the [current register](task-progress-2026-11-04.json), not generated planning exports.

## Priority and scope

Complete working meetup/pickup, social/privacy flows, selected Wall B and remaining reference UI defects first. QA08 performance qualification, including 200 concurrent users, is **DEFERRED_BY_USER** and does not gate current functional delivery. This is not a passing result or capacity claim. Functional races, privacy, authorization, persistence, recovery, accessibility, actual mobile usability and visual fidelity remain required.

Wall B includes opaque erase, four-connected fill of at most 256 changed cells, latest-own-action undo within 10 database minutes only on an unchanged global revision, and a shared 512 changed-cell budget per actor/event/rolling 10 database minutes, including undo. Keep cooldown, confirmation, revision/idempotency, atomic writes, private history and server-owned recovery. Transparency, redo and full-wall fill remain excluded. The approved proposal and revised task definitions govern implementation.

Use the supplied ten-screen reference and selected character pack. Retro styling applies only to post-shaped portions; surrounding Feed controls and other screens retain the reference. No decorative left accent bars, old character fallbacks or static screenshot substitutes for functioning components. Reusable client assets and caches should minimize redundant requests without weakening server authority.

## Feature queues and ownership

| Queue | Immediate outcome | Current routing |
| --- | --- | --- |
| Contracts | Finish C04/C06/C07 and changed-contract fixture coverage | C07 owner 21e; Wall contracts owner 202; exact generated/shared transfers through named publishers |
| Meet/Gifts | Real party, beacon, reservation and pickup through IN02; protected Gifts detail images | Party 19f83/backend 1fc; Gifts 53d/frontend 153; backend lead allocates remaining giveaway/media scope |
| Social/Feed | Real consent, privacy and notifications through IN03; retain the published Thread portrait correction | Existing social/Auth/Notification owners; Feed 209; scoped shared caller owner |
| Wall B | Backend commands, frontend tools/recovery and IN04 | Sole backend 202 and frontend 204; duplicate 735 retired |
| Reference/World | ART05, avatar follow-ups, Meet density and mobile fidelity | Art 231, avatar 83, Meet 213/6950 and existing frontend owners |
| Operations/recovery | Reports/settings, lifecycle, deletion and functional recovery | Reports 205, lifecycle 232, deletion/recovery category owners and backend publisher |

These are routing facts, not blanket file leases. Leads assign unowned remaining scope to capable existing developers. Independent composition pieces may be delegated after the previous writer stops; shared files are not reserved as one indivisible integration assignment. Backend and frontend use separate commits, exact paths and isolated worktrees. Contracts, manifests, lockfiles, root composition, CI and shared artifacts require explicit single-writer ownership.

## Complete feature assignments and direct delivery

1. Assign a coherent feature objective with original task IDs, exact paths, isolated worktree/base, contract pins and completion criteria. Cover implementation, relevant validation/fixes, documentation and committed publication without repeated returns for tiny task packets.
2. Inspect the actual persistent goal. Reuse an aligned active goal; create an unbudgeted goal when no unfinished goal exists. Never falsely complete an incompatible goal. Route completed packets awaiting publication promptly so their owners can begin the next assignment.
3. Start source work when interfaces are usable. Frontend can use frozen contracts and fixtures before backend completion. Live integration gates require actual producers. Do not impose full-wave barriers or a QA08 prerequisite for functional work.
4. Developers own code, SQL, test, runtime and visual validation. SLs manage feature context, dependencies and staffing. Kings manage major outcomes and cross-lane conflicts. A king receiving a technical review request replies: "I don't perform reviews. The responsible developer owns validation; please resolve this with the implementation owner."
5. Named publishers receive validated handoffs directly and use short, serialized, non-force publication turns on master. Preserve concurrent history and run relevant combined checks for changed composition. Independent finished features do not wait behind unrelated features. No king/SL acknowledgment or ledger update is required.
6. One canonical publisher receipt establishes delivery. Share relevant changed-candidate captures across art, CSS and consumer owners. Avoid repeated identical checks, proof-only commits and readback loops. Changed behavior still needs developer validation; unrelated documentation changes do not invalidate unchanged qualified source/dependency sets.
7. Leads assign the next ready related scope or retire a finished context after preserving its work. A developer queued for a resource slot is not an abandoned worker to duplicate. Create chats only for independent ready work with available capacity. Preserve English-only communication and the user's selected models/reasoning.

## Resource admission

The measured ceiling is **two isolated browser jobs**, subject to current host capacity and the existing Rust/source/runtime limits. The coordinator corrected the admission policy on 2026-11-05: Rust compilation or target mutation still requires 16 GiB free at admission; bounded noncompiling source, browser and copied-native work may start from 12 GiB. Their additional-growth caps per job are respectively 128 MiB, 200 MiB of browser temporary output, and 1 GiB. Before admission, subtract all outstanding growth reservations and the proposed job's declared worst-case growth from measured free space; at least 9 GiB must remain. The continuous running floor remains 8 GiB, with the extra 1 GiB as an admission margin. Keep existing process/target limits, private outputs, deadlines and cleanup duties. Uncertain/unbounded growth, hidden compilation, cold targets and bulk pre-allocation copies are ineligible for the 12 GiB class. This replaces a coordinator-set operating default, not an explicit human-fixed threshold, and does not waive functional validation.

The earlier recovery report was **18.19 GB, approximately 16.94 GiB**, not 18.19 GiB. Space subsequently fell again. By 2026-11-04T16:43:56Z, resource lead 21d reported actual acceptance of all five bounded jobs: C07 SOURCE-B, Map SOURCE-A, AR03 browser B1, Gifts browser B2 and Root IN03 copied-native. The last admission measured 14,574,010,368 bytes free (13.57 GiB); 1,677,721,600 bytes of combined declared new-growth reservations left 12.01 GiB. Rust remained queued below 16 GiB. These are dated admission reports, not completed functional checks or standing permission to bypass the next capacity check.

Do not launch unallocated Cargo work, create unallocated cold targets, delete unique work/evidence or waive the applicable resource floor. Safe source work continues independently. Performance-only jobs stay out of the functional queue. Prefer retained worktrees and build outputs over new duplicate checkouts.

## Progress and release

Each SL reconciles individual task progress hourly from existing records: state, remaining outcome, owner/action, material blocker and publication. Consolidate these into ordinary feature digests; avoid per-packet reporting to kings. Preserve historical outcomes and record new scope as explicit deltas on affected IDs. Helper-goal completion is not whole-task completion.

IN05 accounts for all 112 original IDs and 61 required IA entries. Current functional delivery requires 111 active tasks; QA08 remains explicitly deferred with its original scope/history retained and no capacity claim. Actual devices, provider/product inputs, functional safety/privacy/integrity/recovery and reference fidelity remain requirements. The original source and historical records stay available; the revised plan governs current ordering and approved scope changes.
