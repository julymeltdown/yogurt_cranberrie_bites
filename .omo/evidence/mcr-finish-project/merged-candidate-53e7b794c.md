# Merged candidate 53e7b794c

{
  "scope": "Planning provenance, structure, exporter regression, and delivery routing only",
  "result": "PASS",
  "python_version": "3.14.3",
  "checks": [
    "13 original archive checksums match",
    "pinned upstream narrative checksum matches",
    "active and original narrative/table task and IA content match",
    "original: nine regression probes pass and all committed exports reproduce",
    "active: nine regression probes pass and all committed exports reproduce",
    "all active metadata and 118 task destinations use mcr/master; baseline remains NOT_STARTED",
    "release checklist contains 111 prerequisites plus IN05 and 61 required IA entries"
  ],
  "task_count": 118,
  "ia_count": 64,
  "application_build": "NOT_RUN",
  "application_tests": "NOT_RUN",
  "deployment": "NOT_RUN",
  "input_sha256": {
    "MCR_IMPLEMENTATION_PLAN_V2_EN.md": "e19732ac9c5c5d5397652bf047645da0640ec72c97f59869974680c4e7f69a36",
    "PLAN_TABLES_EN.md": "7e81683fcc05f8441b473f673b282a1010931721ffabf15aeef6548811ec8ba5",
    "MCR_EXPORT_PLAN_EN.py": "57cdba6af51feb45bef5b74a0ed2755b448d475b0ceb1ad2c9ff16b72060e348",
    "TEST_PLAN_EXPORT_EN.py": "cd7b1cb49636db9468041d8bf87e7dacfeb483b0679facf50638d0f181557b8e",
    "scripts/mcr/check-plan.py": "a172af72eb033dcc3e9461d8a16c58ac7704702a4ced39801a67b3b3edcc3cd3",
    "docs/mcr/source-provenance.json": "bf249c1e414928d5946803243ca1857f8d6e4435fa0fe49fb2185bc778d54fd8"
  }
}
python3 scripts/mcr/check-plan.py -> exit=0
..........................s...................
----------------------------------------------------------------------
Ran 46 tests in 12.951s

OK (skipped=1)
✔ clean original code and prose containing library/import words pass (10.1815ms)
✔ renamed byte-identical legacy UI fails (4.640959ms)
✔ copied asset bytes under public or tests fail regardless of extension (5.399875ms)
✔ static prohibited dependency and legacy namespace import forms fail (17.475667ms)
✔ CSS and HTML loading references fail (4.053125ms)
✔ prohibited manifest dependency fails even without a source import (4.383666ms)
✔ modified allowlist cannot approve a legacy visual source (3.649791ms)
✔ candidate bytes require reviewed source revision/hash, destination hash, and tests (13.268541ms)
✔ visual destination hash cannot be approved using a headless source (3.535208ms)
✔ inventory hash, exact schema versions, source revision and counts fail closed (8.346083ms)
✔ dependencies and build outputs are excluded; source symlinks fail closed (5.180167ms)
✔ no frontend content reports NOT_RUN rather than a passing application check (3.593459ms)
✔ portable CLI --root, invalid flags, and missing inventory (213.087958ms)
✔ shipped policy pins the audited inventory and only the four reviewed headless extracts (3.128416ms)
ℹ tests 14
ℹ suites 0
ℹ pass 14
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 345.459541
{
  "task": "C08",
  "result": "PASS",
  "source_revision": "5eecdcaffd1a9d505b9ab2a974f8d5623217e90a",
  "inventory_sha256": "9d1202211cd136948a7afeb5588238ba9de13613980dab42616011f47762a079",
  "files_scanned": 5863,
  "content_files": 5855,
  "reviewed_extracts": 4,
  "violations": [],
  "scope": "Exact bytes and static imports only; manual source/visual review and QA02 remain required"
}
bash scripts/mcr/check.sh guards -> exit=0
{
  "operations": 150,
  "outputs": 18,
  "result": "PASS"
}
python3 scripts/mcr/generate-api-client.py --check -> exit=0
{"task": "QA01", "result": "PASS", "candidate": "53e7b794c59f9cf8534cef1023ac7827c0d25e27", "probe": null, "error": null, "fixtures": 198, "responses": 1727, "operations": 150}
python3 scripts/mcr/contracts/qa01-coverage/run.py -> exit=0
    Checking concert-service v0.1.0 (/Users/lhs/Desktop/mcr/backend/services/concert-service)
warning: unused import: `CallerRegistry`
  --> services/auth-service/tests/concert_http_owner_chain.rs:38:17
   |
38 |     grpc_auth::{CallerRegistry, Workload},
   |                 ^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: `auth-service` (test "concert_http_owner_chain") generated 1 warning (run `cargo fix --test "concert_http_owner_chain"` to apply 1 suggestion)
warning: unused import: `grpc_auth::Workload`
  --> services/auth-service/tests/legacy_deletion_receipts.rs:31:16
   |
31 | use platform::{grpc_auth::Workload, grpc_tls::TlsMaterial};
   |                ^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: `auth-service` (test "legacy_deletion_receipts") generated 1 warning (run `cargo fix --test "legacy_deletion_receipts"` to apply 1 suggestion)
warning: constant `OTHER_USER_ID` is never used
  --> services/api-gateway/tests/unit/http_support.rs:43:11
   |
43 | pub const OTHER_USER_ID: &str = "22222222-2222-2222-2222-222222222222";
   |           ^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: fields `clients` and `moderation_repository` are never read
  --> services/api-gateway/tests/unit/http_support.rs:50:9
   |
48 | pub struct RouteHarness {
   |            ------------ fields in this struct
49 |     pub state: Arc<AppState>,
50 |     pub clients: Arc<TestClients>,
   |         ^^^^^^^
51 |     pub moderation_repository: Arc<moderation_repository::TestModerationRepository>,
   |         ^^^^^^^^^^^^^^^^^^^^^

warning: associated items `with_admission` and `bearer_token_without_session` are never used
   --> services/api-gateway/tests/unit/http_support.rs:82:18
    |
59  | impl RouteHarness {
    | ----------------- associated items in this implementation
...
82  |     pub async fn with_admission(policies: AdmissionPolicies) -> anyhow::Result<Self> {
    |                  ^^^^^^^^^^^^^^
...
162 |     pub fn bearer_token_without_session(&self, user_id: &str, roles: Vec<String>) -> String {
    |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: variants `Ended` and `Unavailable` are never constructed
   --> services/api-gateway/tests/unit/http_support.rs:274:5
    |
272 | pub enum SessionAnswer {
    |          ------------- variants in this enum
273 |     Active(Vec<String>),
274 |     Ended,
    |     ^^^^^
275 |     /// The check fails, as when the issuer cannot be reached.
276 |     Unavailable,
    |     ^^^^^^^^^^^
    |
    = note: `SessionAnswer` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: methods `answer_sessions` and `avatar_content_type` are never used
   --> services/api-gateway/tests/unit/http_support.rs:288:12
    |
287 | impl TestClients {
    | ---------------- methods in this implementation
288 |     pub fn answer_sessions(&self, answer: SessionAnswer) {
    |            ^^^^^^^^^^^^^^^
...
296 |     pub fn avatar_content_type(&self, content_type: &str) {
    |            ^^^^^^^^^^^^^^^^^^^

warning: function `response_json` is never used
   --> services/api-gateway/tests/unit/http_support.rs:317:14
    |
317 | pub async fn response_json(response: Response<Body>) -> Value {
    |              ^^^^^^^^^^^^^

warning: methods `insert_report`, `restrict`, `set_content_decisions`, and `mark_target_unavailable` are never used
  --> services/api-gateway/tests/unit/http_support/moderation_repository.rs:46:12
   |
45 | impl TestModerationRepository {
   | ----------------------------- methods in this implementation
46 |     pub fn insert_report(
   |            ^^^^^^^^^^^^^
...
70 |     pub fn restrict(&self, user_id: Uuid) {
   |            ^^^^^^^^
...
74 |     pub fn set_content_decisions(&self, decisions: Vec<ContentModerationDecisionOutboxEntry>) {
   |            ^^^^^^^^^^^^^^^^^^^^^
...
78 |     pub fn mark_target_unavailable(&self, report_id: Uuid) {
   |            ^^^^^^^^^^^^^^^^^^^^^^^

warning: `api-gateway` (test "concert_age_routes") generated 7 warnings
warning: `api-gateway` (test "concert_product_http") generated 7 warnings (7 duplicates)
warning: `api-gateway` (test "concert_feature_routes") generated 7 warnings (7 duplicates)
warning: unused import: `Operation`
  --> services/api-gateway/tests/participant_delivery_retention_support/component.rs:10:54
   |
10 |         concert_participant::{self as client, proto, Operation},
   |                                                      ^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: `api-gateway` (test "participant_delivery_retention") generated 1 warning (run `cargo fix --test "participant_delivery_retention"` to apply 1 suggestion)
warning: variants `Pending`, `Conflict`, and `Expired` are never constructed
  --> crates/concert-application/tests/../src/age_verification/mod.rs:20:5
   |
15 | pub enum Error {
   |          ----- variants in this enum
...
20 |     Pending,
   |     ^^^^^^^
21 |     Conflict,
   |     ^^^^^^^^
22 |     Expired,
   |     ^^^^^^^
   |
   = note: `Error` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` on by default

warning: function `context` is never used
  --> crates/concert-application/tests/../src/age_verification/mod.rs:39:8
   |
39 | pub fn context(
   |        ^^^^^^^

warning: field `0` is never read
  --> crates/concert-application/tests/../src/age_verification/mod.rs:84:29
   |
84 | pub struct ProviderResponse(String);
   |            ---------------- ^^^^^^
   |            |
   |            field in this struct
   |
   = help: consider removing this field

warning: method `expose` is never used
  --> crates/concert-application/tests/../src/age_verification/mod.rs:92:12
   |
85 | impl ProviderResponse {
   | --------------------- method in this implementation
...
92 |     pub fn expose(&self) -> &str {
   |            ^^^^^^

warning: fields `user_id`, `event_id`, `verified_at`, `expires_at`, and `verification_available` are never read
   --> crates/concert-application/tests/../src/age_verification/mod.rs:245:9
    |
244 | pub struct OwnStatus {
    |            --------- fields in this struct
245 |     pub user_id: Uuid,
    |         ^^^^^^^
246 |     pub event_id: Uuid,
    |         ^^^^^^^^
247 |     pub status: AgeStatus,
248 |     pub verified_at: Option<DateTime<Utc>>,
    |         ^^^^^^^^^^^
249 |     pub expires_at: Option<DateTime<Utc>>,
    |         ^^^^^^^^^^
250 |     pub verification_available: bool,
    |         ^^^^^^^^^^^^^^^^^^^^^^

warning: `concert-application` (test "age_verification") generated 5 warnings
warning: field `calls` is never read
  --> crates/concert-application/tests/place.rs:26:5
   |
23 | struct Policy {
   |        ------ field in this struct
...
26 |     calls: usize,
   |     ^^^^^
   |
   = note: `Policy` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis
   = note: `#[warn(dead_code)]` on by default

warning: fields `event` and `calls` are never read
  --> crates/concert-application/tests/place.rs:58:5
   |
57 | struct Repo {
   |        ---- fields in this struct
58 |     event: EventId,
   |     ^^^^^
...
62 |     calls: usize,
   |     ^^^^^

warning: `concert-application` (test "place") generated 2 warnings
warning: unused import: `WorldError`
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/qualified.rs:19:41
   |
19 | use concert_application::world_ticket::{WorldError, WorldSubject};
   |                                         ^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: unused import: `membership::MembershipAdapter`
 --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/world_fixture.rs:8:5
  |
8 |     membership::MembershipAdapter,
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `CanonicalAuthorityPort`
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/world_fixture.rs:12:18
   |
12 |     membership::{CanonicalAuthorityPort, ClockAnchor},
   |                  ^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `adapters::r#in::world_ws::AdmissionPort`
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/world_fixture.rs:15:5
   |
15 |     adapters::r#in::world_ws::AdmissionPort,
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused imports: `PgConnectOptions` and `PgPoolOptions`
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/world_fixture.rs:39:16
   |
39 |     postgres::{PgConnectOptions, PgPoolOptions},
   |                ^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^

warning: unused imports: `os::unix::fs::MetadataExt`, `path::PathBuf`, and `str::FromStr`
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/world_fixture.rs:47:11
   |
47 | use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
   |           ^^^^^^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^  ^^^^^^^^^^^^

warning: unused import: `membership::MembershipAdapter`
 --> services/concert-service/src/world/../../tests/world_read_only_support/qualified.rs:9:5
  |
9 |     membership::MembershipAdapter,
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused imports: `PgConnectOptions` and `PgPoolOptions`
  --> services/concert-service/src/world/../../tests/world_read_only_support/qualified.rs:40:16
   |
40 |     postgres::{PgConnectOptions, PgPoolOptions},
   |                ^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^

warning: unused imports: `os::unix::fs::MetadataExt`, `path::PathBuf`, and `str::FromStr`
  --> services/concert-service/src/world/../../tests/world_read_only_support/qualified.rs:48:11
   |
48 | use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
   |           ^^^^^^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^  ^^^^^^^^^^^^

warning: unused variable: `now`
   --> services/concert-service/src/world/admission_bridge.rs:877:13
    |
877 |         let now = Utc::now();
    |             ^^^ help: if this is intentional, prefix it with an underscore: `_now`
    |
    = note: `#[warn(unused_variables)]` on by default

warning: value assigned to `receipt` is never read
   --> services/concert-service/src/social_live/producer.rs:190:17
    |
190 |         let mut receipt = None;
    |                 ^^^^^^^
    |
    = help: maybe it is overwritten before being read?
    = note: `#[warn(unused_assignments)]` on by default

warning: value assigned to `result` is never read
   --> services/concert-service/src/account_private_data/core.rs:109:17
    |
109 |         let mut result = None;
    |                 ^^^^^^
    |
    = help: maybe it is overwritten before being read?

warning: unused variable: `fence`
   --> services/concert-service/src/account_private_data/aggregate.rs:392:13
    |
392 |         let fence = user_fence(&mut tx, &command, &source).await?;
    |             ^^^^^ help: if this is intentional, prefix it with an underscore: `_fence`

warning: unused import: `crate::product_rpc::settlement::NativeTransactionResolver`
 --> services/concert-service/src/mural_tools_b/witness.rs:5:5
  |
5 | use crate::product_rpc::settlement::NativeTransactionResolver;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` on by default

warning: unused import: `crate::product_rpc::settlement::NativeTransactionResolver`
 --> services/concert-service/src/intro_live/witness.rs:4:5
  |
4 | use crate::product_rpc::settlement::NativeTransactionResolver;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::product_rpc::ProductOwner`
   --> services/concert-service/src/config/social_runtime.rs:115:13
    |
115 |         use crate::product_rpc::ProductOwner;
    |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `crate::product_rpc::settlement::NativeTransactionResolver`
 --> services/concert-service/src/mural_tools_b/witness.rs:5:5
  |
5 | use crate::product_rpc::settlement::NativeTransactionResolver;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService`
   --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/tls.rs:159:13
    |
159 |         use auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService;
    |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService`
   --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/tls.rs:166:13
    |
166 |         use auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService;
    |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `sqlx::Row`
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/qualified.rs:26:5
   |
26 | use sqlx::Row;
   |     ^^^^^^^^^

warning: methods `request_id` and `cancel` are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_auth.rs:205:19
    |
49  | impl WorldOwnerRequest {
    | ---------------------- methods in this implementation
...
205 |     pub(crate) fn request_id(&self) -> Uuid {
    |                   ^^^^^^^^^^
...
208 |     pub(crate) fn cancel(&self) {
    |                   ^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: methods `observed_at` and `revision` are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_auth.rs:254:19
    |
247 | impl WorldAuthObservation {
    | ------------------------- methods in this implementation
...
254 |     pub(crate) fn observed_at(&self) -> DateTime<Utc> {
    |                   ^^^^^^^^^^^
...
260 |     pub(crate) fn revision(&self) -> OwnerRevision {
    |                   ^^^^^^^^

warning: field `0` is never read
  --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:30:14
   |
30 |     Consumed(TicketClaim),
   |     -------- ^^^^^^^^^^^
   |     |
   |     field in this variant
   |
   = note: `ProjectionSource` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis
help: consider changing the field to be of unit type to suppress this warning while preserving the field numbering, or remove the field
   |
30 -     Consumed(TicketClaim),
30 +     Consumed(()),
   |

warning: field `original` is never read
  --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:71:5
   |
65 | pub(crate) struct WorldProjectionParent {
   |                   --------------------- field in this struct
...
71 |     original: AuthorityRpcDeadline,
   |     ^^^^^^^^
   |
   = note: `WorldProjectionParent` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis

warning: methods `original_deadline`, `consumed_ticket`, `pin`, `room_id`, and `connection_id` are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:122:19
    |
76  | impl WorldProjectionParent {
    | -------------------------- methods in this implementation
...
122 |     pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
    |                   ^^^^^^^^^^^^^^^^^
...
131 |     pub(crate) fn consumed_ticket(&self) -> Option<&TicketClaim> {
    |                   ^^^^^^^^^^^^^^^
...
137 |     pub(crate) fn pin(&self) -> &ResourcePin {
    |                   ^^^
...
143 |     pub(crate) fn room_id(&self) -> RoomId {
    |                   ^^^^^^^
...
146 |     pub(crate) fn connection_id(&self) -> ConnectionId {
    |                   ^^^^^^^^^^^^^

warning: methods `credential_expires_at`, `source_expires_at`, `original_deadline`, `cancel`, and `purpose` are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:258:23
    |
258 |         pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
    |                       ^^^^^^^^^^^^^^^^^^^^^
...
261 |         pub(crate) fn source_expires_at(&self) -> DateTime<Utc> {
    |                       ^^^^^^^^^^^^^^^^^
...
264 |         pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
    |                       ^^^^^^^^^^^^^^^^^
...
267 |         pub(crate) fn cancel(&self) {
    |                       ^^^^^^
...
272 | impl WorldProfileProjection {
    | --------------------------- methods in this implementation
273 |     scope_getters!();
    |     ---------------- in this macro invocation
...
277 |     pub(crate) fn purpose(&self) -> &'static str {
    |                   ^^^^^^^
    |
    = note: this warning originates in the macro `scope_getters` (in Nightly builds, run with -Z macro-backtrace for more info)

warning: multiple methods are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:246:23
    |
246 |         pub(crate) fn actor(&self) -> Uuid {
    |                       ^^^^^
...
249 |         pub(crate) fn event_id(&self) -> Uuid {
    |                       ^^^^^^^^
...
264 |         pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
    |                       ^^^^^^^^^^^^^^^^^
...
267 |         pub(crate) fn cancel(&self) {
    |                       ^^^^^^
...
293 | impl WorldPresenceProjection {
    | ---------------------------- methods in this implementation
294 |     scope_getters!();
    |     ---------------- in this macro invocation
...
304 |     pub(crate) fn purpose(&self) -> &'static str {
    |                   ^^^^^^^
...
315 |     pub(crate) fn candidate_credential_expires_at(&self) -> DateTime<Utc> {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
...
318 |     pub(crate) fn candidate_source_expires_at(&self) -> DateTime<Utc> {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: this warning originates in the macro `scope_getters` (in Nightly builds, run with -Z macro-backtrace for more info)

warning: field `subject` is never read
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:365:5
    |
361 | pub(crate) struct WorldProjectionAttempt {
    |                   ---------------------- field in this struct
...
365 |     subject: Uuid,
    |     ^^^^^^^

warning: multiple methods are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:246:23
    |
246 |         pub(crate) fn actor(&self) -> Uuid {
    |                       ^^^^^
...
249 |         pub(crate) fn event_id(&self) -> Uuid {
    |                       ^^^^^^^^
...
252 |         pub(crate) fn session_family_id(&self) -> Uuid {
    |                       ^^^^^^^^^^^^^^^^^
...
255 |         pub(crate) fn request_id(&self) -> Uuid {
    |                       ^^^^^^^^^^
...
258 |         pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
    |                       ^^^^^^^^^^^^^^^^^^^^^
...
261 |         pub(crate) fn source_expires_at(&self) -> DateTime<Utc> {
    |                       ^^^^^^^^^^^^^^^^^
...
264 |         pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
    |                       ^^^^^^^^^^^^^^^^^
...
267 |         pub(crate) fn cancel(&self) {
    |                       ^^^^^^
...
367 | impl WorldProjectionAttempt {
    | --------------------------- methods in this implementation
368 |     scope_getters!();
    |     ---------------- in this macro invocation
...
390 |     pub(crate) fn subject(&self) -> Uuid {
    |                   ^^^^^^^
...
393 |     pub(crate) fn deadline(&self) -> AuthorityRpcDeadline {
    |                   ^^^^^^^^
    |
    = note: this warning originates in the macro `scope_getters` (in Nightly builds, run with -Z macro-backtrace for more info)

warning: methods `subject` and `observed_at` are never used
  --> services/concert-service/src/adapters/out/world_profile.rs:44:19
   |
43 | impl WorldProfileObservation {
   | ---------------------------- methods in this implementation
44 |     pub(crate) fn subject(&self) -> Uuid {
   |                   ^^^^^^^
...
50 |     pub(crate) fn observed_at(&self) -> DateTime<Utc> {
   |                   ^^^^^^^^^^^

warning: field `owners` is never read
   --> services/concert-service/src/world/recipient_owners.rs:133:5
    |
127 | pub(crate) struct RecipientObservation {
    |                   -------------------- field in this struct
...
133 |     owners: RevisionVector,
    |     ^^^^^^

warning: methods `owner_revisions` and `checked_at` are never used
   --> services/concert-service/src/world/recipient_owners.rs:165:19
    |
144 | impl RecipientObservation {
    | ------------------------- methods in this implementation
...
165 |     pub(crate) fn owner_revisions(&self) -> RevisionVector {
    |                   ^^^^^^^^^^^^^^^
...
171 |     pub(crate) fn checked_at(&self) -> DateTime<Utc> {
    |                   ^^^^^^^^^^

warning: associated function `new_auth_fixture` is never used
   --> services/concert-service/src/world/admission_bridge.rs:621:19
    |
337 | impl AdmissionBridge {
    | -------------------- associated function in this implementation
...
621 |     pub(crate) fn new_auth_fixture(
    |                   ^^^^^^^^^^^^^^^^

warning: function `visible_in` is never used
  --> services/concert-service/src/gift_live/moderation.rs:77:21
   |
77 | pub(crate) async fn visible_in(c: &mut PgConnection, event: Uuid, gift: Uuid) -> Result<(), CommandError> {
   |                     ^^^^^^^^^^

warning: function `retain_representation` is never used
  --> services/concert-service/src/gift_live/moderation.rs:88:21
   |
88 | pub(crate) async fn retain_representation(pool: &PgPool, scope: &Arc<OwnerGuardScope>, request: &GiftRequest) -> Result<(), Status> {
   |                     ^^^^^^^^^^^^^^^^^^^^^

warning: associated function `from_checked_fields` is never used
    --> services/concert-service/src/party_live/mod.rs:1736:19
     |
1704 | impl PartyLiveBinding {
     | --------------------- associated function in this implementation
...
1736 |     pub(super) fn from_checked_fields(
     |                   ^^^^^^^^^^^^^^^^^^^

warning: function `error` is never used
    --> services/concert-service/src/party_live/wire.rs:1024:8
     |
1024 | pub fn error(error: PartyLiveError, trace: Uuid) -> PartyLiveReply {
     |        ^^^^^

warning: `concert-service` (lib) generated 24 warnings
warning: multiple variants are never constructed
  --> services/concert-service/tests/avatar_motion_v2_runtime_support/fence.rs:7:5
   |
5  | enum WitnessFault {
   |      ------------ variants in this enum
6  |     Healthy,
7  |     MissingDescriptor,
   |     ^^^^^^^^^^^^^^^^^
8  |     WrongRelease,
   |     ^^^^^^^^^^^^
9  |     WrongBody,
   |     ^^^^^^^^^
10 |     WrongScope,
   |     ^^^^^^^^^^
11 |     Unpublished,
   |     ^^^^^^^^^^^
12 |     Unconfirmed,
   |     ^^^^^^^^^^^
13 |     AfterWitness,
   |     ^^^^^^^^^^^^
14 |     AfterCommit,
   |     ^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: function `release` is never used
  --> services/concert-service/tests/support/avatar/../bootstrap/mod.rs:64:8
   |
64 | pub fn release() -> Arc<ValidatedProductRelease> {
   |        ^^^^^^^

warning: function `selected_release` is never used
  --> services/concert-service/tests/support/avatar/../bootstrap/mod.rs:69:8
   |
69 | pub fn selected_release() -> Arc<ValidatedProductRelease> {
   |        ^^^^^^^^^^^^^^^^

warning: function `release_from_directory` is never used
  --> services/concert-service/tests/support/avatar/../bootstrap/mod.rs:73:4
   |
73 | fn release_from_directory(root: PathBuf) -> Arc<ValidatedProductRelease> {
   |    ^^^^^^^^^^^^^^^^^^^^^^

warning: function `count` is never used
   --> services/concert-service/tests/support/avatar/../bootstrap/mod.rs:107:14
    |
107 | pub async fn count(pool: &PgPool, table: &str, event: Uuid) -> i64 {
    |              ^^^^^

warning: `concert-service` (test "avatar_motion_v2_runtime") generated 5 warnings
warning: field `party` is never read
   --> services/concert-service/tests/support/meeting/mod.rs:144:9
    |
140 | pub struct Fixture {
    |            ------- field in this struct
...
144 |     pub party: Uuid,
    |         ^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "meeting_authority") generated 1 warning
warning: unused import: `grpc_tls::TlsMaterial`
  --> services/concert-service/tests/account_deletion_rpc.rs:45:5
   |
45 |     grpc_tls::TlsMaterial,
   |     ^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: function `identities` is never used
  --> services/concert-service/tests/canonical_owner_composition_support/environment.rs:90:21
   |
90 | pub(super) async fn identities(
   |                     ^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "account_deletion_rpc") generated 2 warnings (run `cargo fix --test "account_deletion_rpc"` to apply 1 suggestion)
warning: struct `Capture` is never constructed
  --> services/concert-service/tests/shared_participant_composition_support/../canonical_owner_composition_support/capture.rs:22:8
   |
22 | struct Capture(Arc<Mutex<Option<VerifiedParentAuthority>>>);
   |        ^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: function `binding` is never used
  --> services/concert-service/tests/shared_participant_composition_support/../canonical_owner_composition_support/capture.rs:42:21
   |
42 | pub(super) async fn binding(
   |                     ^^^^^^^

warning: associated function `new` is never used
  --> services/concert-service/tests/shared_participant_composition_support/../process_child_recovery_support/fault.rs:22:19
   |
21 | impl<S> NegativeComplete<S> {
   | --------------------------- associated function in this implementation
22 |     pub(super) fn new(inner: S, enabled: bool) -> Self {
   |                   ^^^

warning: `concert-service` (test "shared_participant_composition") generated 3 warnings
warning: fields `owner` and `meeting` are never read
   --> services/concert-service/tests/support/meeting/mod.rs:142:9
    |
140 | pub struct Fixture {
    |            ------- fields in this struct
141 |     pub event: Uuid,
142 |     pub owner: Uuid,
    |         ^^^^^
...
145 |     pub meeting: MeetingDescription,
    |         ^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "meeting_migrations") generated 1 warning
warning: function `count` is never used
   --> services/concert-service/tests/support/bootstrap/mod.rs:107:14
    |
107 | pub async fn count(pool: &PgPool, table: &str, event: Uuid) -> i64 {
    |              ^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "participant_producer") generated 1 warning
warning: function `count` is never used
   --> services/concert-service/tests/support/avatar/../bootstrap/mod.rs:107:14
    |
107 | pub async fn count(pool: &PgPool, table: &str, event: Uuid) -> i64 {
    |              ^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "avatar") generated 1 warning
warning: type `fixture::Owners` is more private than the item `fixture::DatabaseFixture::owners`
   --> services/concert-service/tests/world_account_sessions.rs:332:9
    |
332 |         pub(super) owners: Arc<Owners>,
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ field `fixture::DatabaseFixture::owners` is reachable at visibility `pub(crate)`
    |
note: but type `fixture::Owners` is only usable at visibility `pub(self)`
   --> services/concert-service/tests/world_account_sessions.rs:286:5
    |
286 |     struct Owners {
    |     ^^^^^^^^^^^^^
    = note: `#[warn(private_interfaces)]` on by default

warning: fields `owners` and `started` are never read
   --> services/concert-service/tests/world_account_sessions.rs:332:20
    |
329 |     pub(super) struct DatabaseFixture {
    |                       --------------- fields in this struct
...
332 |         pub(super) owners: Arc<Owners>,
    |                    ^^^^^^
...
340 |         pub(super) started: Instant,
    |                    ^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: method `now` is never used
   --> services/concert-service/tests/world_account_sessions.rs:412:23
    |
342 |     impl DatabaseFixture {
    |     -------------------- method in this implementation
...
412 |         pub(super) fn now(&self) -> u64 {
    |                       ^^^

warning: `concert-service` (test "world_account_sessions") generated 3 warnings
warning: method `revoke` is never used
  --> services/concert-service/tests/world_motion_live.rs:42:8
   |
41 | impl Authority {
   | -------------- method in this implementation
42 |     fn revoke(&self, user: Uuid) {
   |        ^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "world_motion_live") generated 1 warning
warning: fields `friend`, `registry`, `settlement_url`, and `effects` are never read
  --> services/concert-service/tests/native_resolver_union_support/fixture.rs:60:16
   |
58 | pub(super) struct Fixture {
   |                   ------- fields in this struct
59 |     pub(super) auth: PgPool,
60 |     pub(super) friend: PgPool,
   |                ^^^^^^
61 |     pub(super) concert: PgPool,
62 |     pub(super) registry: Arc<ScopeRegistry>,
   |                ^^^^^^^^
...
65 |     settlement_url: String,
   |     ^^^^^^^^^^^^^^
66 |     effects: Arc<RejectEffects>,
   |     ^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: associated items `with_native_abort`, `interrupted_fence`, `no_unfinished_root`, `recover_auth_fresh`, and `assert_no_recovery_calls` are never used
   --> services/concert-service/tests/native_resolver_union_support/fixture.rs:80:25
    |
76  | impl Fixture {
    | ------------ associated items in this implementation
...
80  |     pub(super) async fn with_native_abort() -> anyhow::Result<Self> {
    |                         ^^^^^^^^^^^^^^^^^
...
166 |     pub(super) fn interrupted_fence(
    |                   ^^^^^^^^^^^^^^^^^
...
179 |     pub(super) async fn no_unfinished_root(&self) -> anyhow::Result<bool> {
    |                         ^^^^^^^^^^^^^^^^^^
...
183 |     pub(super) async fn recover_auth_fresh(&self) -> anyhow::Result<u32> {
    |                         ^^^^^^^^^^^^^^^^^^
...
199 |     pub(super) fn assert_no_recovery_calls(&self) {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^

warning: `concert-service` (test "native_resolver_union") generated 2 warnings
warning: function `pool` is never used
  --> services/concert-service/tests/canonical_owner_composition_support/environment.rs:46:21
   |
46 | pub(super) async fn pool(owner: &str) -> anyhow::Result<PgPool> {
   |                     ^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: function `identities` is never used
  --> services/concert-service/tests/canonical_owner_composition_support/environment.rs:90:21
   |
90 | pub(super) async fn identities(
   |                     ^^^^^^^^^^

warning: `concert-service` (test "feed_runtime") generated 2 warnings
warning: `concert-service` (test "age_deletion_native") generated 1 warning (1 duplicate)
warning: `concert-service` (test "account_deletion_runtime") generated 1 warning (1 duplicate)
warning: methods `pin`, `room_id`, and `connection_id` are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:137:19
    |
76  | impl WorldProjectionParent {
    | -------------------------- methods in this implementation
...
137 |     pub(crate) fn pin(&self) -> &ResourcePin {
    |                   ^^^
...
143 |     pub(crate) fn room_id(&self) -> RoomId {
    |                   ^^^^^^^
...
146 |     pub(crate) fn connection_id(&self) -> ConnectionId {
    |                   ^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: methods `actor`, `event_id`, `original_deadline`, `purpose`, and `candidate_source_expires_at` are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:246:23
    |
246 |         pub(crate) fn actor(&self) -> Uuid {
    |                       ^^^^^
...
249 |         pub(crate) fn event_id(&self) -> Uuid {
    |                       ^^^^^^^^
...
264 |         pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
    |                       ^^^^^^^^^^^^^^^^^
...
293 | impl WorldPresenceProjection {
    | ---------------------------- methods in this implementation
294 |     scope_getters!();
    |     ---------------- in this macro invocation
...
304 |     pub(crate) fn purpose(&self) -> &'static str {
    |                   ^^^^^^^
...
318 |     pub(crate) fn candidate_source_expires_at(&self) -> DateTime<Utc> {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: this warning originates in the macro `scope_getters` (in Nightly builds, run with -Z macro-backtrace for more info)

warning: multiple methods are never used
   --> services/concert-service/src/adapters/out/relationship_authority/world_projection.rs:246:23
    |
246 |         pub(crate) fn actor(&self) -> Uuid {
    |                       ^^^^^
...
249 |         pub(crate) fn event_id(&self) -> Uuid {
    |                       ^^^^^^^^
...
252 |         pub(crate) fn session_family_id(&self) -> Uuid {
    |                       ^^^^^^^^^^^^^^^^^
...
255 |         pub(crate) fn request_id(&self) -> Uuid {
    |                       ^^^^^^^^^^
...
258 |         pub(crate) fn credential_expires_at(&self) -> DateTime<Utc> {
    |                       ^^^^^^^^^^^^^^^^^^^^^
...
261 |         pub(crate) fn source_expires_at(&self) -> DateTime<Utc> {
    |                       ^^^^^^^^^^^^^^^^^
...
264 |         pub(crate) fn original_deadline(&self) -> AuthorityRpcDeadline {
    |                       ^^^^^^^^^^^^^^^^^
...
267 |         pub(crate) fn cancel(&self) {
    |                       ^^^^^^
...
367 | impl WorldProjectionAttempt {
    | --------------------------- methods in this implementation
368 |     scope_getters!();
    |     ---------------- in this macro invocation
    |
    = note: this warning originates in the macro `scope_getters` (in Nightly builds, run with -Z macro-backtrace for more info)

warning: function `scoped_claim_for_test` is never used
   --> services/concert-service/src/config/social_delivery.rs:289:21
    |
289 | pub(crate) async fn scoped_claim_for_test(
    |                     ^^^^^^^^^^^^^^^^^^^^^

warning: method `checked_at` is never used
   --> services/concert-service/src/world/recipient_owners.rs:171:19
    |
144 | impl RecipientObservation {
    | ------------------------- method in this implementation
...
171 |     pub(crate) fn checked_at(&self) -> DateTime<Utc> {
    |                   ^^^^^^^^^^

warning: function `identity` is never used
  --> services/concert-service/src/world/invalidation/../../../tests/world_remote_invalidation_support/geometry.rs:24:8
   |
24 | pub fn identity() -> PublicIdentity {
   |        ^^^^^^^^

warning: function `identity` is never used
  --> services/concert-service/src/world/../../tests/world_read_only_support/geometry.rs:24:8
   |
24 | pub fn identity() -> PublicIdentity {
   |        ^^^^^^^^

warning: unused import: `producer::SocialRuntime`
  --> services/concert-service/tests/../src/social_live/mod.rs:11:9
   |
11 | pub use producer::SocialRuntime;
   |         ^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: value assigned to `receipt` is never read
   --> services/concert-service/tests/../src/social_live/producer.rs:190:17
    |
190 |         let mut receipt = None;
    |                 ^^^^^^^
    |
    = help: maybe it is overwritten before being read?
    = note: `#[warn(unused_assignments)]` on by default

warning: `concert-service` (lib test) generated 30 warnings (10 duplicates) (run `cargo fix --lib -p concert-service --tests` to apply 9 suggestions)
warning: `concert-service` (test "social_startup") generated 2 warnings (2 duplicates)
warning: unused import: `Instant`
  --> services/concert-service/tests/intro_product_support/fixture.rs:25:27
   |
25 | use std::time::{Duration, Instant};
   |                           ^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: unused imports: `NativeResolverDomain`, `NativeResolverUnion`, `ProducerTransaction`, `proto::TransactionStatus`, `protocol`, and `settlement::NativeTransactionResolver`
  --> services/concert-service/tests/intro_deletion.rs:25:43
   |
25 |         relationship_authority::guarded::{proto::TransactionStatus, OwnerGuardScope},
   |                                           ^^^^^^^^^^^^^^^^^^^^^^^^
26 |     },
27 |     config::native_resolver::{NativeResolverDomain, NativeResolverUnion},
   |                               ^^^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^
28 |     intro_live::{protocol, IntroCursor, IntroRuntime},
   |                  ^^^^^^^^
...
31 |         scope_ledger::{ProducerTransaction, ScopeKey},
   |                        ^^^^^^^^^^^^^^^^^^^
32 |         settlement::NativeTransactionResolver,
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: fields `relationship_signer` and `settlement_url` are never read
  --> services/concert-service/tests/owner_authority_support/fixture.rs:63:5
   |
56 | pub(super) struct Fixture {
   |                   ------- fields in this struct
...
63 |     relationship_signer: platform::actor_delegation::relationship_read::RelationshipReadSigner,
   |     ^^^^^^^^^^^^^^^^^^^
64 |     settlement_url: String,
   |     ^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: associated items `new`, `bind_delivery`, `interrupted_fence`, `no_unfinished_root`, and `recover_auth_fresh` are never used
   --> services/concert-service/tests/owner_authority_support/fixture.rs:76:25
    |
75  | impl Fixture {
    | ------------ associated items in this implementation
76  |     pub(super) async fn new() -> anyhow::Result<Self> {
    |                         ^^^
...
167 |     pub(super) async fn bind_delivery(&self, scope: &OwnerGuardScope) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^^
...
181 |     pub(super) fn interrupted_fence(
    |                   ^^^^^^^^^^^^^^^^^
...
194 |     pub(super) async fn no_unfinished_root(&self) -> anyhow::Result<bool> {
    |                         ^^^^^^^^^^^^^^^^^^
...
198 |     pub(super) async fn recover_auth_fresh(&self) -> anyhow::Result<u32> {
    |                         ^^^^^^^^^^^^^^^^^^

warning: function `signer` is never used
  --> services/concert-service/tests/owner_authority_support/owners.rs:24:15
   |
24 | pub(super) fn signer() -> anyhow::Result<RelationshipReadSigner> {
   |               ^^^^^^

warning: function `auth` is never used
  --> services/concert-service/tests/owner_authority_support/owners.rs:47:21
   |
47 | pub(super) async fn auth(pool: PgPool, producer_url: &str) -> anyhow::Result<Running> {
   |                     ^^^^

warning: function `friend` is never used
  --> services/concert-service/tests/owner_authority_support/owners.rs:81:21
   |
81 | pub(super) async fn friend(pool: PgPool, producer_url: &str) -> anyhow::Result<Running> {
   |                     ^^^^^^

warning: `concert-service` (test "owner_authority_cancellation") generated 5 warnings
warning: function `publish` is never used
   --> services/concert-service/tests/intro_deletion.rs:164:10
    |
164 | async fn publish(f: &fixture::Fixture, r: &IntroRuntime, restricted: bool) -> anyhow::Result<Uuid> {
    |          ^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: fields `profile_addr` and `profile_shutdown` are never read
  --> services/concert-service/tests/intro_product_support/fixture.rs:41:5
   |
36 | pub(super) struct Fixture {
   |                   ------- fields in this struct
...
41 |     profile_addr: std::net::SocketAddr,
   |     ^^^^^^^^^^^^
42 |     profile_shutdown: Option<tokio::sync::oneshot::Sender<()>>,
   |     ^^^^^^^^^^^^^^^^

warning: methods `stop_profile` and `restart_profile` are never used
   --> services/concert-service/tests/intro_product_support/fixture.rs:144:25
    |
143 | impl Fixture {
    | ------------ methods in this implementation
144 |     pub(super) async fn stop_profile(&mut self) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^
...
153 |     pub(super) async fn restart_profile(&mut self) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^^^^

warning: `concert-service` (test "intro_deletion") generated 5 warnings (run `cargo fix --test "intro_deletion"` to apply 2 suggestions)
warning: constant `REQUEST_PAGE` is never used
  --> services/concert-service/tests/../src/social_live/account_deletion.rs:15:11
   |
15 | pub const REQUEST_PAGE: i64 = 32;
   |           ^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: constant `PUBLICATION_PAGE` is never used
  --> services/concert-service/tests/../src/social_live/account_deletion.rs:16:7
   |
16 | const PUBLICATION_PAGE: i64 = 32;
   |       ^^^^^^^^^^^^^^^^

warning: struct `SharingDeletion` is never constructed
  --> services/concert-service/tests/../src/social_live/account_deletion.rs:20:12
   |
20 | pub struct SharingDeletion {
   |            ^^^^^^^^^^^^^^^

warning: associated items `new`, `apply`, `round`, and `recheck` are never used
   --> services/concert-service/tests/../src/social_live/account_deletion.rs:26:12
    |
25  | impl SharingDeletion {
    | -------------------- associated items in this implementation
26  |     pub fn new(
    |            ^^^
...
39  |     pub async fn apply(
    |                  ^^^^^
...
53  |     async fn round(
    |              ^^^^^
...
171 |     async fn recheck(
    |              ^^^^^^^

warning: function `exact_receipt` is never used
   --> services/concert-service/tests/../src/social_live/account_deletion.rs:186:10
    |
186 | async fn exact_receipt(
    |          ^^^^^^^^^^^^^

warning: function `deletion_context` is never used
   --> services/concert-service/tests/../src/social_live/account_deletion.rs:214:10
    |
214 | async fn deletion_context(
    |          ^^^^^^^^^^^^^^^^

warning: function `next_event` is never used
   --> services/concert-service/tests/../src/social_live/account_deletion.rs:245:10
    |
245 | async fn next_event(
    |          ^^^^^^^^^^

warning: function `audience` is never used
   --> services/concert-service/tests/../src/social_live/account_deletion.rs:253:10
    |
253 | async fn audience(
    |          ^^^^^^^^

warning: function `db` is never used
   --> services/concert-service/tests/../src/social_live/account_deletion.rs:275:4
    |
275 | fn db(_: sqlx::Error) -> DeletionFailure {
    |    ^^

warning: method `drain_account_deletion` is never used
   --> services/concert-service/tests/../src/social_live/invalidation.rs:276:18
    |
119 | impl SocialInvalidationSink {
    | --------------------------- method in this implementation
...
276 |     pub async fn drain_account_deletion(
    |                  ^^^^^^^^^^^^^^^^^^^^^^

warning: associated function `new` is never used
  --> services/concert-service/tests/../src/social_live/producer.rs:54:12
   |
49 | impl SocialRuntime {
   | ------------------ associated function in this implementation
...
54 |     pub fn new(
   |            ^^^

warning: variants `Unavailable` and `Configured` are never constructed
  --> services/concert-service/tests/../src/social_live/provenance.rs:11:5
   |
10 | pub enum SocialClaimProvenance {
   |          --------------------- variants in this enum
11 |     Unavailable,
   |     ^^^^^^^^^^^
12 |     Configured(ConfiguredSharingVerifier),
   |     ^^^^^^^^^^

warning: associated function `configured` is never used
  --> services/concert-service/tests/../src/social_live/provenance.rs:15:12
   |
14 | impl SocialClaimProvenance {
   | -------------------------- associated function in this implementation
15 |     pub fn configured(
   |            ^^^^^^^^^^

warning: associated function `new` is never used
   --> services/concert-service/tests/../src/social_live/read_child.rs:113:12
    |
112 | impl SocialReadSettler {
    | ---------------------- associated function in this implementation
113 |     pub fn new(pool: PgPool) -> Self {
    |            ^^^

warning: `concert-service` (test "social_live") generated 16 warnings (run `cargo fix --test "social_live"` to apply 1 suggestion)
warning: struct `Evidence` is never constructed
   --> services/concert-service/tests/event_operations.rs:107:8
    |
107 | struct Evidence {
    |        ^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "event_operations") generated 1 warning
warning: `concert-service` (test "notification_target_runtime") generated 2 warnings (2 duplicates)
warning: struct `Capture` is never constructed
  --> services/concert-service/tests/owner_authority_support/capture.rs:21:8
   |
21 | struct Capture(Arc<Mutex<Option<VerifiedParentAuthority>>>);
   |        ^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: function `binding` is never used
  --> services/concert-service/tests/owner_authority_support/capture.rs:41:21
   |
41 | pub(super) async fn binding(
   |                     ^^^^^^^

warning: function `gateway` is never used
   --> services/concert-service/tests/owner_authority_support/capture.rs:130:15
    |
130 | pub(super) fn gateway() -> anyhow::Result<ActorDelegationSigner> {
    |               ^^^^^^^

warning: function `identities` is never used
  --> services/concert-service/tests/owner_authority_support/environment.rs:83:21
   |
83 | pub(super) async fn identities(
   |                     ^^^^^^^^^^

warning: fields `relationship_signer` and `settlement_url` are never read
  --> services/concert-service/tests/owner_authority_support/fixture.rs:63:5
   |
56 | pub(super) struct Fixture {
   |                   ------- fields in this struct
...
63 |     relationship_signer: platform::actor_delegation::relationship_read::RelationshipReadSigner,
   |     ^^^^^^^^^^^^^^^^^^^
64 |     settlement_url: String,
   |     ^^^^^^^^^^^^^^

warning: multiple associated items are never used
   --> services/concert-service/tests/owner_authority_support/fixture.rs:76:25
    |
75  | impl Fixture {
    | ------------ associated items in this implementation
76  |     pub(super) async fn new() -> anyhow::Result<Self> {
    |                         ^^^
...
79  |     pub(super) async fn with_native_abort() -> anyhow::Result<Self> {
    |                         ^^^^^^^^^^^^^^^^^
...
82  |     async fn build(native_abort: bool) -> anyhow::Result<Self> {
    |              ^^^^^
...
167 |     pub(super) async fn bind_delivery(&self, scope: &OwnerGuardScope) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^^
...
181 |     pub(super) fn interrupted_fence(
    |                   ^^^^^^^^^^^^^^^^^
...
194 |     pub(super) async fn no_unfinished_root(&self) -> anyhow::Result<bool> {
    |                         ^^^^^^^^^^^^^^^^^^
...
198 |     pub(super) async fn recover_auth_fresh(&self) -> anyhow::Result<u32> {
    |                         ^^^^^^^^^^^^^^^^^^

warning: `concert-service` (test "owner_authority_http") generated 9 warnings (3 duplicates)
warning: associated function `from_request` is never used
  --> services/concert-service/tests/../src/media_runtime/canonical.rs:30:12
   |
25 | impl CanonicalOwnerMediaFence {
   | ----------------------------- associated function in this implementation
...
30 |     pub fn from_request(
   |            ^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: field `binding_digest` is never read
   --> services/concert-service/tests/../src/media_runtime/settlement.rs:239:9
    |
237 | pub struct ResolvedMediaTransaction {
    |            ------------------------ field in this struct
238 |     pub state: MediaTransactionState,
239 |     pub binding_digest: [u8; 32],
    |         ^^^^^^^^^^^^^^

warning: method `status` is never used
   --> services/concert-service/tests/../src/media_runtime/wire.rs:249:12
    |
248 | impl OwnerMediaReply {
    | -------------------- method in this implementation
249 |     pub fn status(&self) -> u16 {
    |            ^^^^^^

warning: `concert-service` (test "owner_media_live") generated 3 warnings
warning: field `meeting` is never read
   --> services/concert-service/tests/support/meeting/mod.rs:145:9
    |
140 | pub struct Fixture {
    |            ------- field in this struct
...
145 |     pub meeting: MeetingDescription,
    |         ^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "meeting_rally") generated 1 warning
warning: unused import: `prost::Message`
 --> services/concert-service/tests/invalidation_clients.rs:6:5
  |
6 | use prost::Message;
  |     ^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` on by default

warning: `concert-service` (test "invalidation_clients") generated 1 warning (run `cargo fix --test "invalidation_clients"` to apply 1 suggestion)
warning: methods `context` and `candidate` are never used
   --> services/concert-service/tests/notification_target_policy_support/fixture.rs:206:12
    |
87  | impl Fixture {
    | ------------ methods in this implementation
...
206 |     pub fn context(&self) -> Context {
    |            ^^^^^^^
...
218 |     pub fn candidate(&self, kind: TargetType) -> Candidate {
    |            ^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "notification_projection_routing") generated 1 warning
warning: associated function `healthy` is never used
  --> services/concert-service/tests/support/event_lifecycle/mod.rs:99:12
   |
98 | impl Owners {
   | ----------- associated function in this implementation
99 |     pub fn healthy() -> Self {
   |            ^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: function `count` is never used
   --> services/concert-service/tests/support/event_lifecycle/mod.rs:137:14
    |
137 | pub async fn count(pool: &PgPool, event: Uuid, step: &str) -> i64 {
    |              ^^^^^

warning: `concert-service` (test "event_lifecycle_read_only") generated 2 warnings
warning: function `owned_pool` is never used
  --> services/concert-service/tests/support/meeting/mod.rs:25:14
   |
25 | pub async fn owned_pool() -> anyhow::Result<PgPool> {
   |              ^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: field `party` is never read
   --> services/concert-service/tests/support/meeting/mod.rs:144:9
    |
140 | pub struct Fixture {
    |            ------- field in this struct
...
144 |     pub party: Uuid,
    |         ^^^^^

warning: `concert-service` (test "qa_community_policy_lock_order") generated 2 warnings
warning: function `count` is never used
   --> services/concert-service/tests/support/event_lifecycle/mod.rs:137:14
    |
137 | pub async fn count(pool: &PgPool, event: Uuid, step: &str) -> i64 {
    |              ^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "event_lifecycle_archive") generated 1 warning
warning: unused import: `sqlx::Row`
  --> services/concert-service/tests/intro_product.rs:33:5
   |
33 | use sqlx::Row;
   |     ^^^^^^^^^

warning: fields `friend`, `registry`, `settlement_url`, and `effects` are never read
  --> services/concert-service/tests/age_runtime_support/fixture.rs:58:16
   |
56 | pub(super) struct Fixture {
   |                   ------- fields in this struct
57 |     pub(super) auth: PgPool,
58 |     pub(super) friend: PgPool,
   |                ^^^^^^
59 |     pub(super) concert: PgPool,
60 |     pub(super) registry: Arc<ScopeRegistry>,
   |                ^^^^^^^^
...
63 |     settlement_url: String,
   |     ^^^^^^^^^^^^^^
64 |     effects: Arc<RejectEffects>,
   |     ^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: associated items `with_native_abort`, `interrupted_fence`, `no_unfinished_root`, `recover_auth_fresh`, and `assert_no_recovery_calls` are never used
   --> services/concert-service/tests/age_runtime_support/fixture.rs:78:25
    |
74  | impl Fixture {
    | ------------ associated items in this implementation
...
78  |     pub(super) async fn with_native_abort() -> anyhow::Result<Self> {
    |                         ^^^^^^^^^^^^^^^^^
...
164 |     pub(super) fn interrupted_fence(
    |                   ^^^^^^^^^^^^^^^^^
...
177 |     pub(super) async fn no_unfinished_root(&self) -> anyhow::Result<bool> {
    |                         ^^^^^^^^^^^^^^^^^^
...
181 |     pub(super) async fn recover_auth_fresh(&self) -> anyhow::Result<u32> {
    |                         ^^^^^^^^^^^^^^^^^^
...
197 |     pub(super) fn assert_no_recovery_calls(&self) {
    |                   ^^^^^^^^^^^^^^^^^^^^^^^^

warning: `concert-service` (test "age_runtime") generated 2 warnings
warning: function `identities` is never used
  --> services/concert-service/tests/canonical_owner_restore_support/environment.rs:91:21
   |
91 | pub(super) async fn identities(
   |                     ^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: associated function `new` is never used
  --> services/concert-service/tests/canonical_owner_restore_support/fault.rs:22:19
   |
21 | impl<S> NegativeComplete<S> {
   | --------------------------- associated function in this implementation
22 |     pub(super) fn new(inner: S, enabled: bool) -> Self {
   |                   ^^^

warning: fields `endpoints` and `profile_deny_complete` are never read
  --> services/concert-service/tests/canonical_owner_restore_support/fixture.rs:49:5
   |
40 | pub(super) struct Fixture {
   |                   ------- fields in this struct
...
49 |     endpoints: CanonicalOwnerEndpoints,
   |     ^^^^^^^^^
...
52 |     pub profile_deny_complete: Arc<std::sync::atomic::AtomicBool>,
   |         ^^^^^^^^^^^^^^^^^^^^^

warning: methods `compose` and `seed` are never used
   --> services/concert-service/tests/canonical_owner_restore_support/fixture.rs:239:12
    |
99  | impl Fixture {
    | ------------ methods in this implementation
...
239 |     pub fn compose(
    |            ^^^^^^^
...
256 |     pub async fn seed(&self) -> anyhow::Result<(Uuid, Uuid, Uuid)> {
    |                  ^^^^

warning: variants `Missing` and `Retained` are never constructed
   --> services/concert-service/tests/canonical_owner_restore_support/fixture.rs:273:5
    |
271 | pub(super) enum KeyMode {
    |                 ------- variants in this enum
272 |     Original,
273 |     Missing,
    |     ^^^^^^^
274 |     Retained,
    |     ^^^^^^^^
    |
    = note: `KeyMode` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis

warning: `concert-service` (test "auth_deletion_owner_ordering") generated 5 warnings
warning: `concert-service` (test "intro_product") generated 2 warnings (1 duplicate)
warning: unused import: `EventPhase`
 --> services/concert-service/tests/place_operations.rs:5:22
  |
5 | use concert_domain::{EventPhase, Revision};
  |                      ^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` on by default

warning: unused import: `concert_application::report::ReportFilter`
   --> services/concert-service/tests/place_operations.rs:229:5
    |
229 | use concert_application::report::ReportFilter;
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `bounded_enter` is never used
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:258:14
    |
258 | pub async fn bounded_enter(owner: &Recorder, count: u32) {
    |              ^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: struct `Evidence` is never constructed
   --> services/concert-service/tests/place_operations.rs:107:8
    |
107 | struct Evidence {
    |        ^^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: struct `Review` is never constructed
   --> services/concert-service/tests/place_operations.rs:139:8
    |
139 | struct Review {
    |        ^^^^^^

warning: function `settings` is never used
   --> services/concert-service/tests/place_operations.rs:197:4
    |
197 | fn settings(enabled: bool) -> Settings {
    |    ^^^^^^^^

warning: `concert-service` (test "age_product_admission") generated 1 warning
warning: `concert-service` (test "place_operations") generated 5 warnings (run `cargo fix --test "place_operations"` to apply 2 suggestions)
warning: fields `receipts` and `cursor` are never read
  --> services/concert-service/tests/../src/config/report_runtime.rs:14:5
   |
13 | pub struct ReportStartupSettings {
   |            --------------------- fields in this struct
14 |     receipts: Arc<FingerprintKeys>,
   |     ^^^^^^^^
15 |     cursor: [u8; 32],
   |     ^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: associated items `load` and `compose_shared` are never used
   --> services/concert-service/tests/../src/config/report_runtime.rs:114:12
    |
62  | impl ReportStartupSettings {
    | -------------------------- associated items in this implementation
...
114 |     pub fn load(grpc: bool, shared_canonical: bool) -> anyhow::Result<Option<Self>> {
    |            ^^^^
...
130 |     pub fn compose_shared(
    |            ^^^^^^^^^^^^^^

warning: function `server` is never used
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:103:8
    |
103 | pub fn server(owner: Arc<Recorder>) -> GuardedService<ProductServiceServer<ProductRpcService>> {
    |        ^^^^^^

warning: fields `operation` and `stop` are never read
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:130:9
    |
129 | pub struct Admission {
    |            --------- fields in this struct
130 |     pub operation: Operation,
    |         ^^^^^^^^^
131 |     pub request: ProductRequest,
132 |     pub stop: Instant,
    |         ^^^^
    |
    = note: `Admission` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during dead code analysis

warning: variants `Response` and `Hold` are never constructed
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:136:5
    |
134 | pub enum Mode {
    |          ---- variants in this enum
135 |     Refuse,
136 |     Response(ProductResponse),
    |     ^^^^^^^^
137 |     Hold,
    |     ^^^^

warning: function `tls` is never used
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:226:8
    |
226 | pub fn tls(workload: Workload) -> anyhow::Result<platform::grpc_tls::TlsMaterial> {
    |        ^^^

warning: function `bounded_enter` is never used
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:258:14
    |
258 | pub async fn bounded_enter(owner: &Recorder, count: u32) {
    |              ^^^^^^^^^^^^^

warning: `concert-service` (test "report_runtime") generated 7 warnings
warning: function `selected_release` is never used
  --> services/concert-service/tests/support/bootstrap/mod.rs:69:8
   |
69 | pub fn selected_release() -> Arc<ValidatedProductRelease> {
   |        ^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: unused import: `MembershipPort`
  --> services/concert-service/tests/world_motion_owner.rs:15:43
   |
15 | use simulation::{AuthorityError, Binding, MembershipPort, ResourcePin};
   |                                           ^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: `concert-service` (test "bootstrap") generated 1 warning
warning: field `policy` is never read
   --> services/concert-service/tests/world_motion_owner.rs:235:5
    |
232 | struct Fixture {
    |        ------- field in this struct
...
235 |     policy: Policy,
    |     ^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "world_motion_owner") generated 2 warnings (run `cargo fix --test "world_motion_owner"` to apply 1 suggestion)
warning: constant `SCHEMA_V1` is never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/mod.rs:14:11
   |
14 | pub const SCHEMA_V1: &str = include_str!("schema-v1.sql");
   |           ^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: variants `Unavailable` and `Configured` are never constructed
  --> services/concert-service/tests/../src/adapters/out/age_provider/runtime.rs:38:5
   |
37 | pub enum ProviderMode {
   |          ------------ variants in this enum
38 |     Unavailable,
   |     ^^^^^^^^^^^
39 |     Configured {
   |     ^^^^^^^^^^
   |
   = note: `ProviderMode` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis

warning: associated items `new`, `with_claim_trust`, `verification_available`, and `projection` are never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/runtime.rs:53:12
   |
52 | impl AgeRuntime {
   | --------------- associated items in this implementation
53 |     pub fn new(
   |            ^^^
...
73 |     pub fn with_claim_trust(mut self, trust: ClaimTrust) -> Self {
   |            ^^^^^^^^^^^^^^^^
...
77 |     pub fn verification_available(&self) -> bool {
   |            ^^^^^^^^^^^^^^^^^^^^^^
...
86 |     pub fn projection(&self, scope: Arc<OwnerGuardScope>) -> PgClaimProjection {
   |            ^^^^^^^^^^

warning: associated functions `new` and `with_claim_trust` are never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/scoped.rs:29:12
   |
28 | impl ScopedProjectionAuthority {
   | ------------------------------ associated functions in this implementation
29 |     pub fn new(scope: Arc<OwnerGuardScope>, verification_available: bool) -> Self {
   |            ^^^
...
36 |     pub fn with_claim_trust(
   |            ^^^^^^^^^^^^^^^^

warning: associated function `new` is never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/settlement.rs:87:12
   |
86 | impl AgeTransactionResolver {
   | --------------------------- associated function in this implementation
87 |     pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
   |            ^^^

warning: associated function `none` is never used
 --> services/concert-service/tests/../src/adapters/out/age_provider/trust.rs:8:12
  |
7 | impl ClaimTrust {
  | --------------- associated function in this implementation
8 |     pub fn none() -> Self {
  |            ^^^^

warning: methods `accepts_claim`, `disclose_until`, and `authorize_in` are never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:27:8
   |
24 | pub trait ProjectionAuthority: Send + Sync {
   |           ------------------- methods in this trait
...
27 |     fn accepts_claim(&self, _verifier: &str, _revision: &str) -> bool {
   |        ^^^^^^^^^^^^^
...
33 |     fn disclose_until(&self, _until: DateTime<Utc>) -> Result<(), PolicyError> {
   |        ^^^^^^^^^^^^^^
...
42 |     async fn authorize_in(
   |              ^^^^^^^^^^^^

warning: struct `ProjectionAge` is never constructed
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:62:12
   |
62 | pub struct ProjectionAge<'a> {
   |            ^^^^^^^^^^^^^

warning: methods `query`, `status`, `timestamps`, `verification_available`, and `permits` are never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:68:12
   |
67 | impl ProjectionAge<'_> {
   | ---------------------- methods in this implementation
68 |     pub fn query(&self) -> ClaimQuery {
   |            ^^^^^
...
71 |     pub fn status(&self) -> AgeStatus {
   |            ^^^^^^
...
77 |     pub fn timestamps(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
   |            ^^^^^^^^^^
...
80 |     pub fn verification_available(&self) -> bool {
   |            ^^^^^^^^^^^^^^^^^^^^^^
...
84 |     pub fn permits(&self, bounds: AgeBounds, sharing: bool) -> bool {
   |            ^^^^^^^

warning: type alias `ProjectionFuture` is never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:88:10
   |
88 | pub type ProjectionFuture<'a, T> =
   |          ^^^^^^^^^^^^^^^^

warning: fields `pool`, `authority`, and `clock` are never read
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:93:5
   |
92 | pub struct PgClaimProjection {
   |            ----------------- fields in this struct
93 |     pool: PgPool,
   |     ^^^^
94 |     authority: Arc<dyn ProjectionAuthority>,
   |     ^^^^^^^^^
95 |     clock: Arc<dyn Clock>,
   |     ^^^^^
   |
   = note: `PgClaimProjection` has a derived impl for the trait `Clone`, but this is intentionally ignored during dead code analysis

warning: associated items `new`, `unavailable`, `project`, `project_inner`, and `project_in` are never used
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:98:12
    |
97  | impl PgClaimProjection {
    | ---------------------- associated items in this implementation
98  |     pub fn new(
    |            ^^^
...
109 |     pub fn unavailable(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
    |            ^^^^^^^^^^^
...
121 |     pub async fn project<T: Send, F>(
    |                  ^^^^^^^
...
142 |     async fn project_inner<T: Send, F>(
    |              ^^^^^^^^^^^^^
...
163 |     pub async fn project_in<T: Send, F>(
    |                  ^^^^^^^^^^

warning: struct `PendingAgeProjection` is never constructed
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:266:12
    |
266 | pub struct PendingAgeProjection<T> {
    |            ^^^^^^^^^^^^^^^^^^^^

warning: method `after_commit` is never used
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:276:12
    |
272 | impl<T> PendingAgeProjection<T> {
    | ------------------------------- method in this implementation
...
276 |     pub fn after_commit(self) -> Result<T, PolicyError> {
    |            ^^^^^^^^^^^^

warning: function `validate` is never used
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:292:4
    |
292 | fn validate(query: ClaimQuery, now: DateTime<Utc>) -> Result<(), PolicyError> {
    |    ^^^^^^^^

warning: function `source_deadline` is never used
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:300:10
    |
300 | async fn source_deadline(
    |          ^^^^^^^^^^^^^^^

warning: function `owner_claim` is never used
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:317:4
    |
317 | fn owner_claim(r: sqlx::postgres::PgRow) -> Result<OwnerClaim, PolicyError> {
    |    ^^^^^^^^^^^

warning: function `unavailable` is never used
   --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:341:4
    |
341 | fn unavailable(_: sqlx::Error) -> PolicyError {
    |    ^^^^^^^^^^^

warning: associated functions `new` and `category_owners` are never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/account_deletion.rs:28:12
   |
27 | impl AgeAccountDeletion {
   | ----------------------- associated functions in this implementation
28 |     pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
   |            ^^^
...
33 |     pub fn category_owners(
   |            ^^^^^^^^^^^^^^^

warning: `concert-service` (test "age_claims") generated 19 warnings
warning: methods `bind_delivery`, `interrupted_fence`, `no_unfinished_root`, and `recover_auth_fresh` are never used
   --> services/concert-service/tests/owner_authority_support/fixture.rs:167:25
    |
75  | impl Fixture {
    | ------------ methods in this implementation
...
167 |     pub(super) async fn bind_delivery(&self, scope: &OwnerGuardScope) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^^
...
181 |     pub(super) fn interrupted_fence(
    |                   ^^^^^^^^^^^^^^^^^
...
194 |     pub(super) async fn no_unfinished_root(&self) -> anyhow::Result<bool> {
    |                         ^^^^^^^^^^^^^^^^^^
...
198 |     pub(super) async fn recover_auth_fresh(&self) -> anyhow::Result<u32> {
    |                         ^^^^^^^^^^^^^^^^^^

warning: `concert-service` (test "product_owner_cancellation") generated 7 warnings (6 duplicates)
warning: unused import: `world_ticket::*`
 --> services/concert-service/tests/world_invalidation/fixture.rs:7:5
  |
7 |     world_ticket::*,
  |     ^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` on by default

warning: unused import: `admission_bridge::*`
  --> services/concert-service/tests/world_invalidation/fixture.rs:11:13
   |
11 |     world::{admission_bridge::*, PgWorldRepository},
   |             ^^^^^^^^^^^^^^^^^^^

warning: unused import: `sqlx::Row`
  --> services/concert-service/tests/world_invalidation/fixture.rs:13:5
   |
13 | use sqlx::Row;
   |     ^^^^^^^^^

warning: field `query` is never read
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:63:5
   |
62 | pub struct ProjectionAge<'a> {
   |            ------------- field in this struct
63 |     query: ClaimQuery,
   |     ^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: methods `query` and `timestamps` are never used
  --> services/concert-service/tests/../src/adapters/out/age_provider/claim_reader.rs:68:12
   |
67 | impl ProjectionAge<'_> {
   | ---------------------- methods in this implementation
68 |     pub fn query(&self) -> ClaimQuery {
   |            ^^^^^
...
77 |     pub fn timestamps(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
   |            ^^^^^^^^^^

warning: `concert-service` (test "age_claim_reader") generated 2 warnings
warning: function `signer` is never used
  --> services/concert-service/tests/owner_authority_support/owners.rs:24:15
   |
24 | pub(super) fn signer() -> anyhow::Result<RelationshipReadSigner> {
   |               ^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "owner_authority") generated 3 warnings (2 duplicates)
warning: unused import: `interest`
  --> services/concert-service/tests/world_invalidation/native.rs:16:42
   |
16 |         admission_bridge::*, ingress::*, interest::*, invalidation::*, membership::*, runtime::*,
   |                                          ^^^^^^^^

warning: `concert-service` (test "world_invalidation") generated 4 warnings (run `cargo fix --test "world_invalidation"` to apply 3 suggestions)
warning: method `run` is never used
  --> services/concert-service/tests/../src/config/social_delivery.rs:73:18
   |
39 | impl SocialDeliveryRuntime {
   | -------------------------- method in this implementation
...
73 |     pub async fn run(self) -> anyhow::Result<()> {
   |                  ^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "social_delivery") generated 1 warning
warning: unused imports: `EventPhase` and `Revision`
 --> services/concert-service/tests/report.rs:5:22
  |
5 | use concert_domain::{EventPhase, Revision};
  |                      ^^^^^^^^^^  ^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` on by default

warning: unused import: `Row`
  --> services/concert-service/tests/deletion_source_rpc.rs:41:20
   |
41 | use sqlx::{PgPool, Row};
   |                    ^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: `concert-service` (test "deletion_source_rpc") generated 2 warnings (1 duplicate)
warning: struct `Review` is never constructed
   --> services/concert-service/tests/report.rs:139:8
    |
139 | struct Review {
    |        ^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: function `settings` is never used
   --> services/concert-service/tests/report.rs:197:4
    |
197 | fn settings(enabled: bool) -> Settings {
    |    ^^^^^^^^

warning: `concert-service` (test "report") generated 3 warnings (run `cargo fix --test "report"` to apply 1 suggestion)
warning: unused import: `Row`
 --> services/concert-service/tests/meeting_photo_migrations.rs:6:37
  |
6 | use sqlx::{postgres::PgPoolOptions, Row};
  |                                     ^^^
  |
  = note: `#[warn(unused_imports)]` on by default

warning: struct `Capture` is never constructed
  --> services/concert-service/tests/operations_live_support/capture.rs:26:8
   |
26 | struct Capture(Arc<Mutex<Option<VerifiedParentAuthority>>>);
   |        ^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: function `binding` is never used
  --> services/concert-service/tests/operations_live_support/capture.rs:46:21
   |
46 | pub(super) async fn binding(
   |                     ^^^^^^^

warning: function `call` is never used
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:114:14
    |
114 | pub async fn call(
    |              ^^^^

warning: fields `operation`, `request`, and `stop` are never read
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:130:9
    |
129 | pub struct Admission {
    |            --------- fields in this struct
130 |     pub operation: Operation,
    |         ^^^^^^^^^
131 |     pub request: ProductRequest,
    |         ^^^^^^^
132 |     pub stop: Instant,
    |         ^^^^
    |
    = note: `Admission` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during dead code analysis

warning: variants `Refuse`, `Response`, and `Hold` are never constructed
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:135:5
    |
134 | pub enum Mode {
    |          ---- variants in this enum
135 |     Refuse,
    |     ^^^^^^
136 |     Response(ProductResponse),
    |     ^^^^^^^^
137 |     Hold,
    |     ^^^^

warning: associated items `new` and `len` are never used
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:147:12
    |
146 | impl Recorder {
    | ------------- associated items in this implementation
147 |     pub fn new(mode: Mode) -> Arc<Self> {
    |            ^^^
...
156 |     pub fn len(&self) -> usize {
    |            ^^^

warning: `concert-service` (test "report_product_native") generated 9 warnings (3 duplicates)
warning: function `fixture` is never used
   --> services/concert-service/tests/meeting_photo_support/mod.rs:100:14
    |
100 | pub async fn fixture() -> anyhow::Result<(PgPool, Fixture, Arc<Service>)> {
    |              ^^^^^^^
    |
    = note: `#[warn(dead_code)]` on by default

warning: function `update` is never used
   --> services/concert-service/tests/meeting_photo_support/mod.rs:152:8
    |
152 | pub fn update(f: &Fixture, id: Option<Uuid>, revision: i64) -> PhotoMutation {
    |        ^^^^^^

warning: function `content` is never used
   --> services/concert-service/tests/meeting_photo_support/mod.rs:183:8
    |
183 | pub fn content(f: &Fixture, beacon: Uuid, id: Uuid, actor: Uuid) -> PhotoFrame {
    |        ^^^^^^^

warning: function `state` is never used
   --> services/concert-service/tests/meeting_photo_support/mod.rs:186:14
    |
186 | pub async fn state(pool: &PgPool, id: Uuid) -> anyhow::Result<(String, Option<Uuid>, bool)> {
    |              ^^^^^

warning: function `pending` is never used
   --> services/concert-service/tests/meeting_photo_support/mod.rs:195:14
    |
195 | pub async fn pending(pool: &PgPool, f: &Fixture) -> anyhow::Result<Uuid> {
    |              ^^^^^^^

warning: field `party` is never read
   --> services/concert-service/tests/meeting_photo_support/../support/meeting/mod.rs:144:9
    |
140 | pub struct Fixture {
    |            ------- field in this struct
...
144 |     pub party: Uuid,
    |         ^^^^^

warning: `concert-service` (test "meeting_photo_migrations") generated 7 warnings (run `cargo fix --test "meeting_photo_migrations"` to apply 1 suggestion)
warning: unused import: `grpc_tls::TlsMaterial`
  --> services/concert-service/tests/sharing_deletion_runtime.rs:37:5
   |
37 |     grpc_tls::TlsMaterial,
   |     ^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: field `stop` is never read
   --> services/concert-service/tests/product_feature_admission_support/fixture.rs:132:9
    |
129 | pub struct Admission {
    |            --------- field in this struct
...
132 |     pub stop: Instant,
    |         ^^^^
    |
    = note: `Admission` has derived impls for the traits `Debug` and `Clone`, but these are intentionally ignored during dead code analysis
    = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "report_product_admission") generated 4 warnings (3 duplicates)
warning: associated function `new` is never used
  --> services/concert-service/tests/canonical_owner_restore_support/fault.rs:22:19
   |
21 | impl<S> NegativeComplete<S> {
   | --------------------------- associated function in this implementation
22 |     pub(super) fn new(inner: S, enabled: bool) -> Self {
   |                   ^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: field `observation` is never read
  --> services/concert-service/tests/canonical_owner_restore_support/fixture.rs:48:9
   |
40 | pub(super) struct Fixture {
   |                   ------- field in this struct
...
48 |     pub observation: Arc<Observation>,
   |         ^^^^^^^^^^^

warning: unused import: `Instant`
  --> services/concert-service/tests/mural_tools_b_support/fixture.rs:27:27
   |
27 | use std::time::{Duration, Instant};
   |                           ^^^^^^^
   |
   = note: `#[warn(unused_imports)]` on by default

warning: `concert-service` (test "canonical_owner_restore") generated 2 warnings
warning: function `notification_server` is never used
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:100:14
    |
100 | pub async fn notification_server(pool: PgPool) -> anyhow::Result<Running> {
    |              ^^^^^^^^^^^^^^^^^^^

warning: struct `Hostile` is never constructed
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:151:8
    |
151 | struct Hostile {
    |        ^^^^^^^

warning: struct `HostileRunning` is never constructed
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:175:12
    |
175 | pub struct HostileRunning {
    |            ^^^^^^^^^^^^^^

warning: function `hostile_server` is never used
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:180:14
    |
180 | pub async fn hostile_server() -> anyhow::Result<HostileRunning> {
    |              ^^^^^^^^^^^^^^

warning: function `saved_path` is never used
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:204:4
    |
204 | fn saved_path() -> anyhow::Result<std::path::PathBuf> {
    |    ^^^^^^^^^^

warning: function `save` is never used
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:207:8
    |
207 | pub fn save(value: &Saved) -> anyhow::Result<()> {
    |        ^^^^

warning: function `load` is never used
   --> services/concert-service/tests/account_deletion_runtime_support/mod.rs:217:8
    |
217 | pub fn load() -> anyhow::Result<Saved> {
    |        ^^^^

warning: `concert-service` (test "sharing_deletion_runtime") generated 9 warnings (1 duplicate) (run `cargo fix --test "sharing_deletion_runtime"` to apply 1 suggestion)
warning: fields `profile_addr`, `profile_shutdown`, and `reader` are never read
  --> services/concert-service/tests/mural_tools_b_support/fixture.rs:43:5
   |
38 | pub(super) struct Fixture {
   |                   ------- fields in this struct
...
43 |     profile_addr: std::net::SocketAddr,
   |     ^^^^^^^^^^^^
44 |     profile_shutdown: Option<tokio::sync::oneshot::Sender<()>>,
   |     ^^^^^^^^^^^^^^^^
45 |     pub(super) reader: Arc<PublicProfileReader>,
   |                ^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: methods `stop_profile` and `restart_profile` are never used
   --> services/concert-service/tests/mural_tools_b_support/fixture.rs:146:25
    |
145 | impl Fixture {
    | ------------ methods in this implementation
146 |     pub(super) async fn stop_profile(&mut self) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^
...
155 |     pub(super) async fn restart_profile(&mut self) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^^^^

warning: `concert-service` (test "mural_tools_b") generated 3 warnings (run `cargo fix --test "mural_tools_b"` to apply 1 suggestion)
warning: function `identities` is never used
  --> services/concert-service/tests/owner_authority_support/environment.rs:83:21
   |
83 | pub(super) async fn identities(
   |                     ^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: `concert-service` (test "child_recovery_runtime") generated 1 warning
warning: function `self_signer` is never used
  --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:24:15
   |
24 | pub(crate) fn self_signer() -> anyhow::Result<ConcertSelfProfileSigner> {
   |               ^^^^^^^^^^^

warning: function `raw_settings` is never used
  --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:30:15
   |
30 | pub(crate) fn raw_settings(
   |               ^^^^^^^^^^^^

warning: function `configured` is never used
  --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:52:15
   |
52 | pub(crate) fn configured(
   |               ^^^^^^^^^^

warning: function `configured_with_keys` is never used
  --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:58:15
   |
58 | pub(crate) fn configured_with_keys(
   |               ^^^^^^^^^^^^^^^^^^^^

warning: function `additional` is never used
  --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:73:15
   |
73 | pub(crate) fn additional() -> anyhow::Result<ParticipantProductOwners> {
   |               ^^^^^^^^^^

warning: fields `subject` and `deny_complete` are never read
  --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:95:9
   |
87 | pub(crate) struct Fixture {
   |                   ------- fields in this struct
...
95 |     pub subject: Uuid,
   |         ^^^^^^^
96 |     pub event: Uuid,
97 |     pub deny_complete: Arc<AtomicBool>,
   |         ^^^^^^^^^^^^^

warning: method `restart_owners` is never used
   --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:144:25
    |
100 | impl Fixture {
    | ------------ method in this implementation
...
144 |     pub(crate) async fn restart_owners(&mut self) -> anyhow::Result<()> {
    |                         ^^^^^^^^^^^^^^

warning: struct `Mounted` is never constructed
   --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:169:19
    |
169 | pub(crate) struct Mounted {
    |                   ^^^^^^^

warning: associated items `new` and `stop` are never used
   --> services/concert-service/tests/shared_participant_composition_support/fixture.rs:189:25
    |
188 | impl Mounted {
    | ------------ associated items in this implementation
189 |     pub(crate) async fn new(runtime: ParticipantRuntime) -> anyhow::Result<Self> {
    |                         ^^^
...
209 |     pub(crate) async fn stop(mut self) -> anyhow::Result<()> {
    |                         ^^^^

warning: method `stop` is never used
  --> services/concert-service/tests/shared_participant_composition_support/owners.rs:21:25
   |
20 | impl Running {
   | ------------ method in this implementation
21 |     pub(crate) async fn stop(mut self) -> anyhow::Result<()> {
   |                         ^^^^

warning: function `concert` is never used
   --> services/concert-service/tests/shared_participant_composition_support/owners.rs:180:21
    |
180 | pub(crate) async fn concert(
    |                     ^^^^^^^

warning: `concert-service` (test "intro_startup") generated 14 warnings (3 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.96s
cargo check -> exit=0
$ tsc -b --pretty false
typecheck -> exit=0
DONE
