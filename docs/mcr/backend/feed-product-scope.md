# Scoped Feed product producer handoff

This increment adds the read-only `ScopedProductProducer` adapter for the existing
qualified Feed/Post read composition. It does not activate a production mode or
register a service. Production IN03 main/Gateway HTTP acceptance remains open.

## Source and exact scope

- Existing worktree: `/Users/lhs/Desktop/mcr-worktrees/be-world-recipient-producer`.
- Branch: `codex/mcr-feed-product-scope`.
- Base: `03c54fa379e5b468ea5634311714c057408e5bb5` (runtime source `db25c4fa7db65f5f20b53ee995326b00aec029d3`).
- Root's inspected source: `145a8e939fb7bede7e46931d6559adb2c51f62a7`.
- Common ancestor: `047d892cc6ad5eec6125322b84cf1efc2380078f`.
- Exact new lease: `src/feed/product.rs`, export-only `src/feed/mod.rs`,
  `tests/feed_product_scope.rs`, and this document under Concert service/backend docs.
- No config, main, proto, contract, manifest, lockfile, asset or original-source edits.

The goal API still reports the incompatible original BE13/BE14 World objective as
**BLOCKED**. It was not completed, reset, or represented as active. The explicit
Feed assignment authorizes this increment independently of that stale metadata.

## Constructor and Root integration

`FeedProductConfig::new(post_endpoint, concert_credential, &cursor_key)` requires an
explicit HTTPS host/port without credentials/path/query, a non-placeholder valid
workload credential and a dedicated nonzero 32-byte cursor secret. There are no
environment lookups or default/generated keys. Retain the same cursor secret across
requests/restarts while its cursors are valid; do not reuse signing/recovery keys.

`FeedProductProducer::new(config, ConcertTlsMaterial, ConcertPostReadSigner,
Arc<PublicProfileReader>, clock)` validates the fixed Post TLS endpoint without
connecting. Supply the actual shared public-profile reader from
`CanonicalOwnerComposition`, not a second authority or projection store. Root's
validated settings remain responsible for the actual Concert workload certificate,
key-role independence, endpoint identity, and active product release/resources.

The producer implements only `ScopedProductProducer`, for `Operation::Feed`,
`Operation::Post(id)` and `Operation::Comments(id)`. `handles` exposes that closed
set to Root's existing feature dispatcher. Compose it alongside Intro beneath
**one** `ScopedProductOwner::new(shared.fence, shared.core.delivery(), dispatcher)`.
Do not wrap each feature with a separate canonical scope/delivery registry.

Before any Post read the adapter checks the verified deadline, exact original
protobuf request, and full `GuardBinding` digest reconstructed from the verified
parent/actor/event/session/request/operation/method/raw target/body/original
deadline. It derives `Context` only from the matching supplied scope. In particular,
`authority_digest` is `parent_digest()`, not the guard binding digest. Queries are
copied from `VerifiedProduct`'s admitted parsed queries, preserving cursor, filter,
before and limit; the request is never reparsed as authority. Post detail retains
the existing default initial comment-page limit of 20.

The existing wrapper must run dispatch under its held `scope.run`. The producer
returns only `ProductRepresentation { status: 200, content_type:
"application/json", body: unchanged C04 JSON }`. It does not acquire a scope,
create a delivery token, mark serialization, finish a response, settle children or
start detached work. Read failures use the existing fixed tonic categories:
invalid input -> InvalidArgument, missing resource -> NotFound, denied ->
PermissionDenied, dependency/unexpected mutation-only failures -> Unavailable.
Root's existing error projection and delivery wrapper remain authoritative.

## Production prerequisites retained with Root

1. Integrate the qualified base history below. Root's inspected 145a8e93 tree did
   not yet contain `qualified_read.rs`, `qualified_transport.rs` or Post's
   `QualifiedReadRuntime`; source intake alone is not new runtime qualification.
2. Supply the actual Post HTTPS endpoint, explicit Concert credential/mTLS,
   `CONCERT_POST_READ_SIGNING_SEED` and dedicated Feed cursor secret. Post must
   load matching `CONCERT_POST_READ_VERIFYING_KEYS` for its Post-audience verifier.
3. Wire `QualifiedReadRuntime::connect(PostPool, AuthEndpoint, FriendEndpoint,
   ConcertCallbackEndpoint, PostCredential, &PostTls, PostConcertRootReadSigner)`.
   Post needs at least two pooled connections and migrations through 0032.
   Register **both** `concert_feed::qualified::services` outputs with the original
   deadline layer: actual qualified reads and authenticated Auth/Friend settlement.
   Legacy Feed RPCs remain unavailable. No new configuration names are invented here.
4. Mount `ConcertRootReadService::new(shared.core.registry(),
   shared.public_profile)` behind its exact Post-only policy, credential/mTLS and
   `PostConcertRootReadVerifier`. Post signs via `POST_CONCERT_ROOT_READ_SIGNING_SEED`;
   Concert loads `POST_CONCERT_ROOT_READ_VERIFYING_KEYS`. Preserve platform callback
   capture fixes `2b649965` and `aef031b4`; no Gateway/parity verifier fallback.
5. Reuse the original Auth/Friend owners, shared canonical registry/ledger/process
   instance, retained child recovery keys and typed public-profile settler. Auth and
   Friend must resolve registered Post transactions through the actual Post
   settlement endpoint. Preserve their original deadlines/revisions/epochs.
6. Change product settings/startup only in Root's lease. The old
   `production_owners` rejects Feed, and separate Product+Participant startup is
   rejected. Extend the existing shared Intro/C03 composition and single dispatcher.
7. Retain the capability through actual outer Gateway HTTP body EOF, then call
   CompleteResponse. Concert gRPC EOF only marks serialization. Mount one shared
   Delivery/ProducerSettlement service and supervise existing child/participant
   recovery. Preserve unresolved/UNKNOWN native outcomes and cancellation handling.

Post is the sole durable event post/comment/reaction owner; no separate comments
service or content store is required. Original owner checkout/branch:
`be-post-owner` / `codex/mcr-be-post-owner`; original orchestration:
`be-feed-content` / `codex/mcr-be-feed-content`. The original chat UUID is not
established by source. This increment includes comments **reads**, not writes.
Links/media and non-ALL filters retain their existing unavailable behavior.

## New validation status

Source formatting and whitespace checks: PASS before the source checkpoint.
`feed_product_scope` tests cover explicit endpoint acceptance, unsafe/ambiguous
endpoint rejection, missing/placeholder/malformed credentials, zero cursor key,
and the closed scoped read operation set. They do not construct a substitute
positive authority or claim a real held scope/HTTP test.

Executable validation: **NOT_RUN pending fresh Backend SL OFFER/ACCEPT**. Start
floor 16 GiB, continuous stop floor 8 GiB; Rust 1.88 locked/offline, jobs <=2,
debug=0, incremental=0. No target is held. Required bounded test target is
`concert-service --test feed_product_scope`; no prior native proof rerun is
requested because the underlying qualified reader/authority implementation is
unchanged. Freeze the exact executable/Cargo JSON/source/runtime pins outside the
target, verify zero handles, unconditionally release target before copied-test
execution, then do not revisit it. No PG/service runtime is requested.

Full original-request/scope/HTTP behavior still needs Root's existing actual IN03
consumer, including real restricted/deleted/blocked controls. Prior passing native
component evidence is preserved below and is not evidence for the new adapter.

## Qualified lineage intake order

Merge the published base `03c54fa3` preserving history, then integrate this adapter
increment; do not replay the following WIP checkpoints as independent validated
releases. This is a bounded first-parent chronological inventory, including shared
source intake merges and owner-granted fixes; each merged parent retains its own
history. Root owns only central conflict resolution and composition.

1. `06f07e2bc00ea80b4d89fc7846b516f8126cda2f` — wip(feed): preserve guarded policy and transaction identity integration
2. `b1012f14ef666ad97489b6d3ab80712a7fa4660a` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
3. `161752277076e26ebb4d6c909f063a3694881402` — wip(feed): preserve qualified read protocol and request admission bindings
4. `39b2d20d7b313f6976702fc8b2996c28e7ef743a` — wip(feed): preserve same-transaction producer and durable settlement integration
5. `377319c037d25bfb4475c3f22e89d3ad0a962296` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
6. `3dbb4d3d2b9efae4bcc45fe0d79fe58fcdfc1a36` — wip(feed): preserve scoped Post transport and native storage qualification cases
7. `bcc13595eed22d64213c37eb127a3fe838c60a8e` — wip(feed): preserve fail-closed dependency mapping and terminal evidence maintenance
8. `63f0bb77e54f14754cb7d8c3ec69eee5af00f1e4` — wip(feed): bind native storage checks to owned socket controller evidence
9. `21f5373ca042c5ec7d08ba7793491e1e4dc8e42f` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
10. `a27d1cab6497d16902754a7a3468515208cc5271` — wip(feed): preserve frozen before semantics and comment boundary qualification
11. `3f31d286276afd7ad968aab60e502fa8abb4c881` — wip(feed): preserve author-filter boundary and projection failure coverage
12. `149f7a28aa8ac257cb820721444d95026754af71` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
13. `37396e9bf65fe7694f099bf73d12642a39df37ec` — docs(feed): record consumed SDK2 prerequisite and open qualification gates
14. `4552ccf95695d8a3ff311a3a65b9a767e27ecba0` — fix(feed): isolate canonical authority generation from imported services
15. `1b891359a2d3fdd9d2fdd25d9a7be89dfaed7e61` — docs(feed): record compiled Post and actual native storage results
16. `067e18e63bb50bfb453ec83c761b3e66d00690db` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
17. `5088d3173d5ea2728a83eff0596b8e8f7fd8a54a` — feat(feed): compose current scoped Post and public author readers
18. `b14e5719489640201e47208a89e8f1d2969f1edf` — fix(feed): infer raw client transport before applying interceptor
19. `a7a969a7487cd419a1a67aa323f4f7a60d813cfd` — fix(feed): preserve local policy dependency failures without filtering
20. `7cd2d063e806d66ada77d29839518560dc482336` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
21. `8496730c9abb93cdb0b1cd4d9d4f18b36e954386` — test(feed): add actual Post runtime dependency for native join
22. `7771392780ce66621993b1cdfc991b6b32c3083e` — docs(feed): pin narrow native Post join dependency grant
23. `afa7cc2e44bf0c4c982459b807ae32da4449548e` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
24. `973a5a2cd765bf156c1d932f8d0144d7c389e372` — docs(feed): record consumed coherent Concert generation prerequisite
25. `17ad643f1b8eb3240c2181628bf298d83c1ca9bd` — feat(product): add retained root subject callback for qualified Post reads
26. `a473932517937b08775628fd39dbdcf9d06f7c6d` — fix(profile): fail closed on authority denial during author reads
27. `5c646fc5264acae16846de3e15202798628f6ab6` — fix(authority): reserve subject denial without hiding scope failures
28. `ce48ad5524c18d5d3c9521322a3e94f691fddb23` — feat(product): derive source-bound era manifests and inert operator candidate
29. `635d7efe13898b3b3779f0ced9ffdca4419f3169` — test(feed): stage real five-owner native read join with product release
30. `ca35b63c1d6f4360513349e3d7690c00bf29178c` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
31. `729a5d0f3fb8f7f9e33692c1e61dd774fbbce31a` — test(feed): add frozen owned controller for actual owner join
32. `20c1d58be582658df58ec1b30760df55098bfa43` — test(feed): pin character registry in native join source inventory
33. `64fe6372dec5e77ba925a2e53299386bed8c2f02` — docs(feed): record bounded native denial dependency evidence
34. `0af14eb9985811066fedea0b857b66139cfb1bc0` — docs(feed): track actual public reader local-policy join failure
35. `236a283937b391104fe4ba09222edf4c134a2cef` — docs(feed): identify deferred authority deadline failure from native SQL log
36. `fab50bcba996c393db59de7276248263e3bcfacf` — docs(feed): record actual 20-case adapter compilation and execution
37. `6d9ef8c436732e208a51142429e4659f0443c4e4` — fix(feed): consume original local authority deadline correction
38. `8591175a686159a55b06a1708d5a8baf68ab0b01` — docs(feed): pin exact owner-authorized deadline fix adaptation
39. `45137d44ee74ab7fa38898255ffed355cb6740a4` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
40. `953a8407aad2eeb332f8641ba840e20cc065fece` — test(feed): align native signed GET body and record fixed reader evidence
41. `049540cce327feba88543434e0861a5d679963f5` — Merge remote-tracking branch 'origin/master' into codex/mcr-be-event-feed-read
42. `258dbd9e73d0fbadc2bab801895d41fbcd22ee32` — docs(feed): record consumed published public reader handoff
43. `255229e372593303f9beb0ca772ebdb83d3e9d6d` — test(feed): reject invalid native checkpoints before database creation
44. `404499731b2d2ff11dafc1402cb0bcc95a11e595` — test(feed): remove unused native fixture imports
45. `037e3ce5ff941f306cbc5d76f4831828333709eb` — test(feed): freeze every source-bound resource including provenance
46. `c45c2c7aadae2c092b3a2bb48464aed13c215801` — test(feed): expose qualified list denial in native diagnostics
47. `4162e8f46711bdadd4e712e63c2ce5dbf781a6cf` — docs(feed): record compiled native join and actual failing read boundary
48. `6c78e5648f3eb1bf960c967daef61fd999702ef4` — test(feed): distinguish actor policy from complete Post transport failure
49. `2ab8e50f59c1f4685337bb8f0f1296313ce26ee4` — fix(feed): preserve Post authority denial as whole-read failure
50. `2b6499654da1b1ba611ded38979b0bff3e332268` — fix(platform): capture exact verified Post root callback frame
51. `aef031b4b6dfca7eff00a852f531724a0236fa5f` — test(platform): keep other signed Post routes free of callback capture
52. `2d7f1b3a29a5fca3abe7d05b529312558868d602` — docs(feed): identify current native gate and delegated capture repair
53. `d9a64bbc093b82b4d8ba0ade2400a631a2d644fc` — docs(feed): record qualified callback and actual five-owner list join
54. `03bb0380236073ff2766dc9e106016902009875d` — test(feed): stage real detail comments privacy outage and sealed-scope cases
55. `db25c4fa7db65f5f20b53ee995326b00aec029d3` — test(feed): bind real parent admission and native cursor rejection
56. `03c54fa379e5b468ea5634311714c057408e5bb5` — docs(feed): record eight real five-owner native read cases

## Net qualified lineage paths from the common ancestor

40 paths, retained by the history merge:

- `backend/Cargo.lock`
- `backend/crates/platform/src/actor_delegation.rs`
- `backend/crates/platform/src/actor_delegation/root_read_capture_tests.rs`
- `backend/crates/post-application/src/concert_feed/mod.rs`
- `backend/proto/post/v1/concert_feed.proto`
- `backend/scripts/mcr/feed_live_read_smoke.py`
- `backend/scripts/mcr/test_feed_live_read_smoke.py`
- `backend/services/concert-service/Cargo.toml`
- `backend/services/concert-service/src/feed/author_filter.rs`
- `backend/services/concert-service/src/feed/live_read.rs`
- `backend/services/concert-service/src/feed/mod.rs`
- `backend/services/concert-service/src/feed/native_join.rs`
- `backend/services/concert-service/src/feed/native_join/capture.rs`
- `backend/services/concert-service/src/feed/native_join/environment.rs`
- `backend/services/concert-service/src/feed/native_join/owners.rs`
- `backend/services/concert-service/src/feed/native_join/read_cases.rs`
- `backend/services/concert-service/src/feed/owner.rs`
- `backend/services/concert-service/src/feed/policy.rs`
- `backend/services/concert-service/src/feed/qualified_read.rs`
- `backend/services/concert-service/src/feed/qualified_transport.rs`
- `backend/services/concert-service/src/feed/resources.rs`
- `backend/services/concert-service/tests/feed_live_read.rs`
- `backend/services/concert-service/tests/feed_live_read_support/mod.rs`
- `backend/services/concert-service/tests/feed_live_read_support/native_join.py`
- `backend/services/concert-service/tests/feed_live_read_support/test_native_join.py`
- `backend/services/post-service/build.rs`
- `backend/services/post-service/migrations/0032_concert_feed_read_tracking.sql`
- `backend/services/post-service/src/adapters/in/grpc/concert_feed.rs`
- `backend/services/post-service/src/concert_feed/mod.rs`
- `backend/services/post-service/src/concert_feed/query.rs`
- `backend/services/post-service/src/concert_feed/read_admission.rs`
- `backend/services/post-service/src/concert_feed/read_authority.rs`
- `backend/services/post-service/src/concert_feed/read_clients.rs`
- `backend/services/post-service/src/concert_feed/read_runtime.rs`
- `backend/services/post-service/src/concert_feed/read_tracking.rs`
- `backend/services/post-service/src/concert_feed/read_tracking.sql`
- `backend/services/post-service/src/concert_feed/read_tracking_tests.rs`
- `backend/services/post-service/src/concert_feed/read_transaction.rs`
- `backend/services/post-service/src/concert_feed/tests.rs`
- `docs/mcr/backend/feed-live-read.md`

## Exact adapter source hashes

- `backend/services/concert-service/src/feed/product.rs` SHA-256 `bf59b4d01c706c96c613cf3833cdcaaf2ec41ced5df9976fbf4c3c48689eadfa`.
- `backend/services/concert-service/src/feed/mod.rs` SHA-256 `357d12ae27c042670e9e08d02efb7846af646ca029a2fe979c470887c3d790ed`.
- `backend/services/concert-service/tests/feed_product_scope.rs` SHA-256 `285fd8bd9e37e3b3cb2940d0de1857fb69de6a9de14dec767c0a7a49408ebf54`.

## Preserved native evidence inventory

Source db25c4fa: eight real five-owner PostgreSQL/mTLS native read cases PASS, each on its own marker/inode-bound five-database cluster. Exact migration ledgers Auth17/Friend17/Concert24/Profile9/Post32; explicit component response completion, zero prepared work, all eight owned roots absent. Source2d7f1b3a: prior actual five-owner list and focused platform callback guards. These are historical executed receipts, not rerun by this source preparation.

- `/private/tmp/mcr-feed-native-eight-v2-20261104/checkpoint.json` SHA-256 `3fc1dab72ded00c746a8a8dd87fc07deaaaca925ab43b721bcb5120b9dc0c94b`.
- `/private/tmp/mcr-feed-native-eight-v2-20261104/native-report.json` SHA-256 `f5e904b3386a34b5f4266ba6d4414b131d920c402d62bdb92d8c73bf097ce4a2`.
- `/private/tmp/mcr-feed-native-eight-v2-20261104/verified-receipt.json` SHA-256 `a61ada606e4ed63e778b75b65eda73cb1c9b1e0287b38e3b0261279642243946`.
- `/private/tmp/mcr-feed-root-capture-v2-20261104/checkpoint.json` SHA-256 `0efa47fee7bec4ff6ae359322a318fd731634ad81b093af6d36ad73808045cd3`.
- `/private/tmp/mcr-feed-root-capture-v2-20261104/focused-report.json` SHA-256 `fc51d4228fc4b1f1dc3939a44844db128ade22e54b95a47e9d80d7bd028d2cb7`.
- `/private/tmp/mcr-feed-root-capture-v2-20261104/native-report.json` SHA-256 `df22917d0f7089687d59eb303efbf49e69f326c053daf2b2a3f61a41aa962dc4`.

Current qualified executable pins (preserve; not executed by this increment):

- `/private/tmp/mcr-feed-native-eight-v2-20261104/concert_service`: `b69d1442fc49c61be1ee9d533b43994c45ac6bb7a5db04a23c5ace95e50a6d49`.
- `/private/tmp/mcr-feed-root-capture-v2-20261104/concert_service`: `4924f9fa32766b92ac3c9a305514f0157e63cb95b2eb0b111ad54e3556f1bc9a`.
- `/private/tmp/mcr-feed-root-capture-v2-20261104/platform`: `bd34d902377b500342075acea3a68167e76b76bf2b92e9dc243d9f0718332ced`.
