# QA03 database race qualification

Status: ACCEPTED for the original QA03 database race criteria and PASS for
the current master re-run. Fourteen native cases pass against PostgreSQL 16.14
from base commit `25b2d356f` plus this task's QA03-only changes. The earlier
eleven-case run below remains retained as historical evidence for source
`eb9bb664`; it is not the current candidate. This is task-local database
evidence, not acceptance of canonical owner RPC, public HTTP, IN02, IN04, or the
release. Earlier compilation/native failures remain retained.

The original QA03 row in `MCR_IMPLEMENTATION_PLAN_V2_EN.md` requires parallel
capacity, reservation, cancellation, expiry, claim, idempotency, and mural cooldown
scenarios against PostgreSQL, with forced transaction interleavings and worker
restarts. Its dependencies are BE17, BE22, BE23, and BE34. The invariants in §§7.4,
8.2 and 8.4 and those dependency rows remain in scope; no criteria were removed.

## Ownership and provenance

- Checkout: `$MCR_WORKTREE_ROOT/qa-database-races`.
- Branch: `codex/mcr-qa-QA03`.
- Initial published master: `93ef0d895dcc4b247b727e2cccba0bb84576b262`.
- Writes: `backend/services/concert-service/tests/qa03_races.rs`,
  `backend/services/concert-service/tests/qa03_races_support/**`,
  `backend/scripts/mcr/qa03_races/**`, and this document only.
- Existing application code, feature tests/helpers, manifests, lockfiles, proto,
  contracts, routers, CI, and source archives are read-only.
- Fixture construction is adapted from `community_party.rs`,
  `community_giveaway.rs`, `mural.rs`, and ownership checks from
  `owner_authority_support/environment.rs` at the initial baseline. Dependencies
  are retained SQLx/Tokio/UUID/Chrono/serde/async-trait/anyhow and the real
  Concert application/domain/service crates; no new dependency is introduced.
- Lifecycle/ledger code is imported read-only from
  `backend/scripts/mcr/migration_smoke.py` at that baseline. The new controller
  owns a distinct `mcr_qa_races` database and marker/inode-bound private cluster.
- Exact runtime source and domain-contract hashes are captured before build,
  checked after compilation, and checked again before native execution. Cargo
  output and copied binary are digest-bound in the build receipt. The successful
  source revision is recorded above and in the committed native report. Delivery
  ancestry is `93ef0d89` → `493292f9` → `1ef7bbbb` → `f73d9382` → `e62a7025`
  → `eb9bb664` → `47fb4a77` → this evidence commit. The committed handoff records
  its exact final SHA; only the designated integrator delivers it to master.

Feature owner inputs: Party most recently identified clean source
`425b5feae78bb32a3e414d2f7a4e0fd57123228c` for its newer live projection lock
ordering; the QA03 author performed read-only compatibility inspection and found
no concrete issue. That newer live-runtime change is not imported or certified
by this retained-adapter suite. Wall confirms user source
`57dc31e1c8f8e1ba0f203d71a50cc3f1629bc8fa` and additive operator source
`bcf81212036872e8b0d2b9694a21f6d97b3f091b`; Gift confirms retained database
adapters at this baseline are byte-identical to its qualified source
`5329d3f493a87d597c7fb20a4e0f0ecfc1cf1395`. These messages identify interfaces
and limitations; their earlier test results are not QA03 execution evidence.

## Required scenarios

Every row uses real production PostgreSQL adapters and migrations, not an
in-memory reimplementation. An explicit isolated test ACL supplies policy. This
qualifies database invariants; it does not certify canonical Auth/Friend RPC,
Gateway, real physical-place provenance, private response settlement, IN02, or
IN04. Those joins remain owned by Root and the corresponding feature owners.

| Requirement | Native scenario | Required observation |
|---|---|---|
| BE17 last slot | Two approval transactions blocked behind held party row | Lock graph proves both contenders; one success, one version conflict; active members at capacity with owner retained |
| Approval idempotency | Two exact replay transactions blocked behind root | Same receipt, no added outbox; changed body/key conflicts |
| BE21 stock | Five buyers blocked behind one-unit giveaway | Exactly one hold; aggregate/index counters agree and never negative; exact replay stable; no-stock retry fails |
| Release and re-reservation | Cancel and a stale-revision reservation both blocked behind held giveaway | Valid release commits; stale reservation cannot take the held unit; a fresh-revision reservation takes the released unit exactly once; counters return to one active unit |
| Receipt insertion interruption | Claim blocked on the pickup-secret row after admission and receipt insert, backend terminated, PostgreSQL restarted | Uncommitted receipt disappears, admission remains durable, post-restart replay completes exactly one claim |
| BE22/23 terminal race | Claim and cancel blocked behind held giveaway | Exactly one terminal transition; one stock disposition; no retained secret; successful replay stable |
| BE23 positive/negative | Duplicate same-key claims, wrong code, wrong actor | One claim receipt and consumed unit; wrong code/actor rejected; code issuance replay never regenerates plaintext |
| Expiry competition | Expired claim/cancel wait behind held root while sweep skips it | Expired pickup cannot succeed; cancellation/expiry release once; repeat sweep emits nothing |
| Interrupted work | Cancel/claim tasks interrupted before row admission; sweep skips held root | Due reservation remains durable with no partial command or stock change |
| Worker/server restart | Owned postmaster stops/starts; fresh process constructs sweep service | Durable due work drains with batch one; second sweep is empty; expiry secret removed |
| BE34 cooldown/idempotency | Two same-key writes, then three distinct-key writes blocked behind mural row | One operation/revision/limit/outbox; identical result; all later writes hit cooldown |
| Atomic storage failure | Event-specific outbox constraint fails after pixel effects | Pixel/revision/limit/receipt/budget all roll back; identical key succeeds after constraint removal |
| Restart recovery | Fresh Party and Mural processes after actual PostgreSQL restart | Same receipt/operation; no repeated outbox; original cooldown timestamp remains enforced |
| Cleanup | Ledger and terminal scans before/after restart; owned teardown | Full SQLx migration checksums, zero prepared and PENDING receipts, private root removed |

The lock barrier walks `pg_blocking_pids` transitively from the holder PID. A
case proceeds only once every required contender appears in that real wait
graph. A ten-second bounded polling deadline fails the case if the interleaving
is absent. `tokio::join!`, task creation, or a sleep alone is not a race proof.
The expiry worker uses its real `FOR UPDATE SKIP LOCKED` seam, so a held root
must be skipped rather than blocked or falsely counted as drained.

## Execution

1. Obtain and ACCEPT a fresh named Backend SL offer for an existing build target.
   Rust 1.88.0, locked/offline, one job, debug information off, incremental off.
2. `python3 backend/scripts/mcr/qa03_races/qualify.py snapshot --report ABS_SNAPSHOT`
3. In `backend/`, run `cargo test --locked --offline -j 1 -p concert-service --test qa03_races --no-run --message-format=json`, capturing Cargo stdout/stderr outside the target.
4. Run `qualify.py freeze --snapshot ABS_SNAPSHOT --cargo-json ABS_CARGO_JSON --binary ABS_COPY --receipt ABS_RECEIPT` while the accepted lease is held. Freeze/checkpoint then explicitly release the target.
5. After release, run `qualify.py run --binary ABS_COPY --receipt ABS_RECEIPT --pg-bin /opt/homebrew/opt/postgresql@16/bin --report ABS_NEW_REPORT`.

The controller refuses target executables, mismatched binaries/sources, and an
empty or altered test inventory. Each exact ignored test must actually report
one passed/zero failed. It runs ten cases, performs an actual owned PostgreSQL
restart, then runs four cases in fresh processes. Failure reports use new
exclusive files and preserve previous failures; they are never overwritten by
a later success. Cleanup runs on success, failure, timeout, or termination.

## Current validation

- `python3 scripts/mcr/check-plan.py`: exit 0, planning provenance PASS. This
  checks planning only.
- Rust 1.88 `rustfmt --edition 2021 .../qa03_races.rs`: exit 0.
- `python3 -m py_compile .../qa03_races/qualify.py`: exit 0.
- Default `rustfmt --version`: exit 134, broken stable-toolchain dylib; resolved
  by using the existing explicit 1.88 toolchain, no install/change required.
- Controller mismatched-binary refusal: expected exit 1, PASS; no PostgreSQL
  started and no native case counted.
- Gift owner static review found and resolved a fixture timestamp-precision
  issue: deadlines are normalized to microseconds before aggregate creation.
  No additional concrete interface defect was found. Suggested assertions for
  stable claim attempt/admission counts and cleared terminal scan state were
  added. This review ran no build/database and supplies no runtime PASS claim.
- Cargo command in step 3: exit 101 for an evidenced cached protobuf helper
  pointing at the previous Operations worktree. Scoped
  `cargo clean --offline -p contracts-build`: exit 0, removed 20 files/816 KiB.
  Retry: exit 101 for ambiguous isolated test `authorize_in` trait dispatch.
  The one-line owned fix committed at `e62a7025` compiled with exit 0 (4.26 s).
  Both failed attempts and their Cargo JSON/stderr remain preserved.
- The accepted BACKEND lease passed a fresh 9.01 GiB/no-Rust-process/no-target-
  handles preflight. Binary SHA-256
  `2c4aabe0a2d80bf6b48ec6177ce2068d8029c555aa43d02a96ce3a83ebf7cd29`,
  Cargo JSON, and all 1,756 source/contract hashes were frozen outside the target.
  Final `lsof +D` returned exit 1 with no output, followed by unconditional
  release before any PostgreSQL start. No lease is retained for native work.
- Native attempt 1: controller exit 1; PostgreSQL 16.14 initialized, exact test
  inventory verified, zero cases executed. The retained URL guard rejected the
  fixture name `mcr_qa03` because it permits only letters/underscores after
  `mcr_`. Cleanup PASS. The complete observed report is retained in
  `qa03_races_support/evidence/native-01-namespace-refusal.json`. The exact
  namespace refusal was independently reproduced without starting another DB.
  Commit `eb9bb664` changes only QA03's expected/created name to `mcr_qa_races`
  and preserves structured redacted helper failure details; the shared guard
  is unchanged. A fresh named build lease has been requested for this fix.
- Corrected compilation: offer `QA03-NAMESPACE-BACKEND-20261104T0426Z` was
  explicitly ACCEPTED for one executable with a 0.35 GiB expected peak and
  0.6 GiB hard allowance. Preflight observed 9.74 GiB available, no Rust build
  processes, and no target handles. The first attempt exited 101 on the same
  evidenced stale protobuf helper. Only `contracts-build` was cleaned (20
  files/816.3 KiB, exit 0); retry exited 0 in 42.60 seconds. Rust 1.88.0,
  locked/offline, one job, debug information off, and incremental off were used.
  [Build records](../../../backend/services/concert-service/tests/qa03_races_support/evidence/build-attempts.json)
  preserve commands, exit codes, failure diagnostics, and Cargo output hashes.
- The new copied binary SHA-256 is
  `f63ab37801b184a5ee958cacc23bf6980feabd78fe441af1730fa9741211f0ca`.
  All 1,756 source/contract hashes and the successful Cargo JSON hash are bound
  in the [unaltered native report](../../../backend/services/concert-service/tests/qa03_races_support/evidence/native-02-pass.json).
  Final target allocation was 1,220,608 bytes below its baseline; outside frozen
  artifacts/logs occupied 17,866,752 bytes at release. `lsof +D` exited 1 with no
  output, followed by unconditional release before PostgreSQL initialization.
  No target access was performed afterward. These are allocation checkpoints;
  the private PostgreSQL peak was not sampled before teardown, so no measured
  whole-workflow peak is claimed.
- Native re-run on current master (2026-11-06): fourteen exact cases PASS,
each exit 0. The ten pre-restart cases include two new release/re-reservation
and interrupted-receipt cases; four post-restart cases include the new claim
replay. Observed blocking graphs recorded the
required contender counts, including five buyers at stock 1, two party approvals
at the last slot, two release/re-reservation contenders, and one interrupted
claim backend. PostgreSQL postmaster PID changed `63956` → `64110`.
Both SQLx ledgers matched all 27 migration SHA-384 values; prepared
transactions and PENDING receipts were ZERO; cleanup PASS. QA03_STATE rows in
the native report record party, giveaway, reservation, receipt and outbox rows
before and after each asserted case.
- Native attempt 2: controller exit 0; **11/11 exact cases PASS**, each exit 0.
  Nine observed barriers recorded the required contender counts
  `2,2,5,2,2,2,2,2,3` through actual PostgreSQL blocking graphs. Eight cases ran
  before restart; PostgreSQL postmaster PID changed `28721` → `28861`; three
  cases ran afterward in fresh processes, including a newly constructed expiry
  worker. Both SQLx ledgers matched all 27 migration SHA-384 values. Prepared
  transactions and PENDING command receipts were ZERO. Owned cleanup PASS.
  The unaltered report SHA-256 is
  `2b4591f978af05fdf9155411b4227b48305b110bec14b61d35f6eddac78556e8`.
  The interrupted-command seed proves interruption before admission and durable
  pending expiry recovery; it does not claim interruption inside a sweep commit.
- Mutation check: the receipt request digest comparison
  (`Ok(false) => return Err(CommandError::Conflict)`) was weakened to accept the
  changed request in a temporary edit. The suite then failed in
  `party::last_slot_and_identical_replay_are_forced` at
  `qa03_races_support/party.rs:251`: the changed decision returned
  `Ok(SafeReceipt …)` where the invariant requires `Err(Command(Conflict))`.
  Controller exit 1, cleanup PASS. The guard was restored and all fourteen
  cases passed again.
- Canonical owner/public HTTP and same-release integration gates: NOT_RUN;
  Root/feature owners supply those scopes. QA03 has no application/shared-file
  grant and does not mint an OwnerGuardScope from IDs or a fixture allow flag.

The persistent QA03 objective was preserved through the resource block and
resumed after the fresh external offer. Completion requires this committed
handoff directly to Root and resolution of concrete integration findings; no
main-chat or scrum-lead acknowledgment/review gate is introduced. The handoff
contains this exact branch tip, base, changed paths, qualification evidence, and
the remaining integration limits. New production adapter changes require their
own same-candidate qualification rather than inheriting these results.

Delivery copies redact machine-local checkout and rustup prefixes as
`$MCR_WORKTREE_ROOT` and `$RUSTUP_HOME`. The build-attempt receipt retains
its original source blob and SHA256; native results and compiled source pins
are unchanged by this evidence-only redaction.
