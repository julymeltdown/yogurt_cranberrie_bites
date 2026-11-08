# Yogurt Cranberrie Bites integration — document index

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**. Nothing in this
directory records a delivered feature, a passing integrated test, or an accepted
task. Every row below is a document that governs work; none of them is evidence
that the work exists.

The product being specified is the integrated application delivered to
`julymeltdown/yogurt_cranberrie_bites`, built on the `julymeltdown/mcr`
foundation. The product name **Yogurt Cranberrie Bites** is used only in planning
documents, commit messages, and repository metadata. Product navigation,
interface labels, and all engineering artifacts stay in English (workspace
instruction; see [AGENTS.md](../../../AGENTS.md)).

## Read order

| # | Document | Governs |
| --- | --- | --- |
| 1 | [integration-charter.md](integration-charter.md) | Authority, repository routing, baselines, what is in and out of scope, decision log |
| 2 | [product-plan.md](product-plan.md) | Product entities, ownership rules, user flows, acceptance criteria, open product decisions |
| 3 | [domain-contract.md](domain-contract.md) | `IntroAudience`, `IdentityClaimsPort`, the verified NAVER claim, visibility decision table, data model |
| 4 | [http-contract.md](http-contract.md) | Every operation, request/response shape, error, cache, and enforcement point |
| 5 | [migration-plan.md](migration-plan.md) | Source-to-destination mapping for Bigyeting and MCR code, with adaptation rules |
| 6 | [implementation-plan.md](implementation-plan.md) | Work packages, dependencies, ownership, definition of done, verification matrix |
| 7 | [source-register.md](source-register.md) | Pinned revisions, audit status, provenance obligations |
| 8 | [verification-plan.md](verification-plan.md) | Thirteen mandatory verifications, test-case tables, evidence format |
| 9 | [task-breakdown.md](task-breakdown.md) | 110 implementation tasks with files, symbols, dependencies, acceptance, tests, and evidence |

Machine-readable companions live beside them:

| File | Purpose |
| --- | --- |
| [source-register.json](source-register.json) | Pinned source revisions, audit state, file-level candidate list |
| [work-packages.json](work-packages.json) | Work package IDs, owners, dependencies, write scopes, acceptance |
| [verification-matrix.json](verification-matrix.json) | Mandatory verification IDs mapped to work packages and test paths |
| [task-register.json](task-register.json) | Machine-readable task rows: lane, status, dependencies, write scopes, verification ids |

## Precedence

When documents disagree, the earlier item wins:

1. A direct instruction from the user in the current conversation.
2. [integration-charter.md](integration-charter.md) — routing, scope boundaries.
3. [domain-contract.md](domain-contract.md) and [http-contract.md](http-contract.md)
   — frozen interfaces. Changing either requires a recorded decision in the
   charter's decision log before implementation starts.
4. [product-plan.md](product-plan.md) — product behavior.
5. [implementation-plan.md](implementation-plan.md) and
   [verification-plan.md](verification-plan.md) — sequencing and evidence.
6. [migration-plan.md](migration-plan.md) — how to reuse source code.
7. The MCR planning baseline (`MCR_IMPLEMENTATION_PLAN_V2_EN.md`, 112 tasks) for
   anything this set does not restate.

The MCR baseline remains the authority for the concert-world product it already
specifies. This set **adds** a profile/social/room product on top of that
foundation and explicitly excludes several MCR event features from the integrated
release (charter §5). It does not silently reopen or close any of the 112 MCR
task IDs.

## Repository state at the time of writing

| Item | Value |
| --- | --- |
| Delivery repository | `julymeltdown/yogurt_cranberrie_bites` (public, empty at creation) |
| Delivery branch | `master` |
| Seed commit | `03b673224aaa77842d763f817a660eb2d6f634d6` — empty commit on top of MCR `a2b10bf92d716bdb5d0ca85b481de6265b466bf7` |
| Seed content | MCR `master` tree, byte for byte; no file changed |
| Default branch note | GitHub created the repository with `main`; `master` was pushed and is the only branch. The remote default branch must be switched to `master` before any work is published. |

The seed commit is the integration base for every work package in this set.

## Decisions already taken

The user settled the three blocking questions on 2026-11-08:

1. **Gender** comes from the verified NAVER Login profile, not a self-declared
   field. The donor already implements the flow: OAuth start → callback →
   profile API (`birthyear`, `birthday`, `gender`) → derived exact age plus a
   gender bucket, stored with a 90-day lifetime.
2. **Age and gender restrictions must exist.** They decide who may see a post.
3. **NAVER linking is required only for restricted posts.** The donor's global
   gate is narrowed, not deleted.

Everything else was decided by the lead under the user's delegation. See
[integration-charter.md](integration-charter.md) §7 for the full log.

## What to do first

1. Freeze `IntroAudience`, the identity-verification operations, and the rest of
   the operation list (WP-01) before any feature code.
2. Confirm the NAVER developer console can be reviewed for the `birthyear`,
   `birthday`, and `gender` scope. Without that approval no user can obtain a
   claim, and every restricted post stays invisible. This is an external
   prerequisite, not a code task.
3. Follow [implementation-plan.md](implementation-plan.md) §3 for the wave order.
