# AGE-GW-03 — public Age transport admission

Status: CONTRACT_READY. The bounded Gateway transport qualification passed.
Normal Root publication and independent remote readback are tracked by the handoff;
this does not qualify production Age/provider activation.

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

Source implementation commit: `1111851ba86e20731d7082b6e4464ff6369201ed`.
Formatting and exact tested source: `0f89b3171ea7205f6487c3162796e0df83f3f8c2`.
This evidence-only handoff adds no Rust changes after that tested source.

Fresh allocation: `AGE-GW-03-MEDIA-202611041154Z`; actual ACCEPT
2026-11-04 11:55:34.425484 UTC. Compilation/freeze deadline 12:15:34 UTC,
copied-run deadline 12:22:34 UTC (also at most 420 seconds from run start),
full release deadline 12:25:34 UTC with at least 180 seconds reserved for cleanup.
Only the named existing MEDIA target was accessed. The existing backend target
was not accessed in this round. Rust 1.88.0, locked/offline, two build jobs,
debug info/incremental off, both Rust flag variables unset, and the allowed
contracts-build development opt-level 2 were retained. No metadata repair needed.

| Command / selected executable | Exit | Result |
|---|---|---|
| `rustup run 1.88.0 rustc --version --verbose` | 0 | Toolchain pinned |
| `rustup run 1.88.0 rustfmt --edition 2021 --config skip_children=true` on exactly the three assigned Rust paths, then the same command with `--check` | 0 / 0 | Formatting passed |
| `cargo +1.88.0 test -p api-gateway --lib --test concert_age_routes --test concert_feature_routes --test concert_product_http --test product_delivery_http --no-run --locked --offline -j2 --message-format=json` | 0 | Build completed in 35.55 seconds |
| Copied `api_gateway adapters::r#in::http::concert_product --test-threads=1` | 0 | 6 passed; frozen mapping now covers exactly 13 routes |
| Copied `concert_age_routes --test-threads=1` | 0 | 9 passed, including 8 Age boundary cases and the imported harness key test |
| Copied `concert_feature_routes --test-threads=1` | 0 | 7 passed; ten Intro/Notification routes retained |
| Copied `concert_product_http --test-threads=1` | 0 | 4 passed |
| Copied `product_delivery_http --test-threads=1` | 0 | 4 passed; actual body EOF/drop/failed settlement behavior |
| `git diff --check` | 0 | Bounded patch clean |
| `PYTHONDONTWRITEBYTECODE=1 python3 scripts/mcr/check-plan.py` | 0 | Planning-only PASS; no application acceptance claim |

**30 executed test cases passed**; this includes three imported harness-key
cases across the integration suites. Five selected executables, Cargo JSON,
full source/tree/diff, environment, dynamic-library links, and contract/fixture/
lockfile pins were frozen outside the target. Target release at 11:56:57.850 UTC
recorded zero target handles/compilers, before the first copied case at
11:57:02.876 UTC. There was no target access afterward. Tests ran sequentially;
only the retained EOF suite used its owned ephemeral loopback listener.

Full release at 11:57:44.771 UTC recorded zero owned processes/groups/listeners,
private TMP entries and remaining executable copies. The five copies totaled
154,457,536 bytes (below 200 MiB) and were hash-verified before deletion. New
positive target growth was 243,900,416 bytes. Conservative 209,715,200-byte carry
explicitly includes the prior completed Gateway rounds' 164,950,016-byte positive
growth and 3,805,184 bytes of retained provenance plus source/headroom; history
was not reset. Peak whole accounting was approximately 610.2 million bytes,
well below 2 GiB. Free disk remained above the continuous 8 GiB floor; fresh
start was 19,841,806,336 bytes, exceeding 16 GiB. Retained proof is approximately
1.98 MB, executable copies removed. No PostgreSQL, provider, ordinary Gateway
binary, external services, installation, new worktree or cache cleanup was run.

Private retained evidence directory:
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-age-gw03-buod34pi/`.
It includes `accept-preflight.json`, `lease.json`, source/fixture pins, scoped
command results, Cargo JSON, `executables.json`, runtime links,
`target-release.json`, `test-results.json`, accounting logs and
`full-release.json`. Executable paths in metadata are historical and deleted.
The new boundary suite reuses the existing lazy isolated test pool and
unconfigured Product client. A well-formed authenticated request returning 503
`DEPENDENCY_UNAVAILABLE` proves transport admission only. It does not establish
live Root/SQL/provider dispatch or verified age. Retained Intro/Notification,
Product HTTP and delivery EOF suites passed in this same bounded round.

B91's 21 qualified RPC cases are upstream evidence, not fresh Gateway results.
Actual provider positive verification, production configuration/listener
activation, database acceptance, full IN03, multiuser/device/load and release
acceptance remain NOT_RUN by this patch. `AgeRuntimeUnavailable` remains the
truthful runtime condition until the owning integration qualifies availability.
