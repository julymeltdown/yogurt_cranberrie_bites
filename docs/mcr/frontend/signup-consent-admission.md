# Signup consent and admission — FE10

Owner: developer204. Publisher and shared Auth composition owner: Root153. Backend counterpart: developer207, AUTH-CONSENT-ADMISSION-01. Status: IN_PROGRESS. Original criterion: new mapped signup/verification forms, validation, consent, challenge expiry, authoritative resend countdown; expired verification cannot complete and duplicate submit cannot dispatch additional requests (specification FE10, row 516). This work does not amend that criterion.

## Source and ownership

The reused isolated checkout is `fe-wall-tools-b-204`, branch `codex/mcr-fe-FE10-consent-admission`, base `ff96d53eb817c93dd1bafb5c4f125630f995413b`. Historical Wall and foundation branches remain preserved. Root153 explicitly transferred enrollment.ts, enrollment-contract.ts, SignUp.tsx, VerifyEmail.tsx, enrollment.css, feature enrollment tests/browser runner/fixtures and new feature-local enrollment/consent/admission modules/tests. Only this new document and `evidence/signup-consent-admission/` are assigned documentation. Session, shared HTTP/cookie, PublicBoundary, app tests, routes/ports, generated public contracts, manifests and locks stay with their canonical owners. No backend edits or source-pack changes.

The initial source had two synthetic local checkbox booleans and sent only email/password to the legacy signup endpoint. That is an internal integration gap, distinct from missing approved legal publication. Current work replaces that gap with the registered backend207 public freeze `ed50c2f1`. The exact public DTO/operation source copy is retained in [frozen-public-contract.json](../evidence/signup-consent-admission/frozen-public-contract.json). It is CONTRACT_READY only; it is not real producer qualification or legal approval.

## Behavior being delivered

- Closed public policy documents carry actual title, HTTPS source, digest, version and immutable revision. The user must explicitly select every required acknowledgment ID; no age or eligibility booleans and no generated legal text.
- An HttpOnly bound attempt precedes signup. All cookie/origin/intent/bearer handling remains in canonical HTTP/Gateway owners. Signup and resend use body-only UUID idempotency. Existing-email receipts stay neutral and disclose neither identity nor delivery.
- Attempt IDs and codes replace caller-supplied email/UUID verification. Optional attemptId/code query prefills are captured and removed before requests; cross-browser failures explain the original-browser requirement. The server decides challenge validity; local attempt expiry forbids known-expired dispatch.
- Request-start monotonic deadlines bound policy/attempt expiry and server resend waits. Resend cannot extend attempt/policy freshness. Unknown mutations never automatically replay. An explicit retry retains the exact original command/key in memory only; privacy/session cleanup, cancel and dispose discard retained secrets.
- Existing canonical identities use protected consent plus an authoritative admission read before the Root-owned bootstrap continuation. Local agreements cannot create identity, verified age, World access or a second session/connection.

## Evidence and remaining work

Fresh focused source/browser validation: NOT_RUN. Source is being authored under the exact grant. Existing real-mail receipts in `evidence/real-email-flow/` are historical proof of the unchanged legacy delivery path only; they do not certify this new policy/attempt wire flow. No tests are repeated solely for accounting. Current approved production legal/eligibility publication and external credentials are not supplied. Backend persistence/RPC/Gateway qualification and Root shared composition remain owner deliverables; deterministic fixtures must be labeled as such. Restricted-age provider choice remains separate from basic signup.

No review or approval gate applies. Developer204 owns implementation verification and fixes, then exact-path committed handoff to Root153 for normal publication and canonical receipt. The active substantive FE10 goal remains active until required internal work, validation/fixes, documentation and publication are complete; original FE10 acceptance remains truthful about external/live gaps.

Additional exact Root transfer: only `shared/api/http.ts` enrollment allowlist and per-exact-path credentials selector, plus new `shared/api/__tests__/enrollment-transport/` tests. Existing auth paths retain methods, credential behavior, origin/intent protection and refresh prohibition. Top-level/domain/header idempotency is still rejected for enrollment. Root retains all remaining HTTP hunks and canonical session/cookie/public-boundary composition.

The finite guardian is adapted from developer204's previously authored Wall B runner at the base revision; its FE10 root, accepted-offer task, private evidence and environment are separate. It creates only the absent dependency link and per-round private scratch, monitors all descendants/RSS/byte carry/disk/deadline, then removes only its owned link/scratch. Historical Wall leases are not reused.
