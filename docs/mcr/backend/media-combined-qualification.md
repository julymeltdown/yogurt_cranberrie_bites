# BE24 combined-candidate build and native qualification

2026-11-04 KST: **scoped combined qualification PASS; full BE24 IN_PROGRESS**.
The actual combined candidate compiled successfully with retained Rust1.88.0,
then its newly compiled media executable passed17 nonignored tests and the
restart-only test after an actual owned PostgreSQL stop/start. Source, donor and
frozen artifact checks passed. No compiler or integration failure required a
source change. HTTP, real Auth/BE37 fences and live product journeys remain open.

## Candidate and exact ownership

- Exact source/build/native execution HEAD:
  `fa4b4b3c491c770eda3438f41d155ebf57e426ab`.
- Checkout: `/Users/lhs/Desktop/mcr-worktrees/be-media-combined`.
- Branch: `codex/mcr-be-BE24-combined`.
- Candidate parent baseline: published master
  `58282b4cf3c995e3051c1f5ecf79407bda6b25ab`.
- Only qualification change: this new document,
  `docs/mcr/backend/media-combined-qualification.md`.
  Its final handoff SHA is recorded to Main; the report commit directly follows
  the exact execution HEAD and does not relabel the tested source revision.

Main prepared the candidate by serial integration of ten BE24 producer commits,
including the explicit shared-registration grant24e9995. Its25-path source delta
contains21 nonshared producer paths and four media registration/dependency edges.
This assignment qualifies that combined candidate rather than reusing the older
producer executable's success. Source, tests, harness, contracts, root composition,
original artifacts and other documentation remained read-only. Only Main publishes.

The original source-only native qualification remains separately recorded in
[media-native-qualification.md](media-native-qualification.md); earlier failed
producer reports are preserved. The [source scope/limits](media-upload.md),
specification, project assessment and execution plan still govern acceptance.

## Private build, donor and resource control

New private build root (0700):
`/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/`.
Only its `target/` and `tmp/` were used for compilation. The shared
`/Users/lhs/Desktop/mcr-build/backend` and frozen Concert target/bundles were
neither used as a writable target nor pruned. No dependency install/update or
source-mtime invalidation occurred.

The exclusively created target was populated once with:

```sh
/bin/cp -cR \
  /Users/lhs/Desktop/mcr-build/current-service-683484b/target \
  /Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target
```

Exit0,6.034s, APFS copy-on-write with no regular-copy fallback. The donor's27,921
regular files retained identical inode/size/mtime/mode metadata after cloning,
after compilation and after native execution. All51 sampled destination files
had distinct inodes from their donor counterparts. The full donor inventory is
retained privately. Eleven selected frozen bundle/receipt/binary files also
retained their SHA256 values; this includes the original producer artifact and
receipt, six-service bundle/receipt and frozen Concert binary/receipt.

Only cloned cache paths were invalidated:217 entries beneath the private
`target/debug/.fingerprint/` and `target/debug/build/` matching `contracts-build`,
`auth-service`, `friend-service` or `concert-service`. This forces current helper/
proto generation without touching source mtimes, donor caches or another target.
The exact list is in the build receipt. No cache/source failure or retry occurred.

The runner used one Cargo process at a time, jobs1, debug0 for both dev/test,
`CARGO_INCREMENTAL=0`, and no application/provider credentials. A one-second
monitor guarded an8GiB floor and would terminate only its owned process group.
Minimum observed free space:18,452,041,728 bytes (17.185GiB); no floor termination
occurred. Post-native free space was18,392,027,136 bytes. Both build processes
ended before native execution; no owned compiler remains running.

The retained toolchain and explicit build environment were:

- Cargo1.88.0 `(873a06493 2025-05-10)`; rustc1.88.0
  `(6b00bc388 2025-06-23)`, aarch64-apple-darwin.
- PATH starts at `/Users/lhs/.rustup/toolchains/1.88.0-aarch64-apple-darwin/bin`,
  followed by `/opt/homebrew/bin:/usr/bin:/bin`; explicit RUSTC/RUSTDOC there.
- `CARGO_HOME=/Users/lhs/.cargo`, `RUSTUP_HOME=/Users/lhs/.rustup`,
  `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/media-combined-fa4b4b3/target`,
  `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`,
  `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`.
- `PROTOC=/opt/homebrew/bin/protoc` (retained libprotoc33.4), `LC_ALL=C`,
  TMPDIR at the new private build root's `tmp/`. The earlier qualified Concert
  build report supplied the retained toolchain/library discovery approach.

Actual sequential commands, both exit0:

```sh
/Users/lhs/.cargo/bin/cargo +1.88.0 check \
  --manifest-path /Users/lhs/Desktop/mcr-worktrees/be-media-combined/backend/Cargo.toml \
  --locked --offline -j 1 -p concert-service --lib
/Users/lhs/.cargo/bin/cargo +1.88.0 test \
  --manifest-path /Users/lhs/Desktop/mcr-worktrees/be-media-combined/backend/Cargo.toml \
  --locked --offline -j 1 -p concert-service \
  --test media --no-run --message-format=json
```

Library check:13.092s. Test compile:24.175s. Build runner interval:
`2026-11-03T18:42:34.867344+00:00`–`2026-11-03T18:43:21.195921+00:00`
(2026-11-04 KST), including clone and binding checks.

The executable was selected from Cargo's actual `compiler-artifact` JSON,
privately copied, compared byte-for-byte by SHA256, and set0500:
`/private/tmp/mcr-be24-combined-fa4b4b3-20261104/media-tests-combined-fa4b4b3`.
Size17,592,976 bytes; SHA256
`fc681931df4fdf14c20f10186416e0fbf6ee8d2076e57b2d8c5f06001e5f75d9`.
Its source is the combined candidate, not original producer0d0846c. The original
receipt was never rebound or overwritten.

## Exact source binding and owned native run

The new build receipt pins all1,438 tracked backend/contract files (including
Cargo inputs, helpers, build scripts, proto/OpenAPI inputs, source, tests and
migrations) and the exact combined source revision. Those hashes were checked
before/after compilation and after native execution with zero changes. The
unchanged smoke harness admitted the new executable's exact private path/hash,
receipt success, complete supplied source pins and candidate ancestry before
creating any database. Candidate status was clean through native execution.

```sh
PYTHONDONTWRITEBYTECODE=1 python3 backend/scripts/mcr/media_upload_smoke.py \
  --repository-test /private/tmp/mcr-be24-combined-fa4b4b3-20261104/media-tests-combined-fa4b4b3 \
  --artifact-receipt /private/tmp/mcr-be24-combined-fa4b4b3-20261104/build-receipt.json \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report /private/tmp/mcr-be24-combined-fa4b4b3-20261104/native-report.json
```

Actual exit0, PostgreSQL16.14 Homebrew. The unchanged helper creates a fresh
private root/socket and random marker/token/password; it validates inode/UID,
postmaster and actual data directory before use and teardown. It enables fsync,
disables TCP listening and uses only the new private Unix socket (5432 is the
namespaced socket filename). No existing/default/foreign DB or external service
was attached. Its Rust fixture independently validates the owned root and exact
`mcr_concert_be_media` database before connecting.

| Actual native check | Result |
| --- | --- |
| All13 current source SQL migrations, versions1–12 and16 | All exit0;21 media columns verified |
| Unlimited / shorter caller / max-integer timeout clamps | PASS;1500/5000,100/200,1500/5000ms |
| Expired PENDING/UPLOADED/READY mutations and private read/intent | PASS; zero mutation and no private projection |
| Whole nonignored suite, `--test-threads=1` | Exit0;17 passed,0 failed,1 restart-only ignored,0 filtered |
| Actual owned PostgreSQL stop/start | Completed before second test process |
| `--ignored --exact durable_restart_verify --test-threads=1` | Exit0;1 passed,0 failed,0 ignored;17 filtered |
| Restart persistence / cleanup | PASS / PASS |
| Real production authority | NOT_COMPOSED |
| Public HTTP E2E / SQLx runtime migration ledger | NOT_RUN / NOT_RUN |

All22 command entries in the native report exited0. The suite includes durable
receipts/replay, competing finalize, private/foreign ID denial, transactional
purpose-bound linkage, revocation, metadata/codec/animation and streaming bounds,
quota/state guards, erasure/cleanup and delayed-authority expiry regressions.
The initial suite's restart seed commits READY media; after actual stop/start,
`durable_restart_verify` reads the sanitized4×3 image, confirms the private
metadata marker is absent and checks three SUCCEEDED command receipts.
The harness asserts that its owned cluster root is absent after cleanup.
It does not report before/after PID timestamps, so none are invented here.

## Immutable evidence and exact input pins

All runtime evidence is under the new0700 directory
`/private/tmp/mcr-be24-combined-fa4b4b3-20261104/`; JSON/log/runner files are0600,
executable0500. No credentials or gate config are committed. The unchanged runner
checks generated password/URL/token values before retaining sanitized test
output. No live DB, account credentials or service configuration remains.
The final post-native binding check completed at
`2026-11-03T18:44:12.737994+00:00`.

| Private artifact/evidence | SHA256 |
| --- | --- |
| `build-receipt.json` | `3e097c1fc89a07d75154d5976c90f6e34526c33b8bda64bfcfae04a0f5f71330` |
| `native-report.json` | `c3a3676f3b1c60f0b08b3a044485a174edce6a11ed11cd33db62f74bc5401d72` |
| `final-binding.json` | `2e86f4a5868a36eb9923242f014a718a59af863041a8e613bc66d88040414d95` |
| `build_once.py` | `42aaf3761cdaa7140a41695b33c787430abaa6c10e73de11fab8b7398e465145` |
| `source-before.json` | `252c09a85a2ab955af1fc63b15b8493d65c785bd77445f9dd60332d49e547c74` |
| `donor-before.json` | `b6b1bd91c9351d29f7412510dd6e8e5ef50a3e8be6d212759bfb04a7eb4f139b` |
| `frozen-before.json` | `fee390dacbfb01d621de8cc6951a8b3a8503392dfcb06e30531b82f6ae8138c1` |
| `cargo-check.stderr` | `59070f99adb741160c55742bc4e3be3f8bf22712b77fab3abbc55f84c44056bd` |
| `cargo-media-no-run.stdout` | `a298ea054075ea652f5a75f54016341888708e17001f92dbf144e576defc4a71` |
| `cargo-media-no-run.stderr` | `87d9b446ce35157363ad80a3baa4e50d497fc0c83bb0a8bc2e2eb8e7c2f9b432` |

The build receipt records full commands/environment, raw log byte hashes, donor
inventory result and exact cache paths. The native report retains full sanitized
test output plus all migration and contract hashes. The final binding receipt
rechecks those outputs,1,438 inputs,11 frozen files and27,921 donor metadata rows.

| Current execution input | SHA256 |
| --- | --- |
| `backend/Cargo.lock` | `208bd6ae3e1657ee5cc6128241bf7fbc39c122efc467efa8deba30b98f2a1dc4` |
| `backend/scripts/mcr/media_upload_smoke.py` | `3e2b2b66cb2b8a7e806f95eea9954575ce3fdc99fa0fb7068d8c9227d4c0d613` |
| `backend/scripts/mcr/migration_smoke.py` | `a46f2dc3a6dcab9310230cd75997dfcd86571c88fd066a973a8a0c3b1a8c6eb8` |
| `contracts/domain/v1/feed.schema.json` | `590cce6afc00c7a869b5a9711104550ba280ffa5875c9dc49a0753d272320801` |
| `contracts/domain/v1/operations.json` | `411fcee8222db2f5a7938866ac37791033e85682f11d024ceb895c245c708500` |
| `contracts/domain/v1/policy.json` | `01c6b9e9a44b81bcf1273ebd7ba8361d3cc035a969399b4dad1f9dedec05839a` |

All migration paths are under `backend/services/concert-service/migrations/`:

| Source SQL | SHA256 |
| --- | --- |
| `0001_concert_core.sql` | `9978e75e569c1ba25cb191ddf5b02bbdffcd5e64f3385173f05a885370d9215a` |
| `0002_command_receipts_outbox.sql` | `5f5760dfe6384cb2ab5162bcc27fbc0ed75bdca6bb85229012f6389269ee442f` |
| `0003_places_map_releases.sql` | `b2d4ab07a2addece8ddb4a3e413da2a32605d00ea34082a36f5940d1ff162e7a` |
| `0004_bootstrap_admission.sql` | `cb1c0093eeaef79dbe783052447c34c78aad5c6daebdc2f6f3dc7f4b17112ac9` |
| `0005_world_admission.sql` | `100e8de5a8392a4cd11e83337303e44e9ef79cee7c6f3c7547e333bcd9c7b761` |
| `0006_relationship_policy.sql` | `0905ddf463325c5097a3a732e03a237f99d5f53060c1df255eed126e468b2fd3` |
| `0007_community.sql` | `6c22de4cbfc51db3b3bc23b151d19e5ad43ddc36a794ffdf7cf4507e19090930` |
| `0008_meeting_beacons.sql` | `b8259eb000c15059ddcba60db64518562d2cede7d2d68c42de3df8a85b44c655` |
| `0009_receipt_domain_failures.sql` | `e357529103bb998f337508a30b03bddc372fe387b7e587dcb7f96a8b295fa4c6` |
| `0010_feed.sql` | `a0d3ac6672e37a1bff26129317ddaafdecfc18701c751b340df219c0859d7997` |
| `0011_outbox_maintenance.sql` | `8971093dee1293dfd160920c773d663866dcb5cb688fb7cabf6746eaca2eb61b` |
| `0012_media.sql` | `aae18779fda409d5a9de1844336c3e58d3705197346ce94b9b5e0a926e119224` |
| `0016_meeting_deadline_guards.sql` | `4375673c48b46096cf20dcbac807eace37d96136e347ac9da3b95a884135ff7d` |

## Checks and remaining acceptance

`python3 scripts/mcr/check-plan.py`: exit0, all13 original archive checksums and
planning regressions intact. Explicit staged `git diff --check`: exit0. No extra
unrelated test suite or general review gate was imposed. The handoff contains
this one new document; Main handles coherent source publication separately.

The authority fixture remains a synthetic transaction-locked test adapter, not
canonical Auth or real BE37 remote revocation fencing; production authority is
not composed. Source SQL application here does not qualify the SQLx full runtime
ledger, service startup/grants or public transport. HTTP initialize/PUT/finalize/
read, browser FE04 integration, real Post/Giveaway/Report journeys, cross-service
publication atomicity, root routes/composition, scheduled cleanup, live deletion,
device/load/RSS and IN02/IN03/fullBE24/release acceptance remain open. No fake
owner authority or relaxed guard was added to obtain this bounded result.
