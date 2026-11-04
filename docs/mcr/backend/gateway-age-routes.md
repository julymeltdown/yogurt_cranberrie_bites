# AGE-GW-03 — public Age transport admission

Status: IN_PROGRESS. Source prepared; application validation and publication pending.

## Scope and provenance

Assigned existing worktree: `qa-contract-coverage`, branch `codex/mcr-be-AGE-GW-03`.
Fresh published base: `e397833c0d8c5654974b3d6413a1753d71072636`.
Root explicitly transferred sole writing of these four paths and stopped writing them:

- `backend/services/api-gateway/src/adapters/in/http/concert_product/mod.rs`
- `backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs`
- `backend/services/api-gateway/tests/concert_age_routes.rs` (new)
- `docs/mcr/backend/gateway-age-routes.md` (new)

The ten published Intro/Notification routes remain mounted. Their existing tests,
root router, manifests, lockfile, Product protocol/client, Auth/Profile readers,
provider configuration and SQL are unchanged. No frontend or source archive edits.

Frozen C04 inputs at the base:

| Input | SHA-256 |
|---|---|
| `contracts/domain/v1/operations.json` | `f801d703dfc5509bbe06a2f0fa07a170b6442a48b2204c7ead460fc9a3600112` |
| `contracts/domain/v1/social.schema.json` | `b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2` |

The closed Age body admission is newly authored in the Gateway adapter against
those schemas and B91's `product_rpc/age_admission.rs` at qualified source
`547eef93` (original `a2a5940b`, final evidence-only `1f87d376`). The B91 commits
were not ancestors of this base; they were read via Git objects for compatibility,
not copied or changed. Root owns their separate normal publication. Gateway
imports no Concert implementation; existing serde/serde_json/uuid dependencies
suffice. Product wire and response delivery interfaces are unchanged.

## Behavior

| Method and event-relative path | Frozen operation | Mutation receipt |
|---|---|---|
| `GET /me/age-status` | `getConcertAgeStatus` | No |
| `POST /me/age-verifications` | `startConcertAgeVerification` | Yes |
| `POST /me/age-verifications/callback` | `completeConcertAgeVerification` | Yes |

All paths require a canonical nonnil lowercase hyphenated event UUID and actual
bearer identity. No query, including an empty `?`, is admitted. Unknown suffixes
stay closed 404; incorrect methods and malformed mounted paths stay closed.
No erasure or other Age operation is introduced.

The two POSTs require one exact allowed Origin, one `x-mcr-intent: browser`, one
canonical nonnil `Idempotency-Key`, exact `application/json`, and at most 8192 raw
body bytes. GET requires no body, content type or idempotency key.

Before Product dispatch, POST JSON must be an object with exactly the frozen
fields. Start accepts only the string `return_target: AGE_PRIVACY`. Callback
requires a canonical nonnil verification UUID, nonempty state token of at most
512 Unicode characters and provider response token of at most 4096 characters;
control characters are rejected consistently with B91. Unknown, missing, null,
wrongly typed, duplicate and escaped-alias duplicate fields, arrays, malformed
UTF-8 and trailing JSON are rejected. Parsing never serializes or normalizes the
original body or signed target. These inputs grant no verified status or proof
trust; the canonical Root owner remains authoritative.

Existing dispatch still reads actual Auth/Profile identity, signs through the
Product client, preserves the original Root envelope/body, and uses the existing
response delivery capability and terminal EOF settlement. Private/no-store error
protection also covers the Age namespace. Provider/runtime unavailability is not
replaced with success, fake claims, seed data or a bypass.

## Validation and limitations

`git diff --check`: exit 0 during preparation. Cargo, rustfmt, native tests and
shared target access: NOT_RUN pending a fresh named 21d OFFER and explicit ACCEPT.
The new boundary suite reuses the existing lazy isolated test pool and
unconfigured Product client. A well-formed authenticated request returning 503
`DEPENDENCY_UNAVAILABLE` proves transport admission only. It does not establish
live Root/SQL/provider dispatch or verified age. Retained Intro/Notification,
Product HTTP and delivery EOF tests are planned in the same bounded round.

B91's 21 qualified RPC cases are upstream evidence, not fresh Gateway results.
Actual provider positive verification, production configuration/listener
activation, database acceptance, full IN03, multiuser/device/load and release
acceptance remain NOT_RUN by this patch. `AgeRuntimeUnavailable` remains the
truthful runtime condition until the owning integration qualifies availability.
