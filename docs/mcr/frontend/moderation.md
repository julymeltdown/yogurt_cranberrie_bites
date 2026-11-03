# FE37 moderator review screens

Status: **MOCK_READY**, 2026-11-04 (Asia/Seoul). The assigned feature modules are implemented and verified with deterministic C04 HTTP fixtures through canonical Auth/privacy and desktop Chrome. Production routes, qualified role/media providers, real moderation and server authorization are **NOT_RUN**. This evidence does not accept BE36, BE39, IN05 or a real multiuser flow.

## Ownership and delivered ancestry

- Writer: `mcr(fe) FE37 moderator review screens`, chat `01a19f83-2c2a-7fa0-a98d-1c4f5f65cb3c`.
- Isolated worktree: `mcr-worktrees/fe-moderation`; branch: `codex/mcr-fe-FE37`.
- Exact assignment base: `98c2c793ba5c380b0cca334f4f8647123c9d30a9`.
- Assigned changes: `frontend/src/features/admin/reports/**`, `frontend/src/features/admin/members/**`, `frontend/src/features/admin/content/**`, and this document. The commit containing this document and its direct handoff record provide the delivered SHA and changed paths. Only delivery integrates and pushes.
- Requirements: `MCR_IMPLEMENTATION_PLAN_V2_EN.md` FE37 and O04/O05/O06, `docs/mcr/project-assessment.md`, `docs/mcr/execution-plan.md`, and the frontend/backend ownership agreement. No scope cut is asserted.
- No application root, router, shared API, contract input/generated artifact, backend, manifest, lockfile, infrastructure or source archive changed. No new production dependency, Phaser instance, socket, character art or legacy rendering was added.

## Contract and source provenance

All tables, forms, styles, controllers, privacy projections and fixture entries are newly authored. Existing audited headless MCR modules supply canonical Auth, privacy, immutable snapshots and command-receipt validation. No Bigyeting rendering was copied.

| Source at assignment base | Revision / SHA-256 | Destination or use |
| --- | --- | --- |
| `contracts/domain/v1/admin.schema.json` | Last changed `5c02a31f26d2822f274bb09743178295fdbd110f`; SHA-256 `b8a4f8b91440d561bbcf23dc68bc196919a8893cf744ca09ac05ecaabff72cb8` | Closed report, detail, member, content, command and list DTO validation |
| `contracts/generated/mcr-domain.openapi.json` | Last changed `ca9f955cd22f7e4c1da029d034c1923f1e98bb9d`; SHA-256 `c47e767505e660d8f5eb69bb9759eae1f0aaee6472e4fb963edc82e060385f51` | Read-only resolved C04 schemas in `reports/model/validate.ts` |
| `contracts/domain/v1/operations.json` | SHA-256 `42ed48ee0cad846b94f9f7a5d39a38beeeb5f5b41e0129f157e51ff31c635223` | Existing generated operation IDs and canonical transport policy |
| `frontend/src/features/party-authoring/model/contract.ts` | Last changed `e82a220c9498c2105a73dfb07302a9a1ab59e56a` | Import `receiptDecoder`/`Binding`; adapt its restricted schema interpreter into the owned `reports/model/validate.ts` |
| `frontend/src/features/auth/model/session.ts` | Last changed `7e7179b0fc72d579857ad5d352329762157fb84b` | Reuse canonical `session`, `privacy` and protected `request`; fixtures instantiate this actual controller |
| `frontend/src/features/party-discovery/model/contract.ts`, `frontend/src/world/entities/replicated.ts` | Exact assignment base above | Existing UUID/receipt types and immutable snapshot helper |

The scoped interpreter adds a bounded node budget and accepts the `description` annotation on `ModerateUser`; the older interpreter rejects that annotation. It still rejects unknown assertions, unknown object fields, invalid UUID/time values, invalid enum/state values, noninteger revisions and oversized lists/strings. Shared validation was not weakened or edited. Dependencies remain the retained React, React DOM, TypeScript, Vite, Vitest, Playwright and existing headless module dependencies.

## Feature behavior and public integration ports

The three entry modules export `createReportReview` / `ReportReviewScreen`, `createMemberReview` / `MemberReviewScreen`, and `createContentReview` / `ContentReviewScreen`. They use one controller implementation under the owned reports path. No frontend import reaches backend code.

Pass the active shell's canonical Auth controller and a `ModeratorAccess` provider. A lease contains the exact current actor/event/privacy scope, existing `MODERATOR` or `OPERATOR` role, a server decision ID and finite expiry. The provider must publish withdrawal/replacement synchronously through `subscribe`. Missing, expired, foreign-scope or unavailable qualification fails closed; the exported `unavailableModeratorAccess` never grants access. A boolean admin boundary, token payload, route name, local storage or bootstrap participant is not a qualification source. No role-query endpoint or new role was invented.

`MemberReview` accepts an optional canonical `memberId` from `/admin/users/:userId`. It uses C04 `UserQuery.user_id`, rejects mismatched returned members, and opens the matching current record. An invalid ID makes no private request. Report/content entry uses the corresponding C04 lists with explicit next-page navigation. Report detail is requested only after selection and must match the queue's report ID, event and revision.

The retained member projection contains only user ID, display name, moderation state and revision; full profile bio/avatar fields are discarded after wire validation. Report detail discards `reporter_id` before retention/rendering. Only the explicit private statement and evidence references needed for the review remain. There is no persistence, analytics, private cache, automatic evidence fetch or public URL fallback.

`EvidencePort.open({scope, reportId, mediaId, signal})` is a separate authorized producer boundary. Its implementation must recheck current event-role authority and report/media association and return an authorized Blob, not a URL. This feature accepts the selected media ID and PNG/JPEG/WebP bytes up to 10 MiB, creates an ephemeral Blob URL, and revokes it on replacement, refresh, selection change, command submission, permission/privacy withdrawal or disposal. Stale asynchronous media cannot allocate a URL. Missing composition shows a safe unavailable message. Real protected download behavior remains NOT_RUN.

Use the actual operation candidates: `listConcertAdminReports`, `getConcertAdminReport`, `decideConcertReport`, `listConcertAdminUsers`, `moderateConcertUser`, `listConcertAdminContent`, `moderateConcertContent` and `getConcertCommandStatus`. Canonical protected requests are no-store and carry in-memory access credentials. No extra HTTP endpoint is assumed.

Actions require a current actionable row, a nonblank bounded reason, a separate review step and explicit confirmation with the exact target/revision. Each command uses a new UUID idempotency key and frozen operation/actor/event/target/revision binding. Report resolve/dismiss does not implicitly hide content. Member suspension is event-local, currently indefinite (`expires_at: null`) until explicit unsuspension; it does not revoke global Auth or other events. Terminal deleted members/content expose no action. Actual affected-world-session termination is a server obligation, not a browser claim.

Direct 200 success must name the same target, advance its revision and return the expected state. Pending receipts remain pending. A successful status receipt must match actor, event, operation, key, target and receipt identity, then be corroborated by a current owning projection at or above a newer receipt revision and in the expected state. Failed/partial downstream outcomes never become applied. Conflicts require refresh; unknown, expired or retired outcomes expose only original-command status recovery. No mutation is automatically replayed. A missing target on a reread, including pagination movement, remains unknown rather than guessing success. Known command failures preserve the factual reason until refresh/reselection and prohibit another confirmation against the failed projection.

Identity, event, privacy-policy or qualified-role changes synchronously abort work and remove rows, statement, image URL, reason, confirmation and receipt. Late list/detail/media/command results cannot repopulate the new scope. The integrator must call `controller.dispose()` when the owning route/shell is released. The screen subscribes to that controller; it does not create another Auth controller or dispose a controller owned by the shell.

## Validation evidence

All commands below ran in the isolated feature worktree using Node `24.22.2` and the retained dependency installation. A user-authorized temporary `frontend/node_modules` symlink supplied dependencies, without adding a manifest or lockfile; it is removed before the final boundary check/commit. No shared incremental TypeScript cache is written (`--incremental false`).

| Command | Exit / result |
| --- | --- |
| `python3 scripts/mcr/check-plan.py` | 0 / PASS: 13 source archive hashes, narrative/export regressions, 118 planning tasks and 64 IA records; required release checklist remains 112 tasks / 61 IA entries |
| `frontend/node_modules/.bin/vitest run --root frontend src/features/admin/reports/__tests__/controller.test.ts --maxWorkers=1 --no-cache` | 0 / 49 tests pass |
| `frontend/node_modules/.bin/tsc --noEmit --incremental false --project frontend/tsconfig.json` | 0 / strict frontend typecheck passes |
| `frontend/node_modules/.bin/oxlint frontend/src/features/admin` | 0 / no warnings |
| `node frontend/src/features/admin/reports/__tests__/browser.mjs` | 0 / standalone fixture build and 9 browser configurations pass; desktop Chrome `156.0.8078.80` |
| `python3 scripts/mcr/check-boundaries.py --base 98c2c793ba5c380b0cca334f4f8647123c9d30a9 --lane frontend --allow-doc docs/mcr/frontend/moderation.md` | 0 / PASS after dependency symlink removal |
| `git diff --check`; explicit changed-path/import inspection; `git diff --name-only 98c2c793ba5c380b0cca334f4f8647123c9d30a9 -- docs/source` | 0 / no whitespace errors, out-of-assignment tracked paths or source archive changes |

Controller tests cover ordinary/foreign/expired/unavailable authority, both existing moderator roles, synchronous logout/event/policy/permission withdrawal, deadline expiry, late reads/detail/evidence/command results, 403 scrubbing, minimal member projection/direct entry, revision conflicts, actor/target/receipt mismatch, duplicate submission suppression and fresh keys for separately reviewed intents, pending/unknown/failed/success corroboration, authorized evidence and rejected identity/type/size, reverse member/content actions, and closed C04 DTOs.

The browser runner builds only the test HTML entry into `/tmp/mcr-fe37-browser/site`, serves its build with preview (no shared dependency prebundle writes), and uses native desktop Chrome headlessly. For each of reports, members and content at **360 / 390 / 430 px**, it verifies keyboard focus on selected record/confirm/cancel, zero write on cancel, readable wrapping and no horizontal overflow, enabled controls at least 44×44 CSS px, pending/unknown/failure/applied states, original GET status without another POST, private on-demand decoded Blob imagery, no reporter/private diagnostic text, reverse actions, permission withdrawal, and denied direct entry. It captures **45 full-page screenshots**. Visual inspection caught and corrected a narrow Review button; the final report/member captures were inspected. This is browser viewport evidence, not physical-phone evidence.

Supplemental local artifacts: `/tmp/mcr-fe37-controller.log`, `/tmp/mcr-fe37-types.log`, `/tmp/mcr-fe37-lint.log`, `/tmp/mcr-fe37-plan.log`, `/tmp/mcr-fe37-browser.log` and `/tmp/mcr-fe37-browser/report.json`. The JSON records base, browser, nine results and SHA-256 hashes of all tested feature/test source files. Durable owner-run evidence is retained under `frontend/src/features/admin/reports/__tests__/evidence/browser/`: the exact `report.json` plus nine confirmation screenshots, one per surface/width. The JSON records the existing feature commit as tested checkout HEAD, the original assignment base separately, all 13 tested source hashes, browser engine, assertions, nine results and SHA-256 hashes of the nine retained images. The runner hashes source extensions only, avoiding evidence/self-hash cycles when rerun. Rerunning the committed runner writes fresh local outputs; it does not overwrite committed evidence implicitly. The standalone fixture bundles resolved contract schemas and emits a Vite >500 kB chunk warning; production route bundle size and application build are not certified by this fixture build.

### Durable-evidence follow-up

The original feature handoff is `09f6c685b0f5686c32a9accfaa65db08c8da720f`, a direct child of assignment base `98c2c793ba5c380b0cca334f4f8647123c9d30a9`. Operations then requested durable report/screenshots within the assigned feature paths. The follow-up commit changes only this documentation, the browser runner's evidence bookkeeping and its evidence subdirectory; product runtime is unchanged. A targeted rerun of the existing nine browser configurations passed with the updated runner. Original 49-test/strict-TypeScript evidence is carried forward without a broad retest; the modified runner passes scoped lint. Every retained image and tested source hash is verified against the committed report, and the assigned boundary/whitespace checks pass. Nine representative confirmation images are committed; the remaining 36 captures remain supplemental local evidence. The commit containing this section follows the original feature commit and is delivered separately with exact ancestry. Status remains owner-run MOCK_READY; real authority and production flows remain NOT_RUN.

## Remaining integration and acceptance

Delivery owns root route/admin registry wiring, fresh server-qualified event role decisions, authorized report-media adapter and server endpoints. The BE36 peer confirmed the queue/detail/privacy boundaries and explicitly does not provide qualified download/HTTP composition or BE39 commands. There is no production mock role or fixture fallback in this patch.

Real nonmoderator HTTP rejection, moderator/operator authorization changes across devices, protected evidence retrieval, owner-service suspend/hide/restore/audit, actual world connection termination, DB/HTTP/realtime composition, multiuser flows, physical-phone behavior, load/accessibility-assistive-device audits and IN05 remain **NOT_RUN**. Server authorization, capacity/privacy invariants and idempotency are never replaced by these client checks. Full-stack FE37 acceptance requires those separately owned gates; this committed feature handoff remains MOCK_READY.
