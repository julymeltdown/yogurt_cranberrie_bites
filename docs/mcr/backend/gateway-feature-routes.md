# BE-GW-IN03: public Intro and Notification admission

Status: IN_PROGRESS. Application checks are NOT_RUN pending a fresh named
resource allocation. This unit does not close BE33 or the real IN03 gate.

Source baseline: published `145a8e939fb7bede7e46931d6559adb2c51f62a7`.
The existing `qa-contract-coverage` worktree is reused on `codex/mcr-be-GW-IN03`.
Root transferred exactly the following four paths and remains the publisher:

- `backend/services/api-gateway/src/adapters/in/http/concert_product/mod.rs`
- `backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs`
- `backend/services/api-gateway/tests/concert_feature_routes.rs`
- `docs/mcr/backend/gateway-feature-routes.md`

All implementation is newly authored Gateway adapter code over the retained
port-rust/MCR HTTP, Auth, Product RPC and body-completion boundaries. There is
no copied legacy rendering, new dependency, migration, contract edit or frontend
import. Other workers' provider/composition paths are outside this grant.

## Frozen route set

Let `E = /api/concert/events/{eventId}`. IDs are canonical non-nil UUIDs.

| Operation | Method and path | Query/body | Concert receipt |
|---|---|---|---|
| listConcertIntros | GET E/intros | limit, cursor, q | No |
| getConcertIntro | GET E/intros/{introId} | No query/body | No |
| getOwnConcertIntro | GET E/me/intro | No query/body | No |
| setConcertIntro | PUT E/me/intro | social.SetIntro; at most 16 KiB | Yes |
| publishConcertIntro | POST E/me/intro/publish | common.RevisionRequest; at most 16 KiB | Yes |
| closeConcertIntro | POST E/me/intro/close | common.RevisionRequest; at most 16 KiB | Yes |
| listConcertNotifications | GET E/me/notifications | limit, cursor, unread_only | No |
| getConcertNotificationUnreadCount | GET E/me/notifications/unread-count | No query/body | No |
| markConcertNotification | PATCH E/me/notifications/{notificationId}/read | common.RevisionRequest; at most 8 KiB | Yes |
| markAllConcertNotifications | PATCH E/me/notifications/read-all | common.EmptyRequest object; at most 8 KiB | Yes |

The outer HTTP transport recognizes only these exact mutation method/path pairs
as Concert-owned receipts, bypassing the inherited Gateway replay journal.
PATCH now receives the same browser Origin, intent, exact JSON Content-Type and
single canonical Idempotency-Key checks as existing Product mutations. Reads
reject bodies, Content-Type and Idempotency-Key. Wrong methods, malformed IDs
and arbitrary suffixes do not acquire receipt ownership.

Targets are bounded to 4096 bytes. Only the two list routes accept query fields;
the decoder rejects duplicate decoded names, unknown fields, invalid UTF-8,
bad percent escapes, control characters, empty queries and invalid bounds.
`limit` is 1–100, cursor is 1–1024 base64url characters, `q` is 1–100 Unicode
characters, and `unread_only` is exactly `true` or `false`. Original target/body
bytes and the key are forwarded unchanged. Root retains typed body validation,
receipt replay/conflict rules, expiry and all authorization decisions.

The routes reuse the live `dispatch` handler: JWT access context, current
Auth/Profile composition, signed canonical actor delegation, Product client,
Root response capability, disclosure deadline and outer retained-scope EOF
completion remain in that path. Browser cookies, user-ID headers, query fields
and JSON bodies do not supply canonical identity. Every response, including
outer errors, receives no-store/no-cache/nosniff protection with no validators.
No completion capability is returned to the browser.

## Age gap and readiness limits

At the source baseline, C04 defines getConcertAgeStatus,
startConcertAgeVerification and completeConcertAgeVerification. The Age
provider runtime and native resolver recognize their exact paths. However,
`concert-service/src/product_rpc/mod.rs` has no Age `Operation` variant or
admission/dispatch mapping. A browser route alone would lead to unsupported
Product dispatch. These three routes are deliberately not added by this unit;
Root owns the missing Product operation/producer composition. No schema or
provider fallback is invented. Sharing-offer is outside the six Intro operations.

The ten-route tests use the existing RouteHarness with unconfigured producers
and a lazy unavailable database; reaching DEPENDENCY_UNAVAILABLE is evidence
of public HTTP admission only. They do not prove successful Intro persistence,
Notification delivery, target authority, actual Auth/Friend/PG exclusion, mTLS
production policy or a multiuser privacy flow. Those owners and real IN03 remain
separate obligations. The retained Product delivery tests use a loopback double
to prove body-EOF ordering; they cannot certify actual Root settlement.

## Pinned inputs and regression plan

All paths below are relative to the repository, at the source baseline.

| Source/contract | SHA-256 |
|---|---|
| contracts/domain/v1/operations.json | f801d703dfc5509bbe06a2f0fa07a170b6442a48b2204c7ead460fc9a3600112 |
| contracts/domain/v1/social.schema.json | b275b36f2ad8eee90922f7ad27cfcbece943773a94831720b6dfca588d0201d2 |
| contracts/domain/v1/feed.schema.json | f9b4b8912b2bf4513ab474ad690efdeaa23d64f97b220f9626c5b37c5e2e99ed |
| contracts/domain/v1/common.schema.json | 65f2c2f19594942b89613e67b97bd3181e0889e7f47e4476cda752d9aab10584 |
| backend/services/concert-service/src/product_rpc/feature_admission.rs | 85aa1cbee0ebdf0fa771235ac8d1afb296bcbde76b7705d16b12a41919fcc8c7 |
| backend/services/concert-service/src/intro_live/protocol.rs | c76a210656c6ef6bf7c30091eae9c531eceb32307f9b17cfb3c22b7aa8a9e06e |
| backend/services/concert-service/src/notification_live/protocol.rs | cbe7e9499f7e896c78592bd959b9d45b3a163dbe64e01052f5714c76b74bfb36 |
| backend/services/api-gateway/tests/concert_product_http.rs | 1fa37ae4553bd0e571d0570ca39e1ec1d88054307d479ede39fb694aa0af06c8 |
| backend/services/api-gateway/tests/product_delivery_http.rs | 93fefcceac0f5ba25f2cd61f997ff211ba7536fc0fe7a7f888197f6a6c8b1b84 |

Requested checks: scoped Rust formatting; locked offline api-gateway library and
the concert_feature_routes, concert_product_http and product_delivery_http test
binaries; filtered concert_product library tests and the three complete selected
integration binaries. Compile and executable phases require separate staged
target release within the allocator's fresh accepted round. No PostgreSQL,
production service executable, install or new harness is requested.

Validation commands, exit codes, source pins, limitations and committed handoff
will be recorded here after execution. The publisher integrates serially and
the implementation owner verifies actual published ancestry and postimages.
