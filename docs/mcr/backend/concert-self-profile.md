# Canonical Concert SELF Profile prerequisite

Status: IN_PROGRESS. This checkpoint publishes only the additive owning Profile
migration and source-bound SQL qualification tools. It does not enable the new
SELF RPC service or accept the full C03 participant flow. Its source task is
BE-PROFILE-SELF; developer checkout `codex/mcr-be-profile-self` preserves the
continuing implementation separately. Existing public, World and legacy Profile
producers are preserved. The public Profile owner retains all four existing
registration files and owns its separate migration 0009.

Migration 0008 adds internal epoch/revision tracking of canonical nickname/bio,
including retained legacy updates and deletion changes. Username/avatar remain
unchanged. Protected writes use a durable operation identity and same-transaction
content/receipt/audit with a prepared transaction; a proposed result is not yet
committed. Separate read references retain actual SQL identity without a dummy
write version or receipt. Recovery observations are scheduling hints only; they
never prove a native outcome or grant mutation authority.

The schema SQL controller applied all eight exact current Profile migration SQL
files to marker/inode-owned PostgreSQL 16.14. Three frozen historical storage
cases passed: legacy versions/tombstones, invisible prepared effects with blocked
racing updates, and controlled-restart commit/abort with exact receipts. A new
native read-reference case held the actual full XID live, independently committed
its read reference before positive SELECT, proved ABORTED after rollback, retained
the reference and created no write receipt. Prepared effects survived restart.
Cleanup passed. No mutable Cargo target was accessed.

Amended 0008 SHA-384:
`302533481e533b98362d2452db7f91f85eb6d08fe13a8d307877de05de57588e6358547f577f3d910418d48f7f61afa3`.
Historical helper SHA-256:
`1564b5f50ab484ba14dfd36c4af9aba798e460570904aa942e415ef2497bdf0a`.
The helper is selected through its successful Cargo JSON and frozen-artifact
receipt, not a mutable target path. The first schema attempt applied the DDL but
refused a missing exact test match caused by a doubled filter prefix; cleanup
passed. The corrected second run exited 0.

Command, from the developer checkout:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/concert_self_profile_schema.py \
  --cargo-json /private/tmp/mcr-profile-self-8f89d190-20261104/build-combined-1.jsonl \
  --frozen-artifacts /private/tmp/mcr-profile-self-8f89d190-20261104/frozen-artifacts-1.json \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /private/tmp/mcr-profile-self-8f89d190-20261104/schema-report-2.json
```

The private evidence directory preserves both attempts, source migration checksums,
frozen binaries, original Rust 1.88 locked/offline build receipts and cleanup
receipts. This schema check intentionally uses synthetic local binding metadata;
it proves SQL mechanics only. The amended migration binary CLI, new Rust RPCs,
actual Auth/Friend guard admission, actual Concert decision recovery and joined
multi-store/HTTP flow remain NOT_RUN. No fake guard grants, guessed coordinator
decisions, or timeout-derived aborts are acceptance evidence. New source files are
authored for this task; no source-pack files or retained dependency resolutions
were changed in this checkpoint.
