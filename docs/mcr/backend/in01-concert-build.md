# IN01-CONCERT-BUILD — current binary and PostgreSQL lifecycle

Execution date: **2026-11-04 KST**. Bounded result: **PASS** for the current Concert
binary build and actual full SQLx migration/verification/restart lifecycle.
Serving, readiness, product releases, World and full IN01 remain **NOT_RUN**.
No compiler or service-source blocker was found. This removes the binary and
database-lifecycle prerequisite; it does not compose the seventh service.

## Ownership and source

- Worktree: `/Users/lhs/Desktop/mcr-worktrees/be-concert-build`.
- Branch: `codex/mcr-be-IN01-concert-build`.
- Exact build/execution base: `11d7cb00165f78b2ec9c564dd93669edad104e88`.
- Sole tracked writable path: `docs/mcr/backend/in01-concert-build.md`.
- Private artifact root: `/Users/lhs/Desktop/mcr-build/concert-current-11d7cb0`.
- Immutable binary: `/Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/bundle/concert-service`.
- Binary SHA256: `db5d0e0db054e06a9be25361da8ddb33c791a85c96d4189f9d0a194ebce2c100`.

The earlier worktrees remained stopped. Application/backend sources, tests,
configuration, manifests, lockfiles, contracts, root scripts and composition
were read-only throughout. No asset, Auth identity, role, admission or product
release was manufactured. Only Delivery publishes master; no additional
scrum-lead or main-chat review is required for this bounded handoff.

## Build and resource evidence

The user authorized only a new private target. The frozen source target
`/Users/lhs/Desktop/mcr-build/current-service-683484b/target` existed and its
debug directory had no open handles. An unrelated compiler was active against
`/Users/lhs/Desktop/mcr-build/backend`; it was neither touched nor stopped.

The private runner exclusively created its output root and used:

```sh
/bin/cp -cR \
  /Users/lhs/Desktop/mcr-build/current-service-683484b/target \
  /Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/target
```

This is an APFS copy-on-write clone, with no hardlinks. The source inventory of
27,921 files retained identical inode/size/mtime metadata through clone, build
and final checks. Selected source/destination files had distinct inodes. No
source-target, original bundle, original receipt, shared target or community QA
target was written or deleted.

The actual build command, using the retained Rust toolchain, was:

```sh
/Users/lhs/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin/cargo build \
  --manifest-path /Users/lhs/Desktop/mcr-worktrees/be-concert-build/backend/Cargo.toml \
  --locked --offline -j 1 -p concert-service --bin concert-service
```

Environment included the new `CARGO_TARGET_DIR`, `CARGO_INCREMENTAL=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, explicit toolchain Rust compiler/doc paths and a
private temporary directory. The exact allowlisted environment and argv are in
the build receipt; application/provider credentials were not inherited.

- Cargo: `1.88.0 (873a06493 2025-05-10)`.
- Rust compiler: `1.88.0 (6b00bc388 2025-06-23)`, `aarch64-apple-darwin`.
- Exit: **0**; runner elapsed **18.095 seconds**.
- One owned Cargo process, `-j 1`; no repeated launch or competing owned build.
- A once-per-second monitor would stop only the owned process group below an
  8 GiB free-space floor. Minimum observed free space: **19.19 GiB**.
- All **1,288 backend source hashes** matched before/after the build and again
  after native execution. Final qualification also pins **130 contract files**.
- `backend/Cargo.lock` remained unchanged, SHA256
  `6561f411ed8aa09481baf2fc3c91bfe0589fc36ceb10fa59a37f0d74a8ee5b87`.
- Bundle/root are 0700, copied binary is 0500, receipts/logs/private runners are
  0600. The binary was copied after successful build and matched target bytes.

## Actual full SQLx lifecycle

The private native runner imported unchanged
`backend/scripts/mcr/migration_smoke.py` at the exact base, helper SHA256
`a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8`.
It reused the helper's marker/inode/postmaster ownership, exact SHA384 migration
ledger checks and bounded subprocess/cleanup behavior. No helper source was edited.

Homebrew PostgreSQL **16.14** created a new owned cluster and database
`mcr_concert_build`, which satisfies the current service's strict
`mcr_concert_<suffix>` URL guard. The corrected run held a fresh loopback port
reservation, 58468, and configured the helper's in-process socket-port setting
to match. PostgreSQL itself had **TCP listeners disabled**, serving only the new
0700 private Unix socket directory. No default/foreign database was attached.

The service CLI received only the clean owned Cluster environment plus its
generated `CONCERT_DATABASE_URL`. No serving configuration, metrics endpoint,
caller/actor credentials, external service or product-release file was needed.
The ephemeral database owner password stayed private; log text was redacted.

| Actual command/check | Exit/result |
| --- | --- |
| `concert-service --help` | 0 |
| `--verify-migrations` against the new empty database | 1, expected refusal; `_sqlx_migrations` remained absent |
| `--migrate` | 0; all 12 current migrations applied |
| Exact full ledger check | PASS; versions 1–11 and 16, every successful row and SHA384 checksum matched source |
| `--verify-migrations` | 0 |
| Repeat `--migrate` | 0; full version/success/checksum ledger unchanged |
| PostgreSQL stop/start | PASS; actual postmaster identity/start time changed |
| Independent persistence sentinel | PASS after restart in separate `mcr_build_probe` schema |
| Exact full ledger after restart | PASS, 12 entries |
| `--verify-migrations` after restart | 0; full ledger unchanged |
| Owned cleanup | PASS; cluster root removed, private reservation released |

Migration-owned rows were checked against source: **59 command-operation
registry rows** and **one zero-used command-budget row**. Every other public
application table was empty. The runner inserted only the independent restart
sentinel; no Auth account, event, participant, role, restriction, admission,
ticket, membership or owner lease was inserted. All positive service subprocesses
exited cleanly; the deliberate pre-migration verification refusal is recorded
separately and is not a startup failure.

## Preserved failed probe and corrected rerun

The first native attempt successfully ran help, missing-ledger rejection,
migrate, verify and repeat-migrate, then failed an **external probe assertion**
which incorrectly required every public table to be empty. Migration 0002
legitimately inserts its operation registry and budget control row. The source
and binary were correct; PostgreSQL restart was not reached in that attempt.
Cleanup passed and its owned root was removed.

The failed receipt/log/runner were retained without rewriting. A separate
`qualify_postgres_rerun.py` corrected only that assertion, verifying the exact
migration-owned rows and requiring all other application tables to be empty.
It used another fresh owned cluster and completed the full lifecycle above.
No source fix or second Cargo build occurred.

## Pinned artifacts and reproduction

All paths below are under
`/Users/lhs/Desktop/mcr-build/concert-current-11d7cb0/`:

| Artifact | SHA256 |
| --- | --- |
| `build_receipt.json` | `bc29f79ae045ce0424c28bed47e80b6e384c8f2ec4da93575ee68a706d3bf4bc` |
| `build.log` | `483b68465b1dcd0511d6fbecb5448058abb7c19d5f6978add7d02e9c9d7d6161` |
| Preserved `migration_receipt.json` (probe FAIL, cleanup PASS) | `01c3e73e497196b316dc04bd2176aaf35e2d660131933200c7732ccb3028af0b` |
| `migration_receipt_rerun.json` (PASS) | `b27ad7336ef50e37348742ee5be3d690ab20bb8d21cc1034f81289931a944db9` |
| `migration_rerun.log` | `62d92576fd8efb5a8791bef55be61fe0d3177f3e7657799f732138ae848e5ea8` |
| `qualification_receipt.json` | `2fc1b061f9631cd0f9ef84c6bcef0ce2e5db469cd65c965bfc6e2cfcf62fe7aa` |
| `build_once.py` | `a9cd8f6fa4e941e6f6e2800e7c64899086f172e0c7b772663d4be39e7c752674` |
| `qualify_postgres_rerun.py` | `450c4476ffa2be638ee7e6c5eca0367dd445b284ce74ba8153c8507c2d099f7b` |

The build receipt carries the exact source revision, backend source hash
manifest, toolchain/command/environment/exit, binary hash, source drift and clone
checks. The passing migration receipt binds that receipt and binary, all 12
SHA384 migration checksums, actual command exits, restart, row counts and cleanup.
The final receipt binds both attempts and the private runners, rechecks backend
source/lock/binary bytes, and pins the contract inputs. These artifacts contain
no live credential values. Neither database remains available.

The private runners preserve the exact executed procedure for review. They use
exclusive output creation and cannot simply overwrite this evidence; a future
execution requires a newly assigned private output namespace. The native runner
does not recreate product-serving configuration or leave a running consumer.

## Remaining integration boundaries

This local binary resolves migration files via the compiled-in source path
`/Users/lhs/Desktop/mcr-worktrees/be-concert-build/backend/services/concert-service/migrations`.
Keep that pinned read-only source available for CLI/runtime use; this is not a
qualified relocatable deployment package.

Delivery/composition still owns the seventh-service process and Gateway endpoint,
full serving schema/readiness probes, caller trust and actor delegation, the six
real product-release files, selected-source-pack artwork/default-room pins and
actual HTTP/WebSocket World flow. The earlier provisioner's `mcr_in_concert`
fixture name is still incompatible with the current Concert URL namespace; that
constructor change remains with its assigned owner. This grant did not allow
editing the tool or silently substituting a schema-only fixture for this full
SQLx lifecycle. No serving/readiness, browser, art or full IN01 result is claimed.

`python3 scripts/mcr/check-plan.py` exited 0 with all 13 source archive hashes
intact. `git diff --check` exited 0. Source drift checks passed. Delivery ancestry
is exact base `11d7cb0` followed only by this documentation handoff commit; the
complete changed-path list contains this one document.
