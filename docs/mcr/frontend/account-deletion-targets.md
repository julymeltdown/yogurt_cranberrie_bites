# FE36 canonical eight-target deletion consumer — 2026-11-04

Status: MOCK_READY. The assigned account consumer implementation and synthetic
qualification are complete against published contract `d3b7c38268924d2db6bcdefce7bb84b86dafe329`.
Backend runtime, qualified Auth adapter, root composition, real cleanup, devices and
whole account-deletion acceptance remain NOT_RUN. Producer and consumer integration
is still required; these tests do not establish live deletion compatibility.

## Source, scope and ancestry

- Base: `d3b7c38268924d2db6bcdefce7bb84b86dafe329`.
- Checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-deletion-targets`.
- Branch: `codex/mcr-fe-deletion-targets`; this delivery commit follows the recorded base.
- Prior FE36 checkout `/Users/lhs/Desktop/mcr-worktrees/fe-safety-account` remains
  clean/read-only at `926449d0d239852af3a170e7eaaf4de61ce19f0f`.
- Only assigned account paths, this new document and the later expressly granted
  safety preview fixture changed. Main integrates/pushes;
  this worker does not push, modify shared contracts, or acquire Auth/root ownership.

Changed paths:

1. `frontend/src/features/account/contract.ts`
2. `frontend/src/features/account/panel.tsx`
3. `frontend/src/features/account/__tests__/controller.test.ts`
4. `frontend/src/features/account/__tests__/deletion-fixtures.ts`
5. `frontend/src/features/account/__tests__/deletion-targets.test.ts`
6. `frontend/src/features/account/__tests__/deletion-preview.html`
7. `frontend/src/features/account/__tests__/deletion-preview.tsx`
8. `frontend/src/features/account/__tests__/deletion-browser-smoke.mjs`
9. `frontend/src/features/safety/__tests__/preview.tsx` (subsequent exact fixture-only grant)
10. `docs/mcr/frontend/account-deletion-targets.md`

The account controller, mixed safety smoke script, shared Auth/API/privacy code, root/router, backend,
contracts, manifests, lockfiles, artwork and `docs/source/` are unchanged. UI and
fixtures are newly authored; no upstream implementation was copied. Retained
React/Zod and existing canonical Auth/privacy interfaces are the dependencies.

## Behavior

The exact required inventory is PROFILE, FRIEND, POST, NOTIFICATION,
CONCERT_TICKETS, CONCERT_SESSIONS, CONCERT_SHARING, CONCERT_PRIVATE_DATA.
Each target must appear exactly once. Unknown, duplicate, extra, missing and legacy
four-step inventories fail closed as unconfirmed. No four-of-eight completion is
possible. No heuristic extension or shared-contract amendment is introduced.

Every completion requires all eight SUCCEEDED steps with valid completion timestamps
between saga creation and update, no step completing after the saga completion time,
consistent COMPLETED metadata, exact actor/saga/envelope identity, and a monotonic
updatedAt that is not in the future. The existing inherited date-time offset support
is preserved. The safe projection is ordered canonically even when the wire array
arrives in another order. Labels now use ordinary English, such as “Concert private
data”; status and timestamps do not expose provider diagnostics or lastError.

Pending/retrying/dead-lettered Concert cleanup remains pending or partial failure.
`deleted: true` still means disabled, not completed cleanup. HTTP statuses, envelopes,
saga/step statuses, confirmation guard and once-only request semantics are unchanged.
The existing controller retains actor/event/session/privacy and late-response fences:
uncertain DELETE closes canonical private owners and offers only manual GET recovery;
verified all-step completion uses canonical logout to revoke local identity. There is
no feature fetch, credential store, invented domain CommandReceipt, or repeated DELETE.
An absent qualified canonical Auth deletion port remains unavailable.

## Checks and evidence

Used retained Node 24.22.2 from
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` and existing
`/Users/lhs/Desktop/mcr/frontend/node_modules`. No dependency installation, Rust build
or database action was performed. A temporary `frontend/node_modules` symlink was used
only for resolution and removed before final boundary/clean-tree verification.

Successful commands, using that Node executable and the indicated retained entrypoint:

| Command (cwd) | Result / exit |
| --- | --- |
| `node /Users/lhs/Desktop/mcr/frontend/node_modules/vitest/vitest.mjs run src/features/account` (`frontend/`) | 74 tests, 2 files PASS / 0 |
| `node /Users/lhs/Desktop/mcr/frontend/node_modules/vitest/vitest.mjs run src/features/account src/features/auth src/shared/query` (`frontend/`) | 378 tests, 13 files PASS / 0 |
| `node /Users/lhs/Desktop/mcr/frontend/node_modules/typescript/bin/tsc --project frontend/tsconfig.json --incremental --tsBuildInfoFile /tmp/mcr-fe36-deletion-targets.tsbuildinfo --pretty false` (root) | PASS / 0; private temporary build-info path |
| `node /Users/lhs/Desktop/mcr/frontend/node_modules/oxlint/bin/oxlint src vite.config.ts` (`frontend/`) | PASS / 0 |
| `node src/features/account/__tests__/deletion-browser-smoke.mjs` (`frontend/`) | Native Chrome 360/390/430 PASS / 0; 15 canonical synthetic logout calls, no page errors |
| `MCR_FE36_EVIDENCE=/tmp/mcr-fe36-deletion-targets-mixed-evidence node src/features/safety/__tests__/browser-smoke.mjs` (`frontend/`) | Existing unchanged mixed safety smoke: native Chrome 360/390/430 PASS / 0; 24 synthetic canonical requests, no page errors |
| `python3 scripts/mcr/check-plan.py` (root) | PASS / 0; 13 source archive checksums preserved |
| `python3 scripts/mcr/check-boundaries.py --base d3b7c38268924d2db6bcdefce7bb84b86dafe329 --lane frontend --allow-doc docs/mcr/frontend/account-deletion-targets.md` (root) | PASS / 0; exact paths/imports manually checked |
| `node scripts/mcr/check-ui-origin.mjs` (root) | PASS / 0 |
| `git diff --check` (root) | PASS / 0 |

The initial pinned pnpm script wrapper tried its automatic dependency reconciliation
and aborted on the non-TTY modules-removal guard (exit 1); nothing was installed or
purged. All validation then used the retained tool entrypoints directly. Neither
CI override nor installer was used. The source task does not run a production bundle,
live Auth/deletion HTTP, backend saga, device or IN gate; these remain NOT_RUN.

The 48 new contract-focused cases check both generated inventories, complete and
reordered eight-target progress, each missing/unsuccessful/untimestamped target,
legacy/duplicate/unknown/extra inventory, safe retry/dead-letter projection, timestamp
and identity inconsistencies, and recovery from a legacy completed acknowledgment by
one explicit GET of the full eight-target saga. The 26 existing account cases now use
an independent eight-target fixture, preserving meaningful once-only, unknown-result,
privacy scrub, absent port, suspension/event/identity, synchronous-before-dispatch and
late-callback assertions against the new inventory.

Native account-owned fixture/evidence covers all eight labels, modal initial safe
focus/containment/Escape/focus restoration, pending and retry, partial failure,
private diagnostic suppression, absent adapter, unknown/legacy/duplicate result,
uncertain request, manual-only GET recovery, canonical cleanup/completion logout,
exactly one request, foreign late response, and no horizontal overflow. The fixture
port is explicitly synthetic. Only canonical Auth logout uses synthetic HTTP; no
real Auth deletion adapter, real owner acknowledgment or real erasure is implied.

Artifacts: `/tmp/mcr-fe36-deletion-targets-evidence/result.json`,
`pending-{360,390,430}.png`, `partial-{360,390,430}.png`. Result JSON records source
hashes, viewport assertions, synthetic flags and canonical logout requests. Its
source hashes were compared to the delivered files and match. Pending 390 and
partial-failure 360 screenshots were visually inspected: readable wrapping/scrolling,
no overflow, no visual redesign or left accent decoration. Temporary artifacts can
be reproduced with the committed account-owned browser fixture.

## Contract pins and authorized shared-fixture alignment

| Read-only contract/source | SHA256 |
| --- | --- |
| `docs/mcr/account-deletion-contract.md` | `c6419953fd89cb200900d4e3cfdaed44bf0eef0d5abf612386c1285840f63b05` |
| `backend/scripts/docs/openapi/schemas.py` (schema provenance only; never imported) | `bd35873422cfe2930777423c1367afe725343c766c07ce92b3a8a0b087b85875` |
| `contracts/generated/mcr-core.openapi.json` | `5af06192296d1765cde2bbceedf07a86156715f76ae1850158b42d2c815afcfd` |
| `contracts/generated/mcr-domain.openapi.json` | `a5fbe73124ed069e6357f5797a264e2d2a1bec93b0179bfc8550bf1122b69fde` |
| Unchanged `frontend/src/features/account/controller.ts` | `623d2418bba9e455dc83113ba7606b510ec4a99ba0cd429ae919516fe8f72d63` |

After the initial account-owned check, Main expressly granted the exact test-only
`frontend/src/features/safety/__tests__/preview.tsx` path to align its legacy saga
inventory. That fixture now imports the owned `deletionTargets` export instead of
duplicating a four-target literal; every scenario and underlying behavior is retained.
The mixed `frontend/src/features/safety/__tests__/browser-smoke.mjs` remains byte-for-byte
unchanged and now passes with the eight-target example. Its independently isolated
artifacts are `/tmp/mcr-fe36-deletion-targets-mixed-evidence/result.json` and the
`report-{360,390,430}.png` / `account-{360,390,430}.png` screenshots. This exact
authorized fixture-only difference is included in the ten-path handoff; no other
safety/Auth/root scope was acquired.

No outstanding fix is known in the assigned consumer scope. Main receives the clean
committed handoff, source base, checks and the explicit authorized shared-fixture difference directly;
no extra scrum-lead or broad review barrier is introduced.

Final artifact SHA256 (both source-hash inventories match the delivered files):

- `/tmp/mcr-fe36-deletion-targets-evidence/result.json`: `9f2de4dac04750ce39088ba9b91209898ca6d94ec31318d0fdc5826bdaf70d99`
- `/tmp/mcr-fe36-deletion-targets-mixed-evidence/result.json`: `e13a2395b369991ad9e31b100090e6eca092d90be41d295d87dc15e979bd432e`
