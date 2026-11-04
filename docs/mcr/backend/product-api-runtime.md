# Product API runtime composition

Status: **IN_PROGRESS**. This record covers BE-PRODUCT-API-COMPOSITION only.
No product capability or multiuser acceptance is asserted by this source checkpoint.

## Assignment and persistent goal

The human-authorized assignment is actual Gateway → Concert → Post event Feed
reads and uploader-only media composition, then directly coordinated participant
profile/avatar composition. Completion requires implementation, relevant native
and HTTP validation, documented provenance and limitations, committed publication,
and fixes owned by the developer. Kings and SLs do not review or verify this work.

The actual host goal remains **ACTIVE** with an incompatible historical objective:
“Deliver the MCR integration checkpoint from f0e44bd: preserve and validate pending
BE/FE changes, integrate bounded validated handoffs, implement real Concert startup
and frontend world entry wiring with assigned owners, resolve concrete review/check
failures, document exact evidence and limitations, commit separate BE/FE changes and
publish master with verified remote ancestry. Continue until assigned integration
scope and required fixes are delivered.” The supported API cannot replace an
unfinished objective. It has not been falsely completed or reset. The current
assignment is authoritative and required work continues.

The assigned checkout is `mcr-worktrees/be-media-combined`, branch
`codex/mcr-be-product-api`; the old integration checkouts are not writable by this
assignment. Source work began at `f4b4080897a6b1ad835fa4e5e9372f82d58e489e`,
fast-forwarded to `be629fa0`. Media owner source `cb9f75eb` was cherry-picked as
`fa25fea2` solely to consume its typed API; that source was explicitly **NOT_RUN**.
The qualified media dependency was subsequently published by its owner as
`84126f6adddc1573287b04c6d4e2611e4f2f3970` and reconciled here. Add/add conflicts
from the preparatory cherry-pick were resolved to the exact published owner
versions, preserving its settlement helper and qualification record. Its 9 runtime,
17 retained-producer and 1 restart tests qualify that component; actual canonical
guard/Gateway integration remains unqualified. Feed groundwork
`02b17e0496877636dbe6f0ff42f5d31ab5643faa` is also present, with its owner's
bounded facade/codec evidence, not a live Feed claim.

## Internal and public boundaries

`backend/proto/concert/v1/product.proto` is an internal transport contract, not a
replacement public DTO. `ProductService.Dispatch` accepts only the retained
Gateway workload with a signed field-one actor. Its signed body includes canonical
actor/event/session/request IDs, exact HTTP method and target, original body,
content type, idempotency key, access expiry and bounded owner observations.
Observations are not canonical owner fences.

The route set currently authored is:

| Method | Event-relative path | Producer boundary |
| --- | --- | --- |
| GET | `/feed` | Event Feed read, not inherited `/api/feed` |
| GET | `/posts/{postId}` | Guarded Post detail |
| GET | `/posts/{postId}/comments` | Guarded comment page |
| GET | `/parties` | Party list |
| GET | `/parties/{partyId}` | Party detail |
| POST | `/parties` | Receipt-backed party creation |
| POST | `/media/uploads` | `initConcertUpload` |
| PUT | `/media/{mediaId}/bytes` | `putConcertUploadBytes` |
| POST | `/media/{mediaId}/finalize` | `finalizeConcertUpload` |
| GET | `/media/{mediaId}` | Uploader metadata |
| GET | `/media/{mediaId}/content` | Authorized sanitized private bytes |
| GET | `/commands` | Media command recovery only; closed operation/key query |

All paths are below `/api/concert/events/{eventId}`. Unsupported methods, arbitrary
forwarding, cross-event targets and noncanonical UUIDs are rejected. Feed writes,
linked/public media, People, introduction, sharing, age and inbox are not implied.
Public notices and participant routes still require their concrete owner wiring.

Gateway reuses strict `HttpConcertAccess`, fresh retained identity reads, workload
credentials, TLS and actor signing. The original monotonic deadline survives every
await and RPC; no retry resets it. JSON mutation bodies are bounded to 64 KiB,
upload bytes to 10 MiB, and four product requests may be active at either edge.
Mutations require the configured browser Origin, one browser-intent header and a
canonical UUID idempotency key. All responses use `no-store`; private image
responses additionally use `nosniff`, inline disposition and same-origin resource
policy. Transport failures expose fixed C04 errors without dependency diagnostics.

Only the three exact media mutation method/path tuples and party creation bypass the inherited
Gateway journal. Concert owns their durable receipts; private image bytes must
not be buffered into a second Gateway replay record. All other retained mutation
and idempotency behavior remains in place. This narrow `server.rs`/HTTP root
extension was explicitly assigned by the human through the coordinating chat.

The platform actor-buffer change was separately delegated by its current owner:
only `/concert.product.v1.ProductService/Dispatch` admits 10 MiB plus 16 KiB of
protobuf metadata and five bytes of gRPC framing. Both signing and verification
use that bound; all other methods retain the existing 4 MiB ceiling. Local source
commit `f2c750f9` now has the targeted native verification recorded below. Ownership of
`actor_delegation.rs` was returned to the authority developer for sealed-parent
capture; do not overwrite that subsequent work.

## Startup and producer dependencies

`CONCERT_PUBLIC_FEED_MODE` accepts only `off` (default) or `read`.
`CONCERT_OWNER_MEDIA_MODE` accepts only `off` (default) or `uploader`.
An enabled product requires existing guarded bootstrap gRPC composition. At this
checkpoint both explicit opt-ins fail before database or listener exposure because
their real production authority constructors are not yet composed. Starting an
unavailable handler does not satisfy readiness. Bootstrap capability flags remain
disabled; frontend availability must derive from the qualified producer path.

The media runtime is mapped through `OwnerMediaRequest::from_verified` and its
existing closed C04 projections. `CommandError::Expired` maps to HTTP 409 with
`IDEMPOTENCY_EXPIRED`, as required by C04. Production requires non-default HMAC
fingerprint keys, a bounded image sanitizer and an actual canonical
`OwnerMediaFence::run` around the entire producer/recovery future. No successful
fixture authority or observation-only implementation is selected.

Outstanding concrete prerequisites:

| Owner | Required boundary | Current qualification |
| --- | --- | --- |
| Canonical authority developer | Account/session and relationship exclusion through commit/response; exact signed Gateway parent; durable unknown-outcome settlement | Source active, **NOT_RUN** |
| Public Profile developer | Qualified feed-author projection with real avatar state and held privacy/deletion exclusion | Source active, **NOT_RUN** |
| Feed developer | Guarded Post read constructor, actual owner reader, cursor/schema projection and producer settlement | Facade/codec groundwork qualified; actual guard/PG/HTTP **NOT_RUN** |
| Media developer | Uploader-only local transactional policy and producer settlement under canonical fence | Runtime/settlement helper qualified; actual canonical/root/HTTP **NOT_RUN** |
| Participant developer | Actual self-profile/avatar/preferences authority and constructor | Source active, **NOT_RUN** |

The proposed prepared-transaction owner guard has explicit environment and crash
settlement requirements. A cancellation, timeout, owner death or unknown remote
COMMIT must not blindly release exclusion. The product root must retain sealed
parent/scope through final representation assembly and the qualified response
completion boundary. A detached observation or dropped stream is insufficient.

## Evidence and remaining work

All product transport, configuration, route and probe code is newly authored.
Retained `concert_client`, `concert_access`, bootstrap/world composition and media
serialization patterns informed compatibility; no old rendering was copied.
Source archives, manifests, lockfiles and public C04 schemas were not changed.
New generated Rust clients use the existing Tonic/Prost build and dependencies.

| Check | Result | Limit |
| --- | --- | --- |
| Rust 1.88 `rustfmt --edition 2021 --config skip_children=true` on owned new Rust files | Exit 0 | Parsing/formatting only |
| `git diff --check` | Exit 0 | Whitespace only |
| `python3 -m unittest discover -s backend/scripts/mcr -p test_product_api_smoke.py -v` | Exit 0, 6 tests | Probe/schema safety, not application execution |
| Platform maximum signed upload/retained-limit regression | Exit 0, 1 test | Actual signer → verifier; no product owner claim |
| Concert `product_runtime` | Exit 0, 4 tests | Closed routes, caller/actor policy and startup settings; media wrapper compiled |
| Gateway `concert_product_http` actual transport-boundary fixtures | Exit 0, 4 tests | Three owned HTTP cases plus retained harness check; no real producer authority claim |
| Gateway exact journal bypass and retained idempotency scope | Exit 0, 2 tests | Unknown paths/methods remain outside bypass; inherited scope preserved |
| Actual copied Concert process with unsupported Feed/media modes | Exit nonzero as required, 2 cases | Fixed startup error before listener marker or DB connection |
| Actual multi-service HTTP Feed/media journey | **NOT_RUN** | Qualified producers/configuration required |
| Actual revocation, rollback, recovery and crash-settlement gates | **NOT_RUN** | Required before acceptance |

`product_api_smoke.py` validates each actual response against the exact frozen
endpoint/status schema, refuses redirects and credential-bearing URLs, bounds
response bodies and never prints access tokens or private bodies. A Feed probe
requires a known Post-owner row. The media probe performs initialize → upload →
finalize → exact-key replay → command recovery → private read and foreign-account
denial. `HTTP_SMOKE_PASS` is deliberately separate from owner-fence acceptance;
revocation ordering, rollback and crash settlement remain explicitly `NOT_RUN`
unless their native gates have actually executed.

The native source base was `4e65142e9883a2177364fef709bb2d0b54bf2feb` after
merging current remote `8f89d190` and preserving participant generation/exports.
The MEDIA target was explicitly transferred from its previous owner, accepted
after a fresh 77.77 GiB/zero-handle check, and released after artifact freezing
and a fresh zero-handle/76.99 GiB check. No BACKEND target was acquired. Commands
used Rust 1.88, `--locked --offline`, jobs 2, both profile debug settings 0 and
incremental compilation 0. No PG operation was run by this checkpoint.

The first Concert build exited 101 because the retained `contracts-build` cache
embedded another worktree's absolute proto path. Under the exclusive MEDIA lease,
only `contracts-build`, Auth/Profile/Friend service build artifacts and Concert
were cleaned; the exact retry exited 0. The first Gateway HTTP run found a real
missing-Origin reason regression. Fix `4e65142e` preserves only the closed
browser-denial reason, discards arbitrary inherited diagnostics, and passed the
same affected tests. These failures are resolved, not suppressed.

Exact copied executables and JSON build receipts are frozen outside the target
in the local `mcr-product-api-native` evidence directory. SHA-256 values:

| Copied artifact | SHA-256 |
| --- | --- |
| Platform tests | `493ebefc2d5dbb6930d8d90a51ba9edfa639daf80512d5f76f5befe47b5f0eb9` |
| Concert product tests | `34ebd374820e3a5f354c120aa93bb8ccd07e0443fdbea4dda7ef0586a423a661` |
| Concert service | `27a31745d10d9680e35911266bea934f22afbffda0633a4c0fa174c8f1063706` |
| Gateway product HTTP tests, corrected | `0c3b7e361c204d9e8d4daa8a5fc6c815d54671857779695f4df5de80f92829e7` |
| Gateway library tests | `6c9aa547dd454e7e607cfc2ac2c14802ce11654d9451a58fee8a78708b1726c0` |

These bounded results qualify the transport and fail-closed startup checkpoint.
They do not qualify the currently missing production owner constructors, actual
multi-service HTTP journey or acceptance of the whole assignment. Required work
continues without holding an idle build lease.

## Durable delivery checkpoint: `b59fea237d7514315f0c4121762a493640410dd8`

This later checkpoint uses the published canonical SDK1
`888cb6bd30846d891286b92a9a66e921e8d1ce78`. It adds the root-owned
`0028_product_owner_scope_lifecycle.sql`, `product_rpc/scope_ledger.rs`,
`scope_lifecycle.rs`, and `delivery.rs`; internal Product delivery IDL; and the
Gateway's outer retained HTTP body layer. All product modes remain closed.
The Post/party routes above are source composition, not available producers.

The actual canonical scope must be registered durably before acquisition, and a
private delivery capability must be bound before protected producer work.
Concert gRPC EOF records serialization only. Gateway's actual terminal HTTP body
production invokes the workload-only completion RPC with a separate bounded
system deadline. Neither browser cancellation nor an expired actor deadline
certifies completion. A finished-root marker does not attest owner transaction
settlement. A separate durable completed receipt is written only after the actual
canonical scope finishes its owner and child settlement; absent memory or a lost
receipt stays unknown. This boundary proves server body production, not browser
receipt or rendering.

Transactions persist their exact native identity and opaque producer settlement
reference before owner Track calls. Auth and Friend acknowledgements are recorded
separately; both must match before producer effects. Sealing bars late
registration, capacity never evicts unfinished scopes, and bindings/evidence
cannot be rewritten or deleted. SQL and Rust fixtures use synthetic owner
bindings: they do not mint or qualify real owner authority.

The exclusively allocated BACKEND target was released after three successful
locked/offline Rust 1.88 builds (Concert tests, Gateway tests, Concert binary),
jobs 2, profile debug 0, incremental 0. Seven executables, Cargo JSON receipts and
715 source-file pins were copied to a private evidence directory before release.
The initial cached contracts-build helper referenced another worker's absolute
proto path. Cleaning only affected package artifacts corrected the cache issue.
A subsequent compile found and fixed an ambiguous media response Result and a
SQLx async-trait transaction lifetime/Send issue. The same builds then exited 0.
Release verification found zero target handles and 76,092,981,248 free bytes.

| Actual copied-binary or isolated PG check | Result | Qualification boundary |
| --- | --- | --- |
| Concert strict Feed/comment query parser | Exit 0, 3 tests | Includes exact query decoding and closed bounds |
| Concert product runtime | Exit 0, 4 tests | Closed routing, policy and startup settings |
| Gateway product HTTP transport | Exit 0, 4 tests | Actual loopback transport with service doubles |
| Gateway terminal delivery | Exit 0, 4 tests | EOF, cancellation, expired browser deadline, unknown completion and malformed capability; service doubles |
| Gateway exact receipt scope | Exit 0, 1 test | Retained receipt boundary |
| Native Rust scope ledger | Exit 0, 3 tests | Actual private PG transactions; real in-progress/aborted XID evidence remains distinct from root markers |
| Ledger migration and SQL invariants | Exit 0, 17 cases | Includes actual PG restart and cleanup; rejection cases intentionally exit 1 |
| Current copied Concert startup | Exit 1 as required, 2 cases | Both product modes fail with the expected guarded-composition error before DB/listener exposure |
| Real canonical owners / joined producer / end-to-end HTTP | **NOT_RUN** | Required before enabling product modes |
| SDK2 child intents / restart recovery | **NOT_RUN** | Later published SDK2 is not qualified by these SDK1 artifacts |

Exact private evidence basenames are `native-boundaries.result.json`,
`native-ledger.result.json`, `startup.result.json`, `source-pins.json`,
`binaries.json`, per-executable `.receipt.json`, and `target-release.json`.
The native ledger SHA-256 is
`a7734e80d93870b47b7506b5d227b6c639cb499693b3f4be2510a5fa3838b8cf`;
Gateway terminal delivery is
`059630c2f93b53eb4767f32966b341e5a7d65cc605c8fe9d0702a7461744a058`;
Concert binary is
`f94e88931ec83237f0bc684586d33842b511f56657a736bd80ca35a4000c845b`.
The ledger migration SHA-256 is
`31d0501b54bd08bb9650e4beaa003b4739949bc78c97c070cca06711160cd628`.
The frozen tree also contained the exact four-file optional cooldown contract
patch `dc4a3f93`, subsequently published unchanged in `faeb6ed4`; it was recorded
as an additional source pin, not silently attributed to the root commit.

## SDK2 child recovery and system settlement: `3298445a`

This qualified source incorporates SDK2 `c4ad561cd813f0f7f1e14f21cb2ee90592e09eea`
and published Auth/Friend stores `dc70d62049d8e8430c88b9ebfc6f071740020538`.
Root commits `28c8a4ec`, `7e7c8128`, and `d7abfed5` add the required child hooks,
the owner-only ProducerSettlementService, native rollback resolution, and the
strict registered-operation/key command-status query. No producer is enabled.

`PgScopeLedger::new` now requires explicit `ChildRecoveryKeys` and an actual
`ChildIntentSettler`; neither has a successful default. The operator supplies a
nonzero 32-byte AES-256-GCM key and stable ID, with up to three retained prior keys
for restart/rotation. Closed `PROFILE_PUBLIC` and `LOCAL_PUBLIC` references are
encrypted before dispatch/PREPARE. Authenticated encryption binds the instance,
scope, original binding digest, kind, intent UUID, and key ID. Root checks the
complete original parent/query identity, local producer instance, bounded native
identity and closed child identifier before admission. Exact duplicates are
idempotent; altered references, late admission and more than 202 intents fail.
Completion evidence is immutable, and a pending child blocks the final durable
delivery receipt even after root work finishes. Recovery releases no DB
transaction across the configured owner/native adapter call and records each
actual successful recovery before moving on. Unknown outcomes retain evidence.

`product_rpc::settlement::ProducerSettlementService::new(ledger, resolver)` exposes
the canonical lease protocol to authenticated Auth/Friend workloads only, with no
actor grant, 16 concurrent calls, bounded frames and a separate two-second system
budget. Scope absence returns unfinished. Transaction requests must match the
durable full identity, original binding and that owner's registration. A lost
Track acknowledgement can resolve only through actual native ABORTED evidence;
other outcomes remain UNKNOWN. The concrete `PgNativeAbortResolver` checks the
configured Concert system identifier/database OID/full XID. A raw COMMITTED XID
stays UNKNOWN: a producer-specific same-transaction receipt is still required.

Command status accepts exactly `operation_id` and `idempotency_key`, decodes once,
rejects duplicates/unknown fields and admits registered mutation operations only.
Its outer canonical operation remains `getConcertCommandStatus`. No browser
target field is invented. A producer must recover and reauthorize its actual
durable target before exposing safe receipt metadata. Gift pending/failed reserve
correlation and other producer-specific status adapters remain separate work.

Both locked/offline Rust 1.88 builds exited 0: Concert library/runtime/ledger test
artifacts in 26.67 seconds and the Concert binary in 16.10 seconds. Jobs were 2,
both profile debug settings 0, incremental 0. There were no warnings in the owned
product paths; inherited World warnings were retained. The allocated MEDIA target
was released with zero handles and 70,979,026,944 free bytes before native PG ran.
Four executables and Cargo JSON receipts pin 1,778 source inputs. Current native
results, from copied binaries only:

| Check | Result | Limit |
| --- | --- | --- |
| Full Concert library | Exit 0, 48 passed, 13 ignored | Includes child cipher/AAD/rotation, command parsing and system policy; ignored native gates are not credited |
| Product runtime | Exit 0, 4 passed | Closed policy/routes/settings |
| Actual native ledger | Exit 0, 5 passed | Private PG16.14; partial Track loss, real rollback/full XID, raw commit UNKNOWN, encrypted intent retention and failed/successful recovery orchestration |
| SQL invariants | 26 cases passed, actual restart and cleanup | Includes immutable child evidence and pending-child delivery denial; expected rejection commands exit 1 |
| Actual current Concert process | Two expected exit-1 refusals | Before DB/listeners for unsupported product opt-ins |
| Real owner/root mTLS join | **NOT_RUN by this checkpoint** | Owned by the canonical authority developer's separate native fixture |
| Real Profile child recovery / HTTP producers | **NOT_RUN** | Required before live feature acceptance |
| Gift private resource child and before-byte expiry gate | **NOT_RUN** | A subsequent explicitly typed SDK increment; never impersonated as an avatar child |

The native child recovery success case uses an explicit service double to test
durable orchestration after lost handles; it does not certify actual Profile
recovery. The ledger settlement test uses caller extensions at the service seam;
it does not certify workload authentication. The earlier Gateway HTTP delivery
fixtures remain evidence for `b59fea23`, not a fresh SDK2 HTTP run.

SDK2 copied artifact SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| Concert library | `3166010494f18796ddf9579b0bcf09826d36cb7c7756a94fa295b9c90cd17a6b` |
| Product runtime | `b456d2fa62892f90e02706feb355219b9e91867f107dd14bc13fd5cac69964cc` |
| Native ledger | `304303e4a9f04d4d7c2a0e4e82a8bec2a7894ea077976393f9c3a80673ccf72c` |
| Concert process | `05138b6d3bd2e20d87f82e045580e1e433f778890eba46c50cdf5186201f6315` |

The private `mcr-product-sdk2-native-20261104` evidence directory contains
`native-boundaries.result.json`, `native-ledger.result.json`, `source-pins.json`,
per-artifact Cargo receipts and `target-release.json`. These qualify this exact
SDK2 composition, not later source changes, deployment or whole-product readiness.


## Distinct effect identity and scoped delivery: `188db7c9`

Source commits `cc4c9acf` and `d005b272` were compiled at
`188db7c9b01ed30d63e2dd3d9272b11dad1bef31`. A producer's native effect digest
need not equal its enclosing owner guard digest. The ledger now binds the
original scope digest separately and compares the complete registered transaction
identity at tracking, confirmation and settlement. Altered identities and wrong
owner registrations remain denied. Two native ledger cases use distinct digests.

`ScopedProductOwner` acquires one actual canonical scope and persists its delivery
capability before constructing the producer future. Successful representations
and normalized produced errors carry the same private capability and response
token through serialization. This wrapper does not complete owner exclusion on
return; actual terminal delivery and native settlement remain required.

Fresh Rust 1.88 locked/offline builds used jobs=2, debug=0, incremental=0.
The first attempt failed because cached `contracts-build` supplied another
checkout's absolute proto include directory. Cleaning only contracts-build and
Auth/Friend/Profile artifacts repaired this; the unchanged source retry and
Concert binary build both exited 0. All 1,809 recorded input hashes remained
unchanged during compilation. Four copied executable artifacts were frozen in
`/private/tmp/mcr-product-effect-native-20261104`, with Cargo JSON and full input
receipts. The shared build target was released with zero open handles and
69,326,716,928 free bytes before starting PostgreSQL.

| Check | Actual result | Limit |
| --- | --- | --- |
| Concert library | Exit 0, 48 passed, 13 ignored | Ignored native gates receive no credit |
| Product runtime | Exit 0, 4 passed | Configuration tests |
| Native ledger | Exit 0, 5 passed | Real private PostgreSQL16; service-caller extensions and child recovery fixture remain explicit doubles |
| SQL lifecycle | 26 checks passed, restart and cleanup passed | No real owner RPC or HTTP proof |
| Product opt-in startup | Both exit 1 before DB/listener with missing guarded bootstrap | Product modes remain closed |
| New scoped wrapper with real owners and Gateway HTTP | NOT_RUN | Required follow-up; compilation alone does not prove terminal delivery |

Copied artifact SHA-256:

- concert-lib-tests: `0c7fb9b6cc1e47eeccd8a47acd3d46f3803409f843217e02e5fe03468211af37`
- product-runtime-tests: `7546e66e4fafcf7d1400a1c18fc7a1fd6fec55b4b45260ff1cac1b15d60e3de4`
- product-scope-ledger-tests: `9f12286d4c34d637bce5defd757bbd128d8136240f955aefd3ac279135edd3a4`
- concert-service: `dff6ba30fead9bedd54208758f2a85e9ca4c2a610bb392749de56065d586471b`

Commands: the prior SDK2 bounded Cargo test/bin commands were repeated against
this source. `product_scope_ledger_smoke.py` used the new copied ledger executable
and its receipt, returning PASS/cleanup PASS. Copied library/runtime binaries ran
with `--test-threads=1`. `python3 scripts/mcr/check-plan.py` and `git diff --check`
exited 0. Initial startup probes omitted required local metrics configuration;
those refusals were not credited, and corrected explicit-local probes were run.


## SDK3 private Gift composition: source checkpoint only

Unpublished source consumes SDK `0576c359` and response-wide ceiling
`f646f015`, plus PublicProfile's explicit unsupported-Gift recovery arm
`56f18703`. Root `82ec8b89`/`c9a5081c` adds migration0037, the encrypted-ledger
Gift kind and strict composite recovery dispatch. Published0028 remains unchanged.
The 202-child cap and lifecycle/FK/immutability barriers remain in place. Gift
shape validation is time independent for restart recovery; expiry never certifies
rollback. `GiftPrivateIntentSettler` requires a real Gift native read-child adapter;
public Profile/local-avatar recovery cannot substitute for it.

Root `0797bdd6`/`22590db0` adds internal ProductResponse field5
`content_expires_unix_micros`. ScopedProductOwner obtains the actual minimum
across all durable intents and held children, preserving the original monotonic
stop. Gateway validates that ceiling against original access authority and checks
wall and monotonic time immediately before every HTTP data frame. Expired content
produces a body error, leaving the retained scope unresolved. EOF after valid
data may still settle existing work after expiry; it grants no renewed content.
The new direct http-body manifest/lock edge reuses the workspace resolution.

A privately constructible VerifiedClientAddress is minted only by the actual
Gateway ConnectInfo/trusted-proxy normalizer. It is not a browser-header grant.
The typed-address test and three disclosure-body boundary tests are authored,
**NOT_RUN**. The privacy HMAC/network ProductRequest binding is still absent;
Gift Claim remains unavailable without it. Pickup/code stay closed until the
actual native Gift adapter, coherent build and terminal boundary are qualified.

The updated owned PostgreSQL16 SQL runner applies0028 then0037. Its first run
correctly reached restart but failed an old one-row assertion after adding the
second Gift child; cleanup passed. The assertion now checks both pending rows.
Retry `/private/tmp/mcr-product-sdk3-child-sql-20261104-retry.json` reports PASS,
29 recorded commands (two migration applications and27 lifecycle/kind checks),
physical restart and owned cleanup PASS. No Cargo, owner RPC or Rust binary was
used by these SQL runs. Full SDK3 Rust compilation, cipher regression, Gateway
boundary tests, actual private-child recovery and HTTP remain **NOT_RUN**.
Do not apply the earlier SDK2/native/operator evidence to these new sources.
