# Global enrollment policy and durable MCR consent

Task: AUTH-CONSENT-ADMISSION-01. Implementation owner: backend207. Publishing owner: RootBE1fc. Frontend counterparts: Root153 and developer204. This is global service consent, not event admission, physical arrival, verified age, a legal eligibility finding or a provider result.

Status: CONTRACT_READY for the version 1.0 interface below. Implementation and real database/RPC/Gateway qualification are IN_PROGRESS / NOT_RUN. No approved production policy has been supplied or published by this implementation. Empty publication is an honest POLICY_UNAVAILABLE gate; tests may explicitly publish synthetic test documents only in isolated test databases and may not certify production legal approval.

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

- `python3 backend/scripts/docs/generate-mcr-core.py`: exit 0.
- `python3 backend/scripts/docs/generate-mcr-core.py --check`: exit 0.
- Cargo compilation, isolated PostgreSQL, real guarded RPC and Gateway browser flows: NOT_RUN at this contract checkpoint; require a fresh allocated resource round.
- No new Cargo target, PostgreSQL instance, executable or lease was started for contract authoring.
