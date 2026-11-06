# Task 45 — FE10 eligibility/legal publication

Status: **BLOCKED_ON_EXTERNAL_INPUT** — named gap. Documentation only; no product
code, test, contract or fixture was changed and no text, version, effective date or
acknowledgement is invented here.

Task row: `.omo/plans/mcr-finish-project.md:568-576` (task 45, closes GAP-5).
Base: `13d2ee25f` on `codex/mcr-exec-w8-inputs` (`master`).

Related existing records: `docs/mcr/backend/enrollment-policy.md` (contract and
validation record), `docs/mcr/backend/enrollment-policy-operator-2000.md`
(operator executable), `docs/mcr/backend/enrollment-browser-2015.md` (browser
round), `docs/mcr/backend/enrollment-application-closure-intake-2026-11-05.json`.

## 1. The missing input

### 1.1 What the system stores

The enrollment policy stores a **reference** to separately approved documents. It
never stores or generates legal prose.

| Value | Storage | Binding |
| --- | --- | --- |
| Policy revision (the version identifier) | `mcr_enrollment_policies.revision uuid primary key` | `backend/services/auth-service/migrations/0019_enrollment_policy.sql:3-7` |
| Document set + approval reference | `mcr_enrollment_policies.publication jsonb` | same table, `:4` |
| Publication start | `mcr_enrollment_policies.published_at` | `:5` |
| Publication expiry | `mcr_enrollment_policies.expires_at` | `:6` |
| Current-policy pointer, seeded empty | `mcr_enrollment_current(singleton, revision)` | `:9-13` (insert of a `null` revision at `:13`) |
| Acknowledgement records | `mcr_enrollment_acceptances(user_id, revision, acknowledgement_ids text[], attempt_id, accepted_at)` | `:33-39` |
| Consent idempotency receipts | `mcr_enrollment_consent_requests` | `:47-53` |

Document shape: `PolicyDocument` at
`backend/services/auth-service/src/enrollment_policy/contract.rs:12-18`
(`id`, `title`, `url`, `sha256`, `requiredAcknowledgementId`).
Approval metadata: `ApprovedPublication` at `backend/services/auth-service/src/enrollment_policy/contract.rs:25-31` (`revision`,
`documents`, `approvalReference`, `publishedAt`, `expiresAt`); validation at
`backend/services/auth-service/src/enrollment_policy/contract.rs:34-71` enforces 1-8 documents, an absolute HTTPS URL with no
userinfo, a 64-character lowercase hex `sha256`, unique document and
acknowledgement ids, a non-empty `approvalReference` of at most 512 characters,
and `publishedAt <= now < expiresAt`.

Wire policy: `CurrentPolicy` at `backend/services/auth-service/src/enrollment_policy/contract.rs:97-105` (`contractVersion` fixed
to `"1.0"`, `revision`, `documents`, `publishedAt`, `expiresAt`,
`serverTime`). The API has **no `effectiveAt` field** and no human version
label; the UUID `revision` is the version and `approvalReference` is the only
free-form approval identifier. `effectiveAt` exists only on the public
legal-document surface
(`frontend/src/features/public/model/documents.ts:11-16`).

### 1.2 Exact values still required from the user/operator

1. One to eight approved documents (eligibility, terms, privacy, consent — as
   approved), each with:
   - the final approved text, published at an absolute HTTPS URL with no
     userinfo and at most 2048 characters;
   - the SHA-256 of those exact published bytes (64 lowercase hex);
   - the display title (at most 240 UTF-8 bytes);
   - a stable document `id` and `requiredAcknowledgementId` (1-80 characters
     of `[A-Za-z0-9_.-]`).
2. A non-nil policy revision UUID, chosen by the operator and immutable for that
   content.
3. An `approvalReference` (at most 512 characters) naming the approval record —
   approver, date and scope. This is the only place a human-readable policy
   version (for example "Eligibility v3, approved 2026-11-06") can be carried.
4. `publishedAt` and `expiresAt` in UTC with `publishedAt <= now < expiresAt`.
5. An effective-date decision. Because the enrollment wire has no `effectiveAt`
   field, approved text with a **future** effective date must either wait for
   publication until that date or take a contract change. `publishedAt` must not
   be presented as the effective date.
6. Approved public legal pages for `/legal/terms`, `/legal/privacy`,
   `/legal/safety` and `/legal/contact`, each with `version` (matching
   `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`), `publishedAt`, `effectiveAt` and
   reviewed section text. This surface has **no production provider**: the
   context default is `unavailableDocuments`
   (`frontend/src/features/public/model/document-provider.tsx:3`,
   `frontend/src/features/public/model/documents.ts:25-26`), so today every
   `/legal/*` page renders "This document is not available yet."

### 1.3 Where it must be displayed

- Signup agreement block: `/signup` ->
  `frontend/src/features/auth/ui/SignUp.tsx:116-117` (unavailable message and
  "Load current agreements"), `:132` (policy acknowledgement list).
- Existing-account consent: `AdmissionBoundary` routes `CONSENT_REQUIRED` to
  `frontend/src/features/auth/ui/ConsentAdmission.tsx:33-44`.
- Acknowledgement list component:
  `frontend/src/features/auth/ui/EnrollmentPolicy.tsx:8-18` renders one required
  checkbox per document with the document title linked to its `url`, and prints
  the publication UUID revision. It does **not** render `publishedAt` or
  `expiresAt`.
- Public legal pages: `frontend/src/app/PublicBoundary.tsx:26` ->
  `frontend/src/features/public/ui/DocumentPage.tsx:42-44` renders Version,
  Published and Effective.

## 2. Binding map

### 2.1 Server

| Layer | Location | Behaviour |
| --- | --- | --- |
| Migration | `backend/services/auth-service/migrations/0019_enrollment_policy.sql:3-53` | Tables above; the header states no policy or acceptance is seeded. |
| Domain contract | `backend/services/auth-service/src/enrollment_policy/contract.rs:12-18,25-31,34-71,73-84` | Document/publication shape, validation, exact-set acknowledgement check. |
| Error/status | `backend/services/auth-service/src/enrollment_policy/contract.rs:208-219` | `AcknowledgementsRequired` maps to HTTP 422. |
| Operator publication | `backend/services/auth-service/src/enrollment_policy/store.rs:81-112` | `EnrollmentStore::publish` takes the singleton lock, validates with database time, rejects changed bytes for an existing revision, inserts and repoints. There is no HTTP or RPC publication route. |
| Store construction | `backend/services/auth-service/src/main.rs:122` | The service builds the store; it never publishes. |
| Policy read/gate | `backend/services/auth-service/src/enrollment_policy/store.rs:546-559` (`current`) | Missing or expired policy is `PolicyUnavailable`; every command re-reads it under `for share` with server time. |
| Signup | `backend/services/auth-service/src/enrollment_policy/store.rs:155-317` (acknowledgement checks at `:178` and `:201`) | Missing or extra acknowledgement ids are refused before admitted work and rechecked after the password step. |
| Consent | `backend/services/auth-service/src/enrollment_policy/store.rs:481-535` (acknowledgement check at `:495`) | Authenticated acceptance is revision-bound and idempotent. |
| Admission | `backend/services/auth-service/src/enrollment_policy/store.rs:444-479` | `ACCOUNT_INACTIVE` / `POLICY_UNAVAILABLE` / `CONSENT_REQUIRED` / `ADMITTED` from durable current acceptance. |
| Acceptances | `backend/services/auth-service/src/enrollment_policy/store.rs:562-573` | `insert into mcr_enrollment_acceptances(user_id, revision, acknowledgement_ids, attempt_id, accepted_at) on conflict do nothing`. |
| gRPC | `backend/services/auth-service/src/enrollment_policy/rpc.rs:134-195` (status mapping `:98-111`) | Transport only; reason strings preserved. |
| Public transport | `backend/proto/auth/v1/enrollment_policy.proto:16-24` | `ReadPolicy/BeginAttempt/Signup/Verify/Resend/ReadAdmission/AcceptConsent`. |
| Gateway HTTP | `backend/services/api-gateway/src/adapters/in/http/enrollment_policy.rs:29-35` | Seven `/api/auth/enrollment/*` routes. |
| Gateway failure envelope | same file `:87-116`; gRPC classification `backend/services/api-gateway/src/adapters/out/enrollment_policy/mod.rs:138-158` | Exact `(status, reason)` pairs, including `Unavailable + POLICY_UNAVAILABLE -> 503`. |
| Retired browser signup | `backend/services/api-gateway/src/adapters/in/http/auth_routes.rs:39,64-70` | `/api/auth/email/signup` always returns `422 ACKNOWLEDGEMENTS_REQUIRED` before account lookup. |

### 2.2 Frontend

| Surface | Location | Behaviour |
| --- | --- | --- |
| Wire paths | `frontend/src/features/auth/model/enrollment-contract.ts:7-11` | `/api/auth/enrollment/{policy,attempts,signup,verify,resend,admission,consent}`. |
| Wire schemas | `frontend/src/features/auth/model/enrollment-contract.ts:16-26` | Strict document and policy schemas; duplicate document or acknowledgement ids rejected. |
| Client state | `frontend/src/features/auth/model/enrollment.ts:103-108` (`policyReady`), `:157-176` (`signup`), `:191-196` (`acceptPolicy`) | No policy or expired policy gives `policy-unavailable`; non-exact acknowledgements give `acknowledgements-required`; no request is sent. |
| Signup view | `frontend/src/features/auth/ui/SignUp.tsx:22,116-117,132,142` | Unavailable message, explicit load, acknowledgement list, submit disabled until exact acknowledgements. |
| Consent view | `frontend/src/features/auth/ui/ConsentAdmission.tsx:33-44` | Same list; "Agree and continue" disabled until exact acknowledgements. |
| Admission boundary | `frontend/src/features/auth/ui/AdmissionBoundary.tsx:8-18` | `POLICY_UNAVAILABLE` keeps world entry paused. |
| Public legal pages | `frontend/src/app/PublicBoundary.tsx:26`; `frontend/src/features/public/ui/DocumentPage.tsx:40-44`; `frontend/src/features/public/model/documents.ts:9-25` | Published documents render Version/Published/Effective; otherwise unavailable. |

## 3. Behaviour without the text (recorded evidence)

1. **No policy published** — `GET /api/auth/enrollment/policy` returns
   `503 POLICY_UNAVAILABLE`; attempts and signup cannot start because every
   command calls `current()` first (`backend/services/auth-service/src/enrollment_policy/store.rs:546-559`). The native case asserts
   this at `backend/services/auth-service/tests/enrollment_policy.rs:292-296`.
2. **Retired browser signup** — `/api/auth/email/signup` returns
   `422 ACKNOWLEDGEMENTS_REQUIRED` unconditionally, before any account lookup
   (`backend/services/api-gateway/src/adapters/in/http/auth_routes.rs:64-70`), asserted at
   `backend/services/auth-service/tests/enrollment_policy.rs:296-307`. This is
   the only case where an unpublished policy produces 422 rather than 503: the 422
   is the retired endpoint's fixed response, not a policy decision.
3. **Published policy, missing or extra acknowledgements** — enrollment signup
   returns `422 ACKNOWLEDGEMENTS_REQUIRED`, asserted at
   `backend/services/auth-service/tests/enrollment_policy.rs:368-380`; the exact
   set then returns `202` with a five-field receipt and no `userId`.
4. **Client never sends signup without a published policy** —
   `frontend/src/features/auth/__tests__/enrollment.test.ts:14-16` asserts
   `policy-unavailable` with zero requests; `:65` asserts the authoritative
   reason/status mapping.
5. **With an isolated development publication the flow completes** — the real
   browser round
   `docs/mcr/evidence/signup-consent-admission/browser-main-02/qualification.json`
   records `status: PASS_ACTUAL_AUTH_ONLY_JOURNEY` with five passed cases and
   HTTP 200 policy, 201 attempt, 202 signup, verification, login and renewed
   consent; `docs/mcr/backend/enrollment-browser-2015.md` records the same five
   Auth-only cases passing. The native source/restart case and its exact commands
   are in the validation record of `docs/mcr/backend/enrollment-policy.md`.

## 4. The isolated development publication path is not legal approval

`backend/scripts/mcr/session_deletion_runtime/enrollment.py:42-80`
(`owned_auth_adapters`) supplies the fixture seam: it SHA-256-pins a frozen
operator executable, runs the ignored case
`publish_isolated_policy_into_owned_auth` against the existing owned Auth stack
(`enrollment.py:61`), and requires the emitted receipt to say
`productionApproval is False`, `databaseCreated is False` and
`authMigrations == 19` (`:67-69`). `principal()` (`:113-183`) then drives
real public policy/attempt/signup/mail/verify/login/admission calls with those
acknowledgements, and rechecks `productionApproval is False` (`:118`).

Limits of that path, stated exactly:

- The synthetic policy is an isolated test document
  (`backend/services/auth-service/tests/enrollment_policy.rs:16-30`: title
  "Isolated test document - no production legal approval", approval reference
  `ISOLATED_TEST_INPUT_NOT_PRODUCTION_APPROVAL`). It cannot certify approval.
- At this base the ignored case is **not in the tree**: a repository-wide search
  finds no Rust file or symbol `publish_isolated_policy_into_owned_auth`. The
  last tree containing
  `backend/services/auth-service/tests/enrollment_policy_browser.rs` is
  `dee64fd1a` (`codex/mcr-be-auth-consent-admission`, an ancestor of
  `master`); the file is absent at `13d2ee25f`. Exercising the path therefore
  requires that retained frozen executable and an owned Auth stack, not this
  checkout alone.
- A development publication is not legal approval. Production closure needs the
  approved documents and the operator publication named in section 1.2.

## 5. Verification performed for this record

- Every `file:line` above was re-read at base `13d2ee25f`; the cited SQL, Rust,
  TypeScript and JSON artefacts exist at those paths.
- No Rust, database or browser run was performed for this record: the native case
  needs an owned PostgreSQL 16 cluster plus a copied frozen test binary, and a
  fresh compile on this host is outside the current resource floor. The recorded
  runs cited in section 3 are the behavioural evidence of record.
- The claim that the operator case is absent from the tree was checked with
  `git ls-tree 13d2ee25f` (absent), `git ls-tree dee64fd1a` (present) and a
  repository-wide symbol search (only the Python helper and the operator document
  mention it).

## 6. Inputs required to close task 45

1. Approved eligibility/legal/consent documents with final text, HTTPS URLs,
   SHA-256 values, titles, ids and acknowledgement ids (section 1.2 items 1-2).
2. Approval record: approver, approval date, scope, policy version label and the
   approval reference string (section 1.2 item 3).
3. Publication window: `publishedAt`, `expiresAt`, and the effective-date
   decision (section 1.2 items 4-5).
4. Approved `/legal/*` page content with versions, published/effective dates and
   any reviewed contact destination (section 1.2 item 6), plus the production
   `DocumentProvider` adapter that reads it.
5. Operator publication of the policy revision through
   `EnrollmentStore::publish` (no public route exists), followed by developer
   acceptance: real signup -> verification -> sign-in with the approved text,
   neutral duplicate-email and throttle outcomes, and the policy version recorded
   in the receipt.
