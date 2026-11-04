# C03 Gateway implementation

Status: CONTRACT_READY / IN_PROGRESS. Fresh Gateway build and controlled transport
checks pass at8888ff7a. Actual native owner join passes at3f763d4a with unchanged
producer inputs. Production startup mounting and public C03 save/reload remain
NOT_RUN; these component checkpoints do not certify that flow.

Root transferred these exact paths to Participant: new outbound/inbound
`concert_participant/` modules; `tests/concert_participant.rs` and its support;
this document; one additive isolated Participant/Decision proto pass in Gateway
`build.rs`; and one module export in each parent module. Root retains AppState,
configuration, manifests and root/router/startup composition. Backend only.
Base is qualified Participant handoff88de486f.

Construction:

- `GrpcConcertParticipantClient::connect(endpoint, token, tls, signer, release_digest)`
  validates a trusted configured release digest. It never reads release or actor
  identity from browser JSON or headers.
- `concert_participant::router(Arc<AppState>, Arc<GrpcConcertParticipantClient>)`
  returns the three C03 routes using its own typed state. Sealed HttpConcertAccess
  and actual configured Auth/Profile identity clients remain mandatory.
- Root mounts `protect_response` outside transport rejection layers and retains
  the actual outer RetainedScopeLayer to finish only terminal HTTP framing.

GET/PATCH `/api/concert/events/:event_id/me` and PUT `.../me/avatar` retain original
request bytes and authenticated identity/session. Browser mutations require an
allowed single Origin, browser intent and JSON content type, with an 8KiB body
limit. No idempotency key is invented for these optimistic-CAS operations.
Canonical owner reads and signed RPC dispatch share the original wall/monotonic
stop. C03 success disclosure checks exact object keys, nested public fields,
actor/event identity, revisions, preferences and the frozen avatar variants.
Unknown private fields or inconsistent failure code/reason pairs fail closed.

Success and closed typed errors retain the same internal completion token. The
capability never enters browser JSON/headers. Dropping/erroring a body does not
finish a scope. The actual response-wide disclosure ceiling guards each DATA frame, including
a closed typed error. Public activation still requires Root's actual production
composition and public-route validation.

The focused test target uses actual sealed JWT extraction and actual generated
signed client/guard transport with explicitly controlled Auth/Profile/Participant
peers. It covers forwarding, closed conflicts, EOF versus drop, private-response
refusal and pre-dispatch browser/auth/body refusal. It cannot certify a real
canonical-owner/public-stack flow. The reusable controlled identity fixture is
copied from `tests/concert_identity_composition.rs` at88de486f, without its test
cases or synthetic context constructor; production never imports it.

Root granted the exact response-wide disclosure extension after the draft:
ParticipantResponse field4 carries the actual scope ceiling, captured after
success or typed failure with the original scope Arc retained. Gateway refuses
absent, expired or extended ceilings and wraps both forms of response with the
shared DisclosureBody. The original C04 implementation and its three tests move
unchanged except crate visibility to `http/response_disclosure.rs`; the former
private C04 path is a narrow re-export shim. Party-owned C04 route code is
unchanged. Source provenance: Root published07791018 (which contains4d5c044's
qualified body guard and existing http-body workspace dependency). No new
manifest or lockfile change is required.

The new tests additionally cover actual C03 data withheld after an earlier
response ceiling expires and fresh EOF settlement after already consumed valid
data. The same original wall/monotonic bounds are retained. Production fields
are never copied into browser headers or JSON. The shared body guard is qualified by three focused checks and two additional
C03 DATA/EOF boundary checks. Gateway and Participant compile/native evidence
cover this additive internal-wire revision. Phase A/test response initializers
explicitly use zero, which the real Gateway refuses for public disclosure.

First qualification at3f763d4a: both exact Gateway and Concert no-run builds pass;
shared disclosure tests pass3, Participant focused8/integration8 pass, and the
actual three-main native join passes with the new field4 ceiling assertions,
COMMIT/save/reload/ABORT/owner-delivery/cleanup. The Gateway transport target
aborts on its first default-stack test. A diagnostic-only16MiB-stack replay of
that one case passes, which is not acceptance. The follow-up boxes large
identity/RPC/dispatch futures (as existing Gateway clients do) and test setup
futures. Fresh default-stack Gateway validation at8888ff7a passes all seven tests in
2.87s and the shared three tests; no production stack-size workaround is
introduced. The original compiled3f producer inputs are unchanged by this
Gateway-only correction (all206 recorded source pins match).


Qualification evidence is committed under `participant-live-evidence/`:
`gateway-binaries-8888ff7a.json` contains sanitized original Cargo artifact
metadata, executable hashes and source pins; `gateway-disclosure-8888ff7a.txt`
and `gateway-participant-8888ff7a.txt` contain the default-stack outputs (trailing blank lines normalized).
`gateway-native-join-3f763d4a.json` records actual Auth/Friend/Profile MAIN
migrations, signed canonical join, native COMMIT/save/reload/expired-PREPARE ABORT,
six root/owner completions, lost-envelope quarantine and all clean process exits.
The producer focused/integration outputs record eight passes each and explicit
ignored native cases; the separate actual native join supplies the positive
multi-process evidence. Private original compiler streams and unmodified
receipts remain outside the repository. No live build-target lease is held.

Reproduction (Rust1.88.0, locked/offline, jobs2, incremental0, dev/testdebug0;
use only a coordinator-allocated existing target):

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 -p api-gateway --lib --test concert_participant --no-run --message-format=json
cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 -p concert-service --lib --test participant_producer --no-run --message-format=json
```

Both compile commands exited0. Gateway8888ff7a completed in29.17s with zero
compiler warnings; producer3f763d4a completed in39.57s. Copy the exact Cargo-emitted
executables outside the target and release the target before execution. Run the
Gateway lib copy with `response_disclosure::tests --test-threads=1` and the
participant integration copy with `--test-threads=1`; both exited0, using the
default Rust thread stack. The producer lib copy uses `participant::live::` and
its integration copy uses `--test-threads=1`; both exited0. The native driver
`backend/scripts/mcr/participant_live_join_smoke.py` takes the producer lib copy,
its original receipt, owner/Profile artifact manifests, PostgreSQL16 binaries and
an output report path. Run it against the exact3f source checkout with an
explicit isolated-PG allocation; it exited0. No broader release or browser
acceptance follows from these commands.

Remaining owner: Root composes actual Participant client/router,
`protect_response` outside transport rejection layers, and retained EOF scope
handling. Actual canonical Auth/Profile/Decision clients, supervised recovery,
release configuration and mTLS must be supplied by production startup. Validate
public signed-in GET, avatar PUT, preference/full-profile PATCH, reload, stale
revision denial, session/identity refusal and actual EOF owner settlement before
marking the assigned persistent live goal complete. No AppState, root mount,
configuration, manifest, lockfile, frontend or Party C04 route modification is
included in this handoff.

## Root composition checkpoint

The integrator consumed ordered owner commits `c05abd5d`, `3f763d4a` and
`8888ff7a` as `96e6c513`, `065717b1` and `f2683a7c`. The final six sanitized
receipts and this owner's final documentation came from minimal handoff
`9c819233`; its single equivalent source patch was not applied a second time.

The additive root configuration uses `GATEWAY_CONCERT_RELEASE_DIGEST` with the
existing Concert endpoint, service credential, TLS and Gateway signer. The
digest must be exactly 64 lowercase hex characters, nonzero and operator
configured; no browser value selects it. The root mounts the actual client and
places Participant response protection outside transport errors. Without a
configured digest the new Participant client/routes are not activated. Existing
configuration fixtures default this opt-in to absent.

These root configuration/router changes and their new malformed-input test are
source only, `NOT_COMPILED` / `NOT_RUN` until the next named integrated build.
The owner's qualified component receipts do not certify this newer root
composition or the actual public save/reload path. Concert's actual factory,
Decision verifier, native resolver and supervised recovery are still required.
