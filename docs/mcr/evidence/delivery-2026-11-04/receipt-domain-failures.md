# Domain failure receipts — bounded integration

Migration 0009 extends the BE15 database whitelist and Rust `SafeFailure` enum
with the existing C04 party, giveaway, reservation, event and map error reasons.
Migration 0002 remains unchanged. No new public error code or HTTP route is added.

The native receipt fixture now verifies transactional failure rollback and
authorized same-key replay for each of the ten added reasons. The existing
17-test suite passed on a fresh PostgreSQL 16.14 cluster, including migration
failure recovery, exact combined migration ledger (1, 2, 3, 4, 9), restart
persistence and owned cleanup. See [the native receipt](receipt-native.json)
for source hashes, binary hashes, commands and exit codes. Tests ran against
the explicitly recorded dirty source snapshot at `20503f4`; this commit records
those changes without claiming that the earlier commit already contained them.

Build: Rust 1.88.0, locked/offline, jobs=2, dev/test debug=0, shared target
`/Users/lhs/Desktop/mcr-build/backend`. Exact Cargo executable artifacts were
copied into `/tmp/mcr-delivery-native-pinned-20261104` before releasing the build
lease. The native command was:

```sh
python3 backend/scripts/mcr/command_receipts_smoke.py \
  --binary /tmp/mcr-delivery-native-pinned-20261104/concert-service \
  --repository-test /tmp/mcr-delivery-native-pinned-20261104/command_receipts-2e6a477553d73627 \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /tmp/mcr-delivery-receipts-20261104.json
```

Exit 0; 17/17 tests; restart PASS; cleanup PASS. Production mutation producers,
live owner authority and real multiuser flows remain separate obligations.
