# Gateway to Concert bootstrap admission

Date: 2026-11-03. Internal implementation contract for BE04/BE06. Public C03
remains unchanged. This introduces a generated RPC and its guard, not a running
product route or complete identity/revocation acceptance.

`concert.v1.ConcertService/Bootstrap` receives the exact message in
`backend/proto/concert/v1/concert.proto`. Gateway alone constructs it after its
existing uncached `ConcertIdentityReader` verifies the canonical Auth account,
live session and allowlisted self Profile. Email-only entry requests no provider
binding. No Auth role becomes an event/operator role.

The existing workload credential and Ed25519 actor-delegation guard bind the
whole protobuf body, actor, Concert audience and RPC method. The body additionally
binds the event, canonical session family, correlation UUID and exact public GET
target with no query/body. Public Profile content is limited to display name and
optional bio. Tokens, emails, provider subjects and private Profile fields are
absent. The response carries frozen C03 Bootstrap JSON bytes rather than a second
public schema.

`concert_service::rpc::authorize_bootstrap` returns a sealed `VerifiedBootstrap`
only after verified Gateway, actor and deadline extensions exist and all payload
bindings pass. `BOOTSTRAP_BUDGET` is a two-second processing/observation-age
ceiling, not a revocation SLO. Gateway must bound assertion expiry by the minimum
of original request deadline, browser JWT expiry, both owner observations plus
that ceiling, and remaining monotonic budget. Receiver also preserves the
original received deadline and actual signature expiry; later awaits cannot
restart a budget. The signer attests observed owner facts; neither service claims
instantaneous global revocation after an observation.

The BE06 producer consumes `VerifiedBootstrap`, calls `check()` after awaits and
before committing/responding, and performs current event-phase/restriction checks
under its database transaction locks. The RPC policy is Gateway-only and denies
unlisted methods. Replaying an identical bounded bootstrap across replicas must
remain harmless through unique participant/default keys; a captured assertion
cannot authorize another event, actor, method, body or request target.

## Producer handoff

The worker owns `concert-application/src/bootstrap/**`, application bootstrap
tests, `concert-service/src/bootstrap/**`, service bootstrap tests and support,
new migration `0004_bootstrap_admission.sql`, the exact bootstrap native harness,
and `docs/mcr/backend/bootstrap.md`. One `pub mod bootstrap` export in each
owning crate is explicitly allowed. Shared RPC/proto/platform, manifests/lockfile,
Gateway routing/client, service main/config and build scripts remain integrator
owned. Existing BE08 place paths and migration 0003 are not delegated.

The production constructor should expose a `BootstrapService` implementing the
generated trait, composed from a PostgreSQL pool and an explicitly validated
immutable release bundle. Missing/unreviewed product assets must fail closed;
synthetic metadata is permitted only in explicit isolated test configuration.
Never seed fake production people, event eligibility, approved art, admin roles,
provider availability or feature capabilities. Bootstrap must use the current
event plus reviewed asset/default configuration; optional unavailable features
report accurate C03 capability reasons. Participant/avatar creation is atomic,
idempotent and concurrent-safe; suspended event membership cannot be revived by
another bootstrap. READ_ONLY reads must not overwrite existing preferences.

Required producer proof includes concurrent repeated creation, rollback on bad
release/authority, current restriction/phase changes, cross-event separation,
restart persistence, expired authority after a delayed await, generated guarded
RPC and a schema-valid C03 result. This does not by itself qualify a public
Gateway route, real multiuser world, production art or complete IN01.

## Shared prerequisite verification

Retained Rust 1.88.0, locked/offline, two jobs and debug info disabled:

- `cargo test -p concert-service --lib rpc::tests`: one policy test PASS.
- `cargo test -p concert-service --test bootstrap_authority`: three tests PASS,
  covering one successful generated RPC and sixteen rejection cases through the
  actual workload/signature guard and authority parser. Invalid credentials,
  sibling caller, absent signature, wrong audience/actor/body, absent original
  deadline, invalid target/profile/UUID and expired/future/stale observations do
  not reach the controlled producer.

These checks use a generated in-process gRPC service and controlled producer;
no database, actual owner reads or deployed product endpoint are claimed here.
