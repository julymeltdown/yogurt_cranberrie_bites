# FE36-AUTH-ROOT canonical account deletion composition

Status: IN_PROGRESS. Source base `36b8bc9cc4729997eb2a0ac36f05f77a28b2a5e8`, branch
`codex/mcr-fe-account-deletion-composition`, isolated checkout
`/Users/lhs/Desktop/mcr-worktrees/fe-account-deletion-composition`.

The canonical Auth owner now constructs a typed AccountDeletionPort using its existing
HttpClient, session and privacy coordinator. The generic SessionController.request
`/api/auth/` refusal is unchanged. DELETE `/api/auth/account` is once per captured
identity/generation, without an invented body/key/receipt; GET
`/api/auth/account/deletion` is explicit and never refreshed/replayed. Credentials,
query ownership, protected same-origin/no-store transport and expiry remain canonical.

Recovery grants only the exact next deliberately suspended privacy scope in the
consumer's immediate post-settlement turn. A later policy/identity/event change,
expiry, cancellation, refresh credential replacement or disposal invalidates that
capability. A 401 clears current canonical identity and never clears a newer actor.
Raw response data is transiently returned only to the existing eight-target decoder;
no provider diagnostic, token or raw saga is cached by this Auth adapter.

Initial focused Auth tests passed (14 cases before the immediate-turn hardening).
Root composition, native/default-route evidence, remaining regressions and final
committed handoff are still IN_PROGRESS. All deletion HTTP in tests is explicitly
synthetic; real account deletion, live cleanup, device and IN acceptance are NOT_RUN.
