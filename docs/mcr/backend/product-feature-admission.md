# CF03 Product admission for Intro and Notification

Status: `CONTRACT_READY` (guarded Product admission qualification passed).
This is transport admission only; canonical owner policy,
native feature execution, Product listener activation and public HTTP delivery
are outside this patch and remain `NOT_RUN` here.

Base: `29a42ca1f5d86262ee18141d2bab75996af7e6fd` on
`codex/mcr-be-CF03-product-admission`. The explicitly assigned three existing
Product files are the only production files changed. Root retains all other
composition/configuration/producer paths. The former notification worktree is
stopped and read only.

## Behavior and provenance

`product_rpc::Operation::Intro(product_rpc::IntroOperation)` represents exactly
List, Get(UUID), Own, Set, Publish and Close. Its public `id()` returns the six
frozen C04 IDs, and `mutation()` identifies the three commands.
`Operation::Notification(notification_live::NotificationOperation)` reuses the
existing public type for List, UnreadCount, MarkRead(UUID) and MarkAllRead.
The Notification `protocol` module remains private; callers use its public
reexport.

The source of truth is the frozen `contracts/domain/v1/operations.json`,
`social.schema.json` and `common.schema.json` at the base above, plus the
read-only Intro and Notification parsers at that revision. Admission calls the
actual `intro_live::protocol::{parse_intro_target,parse_query,parse_intro_command}`
and `notification_live::NotificationRequest::parse`. No replacement command
DTO, authority constructor, dependency or generated artifact was introduced.

Intro adds only missing wire-shape checks at Product admission: JSON top-level
and age bounds must be objects, UTC expiry strings must end in `Z`, and decoded
query controls/length are rejected before the existing parser trims `q`.
Both feature families reject a bare trailing query separator; the underlying
Notification parser otherwise treats it as an absent query.
The existing Product form decoder provides lexical validation; the original
query is independently passed to the feature parser without recursively
decoding it or replacing signed bytes. Command bytes go to the real typed
parser unchanged, preserving duplicate-field rejection.

Intro PUT/POST and Notification PATCH follow their exact keyed command parser;
they cannot fall through to the generic read-body rule. Existing Feed, Social,
party, media and command-status routing/body behavior is retained. OwnerMedia
explicitly refuses both new feature families. Original Gateway signature,
actor/event/session/request binding, parent authority, monotonic deadline,
observation ceilings, four-slot capacity, representation and canonical delivery
requirements remain in the same Product RPC path.

## Validation design

`tests/product_feature_admission.rs` drives the actual generated
`ProductServiceServer<ProductRpcService>` inside the real workload/signature
guard. The outer `CallDeadlineLayer` captures the caller's original deadline
before verification. The recorder checks exact parent protobuf/frame, actor,
event, session, request, operation, task-local parent/deadline and expiry ceiling.
It returns an intentional refusal after recording; no successful canonical
owner or delivery capability is fabricated.

Cases cover all ten frozen operation mappings, valid wire boundaries, closed
queries and commands, unknown/duplicate/encoded keys, JSON sequence shapes,
nullable fields, revision bounds, read payload refusal, signed identity/event/
method/path/deadline changes, post-signature tampering, four-slot capacity,
producer cancellation on original timeout and representation/delivery refusal.
Thirty existing routes (17 original and 13 Social) have regression assertions.

One ignored test uses the same actual guarded Product service on an ephemeral
loopback mTLS listener. Fresh owned Gateway and Concert leaves admit all ten
operations; a trusted Friend leaf cannot claim Gateway identity. This uses no
PostgreSQL or canonical peer processes. Cargo automatically emits the ordinary
Concert package binary under the exact integration-test build command; that
artifact is accounted for in Cargo JSON and disk growth and is never copied or
executed by this qualification. No explicit main/peer binary build is requested.
The fixture composition
is adapted from platform actor-delegation tests and the existing Notification
native fixture at the base above; no existing helper was modified.

The new smoke runner accepts only a SHA-256-matching copied artifact outside a
`target` directory. It creates and removes its own inode/token-bound private PKI,
runs ordinary guarded tests and the exact ignored mTLS case, bounds runtime,
free space and output growth, and writes per-phase exit codes. It invokes no
Cargo, PostgreSQL or shared target. Shared-target build access requires a fresh
named finite resource allocation; no previous allocation is reused.

## Qualified result (2026-11-04)

Application source pin: `1814ad37258f9cbcb1ab5ba2955cd53fab97819d`.
The only later handoff change is this evidence document. The worktree was clean
at compilation; all nine changed paths were checked against the exact assignment.
No contract/proto, manifest, lockfile, infrastructure, FE, main/config/producer,
or `docs/source/` path changed.

| Check | Result | Exit |
| --- | --- | --- |
| `python3 scripts/mcr/check-plan.py` | PASS; planning/provenance only | 0 |
| Rust 1.88 formatting of new Rust files and test helper; runner syntax; `git diff --check`; exact-path/import scan | PASS | 0 |
| Locked/offline integration artifact compile/freeze | PASS; 6.901 seconds | 0 |
| Actual signed guarded RPC suite | 10 passed, 0 failed; mTLS case initially ignored; 0.63 seconds | 0 |
| Exact ignored mTLS case, same frozen artifact/qualification | 1 passed, 0 failed; 0.03 seconds | 0 |
| Fresh PKI creation, both test phases, cleanup in one qualification | PASS; 2.134 seconds | 0 |

Build command, from `backend/`, with jobs 2, dev/test debug 0 and incremental 0:

```sh
cargo +1.88.0 test --locked --offline -p concert-service \
  --test product_feature_admission --no-run --message-format=json \
  --config profile.dev.package.contracts-build.opt-level=2
```

After the explicit zero-handle target release, the SHA-256-matching copied
artifact was executed as follows by the pinned runner (no Cargo/target access):

```sh
"$ARTIFACT" --test-threads=1 --nocapture
"$ARTIFACT" signed_gateway_mtls_admits_all_ten_and_refuses_wrong_certificate_role \
  --exact --ignored --nocapture --test-threads=1
```

Final evidence is retained in the private `mcr-be33-cf03-1814ad37` qualification
directory. Artifact SHA-256:
`dfecc2426b4d0648eacc334dddeb601228f39fa581c93ba20ded978a3acb780a`
(19,243,408 bytes). The full tracked backend/contracts source archive SHA-256 is
`e8fc9c83d20d2d081512185bfaee4490e6fc738fc51abdc527f9dfef7f14fcf4`.
The unchanged frozen operation registry SHA-256 is
`f801d703dfc5509bbe06a2f0fa07a170b6442a48b2204c7ead460fc9a3600112`.
Every tracked backend/contracts input hash, exact Cargo artifact event,
stdout/stderr, qualifier source and runtime pin is retained with that archive.

| Receipt | SHA-256 |
| --- | --- |
| `artifact-receipt.json` | `68a80e38ed143424883a65130eeb6167d958d78274c32ada53b740e043f38405` |
| `target-release.json` | `bdfb87d7111f5275722d41e586f82a25422633bdf5e40d7c8e228d8cbeb83d05` |
| `runtime-pin.json` | `09de4ffd4115ffcd4b1e2ecea82537d6d09fc9443b5a849f0688076cacef1fdb` |
| `qualification/report.json` | `7c3bf5acf63ca39e8d9800a5db2edfd65650b322be63ab43e6c8556a833829b3` |
| `full-release.json` | `5c07ff98161ce02f18fc4620b6511748bc8845db7e5adb6b8af91855288e3c37` |

Resource round `CF03-PRODUCT-ADMISSION-ROUND2-MEDIA-202611040957Z` was fully
released at 09:59:45 UTC. Whole measured growth including all 390,156,288 bytes
of retained carry was 416,411,648 bytes, below 1 GiB. Minimum observed free space
was 19,831,848,960 bytes, above the 8 GiB continuous floor. Build target release
at 09:59:06 UTC had `lsof` exit 1 with empty output and no Cargo process group;
copied tests ran only afterward. There was no later target access. The private
PKI was removed, the real listener joined, and anchored process lookup returned
exit 1/empty output. No PostgreSQL or peer-service process was started.

The previous round remains preserved in `mcr-be33-cf03-39d1de66` rather than
being represented as a pass: its first compile exposed a test response-body
annotation error; the later full guarded run had seven passes and three helper
failures caused by comparing raw percent-encoded `grpc-message`. Its separate
same-artifact mTLS diagnostic passed. The final source uses Tonic's actual status
decoder and passed both phases together in the fresh round above. A static
query fix also distinguishes an absent query value from a valid literal `q=?`.

Committed implementation ancestry from the assigned base, in serial order:
`39d1de66ff399ecd4a9b0dc6bcc70e3de6198c83`,
`269a1d37741414efb892fe488e43aa3ab1fcdb95`,
`898e0eb417b978a3c043303391ac7a0eb468e132`,
`029b37b42cbac9ec21060986a621d47063655f12`, and
`1814ad37258f9cbcb1ab5ba2955cd53fab97819d`.
The separate documentation successor records this handoff; the designated Root
publisher owns serial intake and publication, followed by developer remote
readback. No main-chat, scrum-lead or parent review/ACK is an intake gate.

A test recorder is not native feature execution, positive target-policy
authority, Product listener activation, root/public HTTP acceptance or full
IN05 acceptance. Those remain `NOT_RUN` in this leaf. The aligned BE33 persistent
goal retains its historical `BLOCKED` metadata; this enabling patch does not
reset or falsely complete that broader goal.
