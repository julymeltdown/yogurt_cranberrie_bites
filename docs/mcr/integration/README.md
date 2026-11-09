# Yogurt Cranberrie Bites integration — document index

**Implementation status: IN_PROGRESS.** This directory contains product
requirements, planning registers, and delivered package evidence. The integrated
profile/social/room product has not yet demonstrated full release acceptance.

The current source assessment is pinned to
[`02c0b321`](https://github.com/julymeltdown/yogurt_cranberrie_bites/commit/02c0b32163bad1226332b5174aa32c43abee033a)
on **2026-11-09 (Asia/Seoul)**. Start with the
[implementation assessment](implementation-assessment-2026-11-09.md) for actual
behavior, unresolved connections, CI results, and confidence limits. The
`PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED` headings retained in original
planning documents describe their creation baseline; they are not current
repository-wide status.

Delivery is `julymeltdown/yogurt_cranberrie_bites`, branch `master`, built on the
`julymeltdown/mcr` foundation. The product name **Yogurt Cranberrie Bites** is
used in planning, documentation, commit messages, and repository metadata.
Navigation, UI labels, and engineering artifacts remain in English.

## What the integrated product is

Users discover people through deliberately selected bands/artists (K-pop
excluded), anime, and games, then get to know them through profiles, ordinary
photos/artwork, and independent mutuals-intro posts.

- A heart is a like, not an application, approval, automatic follow, or disclosure.
- A personal room belongs to the profile and is independent of an intro post.
- Only room posters use pixel conversion; ordinary photographs do not.
- An intro's age/gender audience determines who may view it. Viewing a restricted
  intro requires the relevant live NAVER claim; unrestricted use does not.
- The intended primary navigation is Discover / Activity / My Profile.
- Event schedules, participation gates, gifts/pickup, and the shared mural do not
  define the new product's release scope. Existing concert code is retained.

See [product-plan.md](product-plan.md) for the complete requirements and
[integration-charter.md](integration-charter.md) for scope boundaries.

## Read order

| # | Document | Governs or records |
| --- | --- | --- |
| 1 | [integration-charter.md](integration-charter.md) | Authority, repository routing, baselines, scope boundaries, and decisions. |
| 2 | [product-plan.md](product-plan.md) | Product entities, ownership, user flows, and acceptance criteria. |
| 3 | [domain-contract.md](domain-contract.md) | `IntroAudience`, `IdentityClaimsPort`, verified claims, visibility decisions, and data model. |
| 4 | [http-contract.md](http-contract.md) | Operations, requests/responses, errors, caching, and enforcement points. |
| 5 | [contract-freeze.md](contract-freeze.md) | WP-01 contract record; read with the subsequent comment-operation amendment and current registries. |
| 6 | [implementation-assessment-2026-11-09.md](implementation-assessment-2026-11-09.md) | Dated source/CI findings, package coverage, missing production work, and validation limits. |
| 7 | [migration-plan.md](migration-plan.md) | Source-to-destination adaptation rules for Bigyeting and MCR. |
| 8 | [implementation-plan.md](implementation-plan.md) | Work packages, dependencies, ownership, and completion criteria. |
| 9 | [source-register.md](source-register.md) | Pinned source revisions, audit state, and provenance obligations. |
| 10 | [verification-plan.md](verification-plan.md) | Thirteen mandatory verifications, case tables, and evidence format. |
| 11 | [task-breakdown.md](task-breakdown.md) | 110 task definitions, exact paths, dependencies, acceptance, and required evidence. |
| 12 | [progress.md](progress.md) | Current implementation progress per feature element, verification state, defects found, and the release gate. |

The assessment describes implementation; it does not amend frozen contracts,
reduce product scope, or accept tasks on behalf of their implementation owners.

### Machine-readable companions

| File | Purpose and current interpretation |
| --- | --- |
| [source-register.json](source-register.json) | Pinned source revisions, audit state, and file-level candidates. |
| [work-packages.json](work-packages.json) | Eleven packages, dependencies, write scopes, and acceptance definitions. Its status fields remain a planning baseline. |
| [verification-matrix.json](verification-matrix.json) | Verification IDs and planned owners/paths. Consult individual evidence files for executed results and limitations. |
| [task-register.json](task-register.json) | 110 task definitions and dependency edges. All status rows still say `NOT_STARTED`; do not use them to calculate current completion. |

The task inventory is 61 backend, 35 frontend, eight contract, and six integrator
tasks. Counts do not demonstrate execution. The assessment also records why the
advertised seven-task critical path and the terminal release dependency closure
must be reconciled before the register can drive release decisions.

## Current implementation and evidence

| Package | Source/evidence present | Outstanding integrated behavior |
| --- | --- | --- |
| WP-01 Contract | [Contract record](contract-freeze.md); 43 added operations in the final registries. | Runtime/schema reconciliation and current conformance checks. |
| WP-02 Schema | [Schema and transport record](../backend/wp02-schema.md). | Profile provisioning/mapping, complete mutations, and production dispatch. |
| WP-03 Discovery | [Query/client evidence](evidence/discovery-wp03.md). | Favorites persistence, projection writer, gateway routes, and screen composition. |
| WP-04b NAVER | [Verification evidence](evidence/v12-naver-verification.md). | Real provider evidence, UI return route, and re-verification invariant checks. |
| WP-04 Audience | Contract/schema groundwork. | Profile-scoped claims port and consistent visibility decisions. |
| WP-05 Intro/hearts | Tables, constraints, operation declarations, and notification kind. | Product commands/reads, hearts, likers, comments, and notification delivery. |
| WP-06 Photos/media | [Implementation record](../backend/wp06-profile-posts.md). | Cross-service dispatch, media/profile contract reconciliation, and composed screens. |
| WP-07 Room | [Implementation record](../backend/wp07-room.md). | Owner flow, dispatch, poster-byte route, shared scene lifecycle, and browser evidence. |
| WP-08 Avatar | [Implementation record](../backend/wp08-avatar.md). | Producer dispatch and complete account-context enforcement. |
| WP-09 Screens | Existing shell with partial room/avatar changes. | New three-section navigation and full owner/visitor profiles. |
| WP-10 Integration | Scenario and verification definitions. | Real-service execution and a truthful release disposition. |

Read each evidence document's limitations. PostgreSQL handler tests, component
tests, and generated contract checks do not establish a complete browser-to-service
flow. Current application CI is
[failed at the assessed snapshot](https://github.com/julymeltdown/yogurt_cranberrie_bites/actions/runs/42525306588);
the assessment separates inherited failures from failures added after the pivot.

## Precedence

When requirements disagree, the earlier item wins:

1. A direct instruction from the user in the current conversation.
2. [integration-charter.md](integration-charter.md) — routing and scope.
3. [domain-contract.md](domain-contract.md) and [http-contract.md](http-contract.md)
   — frozen interfaces; a change requires a recorded charter decision.
4. [product-plan.md](product-plan.md) — product behavior.
5. [implementation-plan.md](implementation-plan.md) and
   [verification-plan.md](verification-plan.md) — sequencing and evidence.
6. [migration-plan.md](migration-plan.md) — source adaptation.
7. The MCR planning baseline for requirements this set does not restate.

The original MCR baseline remains the authority for its concert-world product.
This integration does not silently reopen or close any of the 112 original MCR
task IDs. Observed implementation differences are gaps to reconcile, not
implicit changes to requirements.

## Repository and historical baselines

| Item | Value |
| --- | --- |
| Current delivery repository | `julymeltdown/yogurt_cranberrie_bites` |
| Current delivery/default branch | `master`, confirmed through GitHub on 2026-11-09. |
| MCR foundation | `a2b10bf92d716bdb5d0ca85b481de6265b466bf7` |
| Seed commit | `03b673224aaa77842d763f817a660eb2d6f634d6`, an empty commit over the MCR foundation. |
| Seed content | The MCR master tree at that revision, byte for byte. |
| Product-plan pivot | `745ea437e28deef6d989d075e444e250c3c4ebe9` |
| Assessed application snapshot | `02c0b32163bad1226332b5174aa32c43abee033a` |

The seed describes provenance. It is not a statement that the repository is still
empty or that every later package was authored from the identical commit.
New work is delivered here, not to the historical MCR repository.

## Decisions already recorded

1. Gender comes from the NAVER provider profile, not a self-declared field.
   Birthyear, birthday, and gender produce a claim with at most a 90-day lifetime.
2. Age and gender restrictions exist and determine who may view an intro post.
3. NAVER linking is required to view age/gender-restricted intros; it is not an
   additional default publication requirement for their authors.
4. Rooms and ordinary photo posts have independent visibility; likes and follows
   remain independent.
5. Discovery and rooms remain in scope.

NAVER developer-console review is recorded as **APPROVED** in
[task-breakdown.md](task-breakdown.md) and [task-register.json](task-register.json).
The later verification evidence says its execution environment lacked a usable
test application/credentials/registered callback for a real NAVER round trip.
That is an unresolved runtime-evidence condition, not a request to repeat the
user's product or console-review approval.

See [integration-charter.md §7](integration-charter.md#7-decision-log) for the
decision history.

## Where implementation should continue

The contract freeze has landed. Do not restart WP-01 merely because a planning
header still says `NOT_STARTED`.

Use the [assessment's remaining-work sequence](implementation-assessment-2026-11-09.md#remaining-work-sequence)
to assign the production connections explicitly: identity provisioning/mapping,
profile mutations and aggregation, service dispatch/authority, discovery updates,
intro audience/hearts, UI composition, and lifecycle verification.

WP-10 currently owns end-to-end tests and evidence paths. It does not implicitly
own every backend integration gap. Assign concrete production paths to their
implementation owners before expecting the final scenario to pass.
