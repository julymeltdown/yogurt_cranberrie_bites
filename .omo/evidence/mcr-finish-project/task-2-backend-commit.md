# Task 2 - backend compile fix and QA01 re-pin

Commit: 8fc507fe9 `fix(backend): register Concert workload and repin QA01 inputs`
Base: 8e5cc757f

## Staged scope
```
backend/crates/platform/tests/grpc_auth.rs    | 5 ++++-
scripts/mcr/contracts/qa01-coverage/pins.json | 8 ++++----
2 files changed, 8 insertions(+), 5 deletions(-)
```

`git show --name-only --format=` lists exactly those two paths; no frontend, CI or lockfile change was mixed in.

## Change
- `grpc_auth.rs` adds `Workload::Concert` to the unregistered-workload arm of the exhaustive match, so the workspace compiles after the Concert workload was registered in `platform`. The test still asserts that only the matching caller credential is accepted.
- `pins.json` moves `canonical_candidate` from `7c563cf09fe971b904ac9549f5e3e8fe1391d94a` to `9185c99223bbf7437fff4346a7683dbb4187dfb0` and refreshes the three drifted input hashes (`domain-operations.generated.ts`, `generate-domain-transport.py`, `validate-assets.py`).

## Checks
| Command | Exit |
| --- | --- |
| `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/dev-target CARGO_BUILD_JOBS=4 cargo +1.88.0 check --locked --workspace --all-targets` | 0 |
| `python3 scripts/mcr/contracts/qa01-coverage/run.py` | 0 - `result: PASS`, candidate `501710d293e5309e862997a353714e5753a76da0`, 198 fixtures / 1727 responses / 150 operations |

Cargo emitted only `dead_code` / `unused` warnings in `concert-application` test targets, no errors.

## Not run
- Plan failure probe ("alter one input hash locally, expect `pinned input drift`") requires editing a pinned input in the integration checkout; the executor was instructed not to edit product code, so it is not reproduced here.
