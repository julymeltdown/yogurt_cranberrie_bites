# BE39 moderation runtime

Status: IN_PROGRESS. The bounded command engine and closed C04 adapter are authored. Compilation, native qualification and publication evidence are recorded below when completed. Whole BE39 cross-owner live behavior remains NOT_RUN until actual providers and Root routing are installed and exercised.

## Scope and provenance

Task base: published master `980fc4d85020f297b36fee8195e1ca92079ff33f`. Worktree branch: `codex/mcr-be-moderation-BE39`. Product requirements are the BE39 row of `MCR_IMPLEMENTATION_PLAN_V2_EN.md`, interpreted with the frozen C04 operations, admin schemas and event-local moderation policy in `contracts/domain/v1/`. Assessment/execution documents provide implementation context; they do not change these contracts.

The new application vocabulary is framework-free. Persistence lives in the new audit repository and migration `0043_moderation_runtime.sql`. The only shared edits are `pub mod moderation`, the five Action mappings/permissions in application admin, `pub mod audit` in the actual `repository.rs`, and `pub mod moderation_live` in service lib. Existing migrations, contracts, manifests, locks, Root routes and owner stores are unchanged. The publisher must preserve Wall's four pending Action arms when integrating shared admin changes.

Read-only provenance:

- Retained BE15 `commands`/`repository::receipt` supplies registered operations, canonical server-HMAC fingerprints, key rotation, authorization-before-replay, fixed 86400-second expiry and tombstones.
- Retained BE38 `event_operations` supplies the fenced-role ABI, transaction deadline pinning and legacy audit source. No positive event-role provider is inferred from that ABI.
- Operations owner unpublished `8f9197b9d96656a55bb75d7a8f1c9b1642881437` informed the ProductRequest binding structure. Current source tracing corrected its payload-only digest pattern: the actual sealed parent hashes the complete admitted uncompressed gRPC frame (flag, length and protobuf message). Its subsequent `2f00616e4e11e66d364224d4f1a99092aac36da2` correction establishes same-connection role/evidence interfaces. Those private modules are not dependencies or qualification evidence.
- Retained canonical `OwnerGuardScope`, `GuardedAuthority`, `ProducerTransaction`, and `ScopedProductProducer` supply the actual Root guard/producer interfaces. No second scope is acquired or completed here.
- Native harness ownership checks follow retained `tests/event_operations.rs`; Python reuses `migration_smoke.Cluster` at the task base. Marker/token, UID, inode, private socket, exact database and server data-directory checks precede native mutations. No SQLx ledger rows are manufactured.

## Closed public operations

| Operation | HTTP surface | Role |
| --- | --- | --- |
| `moderateConcertUser` | POST `/api/concert/events/{eventId}/admin/users/{userId}/moderation` | Moderator or Operator |
| `moderateConcertContent` | POST `/api/concert/events/{eventId}/admin/content/moderation` | Moderator or Operator |
| `listConcertAdminUsers` | GET `/api/concert/events/{eventId}/admin/users` | Moderator or Operator |
| `listConcertAdminContent` | GET `/api/concert/events/{eventId}/admin/content` | Moderator or Operator |
| `listConcertAudit` | GET `/api/concert/events/{eventId}/admin/audit` | Operator only |

There is no frozen `getConcertAdminUser` operation. Unsupported routes, query keys, duplicate fields/filters, missing required `expires_at`, unsupported actions/types, noncanonical IDs and malformed queries fail closed. Commands require an exact JSON media type, canonical UUID idempotency key, current expected revision and a nonblank reason of at most 500 characters. Parsed UTC timestamps and query filters are normalized. Replies are validated against the existing generated closed schemas, including public-profile identity and content target type.

## Canonical authority and provider ABI

`ModerationRuntime::new(pool, owners, roles, fingerprint_keys, cursor_key)` implements the existing `ScopedProductProducer`. Its public dispatch accepts the already-held `Arc<OwnerGuardScope>` and original `ProductRequest`. It verifies actor, event, session family, request ID, operation, method, target, HTTP body digest, SHA256 of the exact uncompressed unary gRPC frame containing the full prost ProductRequest and all original deadlines/observation bounds. A detached or altered frame cannot reuse the scope.

`EventRoleProvider::acquire_for_root_terminal(scope)` must acquire the real event-role owner's revision/revocation/disclosure fence and retain it in Root's lifecycle through the actual HTTP terminal, including cancellation and lost responses. Merely returning an `Arc` to this dispatcher is insufficient. `EventRoleFence` binds the canonical Root digest, supplies same-connection `check_in` and target-role checks, and checks disclosure. Headers, JWT role claims, membership, bootstrap or profile fields never supply privileged roles. `roles: None` returns affected-operation unavailable before database/owner effects. This patch intentionally supplies no fabricated production role provider.

Each protected Concert transaction registers its real native identity through `scope.register_transaction_in` **before** `GuardedAuthority.check_in`, on that same PostgreSQL connection. It checks canonical actor safety, ACTIVE event admission and the actual injected event-role lease, clips it to the original scope ceiling and pins deferred SQL deadlines. User moderation additionally checks a fresh same-event target-role lease. A Moderator cannot change an Operator peer. All receipt lookups and repeated responses reauthorize current actor, event and target.

`ModerationOwners` contains separate optional event-restriction, event-world-termination, event-private-authority-revocation, Post, party, giveaway, intro and directory ports. Missing required command owners reject a new reservation; missing directory does not pretend an empty collection. Real adapters must use `ModerationAuthority::canonical_scope()` and the actual owning guards/transactions. `None` is supported only for internal synthetic tests, and must be rejected by production adapters.

Each owner receives a stable `OwnerIntent` binding original command receipt, actor/event, operation, target, step and expected revision. It must enforce current eligibility, phase, revision/CAS, owner idempotency and permission. APPLIED requires an exact matching intent, durable nonnil owner receipt, valid revision and the actual `ProducerTransaction` identity with the same effect digest. An identity is captured from the owning protected transaction, never invented from a counter/PID guess or copied from another producer. Post effects require the Post producer kind. No adapter here directly mutates peer databases.

Root composition, the actual role-owner schema/provider, actual owner adapters and Root operation registration are designated publisher work. The published base does not yet route these five operations through this producer. A successful local fixture cannot establish that admission, owner revocation and final HTTP disclosure work together.

## Event-local effects

SUSPEND requires three confirmed owner steps: Concert restriction/admission epoch change, event World/session termination, and event-derived private-authority revocation. The owning adapters must invalidate event membership/sockets/tickets/private grants under the current policy. Global Auth account state and refresh families, as well as other events, remain unchanged under frozen C04.

UNSUSPEND calls only the restriction owner and requires fresh authority. It cannot restore old sessions, tickets, consent or private grants. Content HIDE/RESTORE routes POST/COMMENT to Post and PARTY/GIVEAWAY/INTRO to their actual owners. Restore eligibility, deleted-target denial and target CAS are enforced by those owners. No saved response body or local flag simulates a remote effect.

## Durable command and audit state

A new BE15 receipt and immutable moderation command/PENDING audit are committed before owner calls. Owner calls run outside the Concert transaction. Each step first appends a durable PENDING intent. Confirmed APPLIED or safe FAILED results append once; prior rows never change. There is one terminal result per step and one terminal audit per command. Migration guards enforce receipt/actor/event/target/operation binding, valid step sequences, intent correlation, append-only rows, required owner proof columns and commit-time authority deadlines.

A timeout, lost acknowledgment, malformed/mismatched proof or uncertain downstream result never creates APPLIED or SUCCEEDED. The durable intent stays PENDING. Partial APPLIED effects remain visible internally if a later owner safely fails; the command becomes FAILED and never receives a success audit. SUCCEEDED requires all required steps APPLIED, then atomically updates the original BE15 receipt, appends the existing outbox invalidation and immutable success audit. Outbox capacity admission uses the existing global command-budget lock. Private reasons, input, owner receipts and native identities stay outside audit read DTOs.

Same-key submit verifies the original server HMAC and returns the original currently authorized receipt without another owner mutation, including while PENDING. A conflicting payload returns idempotency conflict. Unknown keys cannot create work through `resume`. Explicit internal resume first requires the original receipt, verifies HMAC/input and current authority, then resolves each durable native owner identity/status. An existing uncertain intent calls the owner's `recover`, which may resume only under that owner's identical-intent policy; missing status is not evidence of rollback. Expired keys remain tombstoned and cannot execute again.

Recovery still depends on actual owner adapters and an authorized server continuation entrypoint. No public recovery route, automatic retry loop, detached background task or global authorization bypass is introduced. Self-suspension or revocation after an owner effect can leave an intent PENDING because terminal persistence must also reauthorize; qualified reconciliation is a separate live-flow requirement, not grounds to infer success.

## Read boundaries

Audit unifies moderation audits with retained BE38 audit rows that have receipt IDs. Its DTO contains only audit/event/actor/operation/target/outcome/receipt/time. It exposes neither moderation reasons nor report evidence, private content, handles or owner identities. List users/content rely on an actual authorized owner directory and reject malformed, misordered or cross-profile results. Every provider must reauthorize each returned row and obey immutable snapshot bounds.

Pagination is bounded to 1..100 rows, default20. AES-256-GCM tokens use a configured nonzero server key and random nonce, authenticated actor/event/operation/collection/normalized filters/limit, fixed snapshot upper bound and a 15-minute expiry. The unpadded base64url codec rejects noncanonical encodings. The cursor stores position only inside authenticated ciphertext. User order is user ID ascending; content order is type then ID ascending; audit order is creation time then audit ID descending. Invalid or cross-bound cursors return invalid input.

## Validation and reproducibility

Before any Cargo/target/PG access, obtain a fresh named target offer from the backend resource owner and explicitly ACCEPT that exact offer. These commands are templates until a concrete accepted lease is recorded:

```text
cargo test --locked -p concert-application --lib admin::moderation
cargo test --locked -p concert-service --lib moderation_live
cargo test --locked -p concert-service --test moderation_live --no-run
python3 backend/scripts/mcr/moderation_live_smoke.py \
  --test-binary <absolute accepted-target/debug/deps/moderation_live-binary> \
  --accepted-target <absolute freshly accepted target> \
  --accepted-lease-id <fresh accepted lease identity> \
  --pg-bin /opt/homebrew/opt/postgresql@16/bin \
  --report <new absolute owned evidence file>
```

The script creates a new private PG16 cluster with no TCP listener and no inherited database/provider credentials. It applies genuine published SQLx baseline migrations, injects a late 0043 DDL collision to prove atomic rollback, migrates/reapplies successfully, runs the nine native tests serially, restarts the owned postmaster and verifies ledger/count/native-identity persistence. Cleanup verifies ownership before stopping/removing only its own cluster. Diagnostics omit passwords and database URLs; the report identifies synthetic owner/role scope explicitly.

Eight native ledger cases cover successful aggregate completion/replay/HMAC conflict/key rotation/redacted audit; stale/cross-event/lower-role/Operator-peer/revoked-replay denial; committed-effect/lost-ACK recovery with exact identity reuse; partial failure without success audit; unavailable/unknown/mismatched owner results; deleted-restore safe denial and restriction-only unsuspend dispatch; expired tombstones and immutable rows; deferred authority expiry rollback. A ninth case captures a genuinely signature-verified Gateway parent and rejects modified full frames without fabricating an OwnerGuardScope. Wire/cursor/controller tests additionally check closed fields/routes, malformed queries, reason requirements, encrypted cursor binding/expiry, collection order, cross-profile results and audit snapshot/event checks.

Synthetic native fixtures do not qualify actual Concert admission changes, World closure, private-authority invalidation, Post CAS/eligibility, Auth/Friend disclosure, production role revocation, real directory authorization or Root HTTP EOF. Those remain NOT_RUN until actual cross-owner composition is exercised. No capacity, mobile or full BE39 acceptance claim follows from these checks.

### Execution evidence

- Source-level Python syntax: PASS (two new files; no cluster started).
- Explicit Rust 1.95.0 rustfmt on owned new Rust sources: PASS. The global stable shim failed because its `librustc_driver` is missing; no toolchain installation or default change was performed.
- Cargo/native: NOT_RUN pending fresh accepted target allocation.
- Publication: NOT_RUN.

### Frozen input digests

- `contracts/domain/v1/operations.json` SHA256 `622c0e0eba6e1c835c5e953a3acd6825f97ed43aff9bc32b982c207fdfe31839`.
- `contracts/domain/v1/admin.schema.json` SHA256 `953e78a482d998e35c9612c30467600adcead3c9e9d4fb6ab7679a2164aef640`.
- `contracts/domain/v1/policy.json` SHA256 `81b0da4c007543765f120c4ea58163cc1eadea41e10d24db54e1d177ef4e5fbb`.
- `contracts/generated/mcr-domain.openapi.json` SHA256 `79a066b88119e53f6f16876d7e3fe59a18ee956be6ed6d2af7ee1efa187e0feb`.

- Source correction after checkpoint `db360d67`: traced `VerifiedParentAuthority::request_digest` and the admitting interceptor; compare SHA256 of flag + big-endian message length + full prost message, and require the ProductService Dispatch parent method. The signature-verified parent test covers the actual frame boundary. Execution remains pending the fresh target lease.
