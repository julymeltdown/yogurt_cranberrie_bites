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
