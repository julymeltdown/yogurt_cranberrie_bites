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

Only the three exact media mutation method/path tuples bypass the inherited
Gateway journal. Concert owns their durable receipts; private image bytes must
not be buffered into a second Gateway replay record. All other retained mutation
and idempotency behavior remains in place. This narrow `server.rs`/HTTP root
extension was explicitly assigned by the human through the coordinating chat.

The platform actor-buffer change was separately delegated by its current owner:
only `/concert.product.v1.ProductService/Dispatch` admits 10 MiB plus 16 KiB of
protobuf metadata and five bytes of gRPC framing. Both signing and verification
use that bound; all other methods retain the existing 4 MiB ceiling. Local source
commit `f2c750f9` is **NOT_RUN**, not a published validation claim. Ownership of
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
| Feed developer | Guarded Post read constructor, actual owner reader, cursor/schema projection and producer settlement | Source active, **NOT_RUN** |
| Media developer | Uploader-only local transactional policy and producer settlement under canonical fence | Typed source consumed; native qualification pending |
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
| Platform maximum signed upload/retained-limit regression | **NOT_RUN** | Requires explicitly released build target |
| Concert `product_runtime` and media wrapper native tests | **NOT_RUN** | Same resource requirement |
| Gateway `concert_product_http` actual transport-boundary fixtures | **NOT_RUN** | No real producer authority claim |
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

Build targets and private PG clusters remain separately leased. No Cargo build,
shared target write or PG operation has been performed by this source checkpoint.
The first native compile will refresh peer source, preserve their module/proto
registrations, then use Rust 1.88, locked/offline dependencies, at most two jobs,
debug information disabled and incremental compilation disabled. Only exact
copied/hash-pinned executables may enter a private socket-only PG test run.
