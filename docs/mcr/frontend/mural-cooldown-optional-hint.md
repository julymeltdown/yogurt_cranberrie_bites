# FE-TRANSPORT-MURAL-OPTIONAL-HINT

Status: ACCEPTED for the assigned FE01 optional-hint correction and C04 frontend projection. Assignment start base: `482d02c4ac63db4faff2926156e266ef216a4b5a`; isolated branch `codex/mcr-fe-mural-optional-hint` in the retained `fe-ref-gifts` checkout. The qualified source commit and frontend correction are published together to `julymeltdown/mcr:master`.

## Task mapping and ownership

This is a bounded FE01 canonical HTTP/error transport correction and C04 generated metadata support patch. It supports BE34 mural failed-command replay and FE32/FE33 cooldown handling; it does not complete those parents or IN04. Original specification mapping: FE01 (`shared/api/http.ts`, `shared/api/errors.ts`), BE34 server cooldown, FE32 editor, FE33 synchronization, and IN04 shared mural/moderation gate. No Wall route, feature flag, producer readiness or live acceptance changes.

The exact writable grant is `frontend/src/shared/api/domain-operations.generated.ts`, `frontend/src/shared/api/errors.ts`, NEW `frontend/src/shared/api/__tests__/mural-cooldown-optional-hint/**`, and this NEW document. The backend central owner retains canonical domain schema, the two OpenAPI projections, and `scripts/mcr/generate-domain-transport.py`. Direct coordination explicitly assigns generated frontend output only to this writer. No schema, generator, backend, Wall feature, root, dependency, lockfile or unrelated file is edited by this frontend commit. The source owner separately authorized integration and publication of its unchanged standalone source commit alongside this frontend commit, preventing an intermediate master with stale generated metadata.

| Original task | Task-local disposition and remaining action |
| --- | --- |
| FE01 | ACCEPTED for this correction. The original required mock 401/409/429/abort, rejection of old `/v1` routes and memory-only credentials are covered by the passing existing transport suite. Safe multipart upload behavior also remains covered. No remaining optional-hint transport implementation is required. Real Wall production is not part of these FE01 criteria. |
| C04 | ACCEPTED for matching frontend metadata only: all generated reason rules agree with the actual canonical schema. The central source owner is chat `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08`; its broader contract ownership remains unchanged. |
| BE34 | Supporting fix only. Actual producer/concurrency/database qualification belongs to current BE-WALL-LIVE owner `01a1a21e-ab80-7012-b9fa-cdb69eed097b`. This consumer fixture is not backend acceptance. |
| FE32 | Supporting fix only. The 360px editor/drag/bounds/color/cooldown criteria are not reassessed by this transport assignment. No current FE Wall writer is assigned in Community's receipts; Community SL and FE king own next staffing. |
| FE33 | The existing adapter now returns definitive `rejected/MURAL_COOLDOWN` for a reason-only 429, verified through its actual `commands.place` port. No adapter edit is needed. Full two-client convergence, rejected-preview rollback and restore criteria remain separate from this bounded check; the next consumer writer is not yet assigned. |
| IN04 | NOT_RUN here: real clients painting through cooldown, moderation, restore and freeze require a qualified public Wall producer/authority handoff and a separately assigned FE/root consumer writer. This delivery changes no live readiness flag. |

The historical FE32/FE33 author `01a1a213-61d6-7a20-a8e7-476649312f8c` is archived/preserved, not an active writer or publication prerequisite. Community SL (`01a1a1fe-c5f1-7e60-87c2-43785b07f941`) and FE king (`01a1a385-9f32-73c2-8715-ff54bd1e0497`) track the remaining producer handoff and staffing; implementation validation stays with the assigned developers. These dispositions do not silently change any parent task's aggregate acceptance register.

## Contract and consumer behavior

A legitimate failed `MURAL_COOLDOWN` replay can omit `retry_after_ms`. Missing timing is not zero and does not identify a new allowed retry time. The domain consumer must retain the reason and preserve the field's omission. When present, the hint remains an integer from 1 through 3600000 inclusive. Unknown fields/reasons remain rejected. `RATE_LIMITED` continues to require its timing field; inherited core behavior is unchanged.

The coordinated generator format is uniform across non-null retry-hint rules: `{type:'integer',minimum:1,maximum:3600000,required:boolean}`. `MURAL_COOLDOWN.required` is false; `RATE_LIMITED.required` is true. Rules with no hint remain null. The generated registry is produced from the owner's actual published canonical inputs, never hand-edited to anticipate an unpublished schema.

`parseServerError` tests field presence separately from the rule's requirement flag. It rejects an absent required hint, rejects malformed/out-of-range present hints, and copies a hint into the frozen safe envelope only when present. The HTTP client still binds errors to the registered operation/status. A failed cooldown replay remains a failed HTTP 429, never a success or an instruction to resend a POST. `retryAfterMs` remains null when both the optional details field and Retry-After header are missing. Existing header precedence remains a separate bounded transport hint; it does not manufacture a details field or authorize an automatic retry.

## Source and delivery ancestry

The source owner supplied standalone commit `dc4a3f93cdd4d7df658a2fbc2d1ac18f9e7141af` from parent `b01eb708`, excluding unrelated unqualified product ancestry. All four source blobs below match that exact commit after peer reconciliation. The merge parent of the frontend commit is `2d6fabab98f5749f5bb9bc591cfc071a206ab53a`, containing the source commit and peer master `1249430f`. The frontend delivery commit is discoverable with `git log -1 --format=%H -- docs/mcr/frontend/mural-cooldown-optional-hint.md`; remote ancestry is checked against both commits after non-force publication.

| Source/output | SHA-256 |
| --- | --- |
| `contracts/domain/v1/common.schema.json` | `65f2c2f19594942b89613e67b97bd3181e0889e7f47e4476cda752d9aab10584` |
| `scripts/mcr/generate-domain-transport.py` | `7a85c1d47978df991f767989ad4264345f7b0a057bc54259a8f8c0bdc9cb941b` |
| Both `contracts/generated/mcr-domain.openapi.json` and `backend/docs/api/mcr-domain.openapi.json` | `1322a4da374b5f82d4c333b2a9b5e28fca1900be162677f8e8937ba68590b272` |
| `frontend/src/shared/api/domain-operations.generated.ts` | `50dbf66f5f9be756174294a91335537cd4aea7d992ac1dd545c3aa80ee27b6c7` |
| `frontend/src/shared/api/errors.ts` | `07472139ebdaf6d4325ffaff1111e00f060bb18b16e8957915bb5301194ac0a3` |

Source identity is checked with Git blob IDs and `git cat-file`, including the backend projection outside this sparse working tree. No backend build or checkout expansion was needed.

## Verification and limits

The new regression suite exercises parser omission and supplied bounds, mandatory rate limiting, malformed present values, unknown keys/reasons, registered status binding, unchanged core parsing, safe immutable copies, explicit same-key failed replay without stale timing, separate Retry-After header handling, and one-hour fake-clock advancement without any automatic POST. Real loopback HTTP tests use the actual Ky transport without injected fetch, preserving serialized command body and idempotency key across an explicitly requested failed replay. They use fixture identity/data and do not certify a live backend.

Before the source pin, the first red run had 25 passing and five failing tests: the failures all depended on the old generated registry still requiring the cooldown hint. A fixture-only TypeScript union access error in an assertion was corrected without changing production request types. No stale generated output was modified to hide that dependency.

The final checks below passed on the reconciled tree. Node 24.22.2 and the retained dependency tree were used through a temporary `frontend/node_modules` symlink; no install, manifest or lockfile change was made. That owned symlink is removed at STOP.

| Command (repository root) | Result |
| --- | --- |
| `python3 scripts/mcr/generate-domain-transport.py` | Exit 0; actual generation from the pinned owner inputs. |
| `python3 scripts/mcr/generate-domain-transport.py --check` | Exit 0; 108 operations, no drift. |
| `node frontend/node_modules/vitest/vitest.mjs run --root frontend src/shared/api/__tests__ src/features/auth/__tests__/domain-request.test.ts src/features/mural/sync/__tests__/http.test.ts` | Exit 0; 276 tests across 8 files, including 34 new tests in four files. |
| `node frontend/node_modules/typescript/bin/tsc -p frontend/tsconfig.json --pretty false --incremental false` | Exit 0; full frontend typecheck. |
| `node frontend/node_modules/oxlint/bin/oxlint frontend/src/shared/api/errors.ts frontend/src/shared/api/domain-operations.generated.ts frontend/src/shared/api/__tests__/mural-cooldown-optional-hint` | Exit 0; scoped lint. |
| `python3 scripts/mcr/check-plan.py` | Exit 0. |
| `git diff --check` | Exit 0. |

During test authoring, a late `Request.clone()` assertion ran after the transport had disposed its request body; that assertion was removed. Exact serialized body and key are instead verified at the real loopback server. No transport relaxation was made to satisfy that harness failure.

No browser layout, physical device, real database/concurrent actor, live Wall producer, release or IN04 acceptance is implied by this transport patch. No full application build or browser run was needed for this transport-only correction; the actual HTTP error/replay boundary was exercised directly.

Implementation owners exclusively choose and perform code/query/test/runtime validation and direct non-force publication. Kings/SLs track delivery/dependencies/staffing and impose no technical review or rerun gate. Interfaces and concrete fixes are resolved directly between implementation owners.
