# FE17 gap-baseline recovery regression delivery

## Bounded original criterion disposition

**CONTRACT_READY — bounded client gap/baseline regression resolved.** Original FE17 requires a missing delta to request resync, a restart to remove ghosts, and no replay of old pickup/reservation commands. Specification §7.5 requires a fresh snapshot on a gap or epoch change, per-delivered-stream sequencing and no blind replay. This delivery resolves the specifically assigned historical test failure and strengthens the existing gap/replacement-baseline evidence. It does not declare the whole FE17 task complete, alter global task counts, or accept QA05, IN01, IN05, live multiuser behavior or device behavior.

## Diagnosis and behavior

Frozen Root diagnostic source `cb9abce48d78227bbdedf7ea59ce95e9e0270c4a`, published by `62b4bf80`, records the named gap test failing at `replication.test.ts:29`: `TypeError: Cannot read properties of undefined (reading 'open')`. Its broad result remains **NOT_GREEN: 8 failed files, 10 failed tests, 3368 passed, 14 skipped**. That historical log is unchanged and the broad suite was not rerun here. The exact FE17 excerpt and final totals are preserved in this delivery's evidence directory.

At fresh assignment base `1383ae452fc81dfe5fd1e4ca4d37af8950700dda`, the test, replication controller and transport engine bytes match that diagnostic source. The ordinary targeted test passes. Adding75ms latency to the real WebCrypto SHA-256 operation reliably reproduces the same `undefined.open` at the same fixture line before the correction. The experiment delays completion only: actual ticket validation, hashing, transport construction, frame validation and controller behavior remain active.

The fixture's `live()` helper used twenty resolved Promise turns plus a fixed5ms delay. Production `TransportEngine.ticket()` awaits `crypto.subtle.digest()` before creating the socket. A wall-clock sleep does not guarantee that asynchronous work has completed. This is an identified **test fixture readiness race**, not an established product recovery defect or source-version drift. Production source and frozen contracts remain unchanged.

Only the allocated socket-setup hunk now waits with `vi.waitFor` for exactly one transport socket before opening it and supplying the initial snapshot. It still requires successful `start()` and does not fabricate a ready controller, skip hashing or bypass frame guards.

The exact allocated gap test now demonstrates:

1. A pending input and an unsent interaction exist before a genuine missing-sequence frame. Gap detection clears all projected entities and pending input.
2. Exactly one `GAP` resync carries the old stream/epoch and last accepted stream sequence0.
3. Queued old frames containing a renamed/moved ghost and a legitimate-member removal change neither committed views, baseline sequence nor outgoing traffic during resync. No input is sent during recovery.
4. A request-correlated replacement snapshot supplies authoritative self coordinates plus a new peer/avatar revision. Its complete roster replaces all ghosts; metadata contains no retained `players` array.
5. Input sequence restarts at1, held axes may resume, and both sent/unsent interaction state is not replayed. The next current-stream delta applies and acknowledges the new input.
6. The outgoing frame list is exactly authenticate/input/resync/input. Disposal removes scheduled work and socket listeners.

These assertions exercise the actual FE16 transport engine, strict C05 decoder, FE17 controller and entity projection through `MockWorldTransport`/`MockWorldClock`. The183 focused tests also exercise the retained mock/WebSocket-adapter scenario suites and existing lifecycle cases. These are logic/contract fixtures; no real backend, live network, new browser/device gate, art admission or spatial simulation authority is inferred. Reservation and pickup remain HTTP/domain operations outside this WebSocket protocol; this patch verifies no old world input/interaction replay and does not claim end-to-end reservation/pickup recovery acceptance.

## Ownership and provenance

Goal was null at intake; a new unbudgeted persistent FE17 goal was created before the crossed staffing correction. The king subsequently confirmed this active scope should continue and Social should use another context. Work uses isolated checkout `mcr-worktrees/fe-ref-world`, new branch `codex/mcr-fe-FE17-baseline-recovery`; the published FE15 branch is preserved.

Exact accepted existing edits are only `frontend/src/world/network/__tests__/replication.test.ts` fixture `live()` socket setup and the test “a gap clears ghosts, requests one resync, discards queued old changes and replaces the complete baseline”. New files are limited to `frontend/src/world/network/__tests__/fe17-baseline-recovery/**`, this document, and `docs/mcr/evidence/fe17-baseline-recovery/**`. Former FE15/WorldShell/composition/scroll leases remain returned. No other existing tests, production implementation, shared contracts, manifests, lockfiles, backend, art or root composition changed.

The new delayed-digest config/setup are newly authored regression instrumentation. They select only the assigned existing test and preserve the real digest result. Existing MCR-owned C05 movement-heartbeat/ticket/filtered-viewer fixtures are imported unchanged. `checks.json` records base/historical/current source SHA256s, contract pins, evidence checksums, command exit codes and scope. `historical-fe17-excerpt.txt` derives verbatim lines from the frozen published log; its full source hash is `9a8d3884a46dd042ab35e07d91d3da9de6e36e80f813253e1c5d537d2864ddd2`. Other logs replace only this local checkout path with `<FE17_WORKTREE>` for portability.

## Validation

Use the retained Node24.22.2 toolchain and existing dependencies; no install or lockfile change is needed. From `frontend/`:

```sh
node_modules/.bin/vitest run src/world/network/__tests__/replication.test.ts -t 'a gap clears ghosts' --maxWorkers=1 --reporter=verbose
node_modules/.bin/vitest run --config src/world/network/__tests__/fe17-baseline-recovery/delayed-digest.config.ts --reporter=verbose
node_modules/.bin/vitest run src/world/network/__tests__/replication.test.ts src/world/network/__tests__/transport.test.ts --maxWorkers=2 --reporter=verbose
node_modules/.bin/tsc -b --pretty false
node_modules/.bin/oxlint src/world/network/__tests__/replication.test.ts src/world/network/__tests__/fe17-baseline-recovery
```

| Check | Exit / result |
| --- | --- |
| Before-fix ordinary named target | 0;1 passed/22 unselected |
| Before-fix delayed real digest | 1; same undefined.open fixture failure |
| Corrected delayed real digest, strengthened target | 0;1 passed/22 unselected |
| Full replication + transport files | 0;183 passed in2 files |
| Full frontend TypeScript | 0 |
| Scoped lint | 0 |
| Planning/source archive check | 0;13 archives unchanged |

Production build, browser/device and live-server checks are NOT_RUN for this test-only correction. No implementation/output bundle changed. Source-origin/boundary and final publication receipts are recorded below after delivery.


Final source-origin check exits0 with no violations, the boundary check exits0 for13 assigned changes (exact document/evidence file allowances), and `git diff --check` exits0. Changed-path and hunk inspection confirms exactly the two allocated existing-test hunks plus the new owned prefix/document/evidence files. A fresh peer fetch changes none of the tested network/entity/shared-API/realtime-contract/lockfile paths. The temporary dependency symlink is removed. The direct publication receipt follows once remote ancestry is verified.
