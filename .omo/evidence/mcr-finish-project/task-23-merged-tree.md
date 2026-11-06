# Task 23 — merged-tree fix and re-verification (2026-11-06)

- Branch: `codex/mcr-exec-w3-authz`
- Merged tree: `80de26a0c` = `master 6db4ecce4` + `08bcfba8d` (matrix delivery)
- Fix commit: `42b48301e fix(venue): keep the authz matrix out of the location/notes smoke glob`
- Worktree status after commit: clean

## Change

1. `backend/scripts/mcr/venue_postgres_smoke.py` replaces the `venue_*` glob
   with explicit `--test venue_location --test venue_meeting --test venue_notes`
   and adds: "New venue suites must opt in with their own owned database; never
   sweep a venue_* glob." The `mcr_venue_authz` exact-name guard in the matrix is
   unchanged.
2. `venue_authz_matrix.rs` assertion message now states: "product RPC refuses
   venue location/notes; HTTP authorization is proven at the application layer
   only".

## Merged-tree verification (HEAD 42b48301e)

`python3 backend/scripts/mcr/venue_postgres_smoke.py --target-dir /Users/lhs/Desktop/mcr-build/exec-w3-authz --report …`
exit 0, report `result=PASS`, `cleanup=PASS`, `source_revision=42b48301e`,
tests command exactly the three explicit targets, 4 + 3 + 1 = 8 venue tests
passed, restart PASS, dropped/mutated migration oracles exit 1, cluster removed.

| Command | Exit | Result |
| --- | --- | --- |
| `venue_postgres_smoke.py` | 0 | PASS; venue tests 8; restart PASS; cleanup PASS |
| `cargo +1.88.0 test -p concert-service --test venue_authz_matrix -- --test-threads=1` | 0 | 46 passed; 0 failed |
| `cargo +1.88.0 test -p api-gateway --test venue_authz_http -- --test-threads=1` | 0 | 4 passed; 0 failed |
| `python3 scripts/mcr/contracts/route-parity.py` | 0 | `registry=69 contract_commands=63 templates=229 commands=70 missing=0` |

Private PostgreSQL 16.14 cluster for the matrix: socket-only, port 55447, root
`/Users/lhs/Desktop/mcr-build/exec-w3-authz-pg` mode 0700, migrations
0001/0073/0074/0075; `pg_ctl -m fast stop` exit 0 and root removed.

## Failure oracle stays live

- Scratch smoke copy (same three targets plus `venue_authz_matrix`) run against
  the smoke's `mcr_concert_venue_location` database: exit 101, phase
  `cargo-venue-tests`, report `result=FAIL` — the matrix refuses the foreign
  database.
- Direct matrix run with `CONCERT_TEST_DATABASE_URL` naming
  `mcr_concert_venue_location`: exit 101 with `Error: foreign database refused`.
- The post-fix smoke's own migration oracles still exit 1 for dropped and
  mutated `0075_place_notes.sql`.
