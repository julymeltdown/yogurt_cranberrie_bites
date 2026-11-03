# Follow-up source and evidence integration

The remote `julymeltdown/mcr/master` was verified at
`d68eef5eac82f08522ecfa1e1a67ce31bdeaffe0` before this follow-up.

FE34 `a90c6c4`/`f98f938` integrated as `d5fc731`/`ad92e11`; final FE28
consumer evidence `697b3ec` integrated as `a413e69`, and independent report
`c8f7841` as `b392f56`. Delivery ran the affected Me/thread selection serially:
146/146 checks, exit 0; strict build exited 0. Account route composition is a
separate assigned checkout, with the actual paths `/me`, `/me/profile`,
`/me/avatar`, `/me/settings` and the original shell privacy lease.

BE37 source `bcd2cd2`/`b71ae36`/`a5e84da` integrated as
`e1ce179`/`620f8ee`/`914843e`, preserving both existing bootstrap and new
relationship module exports. Its [native report](relationship-native.json)
records eight pure/crypto and 20 actual PostgreSQL producer tests, exact
migrations 1/2/3/6, rollback/recovery, restart and cleanup PASS. Delivery
compared all 17 recorded source hashes: 12 match; the other five are the three
central module export files and Concert manifest/lockfile containing independently
integrated bootstrap/place/meeting exports and owner-test devdependencies.
No claim is made that all 17 hashes match the combined tree.

Meeting source `f974cec`/`6ee0dfe`/`49a5286`/`8d6cb9a`/`c7b0b59`
integrated serially as `0d2f339`/`5e2718b`/`1db0b81`/`9472785`/`51783cf`.
All 12 recorded meeting source hashes match the integration tree. Its
[native report](meeting-native.json) passes six tests before and after a real
PostgreSQL restart, including persisted safe receipt recovery, plus cleanup.
These are source-bound worker executions; a combined parent Cargo/native run
is still required after the shared build queue reaches delivery.

Concrete production composition work remains active: BE37 now owns an
actor-only guard that acquires no pair lock; meeting must acquire party root
before full pair authorization. The earlier self-pair-before-root sequence
could deadlock with party commands. The corrected code requires its own
contention proof; the bounded earlier native suite does not prove this fix.
Neither policy observations nor independent owner revisions provide a
cross-owner mutation fence. Public routes remain disabled where unqualified.

The real IN01 harness `764affa` integrated as `6143bcb`. Delivery separately
ran all 19 offline helper checks and real Playwright discovery (one spec),
both exit 0. Central `vitest.config.ts` preserves Vitest defaults, excludes real
`*.spec.ts` gates, and keeps pure helper tests discoverable. Discovery confirms
the real world spec is excluded from unit execution. Real IN01 is NOT_RUN:
approved art/release, provisioned independent accounts, live service composition
and exact lifecycle observation are still required. No partial green gate.

Accepted task and IA counts are unchanged. Gateway HTTP changes remain a
separate pending patch. Source/archive checks and owned native cleanup do not
certify live service, device, multiuser, capacity or release acceptance.
