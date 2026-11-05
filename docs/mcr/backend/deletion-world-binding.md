# Serving World handle binding for account deletion

## Objective and custody

BE42-SERVING-WORLD-BINDING-01 supplies the actual serving Concert World handles
to Root's account-deletion composer. The complete assignment requires the typed
binding and opt-in startup publication, exact-object/lifecycle validation and
developer-owned fixes, source/provenance documentation, a committed exact-path
handoff and normal publication through publisher412e. Actual canonical Auth
authority, serving Sessions, Sharing publisher drain, all seven PrivateData
categories and four legacy deletion targets remain requirements of the real
joint producer; source-only construction cannot certify them.

Root1fc explicitly stopped and transferred only additive publication hunks in
`backend/services/concert-service/src/config/world_runtime.rs`, new
`config/deletion_world_binding.rs`, one `config/mod.rs` module export, new
`tests/deletion_world_binding.rs` and this document. Root retains literal main,
the Auth/deletion composers, mTLS mounting, PrivateData factory and all existing
World runtime/admission/rule implementations. BE10/BE11 owner6b5d retains its
separate repository/room/admission scope. No shared writable checkout is used.
The isolated authoring branch is `codex/mcr-be-serving-world-binding` in the
existing World worktree. The earlier QA05 attachment and hook source/receipts
remain frozen; hooks are now published as `b60598fe556084c1e89341c11c49613d2c5ece43`.

The immutable Root source base is
`7d74b7fbf397688f3ae41dd7fa5347d3bcc50aa4`. Its `world_runtime.rs` exactly matches
the authoring preimage, SHA-256
`d81292b9cf237d9dfac877072f346bb898516ed87e079aaa41d03a06a8fb068e`.
Root's module registry SHA-256 is
`a7a8dfeab6639ad6bfbfd1006e02232ae1f84c133dab302288ec59f9d0226396`;
it additionally exports `operations_runtime`. The source packet must preserve
that Root export and add only `deletion_world_binding`. It must not replace the
Root registry with the older authoring checkout's entire file.
The publishing/build packet uses a private Git index rooted at that immutable
Root tree, with only the five assigned postimages staged. The local authoring
commit is retained separately; its older registry is not the publication
postimage. No Root worktree is modified, and a future build must use the coherent
packet tree rather than silently compiling the older authoring checkout.

The persistent goal metadata still describes the unfinished original
BE13/BE14/QA05 assignment and was marked blocked during the capacity wait. This
additional whole BE42 assignment is recorded here without resetting, falsely
completing or silently rewriting that goal. The new five-case QA05 hook source
qualification is delivered; real combined recovery remains open independently.

## Typed interface

`config::deletion_world_binding::ServingWorldBinding` privately retains exactly
`Arc<AdmissionBridge>` and `Arc<WorldSocketState>`. Its constructor
`new(bridge, sockets) -> Result<Self, DeletionFailure>` calls the existing
`SessionOwner::new` validation against those same objects. Identical IDs on a
reconstructed bridge do not make it the state currently serving requests.
`as_serving_world(&self)` returns the existing borrowed
`config::account_deletion_runtime::ServingWorld<'_>` adapter, cloning the same
bridge Arc and borrowing the same state. No runtime, registry or authority is
constructed by the binding. The existing composer converts the borrow into its
owned Sessions owner; the binding must remain alive during that synchronous call.

The opt-in method is
`WorldRuntimeSettings::serve_with_ready_social_and_deletion(self, pool, ports,
ready, transport, local, deletion) -> anyhow::Result<()>`, where `local` is the
existing World invalidation-consumer oneshot sender and `deletion` is
`oneshot::Sender<ServingWorldBinding>`. Every existing serve wrapper supplies
`None` internally and preserves its default behavior.

The actual `serve_inner` creates one bridge, owner/runtime and socket-state Arc.
The router receives a clone of that state. Only after room claims, native owner
readiness, fresh remote-journal replay and `bridge.startup_ready()` succeed does
the optional sender publish a binding for those exact same objects. The existing
final ready sender follows successful handle publication. A closed deletion
receiver fails startup before ready publication and follows the original
supervised remote/native/SQL cleanup path. Root must receive both the typed
binding and the ordinary ready barrier and supervise the original task before
mounting deletion RPC. A received handle alone is neither ongoing health nor
canonical Auth provenance. There is no PID discovery, secondary listener,
parallel World runtime, fabricated positive authority or public fixture API.

Root's literal main consumes the binding with
`LiveDeletionOwners { worlds: Some(vec![binding.as_serving_world()]), ... }`,
using its actual Sharing publisher, existing canonical Auth endpoint and mTLS
configuration. These owner joins and PrivateData activation remain outside this
patch. The literal ABI was sent directly to owners00c,232 and Root1fc; owner232
confirmed the current borrowed-adapter/owned-SessionOwner types are compatible.
This interface coordination is not a king/SL review or approval gate.

## Validation and limits

Status: frozen coherent packet `3bf05e7da3967f0aaec48401f263cc338f8a4efa`
compiled successfully and its five copied-native regressions passed under a
separate finite allocation. Source authoring grants no target, child,
database or seven-service stack allocation. Ordinary source preparation is
bounded to 64 MiB including earlier preparation and new evidence. Frozen native
binaries and prior resource carries are retained independently, never erased or
relabelled as new serving proof.

The successful fourth build used Rust1.88 and the single command:

```sh
cargo test --manifest-path backend/Cargo.toml --locked --offline -j2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --test deletion_world_binding --no-run --message-format=json
```

It exited0 in55.820594s on 2026-11-05. The exact2,305-file materialization
includes the immutable backend, generated domain contract, two bootstrap fixture
JSON files and the reviewed avatar-motion JSON from the same Git revision.
The source-pin manifest SHA256 is
`e80ecbf0046f61a61ec38f6310294b9faeb1e2a788c0dae6ca4e792b9d81c5f4`.
After both resource guards were live, byte-identical input timestamp refresh
recorded unchanged device/inode/mode/uid/gid/size/SHA and before/after mtimes.
No shared-target cleanup or peer checkout modification was performed.

The selected copied executable is18,826,976B, SHA256
`82589eea282b4c8c3de8e0144fbed759b4f9c81b5e4e3bd184d7086ed31f3a07`.
Build/copy evidence remains under the private
`/private/tmp/mcr-world-deletion-binding-20261105/round4` root. TARGET released
at1793837673.012688 and FULL at1793837673.502711; both guards were healthy,
all owned groups were released and no target access remains. Full receipt SHA256
is`2cbcddea4ed22ef1e52578e0892e9ec85e8c888305c85afddbb06e8a377d91fd`.
The conservative combined peaks were7 processes,3,165,749,248B RSS,
171,769,856B NEW and34,609,311,744B WHOLE; minimum available16,345,333,760B.

Earlier failed rounds remain historical failures: round1 omitted the generated
contract; round2 reused a newer peer Auth build script as Cargo `fresh:true`;
round3 exposed the missing reviewed-avatar JSON after the timestamp remedy.
The complete include closure was then materialized from the frozen revision.
The proposed extra enrollment IDL was withdrawn and is absent from the final
read set. No failed round or unrelated serving-main build is credited as success.

Five focused regressions exercise exact-object acceptance, reconstructed-bridge
rejection, shutdown while a binding remains retained, oneshot custody and
released receiver failure. A compile-only function also fixes the literal
cross-task `Send` startup ABI. The fixture uses an unconnected lazy pool and
denies every external owner read; it admits no subject, starts no DB and provides
no positive Auth authority. It uses retained synthetic geometry/avatar helpers
from `tests/world_runtime_support/mod.rs` at the Root base without modifying or
promoting that metadata into product art. These cases do not exercise the sealed
production-port startup or certify canonical deletion effects.

The one copied-native command was `deletion_world_binding --test-threads=1`.
It exited0 with5 passed,0 failed and0 ignored; the test body reported0.01s.
The driver recorded1.617004s including its bounded post-child checks. Both
independent full non-target guards were live before the executable, and all2,305
compiled input files passed exact source-manifest checks before and after it.
The copied artifact's device/inode/mode/uid/gid/size and SHA were pinned. No shared
target was read, scanned or modified; no SQL, subject, Auth authority, network or
listener was created. The empty native World tasks shut down under supervision.

The private native evidence root is
`/private/tmp/mcr-world-deletion-binding-20261105/native1`. Its FULL release was
at1793838023.095310, with both guards healthy, all owned groups released and
cleanup0.567364s. The FULL receipt SHA256 is
`aa351995f5aab6952e4ecc30359761a898afda300e4f62e00ec0ad78add59d95`.
The exact command output SHA256 is
`fbde3917ed74ff3375f124a6691e533b06559eff71ca715a0444d213ac9378d1`.
Both samplers recorded peaks4 processes,162,217,984B RSS,28,672B NEW and
34,609,340,416B WHOLE, with minimum available15,827,832,832B. FULL was sent
directly to the allocator and waiting Gift owner before documentation.
This qualification covers only the frozen binding implementation and its five
object/native-task regressions; BE39 termination source is excluded.

Real post-startup handle publication, receiver-drop startup cleanup and the
joint canonical Auth-to-Concert deletion flow remain NOT_RUN until the actual
Root/00c producer runs under a fresh finite allocation. No second seven-service
stack will run alongside IN04. No old QA05 39/5 matrix is rerun for acknowledgment.
