# Global enrollment policy and durable MCR consent

Task: AUTH-CONSENT-ADMISSION-01. Implementation owner: backend207. Publishing owner: RootBE1fc. Frontend counterparts: Root153 and developer204. This is global service consent, not event admission, physical arrival, verified age, a legal eligibility finding or a provider result.

Status: INTEGRATED in isolated native qualification for the version 1.0 interface below; implementation, actual Auth/Gateway compilation and the real database/RPC/HTTP cases below passed. Canonical publication and frontend browser integration are pending. No approved production policy has been supplied or published by this implementation. Empty publication is an honest POLICY_UNAVAILABLE gate; tests may explicitly publish synthetic test documents only in isolated test databases and may not certify production legal approval.

Base: qualified runtime tip `5cee81ee63c3ce2a03882ce5493e43e31d427ddc`, retained in the same isolated worktree on `codex/mcr-be-auth-consent-admission`. Its runtime delivery is independently published at `7f8d38ecc6bd84cfda5931a0f972657596f99501`. Core generator/source/artifact baseline was compared against fetched/shared `origin/master` `ff96d53eb817c93dd1bafb5c4f125630f995413b`; those exact paths were identical before these additive changes. No old rendering, source pack or `docs/source/` edits.

## Public contract

Source DTOs: `contracts/core/v1/core.schema.json`. Registration: `contracts/core/v1/operations.json`. Generator: `backend/scripts/docs/generate-mcr-core.py`. Mirrored OpenAPI: `backend/docs/api/mcr-core.openapi.json`, `contracts/generated/mcr-core.openapi.json`. No handwritten frontend file is owned by this backend assignment. The initially suggested `frontend/src/shared/api/generated/core.ts` does not exist at this source baseline.

The legacy `/api/auth/email/signup` browser handler is retired: generic 422 ACKNOWLEDGEMENTS_REQUIRED before account lookup; private canonical Auth signup remains unchanged. All new paths start `/api/auth/enrollment`. Every response is no-store. Mutations require the existing exact Origin allowlist and `X-MCR-Intent: browser`. Fetch uses `credentials: include`; no client reads or persists a binding secret. The Gateway sets a host-only `__Secure-mcr-enrollment` HttpOnly Secure SameSite=Strict cookie, Path=/api/auth/enrollment, when beginning an attempt. Explicit local HTTP development uses a separately named non-Secure development cookie; production never falls back. The binding is hashed by Auth and is distinct from canonical identity/access/refresh credentials. Attempt IDs and idempotency keys are nonnil canonical UUIDs. Idempotency is in the JSON body only; no header alias is accepted. Timestamps are server UTC; clients bound freshness from monotonic request start and must not extend a server expiry using a local wall clock.

| Method/path | Operation ID | Request | Success |
|---|---|---|---|
| GET /policy | getEnrollmentPolicy | none | 200 EnrollmentPolicy |
| POST /attempts | beginEnrollmentAttempt | EnrollmentBeginRequest | 201 EnrollmentAttempt |
| POST /signup | submitEnrollmentSignup | EnrollmentSignupRequest | 202 EnrollmentAccepted |
| POST /verify | verifyEnrollmentEmail | EnrollmentVerifyRequest | 200 EnrollmentVerified |
| POST /resend | resendEnrollmentVerification | EnrollmentResendRequest | 202 EnrollmentAccepted |
| GET /admission | getEnrollmentAdmission | bearer, no body | 200 EnrollmentAdmission |
| POST /consent | acceptEnrollmentPolicy | bearer, EnrollmentConsentRequest | 200 EnrollmentAdmission |

All DTOs are closed objects. `EnrollmentPolicy` has `contractVersion: "1.0"`, `revision`, `documents`, `publishedAt`, `expiresAt`, `serverTime`. Each document has `id`, `title`, `url` (HTTPS), `sha256` (64 lowercase hexadecimal digits), and `requiredAcknowledgementId`. These references and hashes must identify the actual separately approved documents, not application-generated legal text. This application does not infer legal approval from a syntactically valid approval reference.

`EnrollmentBeginRequest` is `{policyRevision}`. `EnrollmentAttempt` is `{attemptId,policyRevision,expiresAt,serverTime}`. An attempt lasts at most 30 minutes and never outlives the policy. Reusing a browser cookie across multiple tabs is allowed; each attempt is independently bound and serialized.

`EnrollmentSignupRequest` is `{attemptId,policyRevision,acknowledgementIds,idempotencyKey,email,password}`. Explicit acknowledgement IDs must exactly equal the current policy's required set, with no duplicates or extras. Password bounds remain the canonical 8–128 UTF8 bytes. `EnrollmentAccepted` is `{status:"ACCEPTED",attemptId,expiresAt,resendAvailableAt,serverTime}`. No account UUID, account-existence hint or delivery claim is returned. An existing active account or an incorrect password creates a shadow attempt only: it cannot change that account, its credentials, its challenge or its consent. A pending account can recover an expired attempt only after the same admitted canonical password worker verifies its existing password. The transaction rechecks the exact account ID, email, password hash, pending status and live row, then binds a fresh attempt/current acknowledgements and canonical resend. It never changes the password or resumes an active identity. Its public receipt remains indistinguishable from a shadow. New identity/challenge/mail/acceptance and the bound attempt receipt commit together. Same key and same normalized payload replays the original receipt; changing the key or payload after signup conflicts. The request fingerprint is HMAC-SHA256 under the unpersisted browser binding, never a raw password digest.

`EnrollmentVerifyRequest` is `{attemptId,code}`; code is six digits. `EnrollmentVerified` is `{status:"VERIFIED",attemptId,serverTime}`. Verification resolves the email internally from the bound canonical account, never from a caller-supplied UUID/email. Wrong code, shadow attempt, absent usable challenge and exhausted challenge attempts share VERIFICATION_FAILED. Canonical wrong-code attempt increments remain durable. A successful replay is bound to the same unexpired attempt/browser. The client then uses existing canonical email login; verification does not issue a synthetic session.

`EnrollmentResendRequest` is `{attemptId,idempotencyKey}`. Its receipt has the same generic Accepted shape, including a server-authoritative resend time, whether mail was eligible or not. Resend cannot extend the attempt or policy. Original mail throttles and challenge rotation remain canonical. Retry an unknown outcome only explicitly, using the exact original attempt/key/payload; no automatic replay or new key guessing. A network failure is an unknown outcome, not proof that an account or consent was created.

Verification UI query prefills may contain only `attemptId` and `code`; frontend route spelling belongs to Root153. The original browser binding is required. A cross-browser email link must honestly request the original browser rather than bypass binding. The enrollment signup/resend transaction adds the opaque attempt and `/verify-email?attemptId=...&code=...` link to its actual mail payload; the mail worker projects these fields to its configured provider. `AUTH_ENROLLMENT_VERIFICATION_URL` is an explicit full `/verify-email` URL: HTTPS in production, loopback HTTP only in explicit development, no userinfo/query/fragment. Missing configuration refuses signup with DEPENDENCY_UNAVAILABLE. The link remains browser-bound and never carries an email, user UUID or binding secret. Legacy verification mail retains its previous shape.

`EnrollmentAdmission` is `{decision,policyRevision,acceptedRevision,serverTime}`, with nullable revision UUIDs and decision ADMITTED, CONSENT_REQUIRED, POLICY_UNAVAILABLE or ACCOUNT_INACTIVE. Reads and commands use verified canonical actor delegation. `EnrollmentConsentRequest` is `{policyRevision,acknowledgementIds,idempotencyKey}` and records durable current acceptance for an existing active account, including an actual verified OAuth identity, without creating another identity. The client explicitly routes CONSENT_REQUIRED through policy/consent before bootstrap. No external age provider is required for basic email identity; restricted-age features retain their separate gate.

Error envelopes retain `code,message,traceId,timestamp,details.reason`. Reasons: POLICY_UNAVAILABLE 503; POLICY_STALE 409; ATTEMPT_EXPIRED 410; ATTEMPT_INVALID 404; ACKNOWLEDGEMENTS_REQUIRED 422; IDEMPOTENCY_CONFLICT 409; VERIFICATION_FAILED 422; INVALID_INPUT 422; ACCOUNT_INACTIVE 403; DEPENDENCY_UNAVAILABLE 503. Existing origin/media/size/auth/capacity errors remain meaningful. Capacity 429 retains Retry-After. No anonymous duplicate-email-specific status.

## Server admission and publication

A policy revision is immutable, including document hashes and approval provenance. Publication takes the singleton current-policy lock; enrollment and authenticated consent hold a shared lock across their transaction. A missing/expired policy, changed revision, expired attempt, missing acknowledgement or changed idempotent payload cannot positively admit. Server time comes from PostgreSQL; client clocks/booleans cannot establish acceptance.

The canonical Auth current MCR read checks actual durable current acceptance separately from account/session/provider facts. CONSENT_REQUIRED is an exact trusted gRPC FailedPrecondition denial, not a false account_active fact. POLICY_UNAVAILABLE is exact trusted Unavailable. The Gateway preserves these meanings to the MCR caller. Historical runtime evidence at an earlier source revision remains historical; this gate does not retroactively change that proof.

Operator publication is a separate authorized input/action. No public HTTP publication route, fabricated default policy, migration seed acceptance or user-derived publication operation is provided. Production legal text/publication and restricted-age provider inputs remain external gates, while persistence, replay, expiry and server decisions are implementation obligations.

## Validation record

Responsible implementation owner qualification, 2026-11-04 UTC:

- Contract source freeze: `ed50c2f1`; implementation: `c093d8a5`; password-proven pending recovery: `c988365c`; compiler fixes: `2f7992d43ed266f863d6622a20d2974740908316`. The latter is the exact compiled Rust source. Controller-only short-root/database setup fix: `5a9a67dd`. No Rust source changed between the compiled source and the native run.
- `python3 backend/scripts/docs/generate-mcr-core.py` and `--check`: exit 0. Existing schema definitions/operations were semantically unchanged except the explicitly retired public signup documentation and additive enrollment DTOs/operations/reasons. `python3 scripts/mcr/check-plan.py`: exit 0; this checks planning/source archives, not application acceptance.
- All three commands used `cargo +1.88.0`, `--manifest-path backend/Cargo.toml --locked --offline -j 2 --config profile.dev.package.contracts-build.opt-level=2 --message-format=json`, debug 0/incremental 0, and unset both RUSTFLAGS variables and RUSTC_WRAPPER. Commands: `test -p auth-service --test enrollment_policy --no-run`, `build -p auth-service --bin auth-service`, and `build -p api-gateway --bin api-gateway`: all exit 0. No manifest, dependency resolution or lockfile edits. The first compile failed on an extracted connection variable, a necessary exhaustive shared error match and use of a test-only random dependency. The scoped repairs are in `2f7992d4`; prior failed artifacts remain retained.
- Exact compiler-artifact copies: enrollment test SHA256 `85eaf6187a14e620206f4004888b99ef0248fa49972ab12a38ab19d5795f19a4`; Auth actual main `2a97e0420661f23b5618da0a54b800b975cc6a4ea2ea479a815a2409856f0016`; Gateway actual main `bb3b10e836e4855561a77c7a9b79ad357b4c1be26fd7cc5df98c490772cc08ee`. Combined copies 161,936,032 bytes. The mains were compiled; the native case starts the actual service/router implementations inside the test executable, not three separately launched main binaries.
- Copied test executable `--exact publication_and_closed_inputs_reject_fabricated_or_ambiguous_values --nocapture`: exit 0, one passed. The internal store HMAC unit test was not part of this target and is NOT_RUN.
- `python3 backend/scripts/mcr/enrollment_policy_native.py --binary <frozen-test> --sha256 <above> --output <owned-evidence> --absolute-stop <original-native-cutoff> --guardian-pid <independent-guardian> --cluster-root <new-short-owned-PG-root>`: exit 0. Initial long temporary socket-path startup failed before any test phase; its owned cluster was cleaned, failure retained, and controller-only fix `5a9a67dd` reran inside the original allocation clock without target reentry.
- `native_policy_signup_atomicity_guarded_rpc_gateway_and_restart --ignored --exact --nocapture --test-threads=1`: SOURCE exit 0, 10.736 seconds; separate RESTART process exit 0, 2.542 seconds. Real runner-owned PG16, 19 successful actual Auth migrations, zero prepared transactions, postmaster PID changed 33617 to 34571. A fresh test PID rebuilt its real RPC and Gateway HTTP servers after the physical restart.
- SOURCE exercised absent policy, retired legacy signup generic denial, browser Origin denial, invalid RPC workload credentials, cookie-bound attempts, exact acknowledgements, actual admitted canonical password work and identity/challenge/mail/consent transaction, exact receipt replay and changed-payload conflict, actual mail-worker projection to an owned local HTTP capture, wrong-password shadow isolation, wrong cookie/code denial with durable challenge failure count, expired attempt denial, password-proven pending recovery without password/identity replacement, verification, real canonical JWT login and high-risk session read, current Auth MCR admission, policy rotation/CONSENT_REQUIRED, authenticated exact acknowledgement acceptance and replay, and acceptance-insert failpoint rollback with no orphan account/challenge/mail. RESTART proved durable current policy/acceptances and byte-equivalent stored signup receipt; it did not insert positive user or consent fixtures.
- Native report SHA256 `4fd95bb8dc771191f2e3fbeb1d39e75d6aa2a79e815808e97258bb63006c28c6`. Private custody contains exact Cargo JSON/argv, copied binary hashes, source/SQL/tool pins, both phase logs, all failure artifacts, independent guardian receipts, and target/FULL releases. No provider mail was sent externally and no real user's data was used.
- Allocation accepted 18:01:42.660785 UTC; target released with compiler/handles/readers 0 at 18:04:06.250833, before native qualification. FULL released 18:05:52.668491, 106.418 seconds after target release. Owned PG/RPC/workload/guardian processes 0, owned cluster removed, no target reentry. Retained whole peak 4,701,802,496 bytes / NEW peak 321,359,872 bytes under the original bounds; no history deletion or carry shrink.

Limits: isolated native Auth/Gateway HTTP qualification is not actual frontend browser/device acceptance, OAuth-provider end-to-end proof, actual external mail delivery, production legal approval/publication, a restricted-age decision, load/performance proof, populated Sessions deletion proof or full BE42 acceptance. Missing production policy remains POLICY_UNAVAILABLE. No static/unit result substitutes for those separate gates.


## Paired browser serving fixture source (NOT_RUN)

An additive `tests/enrollment_policy_browser.rs` target and `scripts/mcr/enrollment_policy_browser.py` controller prepare the next real browser boundary. They are source-authored only; the already-qualified native target above is unchanged. This requires a fresh BE-owned finite allocation and one new copied test executable. There is no current resource hold or reused expired lease.

The fixture accepts only an explicitly owned local frontend origin `http://localhost:3000`. The FE owner supplies an actual Vite same-origin `/api` reverse proxy to the ready artifact's ephemeral `gatewayOrigin`, preserving Origin and Set-Cookie. The browser sees localhost on both sides and retains Strict HttpOnly cookies. The fixture runs the actual Gateway transport/router with canonical Auth RPC clients for login, refresh, logout and high-risk admission; unrelated fake ports remain unused/unclaimed. Auth and Gateway have separate fresh runner-owned PG16 databases and actual migrations. The operator API publishes explicitly synthetic test documents; there are no positive account, password, verification, session or consent SQL fixtures. The production policy remains absent.

The actual mail worker sends to the owned local capture. Private `browser-mail-N.json` files expose only fixture attempt ID, verification link/code and expiry to the FE pairing owner. `browser-ready.json` reports the Gateway/frontend origins, synthetic policy revision and explicit serving-only verdict. Writing `browser-stop`, the original fixture deadline or the outer guardian stops the owned fixture. The controller enforces a maximum 300 seconds plus the fresh allocation's stricter original cutoff, short owned PG socket root and no Cargo/target access. All PG/RPC/workload cleanup remains BE-owned. A serving fixture exit is never labeled browser acceptance; the FE owner must actually exercise and record browser signup, outbox-link verification, canonical login and admission. Production legal documents, external mail and unrelated World/bootstrap flows remain outside that fixture.


The browser fixture controller additionally requires `--migration-pins` pointing to its producing build's private compiled-source `source-sql-pins.json`. Auth and Gateway migrations are runtime reads from the compile checkout, not embedded SQL proof. Before creating PG or any serving child, the controller refuses an added/missing SQL file or changed byte versus the exact producing-source inventory, and records the actual runtime SQL SHA256 closure in its separate serving report. The source-only Sessions additions do not alter the qualified Auth/Gateway application source or their runtime SQL. This controller guard itself remains NOT_RUN until the fresh queued boot round.

The source-authored `browser-ready.json` now also carries the controller-verified `binarySha256`, producing `sourceRevision`, actual `runtimeMigrationPaths` and complete `runtimeMigrationSha256` inventories. The controller verifies those SQL inputs again after serving; drift fails the round. These bindings are fixture provenance, not frontend acceptance. This updated test/helper has not yet compiled or run.

The source-authored serving fixture performs bounded actual HTTP boot probes before publishing ready: actual published Auth policy (200, no-store, no cookie mint), denied missing-Origin begin (403), and a real Origin-bound begin (201 with the HttpOnly Strict scoped local-development cookie). No account or acceptance is created by this boot probe. The ready descriptor also exposes `gatewayLoopbackOrigin` using the actual bound IPv4 loopback address for a DNS-independent Vite upstream; browser origin stays exactly `http://localhost:3000` and all Origin/cookie protections stay in effect. The stop receipt records these boot statuses separately from FE-owned browser acceptance. NOT_RUN until the fresh named boot round.


The actual browser fixture's remaining original criteria now have source-authored private one-shot controls. Canonical challenge TTL is hardcoded one hour; production TTL and resend quotas are unchanged. A real browser signup must first create the actual pending identity/challenge and deliver its actual mail. Atomically create mode0600 `browser-expire.json` with exactly `{"attemptId":"<canonical UUID>"}`. The fixture requires that actual captured attempt/current pending canonical user and code, and updates only that current `email_verifications.expires_at` into the past. No identity, acceptance, attempt state, mail capture, audit or receipt is fabricated. `browser-challenge-expired.json` contains `attemptId`, `actualChallengeExpiresAt`, `serverTime`, and `negativeOnly:true`. Its original actual mailbox `expiresAt` is preserved. Real verification then must reject the expired challenge through the ordinary HTTP/RPC/store path; actual resend can replace it through the ordinary canonical transaction.

Actual signup/resend receipts carry the server's60-second `resendAvailableAt`; an early resend stays a generic202 with the same deadline and emits no new mail. After that deadline (with a small real scheduling margin), explicit resend must yield the latest actual mail/code and a fresh60-second cooldown. The canonical one-minute/20-per-day limits remain in force; the bounded fixture does not claim daily-quota exhaustion. This sequence fits the separately admitted<=300-second paired round without a fake clock or TTL change.

After actual verification, before login, atomically create mode0600 `browser-rotate.json` with exactly `{"expectedRevision":"<initial policy UUID>"}`. The fixture requires a real verified active signup under that revision, then invokes the actual operator `EnrollmentStore.publish` with a distinct explicitly synthetic v2 policy; it inserts no acceptance. `browser-policy-rotated.json` contains `previousRevision`, `policyRevision`, `requiredAcknowledgementIds`, `serverTime`, and `syntheticPolicyOnly:true`. The immutable initial ready file remains truthful. Ordinary canonical login/current admission must now require the real user's renewed policy acknowledgements/consent, followed by fresh admitted state. These are private fixture file controls, with bounded owned-file/inode checks, not public product endpoints or production legal approval.

The fixture now refuses every non-`/api/auth/` path with a real503 error, so the helper's unused foundation test ports cannot accidentally supply a synthetic bootstrap/profile to the actual frontend. `browser-ready.json` identifies `authOnly:true`. Full World continuation remains a separate real seven-service gate. All newly added controls remain NOT_COMPILED / NOT_RUN until the fresh backend build/boot and actual paired-browser rounds; no server lifecycle exit alone qualifies these cases.

## Additive Enrollment + Wall Gateway dependency checkpoint (NOT_COMPILED / NOT_RUN)

Root's IN04 dependency grant explicitly includes the selected Wall B participant
routes as well as Wall202's four admin routes in the single Gateway feature file
`backend/services/api-gateway/src/adapters/in/http/concert_product/feature_routes.rs`.
The checkpoint starts from Enrollment branch revision
`71b1b8ab88d157bb0433a91843cd68a76ab5705e`; it does not replace that file wholesale
or import another domain, service main, contract, dependency or migration.

Published participant source is
`e0784fa59243c923c620a88cf328c94a363183c9`, with the same feature-file blob
`005d7a86de293878ce86e0795deceee4271f9051` also present at
`b97608d4235eb7f82cba29c4807e219f7f606e3c` and
`13d0a6ad57def630dcb76ed3747c5a5dbe4c93ac`.
The nine participant operations cover snapshot, operations after a revision,
pixel placement and its own result, tools, own action result, erase, fill and
undo. Their original closed DTOs, byte limits, canonical UUID checks and exact
query parsing are retained. Published source provenance does not qualify this
new combined executable.

Separate, not yet qualified admin source is
`13ca63ad960ea33cd43ad96ce12cfca712229f29`, feature-file blob
`cab545eeaa5f865850b71511d19384c7d22bd93f`, SHA256
`5c7f840d785a85533803e12be02b0c59e6005a690a0ed619da3b6815842eb6db`.
Wall202 supplied this immutable source directly. The four admitted operations
are GET `/admin/mural`, POST `/admin/mural/control`, POST `/admin/mural/locks`
and POST `/admin/mural/restore`, below the existing event prefix. Same-file
variants, mutation/receipt selection, 8192-byte command limit, closed typed
bodies, operation IDs and protection of unknown admin suffixes accompany those
routes. Current authorization, held operator role and actual settlement still
belong to the production Concert owners; Gateway admission grants no role.

The original thirteen feature operations remain. The frozen-contract unit
probe now expects twenty-six operations, including both sets of Wall routes;
its original substitutions additionally resolve the two Wall result-key
parameters. Imported participant and admin admission tests remain executable
source, not passing evidence. Rust formatting and `git diff --check` pass.
Compilation and all combined HTTP/native cases are NOT_RUN. In particular,
`2f7992d4`'s copied Gateway remains historical Auth qualification and cannot be
used as the Wall-capable IN04 producer.

The genuine combined Gateway build plan is one new actual main executable:

```sh
cargo +1.88.0 build --manifest-path backend/Cargo.toml --locked --offline -j 2 \
  --config profile.dev.package.contracts-build.opt-level=2 \
  -p api-gateway --bin api-gateway --message-format=json
```

This command requires its own explicitly updated/named allocation and actual
frozen readset before execution. Retain debug=0, incremental=0 and unset both
Rust flags and both compiler-wrapper variables; use only the existing retained
warm target. Preserve the Cargo artifact record, immutable producing source,
all inputs, exact copied EXE SHA256 and actual Gateway runtime SQL closure.
Release the target before any separately allocated copied-native use. Root's
single-stack assembler consumes the new Gateway alongside current Enrollment
Auth19 and its coherently composed Concert main; no old Auth substitution or
second stack is introduced. Build success alone will not qualify IN04.

The queued `ENROLLMENT-BROWSER-FIXTURE-MEDIA-20261104T1902Z` was unaccepted when
this source changed. Its old frozen readset remains preserved and is ineligible
for admission until explicitly updated; this note does not add another build
output to that one-test-executable offer. No accepted epoch, cap or deadline is
changed, and no compiler, PostgreSQL process or target handle is held.

### Qualified copied browser-fixture boot — producing source `857a6955`

The single `auth-service --test enrollment_policy_browser --no-run` target
compiled at `857a69559631db52f1febdb7692888f089a9575c`, using the declared
Rust 1.88 locked/offline/j2 profile, debug 0 and incremental 0. Cargo exited 0.
The one frozen test EXE is 57,061,232 bytes, SHA256
`8c0c8f06e290717ac877c6d068d023ee58f2e60434327850180d1171c31a2e55`.
This does not qualify an actual Gateway main or any Wall operation.

After releasing the target, the copied fixture ran once against a fresh owned
PG16 root. Its exact ignored test exited 0 in 3.25 seconds. Actual boot probes
returned current synthetic policy 200, begin without Origin 403, and bound
begin with the allowed local Origin 201 and the scoped development HttpOnly
Strict cookie. The runtime SQL closure remained exactly Auth 19 and Gateway 17;
there were zero prepared transactions. The controller exited 0, removed its
owned PG root, and reported `SERVING_STOPPED` with an explicitly FE-owned
browser verdict. No account or mail was expected or created by this boot.
Expiry and rotation controls compiled but were not exercised. Real signup,
resend timing, expiry, renewed-consent browser cases and actual frontend
acceptance remain NOT_RUN.

Private custody is `/private/tmp/mcr-browser207-1902-d27qutm4`, containing the
actual Cargo artifact/argv, source/SQL/tool/helper pins, copied EXE, ready/stop
artifacts, independent guards and build/native/target/FULL receipts. Controller
report SHA256 is
`eca32e5d1f05e2f4b8d054f8ddcbaaac0c0d2c47274d52cb4a2498d74881cc2f`.
The boot descriptor's ephemeral listeners are stopped; it is protocol and boot
evidence, not an available server. A future paired run must have fresh finite
native/browser allocations, a new private output directory, both named input
controls and their owner-only atomic publication paths, and the same producing
EXE/source/SQL identity. The FE owner records the actual browser verdict.

Allocation accepted at 19:26:20.384748 UTC on 2026-11-04. Target release was
19:27:42.079189, sent directly to Operations before any PG/native child. FULL was
19:28:36.686532, 54.607343 seconds after target release. All owned compilers,
guards, PG/RPC/HTTP processes, binary readers and workflows were zero; the root
was absent and no target reentry occurred. Inclusive positive growth peaked at
200,519,680 bytes and retained whole at 4,902,322,176 bytes, which becomes a
nonshrinking future carry floor. The final copy endpoints were reconciled
explicitly because a copy can finish between periodic guardian samples; the
original guardian sample and reconciliation are both retained. This resource
release adds no browser acceptance or production legal-policy approval.

### Additive current-role discovery source for the queued Gateway main

Before the separate Gateway-main turn was accepted, Root additionally assigned
only the same-file `OwnOperationsAccess` leaf from Operations source
`27f674a2932e66b8f728b5430717d1bd9ea5329e`, feature-file blob
`59d7bea64358844e04c213dd8c1d4fba1b5bb8e7`. The exact route is bodyless SELF GET
`/api/concert/events/:event_id/me/operations-access`, operation ID
`getOwnConcertOperationsAccess`. Router, enum, protected namespace, exact
method/path and operation mapping are additive; existing zero read-body limit,
no query and no receipt/mutation defaults remain. The frozen feature-operation
probe now expects twenty-seven. No Report or other Operations endpoint, main,
contract, shared producer or role factory was copied. Root owns the one prepared
Operations factory, current-role producer and shared Wall dispatch composition.
The already compiled fixture retains producing revision `857a6955`; this newer
Gateway source requires its own immutable compile pin and main qualification.
