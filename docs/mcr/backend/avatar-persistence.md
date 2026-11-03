# BE07 avatar and preference persistence

Status: **IN_PROGRESS**. Implementation and focused native validation are in
progress. This is not BE07/IN01 acceptance, a live HTTP producer, or artwork
qualification.

Base: `1db1680e6a5041250b79ca0ffb1dfd965a279edc`, isolated branch
`codex/mcr-be-avatar-persistence`. Contract sources: C03
`contracts/core/v1/{core.schema.json,operations.json}` and C06
`contracts/assets/v1/{avatar-config,avatar-manifest,common}.schema.json` at this
base. The specification's BE07 card and sections 5.2–5.3 remain authoritative.


Pinned contract SHA-256 values:

| Source | SHA-256 |
|---|---|
| C03 core.schema.json | `9ddf3cd3c853ef6a8258a6215c546bd84eccde620ee73b636888d868cfca1c88` |
| C03 operations.json | `2f5b972dbccea09340e1519a1a2c522e3c42f63690fa26b9e58d721e9927ba62` |
| C06 avatar-config.schema.json | `37b9be22cff7cbde882ab144aec3b75b5edc24f70653f97c9dcec14e87c652bd` |
| C06 avatar-manifest.schema.json | `cb98594db269ef017545806b368d5edd1d0a47b53819809392f8ac57cf30da98` |
| C06 common.schema.json | `b7be87ce387dd286bcd2ea895da04f1a8b886c1ea3dc35494f761ac05aad75b4` |

## Behavior and integration API

Newly authored domain code under `backend/crates/concert-domain/src/avatar/`
parses exactly `SetAvatarRequest {expected_revision,config}`. Inputs have a
4096-byte bound; revisions are positive JavaScript-safe integers. Duplicate or
unknown properties, missing nullable accessory, URLs, unsupported contract tags,
noninteger revisions and invalid IDs are refused. The immutable C06 inventory
checks exact manifest/version, item layer and skin tone. C06 v1 has no additional
pairwise-combination/entitlement fields; no invented combination or source-sheet
inventory is added. Compatibility means the five frozen slot categories and
shared manifest. Additional restrictions require a reviewed contract change.

`AvatarPersistence` in
`backend/services/concert-service/src/adapters/out/repository/avatar.rs` exposes:

- `from_product(PgPool, Arc<ValidatedProductRelease>)`: projects the existing
  validated release, recomputes avatar content SHA-256 using C06 canonical bytes,
  and refuses synthetic/unreviewed/unprovided metadata in product mode.
- `from_synthetic_test(..., true)`: explicitly marked metadata test seam, never a
  product fallback. It uses the same strict inventory and persistence path.
- `read(key, authority) -> AvatarSnapshot {avatar,participant}`: scoped persisted
  state for the producer's canonical Profile join. It creates no defaults.
- `set_avatar(key, raw_bytes, authority) -> JSON AvatarState {revision,config}`:
  a successful update increments only avatar revision, exactly once. The result
  config is exactly the submitted validated selection.
- `set_preferences(key, raw_bytes, authority) -> AvatarSnapshot`: accepts the
  preference-only C03 PATCH subset with all four preference fields. It advances
  only participant revision and never rewrites avatar identity or revision.

Profile `display_name`/`bio` writes remain canonical Profile owner work. This
module must not be installed as an adapter that silently ignores other valid
UpdateParticipantRequest fields. Delivery must explicitly compose/dispatch that
wider operation and construct the complete PrivateParticipant response from
current canonical Profile data; AvatarSnapshot is internal, not a wire DTO.

Application ports live under
`backend/crates/concert-application/src/avatar/`. `AvatarAuthority::check` must
come from a server adapter validating canonical actor, exact user/event/operation
and HTTP target, current release digest, live bounded authority, and current
server mutation capability. It returns a matching AvatarGrant or a refusal.
Missing, mismatched, expired or unavailable authority fails closed. A client's
capability flag, Bootstrap read proof or test grant cannot establish product
write authority. Production authority/root HTTP/RPC wiring belongs to delivery;
this patch exposes no new route and enables no mutation capability. Product
resource byte/rights/selected-source qualification remains the release owner's
obligation; metadata/hash validation does not establish it.

## Transaction and error rules

Every transaction locks `concert_events` then `concert_event_admissions`,
`concert_participants`, and `concert_avatar_settings`, matching BE06. Authority
and current release are checked before work, after every await and immediately
before commit/response. Draft/missing events and missing/suspended admissions
are refused. OPEN/LIVE allow writes; READ_ONLY/ARCHIVED permit scoped reads but
refuse mutations. The current selection and resulting selection must both
validate against the captured release. No unavailable item is silently replaced.

Expected revisions are compared under these locks. Competing identical expected
revisions produce exactly one commit and one increment; losers cannot overwrite
current state. Existing database guards independently enforce immutable scope,
creation time and one-step revision advancement. No migration is needed: BE05's
0001 values and BE06's 0004 admissions are used without rewriting them.

A check failure before commit rolls back the SQL update. A failure after commit
is an uncertain response and the client must reconcile by reading current state,
not replay automatically. The local check does not promise instantaneous remote
revocation beyond the upstream authority's bounded observation semantics.

Producer error mapping uses existing frozen C03 reasons:

| Internal error | HTTP / CoreErrorDetails.reason |
|---|---|
| InvalidInput (including catalog/version/item incompatibility) | 422 / INVALID_INPUT |
| Unauthenticated | 401 / AUTHENTICATION_REQUIRED |
| Denied | 403 / PERMISSION_DENIED |
| Missing | 404 / RESOURCE_UNAVAILABLE |
| Conflict | 409 / VERSION_CONFLICT |
| ReadOnly | 409 / EVENT_READ_ONLY |
| Unavailable | 503 / DEPENDENCY_UNAVAILABLE |
| InvalidState | 500 / INTERNAL_ERROR |

Root transport owns media type/body-limit normalization and safe error envelopes.
No private grant, SQL detail, item list, user identifier or fixture provenance
should be included in errors. FE13 preserves the draft on conflict and explicitly
reads GET /me, then asks for a keep-draft/use-server decision before retrying.

## Ownership, provenance and evidence

All avatar implementation/tests and `avatar_smoke.py` are newly authored.
Existing BE05 values, BE06 validated-release and native fixture helpers, and C06
canonical JSON encoding are dependencies, not copied old rendering/code.
`avatar_smoke.py` imports the existing owned native Cluster and bootstrap test
execution helpers. New support/avatar delegates to the existing bootstrap
marker/inode/socket validation, avoiding a weaker duplicate harness.
No frontend implementation, source art, docs/source archive, contracts,
manifests, lockfiles, config, migrations or RPC roots are modified. Delivery
explicitly granted only the three `pub mod avatar;` exports in domain lib,
application lib and service repository. Only delivery integrates/pushes master.

The native harness requires compiler-artifact JSON from the exact service avatar
test target, creates a private PG16.14 cluster/database via Unix sockets, applies
service schema SQL, executes real adapter tests, restarts its postmaster and
verifies persisted values, then cleans only its marker/inode-owned root. Direct
schema application does not qualify service migration-runner/ledger behavior.

Validation results and final commit ancestry will be recorded here before the
committed handoff. Build phases use only the leased shared target, Rust 1.88.0,
two jobs, debug=0 and `--locked`; no duplicate Cargo target is created.
