# Real local email-account browser journey

The current-source native browser journey passes **17 checks** against actual Gateway/Auth and their isolated six-service stack. This is local HTTP interoperability evidence for FE09/FE10/FE11, not production consent, TLS, world or full-task acceptance. No Auth/bootstrap response is mocked, and no frontend application defect required a code change.

## Delivery and provenance

Initial assignment base: `b381d03cdc3b0168eba47eb0599f801843eff827`. Parent-authorized prerequisites are separate from the frontend handoff: launcher `66f749a` cherry-picked as `57c3d9830ca30080858a26547be01d174fafe3d3`; Profile query correction `600582d6b5deda6037c190c303f80752ba4a6bb3` cherry-picked as `abc3293`. The frontend-only delivery commit follows `abc3293`; integrate that delivery commit alone because the parent already owns both prerequisites.

Owned delivery paths: `frontend/e2e/real/auth/entry.html`, `entry.tsx`, `run.mjs`, and this document. Existing app routes, session/privacy/cookie owners, signup/verification/sign-in/reset forms and native fetch are consumed unchanged. Backend scripts are invoked only as a separate CLI; no backend implementation is imported into browser code. No dependencies, manifests, contracts, root composition, shared UI, product artwork or `docs/source/` changes.

Executed bundle: `/Users/lhs/Desktop/mcr-build/current-email-bundle/600582d-1793708008138257000/bin`, with its adjacent `receipt.json`. The parent built all six binaries at source `600582d` using locked/offline Rust 1.88. The runner verifies every binary SHA-256 and all **1,108 declared backend source inputs** before startup; source differences are zero. The owned launcher independently qualifies migration ledgers, restarts PostgreSQL, starts services, and records binary hashes. Final cleanup also asserts six matching executed hashes, matching before/after ledger counts, successful migration verification, launcher exit 0 and cleanup PASS. Browser: installed Chrome **156.0.8078.80**, 390×844; frontend Node 24.22.2 / pnpm 11.7.0.

## Real behavior covered

- Production `src/main.tsx` signup remains policy-blocked. A separate, visibly labeled test-only entry opts into the existing synthetic eligibility/consent acknowledgements; it uses actual product forms/controller and actual server account creation. **The approved eligibility/consent persistence contract is still missing; production consent acceptance is NOT_RUN.**
- Actual signup queues verification delivery to the launcher's private local HTTP mailbox; an incorrect code is rejected, the delivered code succeeds, and sign-in establishes the same canonical Auth identity.
- Native HttpOnly SameSite=Lax refresh cookie behavior is exercised with the launcher's explicit local-HTTP exception. Access is memory-only; local/session storage stay empty. This does not qualify Secure cookies/TLS.
- A second tab's login invalidates the first tab. Logout invalidates the peer's generation and clears its real form draft; a separate native browser cookie context proves the old logout refresh credential receives 401.
- Forgot-password uses actual mail delivery. The browser opens the **actual delivered app-origin link**, the fragment is scrubbed, reset succeeds, peer access is revoked, the old refresh session receives 401, the old password fails, and the new password signs into the same identity. No manual token substitution or fabricated successful link.
- The real bootstrap endpoint returns an unsuccessful response. No canvas/world is opened; bootstrap/world acceptance remains NOT_RUN. The proxy forwards original request bodies, Origin, cookies and actual Gateway response bytes/status/headers without substitutions.

Owned ephemeral PostgreSQL/Redis/services, OS-selected frontend port and browser contexts are disposed. The test uses only throwaway `.invalid` accounts and private local mail. Reports omit emails, passwords, codes, tokens, cookies and response bodies; screenshots mask inputs. The private `ready.json` is an ownership descriptor, not a publishable receipt. No HAR, tracing, external mail/accounts, Cargo/target writes or push.

## Reproduction and evidence

From `frontend/`, with the pinned Node on PATH:

```sh
node e2e/real/auth/run.mjs --bin-dir /Users/lhs/Desktop/mcr-build/current-email-bundle/600582d-1793708008138257000/bin --receipt /Users/lhs/Desktop/mcr-build/current-email-bundle/600582d-1793708008138257000/receipt.json --evidence /tmp/mcr-real-email-delivery
```

Use a **new** evidence directory for a replay. Source/binary mismatch fails closed; historical exploratory runs require explicit `--historical` and cannot pass the current-candidate gate. `--service-source-root` optionally names the exact read-only backend source tree corresponding to a qualified bundle.

| Check | Exit and evidence |
|---|---|
| Final actual browser journey | 0; `/tmp/mcr-real-email-delivery/report.json`, `stack.json`, `/tmp/mcr-real-email-delivery.log` |
| Focused auth + enrollment/recovery app regressions | 0; 247 tests / 9 files; `/tmp/mcr-real-email-auth-regression.txt` |
| Production strict build | 0; `/tmp/mcr-real-email-build.txt`; existing 544.82kB chunk warning retained |
| Strict standalone e2e entry typecheck | 0; `/tmp/mcr-real-email-entry-typecheck-final.txt` (same strict options as app, explicit `--ignoreConfig`) |
| Source/e2e lint and runner syntax | 0; `/tmp/mcr-real-email-lint.txt`, `/tmp/mcr-real-email-harness-final-lint.txt`; `node --check` |
| C03 generation check, C08 origin, planning, exact boundaries, whitespace | Recorded in accompanying delivery receipt; no acceptance inference from these static checks |

The delivery receipt binds source/runner bytes, exact commit ancestry, bundle receipt, final reports, masked screenshots and check logs. Historical `/tmp/mcr-real-email-historical-first` failed safely at current Auth's added `0013_identity_binding_epoch.sql` ledger mismatch and cleaned up; it did not execute a browser journey. Current-first failed before startup because the harness initially expected the older receipt shape. Current-second and final passed their then-current runners; delivery adds strict ephemeral-port selection and executed migration/binary assertions. These development attempts remain preserved and do not replace the final evidence.
