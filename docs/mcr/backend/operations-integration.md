# Moderation and operations integration — 2026-11-04

The recovered report, event settings, notices and bounded place-closure producers
are integrated with forward Concert migration **0021**. Their production Auth,
operator, evidence/review and cross-feature bindings remain explicitly unavailable
until supplied by their owners. This is implementation and database qualification,
not full BE36/BE38 or live operator acceptance.

Author `a93a04135a0ba44da3993a5df187e63da51ec75a` is a nine-commit stack over
`1163b827`. Its original six author commits retain cherry-pick provenance. Main
integrated the full stack serially as `8d592d8c`, `5abb4fb6`, `97c7d791`,
`6326fd95`, `aa47b3a2`, `5f1ce09b`, `9b0d3442`, `88493898`, `b99e0929`.
Only the final tree is delivered; its migration is 0021, never the intermediate
historical 0015 path. All 16 previously published Concert migrations are unchanged.

All 21 changed paths match the assignment. Nineteen match author bytes exactly;
the two shared registration files additionally preserve the already delivered
Intro exports. The World recipient export remains intact. All 229 native source
pins match the author tree and all four required domain contract pins match the
integration base. No frontend, proto, manifest or lockfile changes are included.

The author qualified ten domain/application tests, five harness safeguards and
17 native cases: six report, seven operation, three place, and one genuine SQLx
baseline helper. Fresh and forward 0020-to-0021 ledgers, deliberate late-DDL
rollback, reapplication, preserved state, actual PostgreSQL restart and cleanup
passed. The direct acquisition-guard assertion confirms rejection of an already
shared place release. These results are bound to source `f485420d`; final
`a93a0413` adds documentation only. Main verified the pinned report hashes and
used the qualified handoff without another general review or native rerun.

Fresh combined compilation of all three native test executables on `b99e0929`
passed (Rust 1.88, locked/offline, two jobs, debug/incremental disabled; 15.95s).
The first attempt failed because a retained build script referenced another
worktree. Cleaning only 33 workspace-local packages corrected that cache issue;
third-party dependencies and source were preserved. The exact three executables
were copied and hashed, all target handles ended, and BACKEND was transferred
to the account-deletion worker before further work.

Full monotonic event/place revisions, reviewed new-release operations, subscriber
invalidation, owning mutation adoption of FeatureGuard, notice feed/private outbox
composition and real production authority still require their assigned follow-ups.
No physical arrival, safe route, browser/device, load or release claim is made.

See [author evidence](moderation-operations.md) and the
[integration receipt](../evidence/operations-integration-2026-11-04.json).
