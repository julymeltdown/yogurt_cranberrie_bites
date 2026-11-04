# FE19 Map source03 qualification

PASS_SOURCE_ONLY. Lease `FE19-MAP-SOURCE03-202611041223Z`, receiving `84c622efaf0676428e6507a1b05fea0747b718cf`, qualified code `43b80c915eea35a5eb46ec6dcc95dd677abae57b`. Production remains v1: the v2 accessor returns null and no new artwork pins are registered. This receipt is neither live admission nor v2 native/reference acceptance.

The accepted conservative clock was `2026-11-04T12:23:10Z`, active cutoff `12:28:10Z`, hard release `12:29:10Z`. Actual first child started `12:23:15.124220Z`. Existing checkout/branch only, one Node child at a time, 180-second per-child bound, 32MiB additional/64MiB whole limits, 16GiB fresh floor and 8GiB continuous floor. Precheck: clean frozen HEAD, 1,725,371 accounted bytes, 17,998,311,424 free bytes, matching retained lock, absent own link/TMP. The original aligned goal was read and retained; historical blocked metadata was not reset or falsely completed.

From `frontend/`, the three attempts used the exact committed supervisor:

```sh
MCR_MAP_SOURCE_LEASE='FE19-MAP-SOURCE03-202611041223Z' \
MCR_MAP_SOURCE_DEADLINE='2026-11-04T12:29:10+00:00' \
python3 ../docs/mcr/evidence/map-production-binding/source-round03/supervisor.py
```

`attempts.jsonl` records every exact argv, PID, timestamp and expected/actual exit. Vite uses `--configLoader runner`; the retained dependency link is absent-only and privately removed after each attempt; cache, TMPDIR and TypeScript build info use the owned stage. No install, build, server, browser, native, new worktree, ART export or private C03 operation ran.

| Attempt | Tests | Scoped TypeScript | Oxlint |
| --- | --- | --- | --- |
| 01 | 115/17 files, exit 0 | Exit 2: test-only catalog type inference, literal hash assignment and strict optional-property fixture | Exit 0 |
| 02 | 115/17 files, exit 0 | Exit 2: readonly property deletion in omitted-hash negative fixture | Exit 0 |
| 03 | 115/17 files, exit 0 | Exit 0 | Exit 0 |

The first correction parses catalog rows through the existing `placeSchema`, widens only the deliberately corruptible fixture hash, and models omission separately. The second uses object rest to omit the readonly optional property. Fixes are confined to the three new test files (`a145d5f78900d7640dee519c8f8e764ccc208f3a`, `43b80c915eea35a5eb46ec6dcc95dd677abae57b`); no product invariant or assertion was weakened. An initial staging command used root-relative paths from the frontend cwd and failed; staging was corrected in the root checkout. Final qualification ran after the fixes were committed. Both type failure logs remain intact.

The 28 new cases cover closed layout/identity/semantics, selected-era-only resource verification, byte/hash/dimension rejection, abort-resistant late image/font cleanup, complete-scene draw/label/encode atomicity, four service routes, qualified v1 compatibility, canonical marker/leader/selected-pin display alignment and honest closed/unavailable status. The same config includes the 87 prior regressions; no separate regression-only or CLI rerun occurred. Types include root provider construction and the actual IllustratedMap/MapServiceLinks source; lint covers those shared hunks and new modules/tests.

`source-pins.json` fingerprints 52 code/input/lock paths at the qualified commit. Its SHA-256 is `621167c13038b1f73ec5567d6f720580c1d09785c8ec87d1ca6519a135564a19`. The actual tested caption line exactly matches `916e67f5107b390d21a61a87798c1da789b9fc8d`, line SHA-256 `80888d5ab631881ba3cfe08c7e1a2b00be32cb892570f98c5df0efe47675f3ce`; the full tested IllustratedMap postimage hash is `a655570fcaef239ee8ec72b876a881c2b76b03815b0a7fa08125588b7f076112`. This qualifies that retained v1 caption hunk; the additional inactive v2 source is not an instruction to activate or include v2 in the frozen v1 cohort.

FULLRELEASE at `2026-11-04T12:26:03.052979Z`: all nine owned child PIDs and process groups absent, own dependency link/private TMP/cache absent, zero server/browser/native jobs. Additional peak 11,739,270 bytes; whole peak 13,458,517 bytes; retained whole before release receipt 1,751,885 bytes; final free 17,921,519,616 bytes. `cleanup.jsonl` preserves each attempt's cleanup; `release.json` records final process/accounting checks. The slot was explicitly returned to Resource21d before either cutoff. Subsequent engineering prose/commits create no job or resource lease.

Required next work: named Root publication of qualified bounded v1 and owner remote proof; final ART correction/pins; immutable v2 registration; relevant source checks/fixes and reserved canonical v2 native run; actual visual convergence assessment. Existing failed native packets remain failed. No king/SL acknowledgment or technical review gate applies.
