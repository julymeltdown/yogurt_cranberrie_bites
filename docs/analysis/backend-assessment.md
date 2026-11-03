# MCR backend assessment

Assessment date: 2026-11-03. Scope: understand the supplied specification and identify implementation prerequisites. This report does not implement application features, import foundations, certify C01, or authorize live-data access. Delivery is **`julymeltdown/mcr`, `master`**, as directed by the user; references to `julymeltdown/bigyeting` in the supplied package are inherited delivery instructions superseded by that direction. Bigyeting remains a proposed source of selected logic.

## Evidence and present conclusion

The proposed architecture is coherent: reuse port-rust's service foundation, add one Concert service for the event domain, and extract selected Bigyeting policies without introducing another identity system. Its main engineering burden is making identity, authorization, durable commands, and realtime projections agree under failures. The plan describes these requirements substantially better than it proves their implementation.

This assessment read the local [implementation plan](../source/MCR_IMPLEMENTATION_PLAN_V2_EN.md), the [portable ledger](../source/mcr_v2_en/PLAN_TABLES_EN.md), the [delivery README](../source/mcr_v2_en/MCR_DELIVERY_README_EN.md), and supplied generated task/validation artifacts. A scoped local search found no application Rust source, Cargo manifest/lockfile, or package manifest at review time. That observation is limited to this workspace snapshot; concurrent integrator work may change it. The external source repositories and their cited files were not independently inspected here. No application build, migration, provider login, database concurrency, socket, capacity, or deployment test was executed.

The supplied `generated/VALIDATION_EN.json` explicitly limits its PASS to planning structure. The README's exporter regression results likewise concern planning tools. They establish neither reusable-source quality nor working MCR behavior. Source selection and this document do **not** complete C01. [Plan §§2.1, 4, 18; README “Actual validation performed for this planning delivery.”]

## Architecture and useful foundations

The intended request path is browser → same-origin `/api/*` → Gateway → owning internal service. The parallel world path is browser → `/world-ws` → Concert's room-owning listener, admitted by a ticket obtained through authenticated HTTP. Axum/Tokio, Tonic/Prost, SQLx/PostgreSQL, Redis integration, and the inherited Auth, Profile, Friend, Post, Notification, and Gateway boundaries are selected foundations, not technologies to redesign in an initial slice. [Plan §3.2, §§7.1–7.3; BE01–BE05.]

Concert should contain pure domain rules, application use cases and ports, and adapters for persistence, identity, relationships, media, notifications, and transports. Both HTTP/gRPC commands and realtime admission/projections must call the same authorization rules. Adding a service for each party, giveaway, or mural would expand distributed transaction and operations costs without serving the stated design. An ADR is required for a deliberate ownership change. [Plan §3.2; BE05, BE37.]

| Candidate reuse | Value described by the source | Required adaptation or evidence |
|---|---|---|
| port-rust workspace, lockfile, service skeletons | Runtime, RPC contracts, database integration, existing service ownership | Verify exact source commit, workspace root, build features, generated code, migrations, process dependencies and deployment assumptions before import. The earlier inspected SHA is a historical pointer, not a qualified candidate. |
| port-rust Auth and Gateway | Canonical identity and access/refresh basis | Verify route operation IDs, token/session claims, refresh and revocation behavior, cookie/CSRF support, internal caller authentication, and real provider configuration. |
| Bigyeting headless HTTP logic | Cancellation, normalization, CSRF machinery, upload orchestration | Replace old cookie-only authorization and `/v1` assumptions; extract only audited logic and dependencies. |
| Bigyeting introduction/age/access/reveal/block rules | Existing policy semantics and boundary fixtures | Remove source database and identity coupling; bind every claim and decision to Auth UUIDs; inventory the complete call chain and tests. |
| Inherited Post and Notification | Existing content and delivery ownership | Add event scoping and safe projections through owner APIs; inspect cursor behavior, deduplication and privacy invalidation. |

These are source assertions and proposed reuse boundaries, not findings from running those foundations. The plan names concrete frontend source files and the age helper, but does not supply a complete extraction map for every backend policy. C01 must close that gap. [Plan §§2.1–2.2, §18 S02–S06/S10; C01, BE25–BE33.]

## Authentication and canonical identity boundary

Auth supplies one canonical UUID `user_id`; Concert participant uniqueness is `(event_id, user_id)`. X/Instagram handles are optional private data. An email-only eligible user must be able to participate. A provider account, profile projection, verified-age claim and event participant are distinct records linked explicitly to that identity, not competing login principals. [Plan §7.2; BE04, BE06, BE26.]

The selected browser arrangement holds a short-lived access token in memory and a refresh credential in a Secure HttpOnly cookie. It therefore requires a real adapter, not a copied Bigyeting client. C03 must inspect and freeze exact endpoint paths, operation IDs, refresh responses, session identifiers, credential expiry, error envelope, cookie scope, Origin/CSRF rules and logout semantics. BE03 then proves those flows against isolated services. Refresh must be single-flight; an ambiguous command timeout cannot be repaired by blindly replaying a mutation. Identity changes must cancel pending work and clear private caches, sockets and sensitive drafts; late responses for account A cannot populate account B. [Plan §§7.2–7.4; C03, FE01/FE02/FE09, BE03.]

A world ticket is user/session/event/room-bound, expires after 30 seconds, and is consumed atomically once. First-frame authentication has a three-second deadline; no secret belongs in a WS URL or access log. Ticket validation is only admission: session expiry, suspension, blocks and membership changes must continue to affect an established socket. The current materials do not determine the actual imported session-revocation mechanism or how Concert learns of changes. That is an inspection and contract blocker, not a reason to assume JWT validation alone satisfies revocation. [Plan §7.5; BE10, BE14, BE37, BE42.]

## Data ownership and failure boundaries

Concert owns its database and event-specific aggregates: participants/settings, places and map releases, physical beacons, parties and rally state, giveaways/reservations/receipts, introductions/age claims/social grants, mural state, reports/audit/outbox/cleanup. Auth/Profile/Friend/Post/Notification retain their existing data. Concert must use explicit APIs/ports rather than joining or mutating another service's tables. Denormalized public profiles and feed previews are projections that must be reauthorized, not alternate authority. [Plan §§3.2, 8.1, 8.4; BE04, BE31–BE33, BE37.]

This makes local Concert transactions possible for stock, capacity, command receipts and outbox insertion. It does not make cross-service moderation, notification, account deletion or privacy propagation atomic. Those need durable steps, deduplication, retries and visible partial failures. A notification failure cannot roll back an already committed pickup; a successful request to a downstream service is not proof that every deletion step completed. [Plan §8.4; BE15, BE33, BE39, BE41/BE42.]

MCR must receive isolated databases, credentials, volumes, object-storage namespace and environment configuration. The plan authorizes a code foundation import, not automatic reuse or migration of Bigyeting or port-rust personal data. C01 records existing targets and safe separation; BE02 proves fresh migration and restart persistence in isolation. Redis Pub/Sub can accelerate delivery but cannot be the sole record of durable transitions. [Plan §§3.2, 4, 8.1; C01, BE02, BE15.]

## Realtime correctness

The P0 world has a single process owning a room, bounded membership, server-side movement and recipient-specific replication. Clients send normalized input intentions, not positions. The proposed 20 Hz simulation, at-most-10 Hz updates, 50-user room cap and 200-connection/four-room scenario are tuning and qualification targets. None is demonstrated capacity. Theme selection changes presentation, not geometry or room population. [Plan §§5.2, 7.5, 16; BE11–BE14, QA08.]

The implementation must establish these invariants before IN01 can mean real multiplayer acceptance:

- **Admission and membership:** capacity checks and lease changes cannot overfill rooms or duplicate membership on concurrent reconnect/switch. Specify multi-tab/session policy, lease duration and failed-switch behavior in C05/BE11; these details are not yet frozen.
- **Snapshot handoff:** capture a consistent baseline and deliver subsequent deltas from that baseline. Use room epochs for restarts/resets and per-delivered-stream sequences for each viewer's filtered subscription; a filtered room-global sequence would produce false gaps.
- **Authorization changes:** remove blocked peers in both directions and revoke unauthorized subscriptions promptly. Define policy-cache staleness, invalidation delivery and failure behavior; previously cached allow decisions cannot remain authoritative indefinitely.
- **Backpressure and recovery:** bound queues, coalesce replaceable movement, and force resync when required history is lost. Reconnect after foregrounding/restart obtains an authoritative snapshot and never replays old movement blindly. Durable domain outcomes remain recoverable through HTTP reads.

[Plan §7.5; C05, BE10–BE14, BE37, QA05, IN01.]

The mural shares transport but has a different durability model: a pixel write, per-user cooldown, revision and outbox event must commit atomically. A snapshot plus retained ordered operations must converge; restoration creates a new revision and requires resync. Mural writes cannot use the ephemeral movement model. [Plan §§8.2, 8.4; BE34/BE35/BE40, QA03, IN04.]

## Atomic parties and reservations

Database constraints and transactions are required; UI button disabling, process locks and Redis counters do not prove correctness across requests or restarts. The first persistence design should make the linearization point of each command explicit and keep receipt/outbox writes in the same transaction as the aggregate change. [Plan §§7.4, 8.1–8.2; BE15–BE23.]

| Aggregate | Required invariant and mechanism | Evidence needed |
|---|---|---|
| Party/application | Unique application/membership, guarded approval under aggregate lock/version, approved count within capacity, actor/lifecycle checks | Two approvals for the last slot create one membership; duplicate apply/decision returns a stable outcome; approval versus capacity edit/cancel is deterministic. |
| Party membership | Owner departure cannot orphan an active party; removal releases capacity and access | Explicit cancel/transfer rule; removed member cannot read rally/beacons or retain subscriptions, including during delayed invalidation. |
| Giveaway stock | `available = quantity_total - reserved_unclaimed - quantity_claimed`, never negative | Guarded update/row lock plus unique active reservation; five contenders for one unit yield one hold; edits cannot reduce total below committed units. |
| Release/expiry/claim | One conditional transition wins; stock released once; CLAIMED terminal | Forced cancel/expire/claim interleavings, DB-clock expiry, restarted worker, duplicate claim returning the same receipt. |
| Command receipt | Actor/operation/key/body hash identifies one outcome; changed body conflicts | Simultaneous duplicate requests and commit-before-response crash return one durable result without repeating effects. |

[Plan §8.2; BE15–BE23; QA03, QA09, IN02.]

C04 still needs exact decisions: whether the party owner consumes capacity; application reapplication and duplicate-key scope; owner-transfer policy; permitted event/party/giveaway transition edges; who moves a reservation from HELD to READY; which actor presents and confirms pickup; grace-period and boundary-time semantics; cancellation rules for committed holds; receipt retention and retry behavior after its expiry. These are unresolved command contracts, not permission to invent product behavior silently. Reservation expiry follows the pickup window plus configured grace, not an arbitrary short countdown from an early reservation. Pickup codes must be reservation-bound, hashed, expiring and guess-limited; no ticket QR exposure is needed. [Plan §§7.4, 8.2; C04, BE17–BE23.]

## Privacy, age and physical presence

Age semantics are unusually important reuse material. Preserve half-open bounds `minimum_age <= age < maximum_age_exclusive`, supported nullable bounds, claim status/expiry, verifier provenance, canonical replacement and next-birthday computation using the Seoul date. The cited source helper is Bigyeting `backend/services/bigyeting-api/src/adapters/out/repository/current_age.rs`; C01 must inspect its dependencies and boundary tests, not copy the arithmetic alone. The stated 14–100 supported claim range is a source implementation boundary, not a concert admission rule or legal signup-age decision. [Plan §§2.1, 8.3; S06, BE25/BE26.]

Keep service eligibility, event admission, self-declared age and verified-age policy separate. Unknown, expired, unsupported or unavailable verification fails closed for restricted discovery/sharing, while unrelated basic world use remains available if service eligibility is satisfied. No real verification provider or callback-to-Auth mapping is qualified by this package. C03/C04 must expose honest capability states and decide how restricted sharing consumes age-policy results. [Plan §§7.3, 8.3; BE04, BE25–BE30.]

Private handles require encryption at rest, per-platform explicit mutual consent, version-bound grants, expiry and checks on every reveal. Block, suspension, revoke, removal and handle replacement must invalidate relevant access and private projections. Public profiles, HTML, lists, logs, notifications and unauthorized WS frames cannot contain handles or exact verified age. A reveal uses `no-store`; cache invalidation prevents future application disclosure but cannot erase what another person already copied. [Plan §§7.4, 8.3; BE28–BE30, BE33, BE37; QA04, IN03.]

Virtual coordinates are never evidence of physical arrival. A physical beacon has a reviewed place, authorized audience, revision and expiry (initial target 15 minutes). Reads must enforce expiry even if cleanup lags. Location changes invalidate previous guidance; removed party members lose audience access. No continuous GPS or invented distance/walking-time accuracy belongs to P0. Closing the event or deleting an account requires resumable expiry/scrubbing, with private handles, recipients and physical locations excluded from public archives. [Plan §§5.4, 8.3–8.4; BE08/BE09/BE18/BE19, BE41/BE42.]

## Dependencies and practical blockers

The useful early dependency chain is C01 → C02/BE01/BE02 → BE03/BE04 plus BE05 → bootstrap and relationship policy → admission/membership/movement/replication/reconnect → IN01. C03/C04/C05/C06 contracts and C07 fixtures unlock parallel work. BE37 is an early prerequisite for safe presence and almost every private feature despite its high task number. BE15 is a reusable transaction foundation for subsequent commands, not an optional notification enhancement. [Plan §§9–11, 15.1–16.]

The scheduling graph is not an exhaustive semantic or release checklist. In particular:

- BE34 requires atomic outbox behavior in its text but does not list BE15 directly; its implementation must still satisfy that behavior.
- BE28–BE30 do not explicitly depend on BE26 although restricted sharing must respect age policy. C04 must define that enforcement path; satisfying only the listed dependencies could leave a privacy gap.
- BE06 needs valid default avatar/manifests, although BE07/C06 are not explicit direct dependencies. Bootstrap fixtures and asset compatibility must agree before integration.
- BE38's operations/flags and BE43's hardening are later cards, but admission, commands and sockets need their applicable server gates and bounds from the first real slice. Phase numbers are not reasons to postpone those invariants.
- BE44's full stack comes late. BE02 still needs a minimal isolated startup/migration harness early; otherwise auth and domain persistence cannot be qualified. This does not claim BE44 completed early.

[Plan §§7–11, 13, 15.1; README “Release-policy clarification.”]

No source-level compatibility audit, actual provider capability map, room revocation design, isolated migration evidence, database race evidence or capacity measurement is available in this review. Those are concrete gaps to close before accepting their corresponding tasks. Source inspection can identify defects and reuse candidates; only execution against the selected candidate can establish working behavior.

## C01 inspection checklist

C01 should produce a reviewable source map, not just a repository choice. Its backend inspection must record:

1. **Immutable source identity:** actual port-rust and Bigyeting commits, selected workspace/subdirectory, dirty-tree disposition, license/provenance, and every copied module's source/destination/dependencies/test. The plan's Bigyeting `5eecdcaffd1a9d505b9ab2a974f8d5623217e90a` and prior port-rust `704f42149d5e8e35e1d578676d8ae2dac4f0f9c2` are inspection pointers, not current qualification evidence.
2. **Build and contract topology:** real Cargo workspace and lockfile, Rust/tool/protoc requirements, proto/build-script roots, SQLx compile-time requirements, OpenAPI generation inputs, generated artifact policy, path-sensitive scripts, enabled features and required external services. This feeds C02/BE01/BE02 without pretending their builds passed.
3. **Identity/auth topology:** Gateway `auth_routes.rs` and called handlers, token/session model, refresh storage/rotation, logout/revocation, provider discovery and callback binding, CSRF/origin middleware, safe redirects, internal-service credentials and profile/suspension authority. Produce the mapping needed by C03/BE03/BE04/BE10.
4. **Policy extraction graph:** introduction eligibility, current-age/canonical-claim helpers, access requests, mutual consent/reveal, encryption, blocks and deletion hooks, including pure versus database-bound portions and original fixtures. Identify exact source paths still missing from the plan and every old X-account assumption.
5. **Ownership/integration seams:** inherited Friend block APIs and invalidation events, Profile public/private projections, Post event-scoping support, Notification idempotency, Auth deletion orchestration and authorization behavior when dependencies are unavailable. Record gaps requiring adapters rather than direct database access.
6. **Environment and cutover map:** process/port map, entrypoints to retire after import, new database/storage/volume targets, migration ownership and forward recovery, mock versus real providers, secret references without exposing values, and safeguards against targeting old environments. Do not inspect live records to establish this map.
7. **Gap register and evidence plan:** classify each reuse claim as inspected, uninspected, incompatible or needing extraction; assign unresolved decisions to C03/C04/C05/C06 or the relevant BE card; identify bounded checks and acceptance evidence. Update delivery metadata in future integration artifacts to `julymeltdown/mcr/master` while preserving provenance of the supplied documents.

[Plan §§2–4, 7, 10 C01/C02, 11 BE01/BE02/BE25, 18.]

## Recommended first bounded work packages

These are recommendations for later authorized development; no package is implemented by this assessment. Keep contracts and shared manifests under integrator ownership and publish coherent verified checkpoints through that integrator.

| Package | Bounded scope and prerequisites | Exit evidence |
|---|---|---|
| 1. Source qualification | C01 inventory above, including backend extraction and environment maps | Immutable source/destination/test inventory and explicit unresolved decisions. Selection alone is insufficient. |
| 2. Reproducible foundation | After C01: relevant C02 backend/toolchain work, BE01/BE02 import and isolated minimal harness | Workspace build, fresh migrations, process/port inventory and restart persistence against new test-only stores; no claim of full BE44 completion. |
| 3. Contract freeze | After C01: C03 identity/core, C04 transition/policy, C05 WS, C06 assets; C07 frozen-subset fixtures | Reviewed operation/actor/state/error/retry matrix, wire schemas, deterministic examples, reproducible generation and deliberate mismatch failure. No product features required. |
| 4. Identity and participant core | After foundation/contracts: BE03/BE04/BE05/BE06, then BE37; BE07/BE08 with asset contracts | Isolated real auth lifecycle, email-only canonical participant, repeatable bootstrap, safe DTOs and fail-closed relationship tests. Preserve each card's independent acceptance record. |
| 5. Small real multiplayer slice | BE10 → BE11 → BE12 → BE13 → BE14 with policy and asset prerequisites | Ticket replay rejection, guarded room capacity, authorized two-user movement, filtered resync, revocation and restart tests; IN01 only after frontend/art prerequisites are also exercised. |
| 6. Durable command foundation and first race slice | BE15, then separately BE16/BE17 or BE20/BE21 once their policy/contracts exist | Receipt/outbox crash recovery and real PostgreSQL last-slot/last-unit contention tests. Follow with removal, expiry and claim before claiming complete parties/pickup or IN02. |

Age-policy extraction (BE25), once C01/C04 are ready, can proceed as a separate pure-rule task alongside runtime work. Full social revelation must wait for canonical binding, verification capability and privacy enforcement. Mural and social tracks can then advance independently of meetup work, as the source recommends. Recovery, closure, moderation and release gates remain separate acceptance obligations. [Plan §§11, 13, 15–16; README “Release-policy clarification.”]
