# BE33 target callback startup composition

Status: `IN_PROGRESS`. New configuration/composition checks are `NOT_RUN` until
a fresh bounded resource offer is accepted. This is original BE33 composition,
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
| New settings/composition cases (5) | NOT_RUN | Fresh finite offer required |
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
