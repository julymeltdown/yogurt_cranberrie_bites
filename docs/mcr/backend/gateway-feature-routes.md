# BE-GW-IN03: public Intro and Notification admission

Status: CONTRACT_READY for the bounded Gateway admission unit. All selected
checks passed on the repair revision. Normal publication and the owner readback
remain required before goal completion. This unit does not close BE33 or real IN03.

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

Executed checks: scoped Rust formatting; locked offline api-gateway library and
the concert_feature_routes, concert_product_http and product_delivery_http test
binaries; filtered concert_product library tests and the three complete selected
integration binaries. Compile and executable phases require separate staged
target release within the allocator's fresh accepted round. No PostgreSQL,
production service executable, install or new harness is requested.

Validation commands, exit codes and source pins are recorded below. Root is
the designated publisher and integrates serially; the implementation owner
checks actual published ancestry and exact postimages after publication.

## First validation round and repair

Allocation BE-GW-IN03-BACKEND-202611041037Z tested formatted source
`eb266909f6dd8d23d8f2e55eb3ecfee530b49d63`. Scoped formatting and the selected
locked offline Rust 1.88 compilation exited 0. The six filtered Product library
tests, four retained Product HTTP tests and four retained EOF tests passed. The
new HTTP binary passed six tests and failed its unknown-suffix test (exit 101):
an unmounted suffix returned 503 instead of 404. This failure is preserved, not
counted as a pass. The shared target was unconditionally released before all
copied runs; the whole round was then fully released with zero private handles
or test groups and the four closed executable copies removed.

Repair: explicitly close unmounted feature namespaces with C04 404 before the
shared fallback, using Axum's actual MatchedPath presence. Mounted routes still
retain the existing authentication, metadata, body and producer boundaries.
The failing assertion now records the method and target. Repair validation was initially
NOT_RUN pending a fresh allocation. The first round does not qualify the repair;
the independent fresh repair-round results follow below.

First-round logs, Cargo JSON, source tree/diff, runtime/fixture pins, executable
hash manifests, accounting and release receipts are retained at
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-gw-in03-azs_rgop`.
Planning-only check `PYTHONDONTWRITEBYTECODE=1 python3 scripts/mcr/check-plan.py`
passed with exit 0; it independently verified original source provenance and
planning exports, not application behavior.


## Successful repair validation and bounded handoff

Allocation `BE-GW-IN03-REPAIR-BACKEND-202611041106Z` was explicitly accepted
with fresh 23,446,286,336 bytes free, zero compiler processes and the unchanged
four-path grant. The tested, clean formatted revision is
`6bf0d91c087a9aad0a06431be3daf5c181fe2250`, descending from the source baseline
through `2b771be0`, `eb266909`, `2eda7f3b` and `6bf0d91c`. The final handoff adds
only this evidence document; the three Rust postimages remain identical to the
tested revision.

Rust is `1.88.0 (6b00bc388 2025-06-23)`, host `aarch64-apple-darwin`.
Commands ran from the worktree's `backend/` directory. Scoped formatting and
its `--check` repeat used `rustup run 1.88.0 rustfmt --edition 2021 --config
skip_children=true` on exactly the three granted Rust paths; both exited 0.

```text
CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
CARGO_PROFILE_DEV_PACKAGE_CONTRACTS_BUILD_OPT_LEVEL=2
cargo +1.88.0 test -p api-gateway --lib --test concert_feature_routes --test concert_product_http --test product_delivery_http --no-run --locked --offline -j2 --message-format=json
```

The exact argv and environment are also preserved in `build.result.json` and
`environment.json`. Compilation exited 0 in 17 seconds. Four selected test
executables were copied, byte-hash verified, and pinned with Cargo JSON,
full Git tree/diff, source commit, runtime links and fixture/lock hashes outside
the target. The ordinary Gateway executable was recorded as automatic Cargo
metadata only; it was never explicitly built, copied or executed.

The shared target was unconditionally released with zero open handles and
zero own compilation groups **before** any copied test or loopback listener.
Each copied executable then ran sequentially from the same frozen source
worktree with `--test-threads=1`:

| Executable / filter | Exit code | Actual result |
|---|---:|---|
| api_gateway, filter `adapters::r#in::http::concert_product` | 0 | 6 passed, 135 unrelated tests filtered |
| concert_feature_routes | 0 | 7 passed, including the previous unknown-suffix failure |
| concert_product_http | 0 | 4 passed |
| product_delivery_http | 0 | 4 passed |

All 21 executed tests passed, with no ignored tests in these runs. The two HTTP
binaries each include the retained shared-key harness regression, so two of the
21 are harness checks. This remains HTTP/loopback fixture evidence, with the
production and real IN03 limitations stated above. No additional PostgreSQL,
service startup, multiuser, provider or frontend test was run or claimed.

Private repair-round evidence:
`/var/folders/j4/41h_5mjj7j9d8f2j2bzcsngw0000gn/T/mcr-gw-in03-repair-n8w1qaft`.
It includes per-command stdout/stderr/results, `test-results.json`,
`executables.json`, source/fixture/runtime pins, accounting and both release
receipts. The earlier failing round remains separately preserved. The repair
round fully released all resources after fresh zero own handles, process groups
and listeners; private TMP was empty and private Python cache was removed.
The four closed executable copies were hash-verified and removed; their hash
manifests, exact source and all pass/failure logs remain. No shared target was
accessed after TARGETRELEASE and no old-output cleanup was performed.

The four copies totaled 121,436,224 bytes, below 200 MiB. Peak sampled whole
accounting was approximately 290.4 MB, including the prior round's 166,457,344
bytes of declared growth/proof carry, source, new target growth, copies, logs,
pins and TMP, below the 2 GiB cap. Final new target growth was 430,080 bytes;
final whole accounting after cleanup was 168,861,751 bytes. The 8 GiB stop floor
was maintained throughout. The exact absolute time and disk samples are in the
receipts; the original accepted clock was never renewed.

Tested Rust postimages (SHA-256):

| Path | SHA-256 |
|---|---|
| backend/services/api-gateway/src/adapters/in/http/concert_product/mod.rs | 3fb291886755613f0d06fedcb2074349dad2eebf4179f14d080cc637b06226b4 |
| backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs | 7269e10eecbc1e279b64f5c20a205aa10cda2b4c7850bfd53a5ffb21312e2429 |
| backend/services/api-gateway/tests/concert_feature_routes.rs | 80e3c55f072c4f404b88e621a490a7d79ec1f5b45dc403fb0308fe3ada810ec9 |

`git diff --check` exited 0. The owner confirmed that the complete changed-path
set equals the four-path grant, with no cross-lane imports and no changes under
`docs/source/`. Root receives the exact base, ordered commit range, all four
postimage hashes and these validation limits directly. Publication is routing,
not a king/SL technical approval stage. Actual remote ancestry and postimages
must be checked before the assigned goal is complete.
