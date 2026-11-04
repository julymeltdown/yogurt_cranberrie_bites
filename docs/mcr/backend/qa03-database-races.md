# QA03 database race qualification

Status: IN_PROGRESS. Runtime qualification is NOT_RUN until a source-bound copied
binary completes the owned PostgreSQL controller below. No mock/unit result is
counted as database acceptance.

The original QA03 row in `MCR_IMPLEMENTATION_PLAN_V2_EN.md` requires parallel
capacity, reservation, cancellation, expiry, claim, idempotency, and mural cooldown
scenarios against PostgreSQL, with forced transaction interleavings and worker
restarts. Its dependencies are BE17, BE22, BE23, and BE34. The invariants in §§7.4,
8.2 and 8.4 and those dependency rows remain in scope; no criteria were removed.

## Ownership and provenance

- Checkout: `/Users/lhs/Desktop/mcr-worktrees/qa-database-races`.
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
  source revision and delivered ancestry will be recorded after execution.

Feature owner inputs: Party confirms source `7d3e21a3` (runtime code `c7d8eff6`),
with fresh native validation pending after fixes; Wall confirms user source
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
one passed/zero failed. It runs eight cases, performs an actual owned PostgreSQL
restart, then runs three cases in fresh processes. Failure reports use new
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
- Cargo/native PostgreSQL: NOT_RUN; fresh build slot requested from Backend SL.
- Canonical owner/public HTTP and same-release integration gates: NOT_RUN;
  Root/feature owners supply those scopes. QA03 has no application/shared-file
  grant and does not mint an OwnerGuardScope from IDs or a fixture allow flag.

The persistent QA03 goal remains ACTIVE through implementation, native
qualification, committed handoff, and concrete fixes. Delivery goes directly to
Root; no main-chat or scrum-lead acknowledgment/review gate is introduced.
