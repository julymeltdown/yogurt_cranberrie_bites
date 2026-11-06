# Task 1 - frontend build and test repair

Commit: 8e5cc757f `fix(frontend): restore build and test suite at 9185c9922`
Base: 9185c99223bbf7437fff4346a7683dbb4187dfb0

## Staged scope
107 paths, `107 files changed, 446 insertions(+), 181 deletions(-)`:

| Group | Files |
| --- | --- |
| `frontend/` (modified) | 88 tracked files under `src/app`, `src/features`, `src/world`, `src/shared`, `e2e/`, `vite.config.ts`, `vitest.config.ts` |
| `frontend/` (new) | `src/virtual-modules.d.ts`, `src/test-setup.ts`, `src/features/auth/__tests__/fixtures/admission.ts`, `src/app/world-entry/__tests__/admitted-auth.ts` |
| generated client move | 8 pure renames `R100`/`R097` from `contracts/generated/api-client/*` to `frontend/src/generated/api-client/*` (git records the removal as the rename source; `git ls-tree HEAD -- contracts/generated/api-client` is empty) |
| generator | `scripts/mcr/generate-api-client.py` |
| contracts scripts | `scripts/mcr/contracts/c07/run.py`, `scripts/mcr/contracts/c07/test_inventory.py`, `scripts/mcr/contracts/qa01-coverage/run.py`, `scripts/mcr/contracts/qa01-coverage/test_coverage.py` |
| docs | `docs/mcr/c07-generation.md`, `docs/mcr/evidence/c07-social-client-in03/inputs.json` |

A `git show --name-only` filter with the inverted allowlist returned nothing, so no backend, CI, `.omo/`, or `docs/mcr/evidence/delivery-2026-11-04/` path entered the commit. `scripts/mcr/contracts/qa01-coverage/pins.json` was deliberately left for commit 2.

## Change
- API client is generated into `frontend/src/generated/api-client` and the 8 stale `contracts/generated/api-client/*` files are deleted; generator output count is 18.
- `vite.config.ts`/`vitest.config.ts` add the `virtual:venue-root-art` plugin, test setup file with `asyncUtilTimeout: 5000`, and collection excludes; `src/virtual-modules.d.ts` declares the virtual modules.
- Route test derives from `generated/IA_EN.json`; admission fixtures and four shared test fixtures are wired to the new shells.
- Product fixes retained in the commit: `Landing.tsx` microtask redirect and the gift owner portrait reference.

## Checks (all run on the pushed tree, HEAD = 501710d29, tracked tree clean)
| Command | Exit |
| --- | --- |
| `export PATH=/Users/lhs/.local/node-v24.22.2/bin:$PATH; cd frontend && corepack pnpm@11.7.0 run typecheck` | 0 |
| `corepack pnpm@11.7.0 run build` | 0 - `✓ built in 1.19s` |
| `corepack pnpm@11.7.0 run lint` | 0 - warning-level `unicorn(no-thenable)` in the generated client only |
| `corepack pnpm@11.7.0 run test` | 0 - `Test Files 411 passed (411)`, `Tests 5683 passed \| 14 skipped (5697)` |

## Not run
- The plan's failure probe (run `vitest run src/app/__tests__/routes.test.ts` after removing an IA path) mutates a generated/product input in the shared integration checkout; the executor was instructed not to edit product code, so it is not reproduced here.
