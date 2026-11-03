# Mural native implementation integration — 2026-11-04

BE34/BE35's transactional pixel writes, private cooldown and receipt recovery,
consistent checkpoints, ordered deltas and bounded subscription recovery are
integrated. Current production authority, authenticated public routes, operator
composition, BE40 and the real IN04 flow remain open; this is not whole-task or
release acceptance.

The qualified author chain starts at published `7fc3e38` and ends at
`1bc041f5ae60f54f0875e53c1214c4b4deba74ec`. Its eight commits were integrated
serially with original ancestry preserved through `-x`: `8c94a68c`, `6de9f524`,
`97977477`, `8d4977a3`, `9cd42dc1`, `070ccfd9`, `e72a1940`, `505bb8c2`.
Previously published shared contract imports were not duplicated.

All 21 final paths match their exact assignment and author bytes. All five
existing-file preimages match. No backend file had changed between the author's
published base and integration base `e40151fe`; all 42 native source pins match.
The existing World implementation is retained with its mural export appended.
The service root is unchanged. Published migrations 0016 and 0018 are unchanged;
the new migrations use reserved 0019 and 0020. No frontend source changed.

The author and Operations lead supplied qualified evidence, so Main added no
general review or repeated database run. The exact copied native executables
passed eight PostgreSQL tests and two subscription tests before and after a
real PostgreSQL restart: 20 passing invocations of ten tests. Fresh installation,
forward upgrade from published 0018 preserving the event and receipt, exact
16-migration SQLx ledgers in three isolated databases, reapply, restart
persistence and cleanup passed. The earlier path-alias harness refusal remains
recorded separately. Build targets were released before native PostgreSQL runs.

Main verified the final bytes and retained migrations, then ran both Python
harness-integrity tests and the combined whitespace check, all exit 0. The
author's actual bounded goal is complete and its writer stopped. See the
[author handoff](mural.md),
[native receipt](../../../backend/services/concert-service/tests/mural_support/native-evidence.json)
and [integration receipt](../evidence/mural-integration-2026-11-04.json).
