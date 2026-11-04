# Shared Feed and Notification target startup

Status: `INTEGRATED` for the actual startup source. The combined build, four copied configuration cases and three actual-main negative startup checks passed under the bounded qualification below. Positive public Gateway, five-owner Feed HTTP, nonempty six-kind Inbox and full original IN03 remain `NOT_RUN`.

## One existing canonical composition

`main.rs` validates Inbox/target and Feed settings before opening the database. Target mode uses the qualified `notification_target_runtime` settings and `TargetReadSettler` installed by the existing canonical composition. It constructs the callback using that exact core registry and pool, then mounts it once with `guard_without_delegation`, the dedicated Notification caller registry, the actual listener TLS and existing `CallDeadlineLayer`. Live Inbox with target off refuses startup. No additional registry, ledger, core, listener or recovery worker is created.

Feed read mode uses the qualified `FeedProductProducer`, existing shared fence, canonical public-profile reader and delivery publisher. Its closed overlay handles only Feed, Post detail and comments; Intro and Inbox continue through their existing owners. The standalone `production_owners` constructor still refuses Feed. Main consumes `public_feed_read` only after validating the shared settings, then uses the qualified overlay. Owner-media remains unavailable and a requested uploader mode still fails.

The actual Post callback is `ConcertRootReadService` on this same Concert listener, guarded by a dedicated Post-only current/previous caller registry and `PostConcertRootReadVerifier`. It uses the same held scope registry and canonical public-profile reader as Feed. The parent actor, original signed frame, exact deadline and actual owner-held guards remain required by the existing callback. A constructed callback is not evidence of successful owner access or HTTP completion.

## Explicit settings

Feed activation requires `CONCERT_PUBLIC_FEED_MODE=read`, the existing guarded bootstrap and shared live Intro/Participant composition, and valid actual mutual TLS. Off mode does not inspect Feed keys or unrelated credentials. Required inputs are:

- `CONCERT_POST_GRPC_ENDPOINT`: explicit Post HTTPS host and port, no user information, query, fragment or non-root path.
- Actual `CONCERT_SERVICE_CREDENTIAL`, `POST_SERVICE_CREDENTIAL` and optional `POST_SERVICE_CREDENTIAL_PREVIOUS`; no parity/default credential. Post credentials must differ from every supplied workload's current and previous credential.
- `CONCERT_POST_READ_SIGNING_SEED` and `POST_CONCERT_ROOT_READ_VERIFYING_KEYS`: the established platform **standard Base64** key format, parsed by the existing dedicated platform constructors. Post's matching read-mode configuration remains independently required.
- `CONCERT_FEED_CURSOR_KEY_HEX`: dedicated nonzero 32-byte lowercase hexadecimal cursor secret. Its key must differ from the outbound signing key; callback verification keys must also be independent from that outbound key and Gateway keys.

Target activation uses the independently documented `CONCERT_NOTIFICATION_INBOX_MODE=live` and `CONCERT_NOTIFICATION_TARGET_MODE=live` settings. Neither flag grants access or creates an identity. Defaults remain off.

## Source and validation provenance

The Feed adapter preserves original history through `1c511f0c` (qualified Rust at `cca1723c`); the four-path source intake and exact test limits are recorded in `feed-product-scope-intake.json`. Provider source `8e2a42a8` and startup source/evidence `f73f84b5` / `2ced9803` are recorded separately in the target intake documents. The three Age Gateway routes were independently qualified by their owner; this increment does not compose an Age producer or activate an external verification provider.

New application paths are `src/config/feed_runtime.rs`, exported under `config::product_runtime::feed`, plus the Root-owned `main.rs` and product-runtime export. `tests/feed_runtime.rs` covers off-mode isolation, required canonical composition/TLS/keys, missing inputs and unsafe endpoints, and independent secrets/rotated workload identities. These configuration cases do not behaviorally qualify live signed owner calls, retained-scope mismatches or outer HTTP EOF.

No contract, SQL migration, selected artwork, frontend source, manifest or lockfile changes are required. All supplied source originals remain unchanged. Actual Social publisher/outbox composition, server expiry, real verification-provider input and public IN03 remain independent unfinished obligations.

## Executed combined qualification

Qualified source is `7c6cf64f1ade1de4100c705f46bb4f39902b0a22`. Offer `ROOT-FEED-TARGET-MAIN-BACKEND-202611041203Z` was actually accepted at 12:07:19.871975 UTC on 2026-11-04. Rust 1.88, locked/offline, two jobs, debug information and incremental compilation disabled, and the existing contracts-build development optimization override were used for exactly two commands:

```text
cargo +1.88.0 test --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --test feed_runtime --no-run --message-format=json
cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 -p concert-service --bin concert-service --message-format=json
```

An initial compiler failure in the new test used `?` on the infallible `TlsMaterial::from_pem` constructor. Its source, Cargo JSON, error and resource receipt were retained. Only that owned test line changed, without resetting the accepted clock, positive target growth or byte caps. The subsequent test build and actual main build passed in 1.016 and 25.051 seconds. No application-source repair, stale-script touch or cache cleanup was needed.

Exactly 3,703 source/runtime/fixture/proto/migration files were copied and SHA256-verified outside the target, allocated 529,530,880 bytes. Exactly two Cargo-JSON-selected executables were copied, totaling 64,173,280 bytes: the Feed test executable and the actual non-test Concert binary. The portable `feed-target-main-build.json` records their hashes and commands; the private copy's historical `-tests` filename suffix does not change the Cargo artifact's actual binary kind.

The target was unconditionally released before all copied execution, with zero handles and absent owned compiler group. The four new configuration tests passed with no failures or ignored cases. The actual main independently refused target-live without Inbox/shared composition, an invalid target mode, and Feed-read without guarded bootstrap. Each exited 1 with the specific expected configuration error before the database connection path. No database, service listener or successful owner call ran. The copied stage took 2.090 seconds. The separately intaken QA05 source increment also passed 55 offline checks in 6.006 seconds; these are source checks, not native acceptance.

Full release at 12:09:59.022032 UTC removed private PKI/TMP and verified zero own handles and absent build/test process groups. Accept-to-full was 159.150 seconds; target-to-full was 15.524 seconds. Conservative final allocation including receipt reserve was 3,321,344,000 bytes against the 4.5-GiB cap; the separately sampled copied-stage maximum was 3,321,352,192 bytes. Final free space was 18,698,293,248 bytes. No target access occurred after target release. Positive Feed, Inbox, public Gateway EOF and original IN03 acceptance remain open.
