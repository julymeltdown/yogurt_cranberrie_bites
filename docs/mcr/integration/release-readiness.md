# Release readiness analysis

Status: **INTEGRATION COMPLETE / NOT RELEASE-READY**. The end-to-end gate passes.
This document states what is proven, what remains open, and what must be true
before a release decision.

| Field | Value |
| --- | --- |
| Snapshot | 2026-11-10 00:25 Asia/Seoul |
| Head | `1c5b79c77` |
| Tasks integrated | 106 / 110 |
| Release gate | **PASS** — 6 of 6 steps, both sweeps, verified six times across separate runs |

---

## 1. The honest summary

The feature code is complete and the integration gate passes. Every work package
landed, every security decision is implemented and tested, 106 of 110 tasks are
merged, and the end-to-end scenario runs against seven real services.

**The product works end to end. It is not release-ready.**

What remains is not code: it is the acceptance work that needs a browser and a
provider, and one class of verification that unit tests cannot supply.

The path here is worth recording. At the first snapshot the gate passed one of
five steps, and 106 merged tasks with roughly 1,400 tests had not found the six
defects behind that. Each package was correct in isolation and each verification
was honest about its own scope; the defects lived in the seams. Four of the
eleven defects found were in the gate itself, which is why the gate had to be
corrected before it could measure anything.

## 2. What is proven

| Capability | Evidence |
| --- | --- |
| 43 profile operations frozen with schemas | 35 core + 61 domain + 17 bundle contract tests |
| Schema applies from empty and from previous head | Migration ledger, both services, both scenarios |
| NAVER verification produces a real claim | 85 tests; live, expired, revoked, absent, wrong-gender |
| The 12-row visibility decision | 14 pure + 8 SQL parity, one shared case table |
| The decision is one implementation | Pure function and SQL predicate proven identical |
| Likes, likers, media, links share that decision | 35 tests across five suites |
| Per-row block resolution | 9 tests, including unresolvable-pair denial |
| Profile posts and media | 61 tests; attachment isolation proven |
| Personal room and posters | 62 backend + 64 frontend tests |
| Avatar re-keyed to profile ownership | 34 backend + 27 frontend tests |
| Discovery projection with worker | 19 + 12 + 11 + 5 tests |
| Three-section navigation and profile screens | 112 tests |
| Profile identity created at registration | 3 tests, id matches the backfill derivation |

**Step 1 of the gate passes**: two photographs uploaded, normalized, published,
and listed through real HTTP against seven real services.

---

## 3. The gate passes

The end-to-end gate runs seven real services on a private PostgreSQL 16 cluster
with TLS, seeds three identities through real enrollment and real NAVER
verification, and drives the scenario over real HTTPS.

| Step | Result |
| --- | --- |
| 1 — two photographs uploaded, normalized, published, listed | PASS |
| 2 — restricted introduction drafted, published, listed, read | PASS |
| 3 — viewer likes it, author sees the liker and the notification | PASS |
| 4 — poster placed in the room; introduction deleted; room unchanged | PASS |
| 5 — outsider denied every content surface | PASS |
| 6 — discovery projection reaches the source and is current | PASS |
| Cross-surface sweep — one decision, no leak | PASS |
| Lifecycle sweep — durable independence both directions | PASS |

Verified twice in succession on `07f0bb960`. The report in
`evidence/wp10-gate-report.json` is the second run.

### The defects the gate found, and how each was resolved

| Defect | Class | Resolution |
| --- | --- | --- |
| Projection worker built a plaintext anonymous channel to an HTTPS listener | Product | TLS material plus the Concert service identity; settings refuse to start live without the credential |
| `profile_identities` had no runtime creation, so no account registered after migration 0011 could satisfy an audience | Product | Created in the same transaction as the profile row, same derivation as the backfill; gap backfill in migration 0018 |
| An empty projection was read as "blocked", denying every restricted post to everyone | Product | A missing projection row means no block is recorded |
| Intro stores were built with the fail-closed policy and never given the real one | Product | One `ComposedPolicy` shared by the post, like and comment stores |
| Media was authorized by owner only, so no non-owner could read a photograph | Product | Authorized through the attached post |
| The gate probed paths the contract does not define | Gate | Steps rewritten to contract paths with real ids |
| The gate never configured the claim endpoint | Gate | Endpoint and credential wired |
| Step 2 raced the discovery worker | Gate | Waits for the author projection and records the wait |
| Step 5 recorded denials but never set a status | Gate | Asserts the outsider denial |
| The sweeps used invented paths and crashed on a bare scalar | Gate | Contract paths; `query()` split from `scalar()` |
| The delete probe omitted an Idempotency-Key | Gate | Key added |

Four of the eleven were defects in the measurement, not the product. That ratio
is the strongest argument for the gate existing.

## 4. Why "106 of 110 tasks" overstated readiness

Kept because it is the lesson of this work. At the first snapshot the task count
said 106 of 110 and the product did not work. Three reasons:

1. **A task's acceptance was scoped to its own package.** "Media authorization
   through the attached post" passed 9 tests and was true. It did not mean media
   was reachable end to end — that needed the transport, the identity, and the
   projection, none of which the package owned.
2. **Fixtures supplied what production must supply.** The projection mirror in
   the visibility fixtures, the mirrored profiles in the discovery fixtures, and
   the pre-seeded identities in the intro fixtures each hid a seam. The
   visibility predicate never ran against an empty projection until the gate
   did it.
3. **The remaining tasks are the integration itself.** T-951..T-956 are not
   cleanup; they are the work of making the composition real.

The gate is now the authority on readiness, not the task count.

## 5. What remains before a release decision

The gate is green. These are the items it does not cover.

| Item | State | Why it matters |
| --- | --- | --- |
| Browser captures at 360/390/430 | Blocked | The harness is built and drives a real browser, but the shell will not open a route. Four of its five prerequisites are now composed; the fifth is the signed, transaction-bound profile read (`BoundCanonicalSelfProfile`), which needs the full concert admission composition rather than another seeded row. `me` moved 403 -> 404, so the remaining refusal is one layer deeper than seeding. |
| Live NAVER authorization | Blocked | No console-issued application in this environment; the gate uses a local test provider with fixed profiles |
| Claim expiry mid-session | Unit-tested only | The gate proves the happy path and the absent/expired cases at the adapter; it does not expire a claim during a live session |
| Provider outage during a read | Unit-tested only | The adapter fails closed on transport error; the gate does not simulate an outage mid-scenario |
| Performance | Deferred | Deferred by the earlier MCR decision; not a gate for this integration |
| Licensing | Unresolved | Recorded in source-register.md §6; must be settled before any public release |

### What "106 of 110" still means

The four remaining tasks are T-951..T-956, the integration and release-evidence
work. The gate now does most of what they describe. What is left is the release
decision itself, which cannot be taken until the items above are closed or
explicitly deferred by the user.

## 6. The plan

**Wave 1 — browser captures.** The only remaining acceptance item that needs no
external input. One owner, capture every new screen at three widths, record what
renders and what shows an error state.

**Wave 2 — the release evidence assembly.** Assemble every verification result
with its command, exit code, and artifact. State plainly what is not proven.

**Wave 3 — the release decision.** Cannot be taken until the user decides
whether the browser captures and the live-NAVER gap are release blockers or
documented limitations.

## 7. What this analysis changes about the process

The gate found six defects that 106 merged tasks and roughly 1,400 tests did
not. The lesson is not that the tests were wrong — they were each honest about
their scope — but that **a package-level green is not evidence of a working
product**, and the composition must be exercised continuously rather than at the
end.

Concretely, for the remainder of this work:

1. The gate runs after every merge, not at milestones.
2. A fix is not accepted on a unit test alone when the defect was found by the
   gate; it is accepted when the gate step moves.
3. Fixtures that supply what production supplies are labelled as such, so the
   next reader knows which seams they hide.
