# CF03 Product admission for Intro and Notification

Status: `IN_PROGRESS`. This is transport admission only; canonical owner policy,
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
PostgreSQL, canonical peers or main executable builds. The fixture composition
is adapted from platform actor-delegation tests and the existing Notification
native fixture at the base above; no existing helper was modified.

The new smoke runner accepts only a SHA-256-matching copied artifact outside a
`target` directory. It creates and removes its own inode/token-bound private PKI,
runs ordinary guarded tests and the exact ignored mTLS case, bounds runtime,
free space and output growth, and writes per-phase exit codes. It invokes no
Cargo, PostgreSQL or shared target. Shared-target build access requires a fresh
named finite resource allocation; no previous allocation is reused.

Validation results, emitted artifact identity and committed handoff ancestry
will be appended after the allocated round. A test recorder is not native
feature execution, positive target-policy authority, root/public HTTP acceptance,
or full IN05 acceptance.
