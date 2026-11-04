# BE33 target callback startup composition

Status: `CONTRACT_READY`. All five new configuration/composition checks passed
under the finite offer below. The committed handoff awaits normal publication. This is original BE33 composition,
not a new required task or full BE33/IN03 acceptance.

## Source and ownership

Base: `058e516db6042bf21f618982219b4d5540ed0075` from normal `origin/master`
intake, in the existing isolated `be-product-feature-admission` worktree on
`codex/mcr-be-BE33-target-startup`. No new worktree, dependencies or lockfile
changes are introduced. `docs/source/` remains byte-identical.

Qualified owner 207 dependencies were imported normally and remain unchanged:

| Owner commit | Local intake |
| --- | --- |
| `4394b409` | `9350499e` |
| `59ced660` | `ffd44712` |
| `ef0f0785` | `341a32e1` |
| `8e2a42a8` | `5e4ee1b5` |

The earlier `50a50033` wire/client dependency was already published as
`55a52ce2` and was not imported twice. All sixteen provider postimages remain
owned by 207. Their existing 4 pure plus 1 real PG/mTLS qualification and this
writer's previous 11 delivery/startup checks are historical qualified boundaries,
not rerun or promoted to this composition's actual-main proof.

This writer owns only the narrow canonical settler wiring, new settings/helper,
one module export, new test file and this document. Root retains actual main,
listeners, supervisor, feature dispatch and outbox integration. The latest human
policy applies: developers own implementation validation and fixes; normal
publication has no king, SL or QA review/acknowledgment gate. The aligned original
BE33 goal remains unfinished; its historical `BLOCKED` API metadata is preserved
honestly and is neither reset nor falsely completed.

## API and configuration

```rust
use concert_service::config::notification_target_runtime::{
    NotificationTargetRuntimeSettings, POLICY,
};

// Root calls this BEFORE connect_pool, using existing validated settings and
// the actual TLS material for its existing Concert gRPC listener.
let target_settings = NotificationTargetRuntimeSettings::load(
    inbox_settings.enabled(),
    intro_settings.is_some(),
    runtime.as_ref().and_then(|r| r.tls.as_ref()),
)?;

// After existing compose_shared: use that SAME pool and canonical core.
let target = target_settings.map(|settings| {
    settings.compose(pool.clone(), &shared.owners.core)
});
// Root mounts target.service.server() behind guard_without_delegation(
// &POLICY, ..., target.callers) on its EXISTING actual mTLS listener.
```

`CONCERT_NOTIFICATION_TARGET_MODE` accepts only `off` (default) or `live`.
Both target and inbox off return no callback settings and inspect no global
credentials. Global `NOTIFICATION_SERVICE_CREDENTIAL` and `_PREVIOUS` may belong
to another independent feature; their presence alone never invalidates off mode.
There are no new feature-specific credential inputs.

An enabled inbox with target mode off fails before database acquisition. Target
live requires the enabled inbox, shared live Intro/Participant path and actual
Concert TLS material. The helper parses certificate/key/CA using the same tonic
server TLS builder before database/listener acquisition; a merely present but
malformed PEM object is rejected without binding a socket.

Live serving requires explicit `NOTIFICATION_SERVICE_CREDENTIAL` and supports
optional `NOTIFICATION_SERVICE_CREDENTIAL_PREVIOUS`. Existing platform config
resolvers reject missing/blank/shared placeholder credentials. ASCII metadata
validation rejects malformed tokens. Current and previous must differ and cannot
reuse any configured current/previous credential of another workload. Only the
dedicated Notification caller is registered; inherited Participant/Gateway
registries are neither replaced nor expanded.

The pure `from_values(read, inbox_enabled, shared_intro, tls)` constructor has the
same checks as `load` without mutating process environment.
`compose(self, pool, &CanonicalRuntimeCore)` returns `NotificationTargetRuntime`
with public `service` and `callers`, and reexports the provider's original `POLICY`.
It passes the supplied same pool and `core.registry()` to the qualified provider.
The mounting caller must pass its existing canonical pool; this API does not
discover, construct or replace a core, fence or registry, and starts no task.

## Canonical recovery

`CanonicalOwnerComposition::compose` now constructs the actual
`TargetReadSettler::new(pool.clone(), config.instance)?` before the existing core
and adds `.with_target(...)` to the existing closed child-settlement composite.
The one existing core, public Profile reader, owner fence, instance, pool,
Notification binding reader, retained keys and other child adapters are retained.

The typed target settler is installed in shared composition independently of
callback activation so original pending target children remain recoverable.
Nothing changes the provider's durable linked-inbox dependency, native identity,
0050 migration, Root-finished requirement or release order. No successful fallback
or duplicate native/recovery implementation is introduced.

## Validation ledger

| Check | State | Limit |
| --- | --- | --- |
| Rust 1.88 explicit-path formatting | PASS | No recursive peer-source formatting |
| `git diff --check` | PASS | No application/runtime acceptance implied |
| New settings/composition cases (5) | PASS | Exact copied executable; five passed, zero failed/ignored |
| Existing 207 native and previous Notification 11 cases | NOT_RUN in this stage | Unchanged implementation; no unrelated repetition |
| Actual Root main/mTLS listener activation | NOT_RUN | Root owns and validates the actual mount |
| Real positive nonempty target policy / public IN03 | NOT_RUN | Required original product boundary remains open |

New tests reuse the existing CF01 private-PKI ownership guard read-only. Settings
checks cover off/global-credential independence, inbox/target dependencies, actual
TLS parsing, explicit mode/credential rotation and cross-workload separation.
The composition check uses a closed lazy SQLx pool and a negative-only child
adapter: it verifies registry isolation, original core/instance retention, actual
provider construction and refusal of missing Root scope. This grants no owner or
target authority and cannot connect to an ambient database.

The planned exact build is Rust 1.88 `cargo test --locked --offline -p
concert-service --test notification_target_runtime --no-run --message-format=json`,
two jobs, debug/incremental disabled and only offered target-specific build flags.
The emitted test must be copied/hash-pinned outside the selected target and the
target released unconditionally before any copied test or private PKI process.
No PG, application service, listener or old native test is needed for this scope.
Offer, exact commands/exit codes, source/runtime pins, evidence, release accounting
and publication ancestry will be appended after actual execution.


## Completed bounded qualification

Implementation source: `f73f84b555e73f1c005144c8ceab9d2a67fac013` (clean),
exactly the five assigned paths. No provider source changed. Offer
`BE33-TARGET-STARTUP-BACKEND-202611041151Z` was accepted at
2026-11-04 11:54:40 UTC, with absolute compile/freeze 12:04:40, copied-test
12:06:40 and full cleanup 12:09:40 cutoffs. The target was the existing
`/Users/lhs/Desktop/mcr-build/backend`; no new target or worktree was created.

Exact build (exit 0):

```sh
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend \
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline \
  -p concert-service --test notification_target_runtime --no-run \
  --message-format=json --config profile.dev.package.contracts-build.opt-level=2
```

Both `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` were unset. Compiler:
`rustc 1.88.0 (6b00bc388 2025-06-23)`. Cargo reported 45.19 seconds; the
supervised archive/build/freeze stage took 49.31 seconds. No failure, source
fix, retry, timestamp repair or cache deletion was needed. Cargo's automatic
main artifact was recorded as metadata only; it was not copied or run.

One 18,325,696-byte test executable was copied outside the target with SHA-256
`e136f4bcee4e5c22ac4e90182f16fd82a65bf4a1a0626f83d8ae1610e696ad0b`.
Full tracked backend/contracts source (29,673,166 bytes), source hashes, contract
and fixture inputs, runtime executable hashes, linked-library inventory, Cargo
JSON and stderr were frozen in `/private/tmp/mcr-be33-target-f73f84b5/`.
Source archive SHA-256:
`14ac3ead429c238e6479d6db2e63f540aba8e492e7d70721cf278557e2fbe23b`.
Artifact receipt SHA-256:
`c0ec9371e06bf252c1f7d6e2cdde2b6e986f75b156faf723ca28083b1bc063c1`.

At 11:57:25 UTC a fresh target `lsof +D` returned exit 1 with empty output:
zero target handles. The target was unconditionally released before any PKI
or copied test process and never accessed afterward. Positive target allocation
growth was 27,504,640 bytes, retained and charged.

The existing owned PKI helper generated an isolated private Concert certificate
fixture. No database initialization/start, application service or listener ran.
The copied command exited 0:

```sh
/private/tmp/mcr-be33-target-f73f84b5/notification_target_runtime \
  --test-threads=1 --nocapture
```

All five named cases passed; zero failed, ignored or filtered. The test body
reported 0.01 seconds; private PKI plus copied qualification took 3.81 seconds.
The original CF01 root/inode/token guard was used. Source, runtime and copied
binary hashes matched their frozen receipts. Qualification receipt SHA-256:
`bbc9f351d1487830e5c8779b20f3a65e31cfa7347ab6ffa706a1299e5ad261ae`.

Full release completed at 11:58:08 UTC, before every cutoff. Owned PKI/TMP was
removed; fresh private `lsof` proved zero copied handles and no owned process
remained. Process peaks: compile 5, rustc 2, copied/PKI 1, within 12/2/8 caps.
Conservative retained carry was 708,837,376 bytes (covering prior 675,188,736,
new proof and source growth); retained target growth 27,504,640 and private
output 25,227,264 gave a 761,569,280-byte release snapshot. Whole peak including
temporary PKI was 761,647,104 bytes, below the 3 GiB cap; minimum available
space was 19,395,743,744 bytes, above the continuous 8 GiB floor. Subsequent
small documentation/proof growth remains charged to this retained carry.
Full-release receipt SHA-256:
`1ff4bbb185ee71d243929248eed05e53d9fcf546601f7e909370c32d4de737ec`.

These are settings and negative-only composition results. Actual Root main
mounting and real positive nonempty inbox/target policy remain separate required
implementation-owner boundaries; this does not certify full BE33 or public IN03.
