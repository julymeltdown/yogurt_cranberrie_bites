# Canonical Auth and Profile owner composition

Status: **IN_PROGRESS**, cohesive composition delivered and runtime verified;
pending parent review and the separate HTTP/native-owner join. Base
`60ca208d23e67b2372f6a27db7e7cecc4ff7cdc6`, branch
`codex/mcr-be-identity-composition`. This is the cohesive BE04 owner-composition
feature, not a public admission or whole-product claim.

The cumulative change is limited to:

- new `backend/services/api-gateway/src/adapters/out/concert_identity.rs`;
- one module export in `backend/services/api-gateway/src/adapters/out/mod.rs`;
- new `backend/services/api-gateway/tests/concert_identity_composition.rs`;
- this document.

No HTTP, app state, client configuration, proto, manifest, lock, platform or
application-crate change is included. The parent owns the later AppState join and
HTTP original-stop capture.

## Composition behavior

`ConcertIdentityReader::new` accepts clones of the existing configured
`GrpcAuthClient` and `GrpcProfileClient`. `read` borrows one sealed
`ConcertAccessContext` and accepts the frozen optional `BindingReference`. It
captures one absolute authority/context stop and runs both configured signed
clients inside that one `call_deadline` scope.

Auth executes first. A missing/failed Auth read, inactive account, ended session,
unexpected binding state, or any Auth refusal returns before Profile is polled.
Absent binding requires exact `NotRequested`, preserving email-only operation
without an X prerequisite. A requested binding requires exact `Bound`; `NotBound`
is not eligible.

Only an eligible Auth result permits the fresh self-Profile read. Both clients
use their existing identity interceptor, actor signer, fixed public error classes
and process-global per-edge limits/metrics. Auth is rechecked after the Profile
await. Profile must be for the strict JWT subject and is read through its existing
exactly bounded minimal projection. No retry, cache, TTL, diagnostics, broad DTO,
suspension policy or event rule is introduced.

`ConcertIdentityObservation<'a>` privately contains the borrowed context and both
borrowed child observations. Every accessor calls the combined `check`, which
rechecks all three. It has no public constructor, Clone, serde, rebind or
portable-grant conversion. Debug is fixed and redacted. Its observation times are
current owner statement observations, not freshness leases or revisions.

## Validation boundary and provenance

Foundation is retained port-rust
`704f42149d5e8e35e1d578676d8ae2dac4f0f9c2`, imported under `backend/` per the
unchanged source map/import manifest. The implementation newly composes the two
accepted owner clients at this base. The test harness adapts their accepted
generated signed transport fixtures and uses the retained strict RS256 context
helper. It uses guarded controlled Auth/Profile owners through configured signed
clients, not native databases or production mTLS.

Planned focused checks use Rust 1.88.0, `--locked --offline -j 2`, disabled dev and
test debug information. They cover fresh email-only current content, exact bound,
Auth false/error no-Profile-dispatch, subject mismatch, Profile outage/content
failure, Auth-held and Profile-held expiry, propagated original stop and both-
ownership expiry. Actual HTTP plus both real owners and PG remain parent/next-gate
obligations.

## Executed evidence

After explicit target transfer, Rust 1.88.0 ran with `--locked --offline -j 2`,
retained target `/Users/lhs/Desktop/mcr-build/backend`, and both dev/test debug
levels zero. Final results:

- `cargo +1.88.0 test -p api-gateway --test concert_identity_composition`:
  **7 passed**, zero failed/ignored/filtered, exit **0**.
- `cargo +1.88.0 test -p api-gateway --test concert_profile_transport`:
  **15 passed**, zero failed/ignored/filtered, plus its isolated capacity child,
  exit **0**. Final metric lines restore 256/256 permits.
- `cargo +1.88.0 test --doc -p api-gateway`: all **14** compile-fail doctests
  pass, zero failed, exit **0**.
- `cargo +1.88.0 check -p api-gateway --lib --bins`: exit **0** for both.
- Planning, exact four-path boundary, Rustfmt, `git diff --check` and manual
  source/import review pass.

The first composition compile exited 101 because the composition called a private
Profile child check. It was replaced with the frozen child's public accessors;
the failed log is retained. Test-only compile runs then exposed scaffold errors
and assertions for duration units and Display text; each is preserved and the
final execution passes. No dependency, manifest, platform or additional path was
changed. Runtime logs and receipt hashes remain outside Git under
`/Users/lhs/Desktop/mcr-build/identity-composition/`.

The controlled owners establish actual signed generated-client sequencing and
privacy, not HTTP original-entry capture, AppState composition, native Auth/
Profile/PG owners, TLS or a public participant flow. Parent alone integrates and
pushes. The goal remains ACTIVE through the committed exact-path handoff and
review; no push is performed by this worker.
