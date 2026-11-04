# Original QA04 frontend harness

This is developer-owned implementation, not a review gate. Writes are restricted to this directory and `docs/mcr/evidence/privacy-frontend/`. No product UI, Auth model, shared interface, central configuration, backend, `TEST/authorization/`, source original or artwork is changed.

Use the retained Node 24.22.2 and existing frontend installation. `MCR_QA04_DEPENDENCIES` selects the existing installation directory (default `/Users/lhs/Desktop/mcr/frontend`). Create a temporary **directory-local** `node_modules` symlink to that installation for Vite's deduplicated packages and TypeScript. Do not stage the link, install dependencies or create a central checkout link. Remove the local symlink after qualification. The runner removes its own cache on exit.

Run from the repository root:

```sh
NODE frontend/e2e/privacy/run.mjs
NODE --test frontend/e2e/privacy/live-gate.test.mjs
NODE frontend/e2e/privacy/typecheck.mjs
NODE frontend/e2e/privacy/node_modules/oxlint/bin/oxlint frontend/e2e/privacy
NODE frontend/e2e/privacy/live-gate.mjs
```

`NODE` means the selected existing Node executable, not a package command. The default Chrome executable is the installed macOS Chrome; `MCR_CHROME_EXECUTABLE` can select an existing compatible browser. The native runner blocks actual `/api/` network dispatch, injects deterministic fetch into the real canonical SessionController and SocialSharingController, and exercises the actual QueryPrivacy/SessionPort/TanStack observers in Chrome. It does not mount the application or certify cached product UI, a deployed frontend, real authorization, WS delivery, device behavior or server data filtering. The page is an unstyled test shell; no product styling is introduced.

`fixture-report.json` retains all 35 social/client cells (seven roles by five operations), plus a 175-cell domain ledger. The non-handle domain list/detail/mutation/WS cells are explicitly unsupported in this increment; public DTO injection checks do not stand in for those role checks. Cache cells taint the real shared observers/drafts with five domain canaries, then prove synchronous cleanup and late-result rejection. These are local public lifetime calls, never fabricated WS events. The authorized beacon DTO shape is not proof of its audience. Admin has no implicit social-handle bypass.

The synthetic stranger/admin SELF-list denial is a scenario input, not an authorization expectation for real services. `/me/social-requests` is a SELF collection; an active stranger/admin can correctly receive a 200 empty own collection. The actual seed plan supplies per-resource/per-canonical-actor expectations. A role label never replaces Auth, event membership, block or consent checks.

## Real-service gate

Default execution requires `MCR_QA04_REAL_PLAN` pointing to a **private local** JSON file supplied by actual seed/service owners. No plan, token or payload is copied to evidence. Missing input exits **2**, records `NOT_RUN`, and dispatches no request. Even a qualified HTTP plan cannot make the default full gate pass: the actual social WS/revocation/cached-product-UI handoff is not available. It refuses that setup before any seed mutation. `--http-only` explicitly executes a ready real HTTP subset and still exits **2/PARTIAL**, keeping original QA04 open.

The seed owner's plan has `version: 1`, `isolatedEnvironment: true`, `origin` (HTTPS or loopback HTTP origin only), `environmentId`, `seedRevision`, `serviceRevision`, `principalHandoff`, and explicit `allowSeedMutations: true`. `principals` contains exactly the seven named roles' actual canonical `userId`, `eventId`, and `tokenEnv`. Tokens are read from those environment names in memory, never printed. Distinct real principals must come from the seed owner; selftest UUIDs/tokens are not real inputs.

`httpCells` contains each of the 21 unique role × list/detail/mutation cells. Each cell has `role`, `operation`, and `probes`. A probe names the existing `operationId`, canonical `parameters`, typed `query`, optional contract-valid `body`, a fresh `idempotencyKey` for seed mutations, `expectedStatus`, `domains`, `forbiddenValues`, and `requiredValues`. Every cell must cover handle/claim/evidence/beacon/recipient domains. The seed owner chooses forbidden canaries appropriate to each role, and positive seeded values for authorized responses. Successful responses require a nonempty positive assertion so empty payloads cannot pass vacuously. Seed changes and expected denial policy must be provided by the actual implementation owners, not inferred by this harness.

Request method/path, query/body rules, success/error response schema and allowed status come from the pinned public OpenAPI. Unknown operations/queries, malformed/cross-event IDs, missing role cells, non-isolated setup, unavailable credentials and unqualified mutations refuse setup before network activity. HTTP redirects are rejected; responses must have `no-store`, match the exact operation/status schema, remain below 1 MiB and omit forbidden decoded values, including JSON-escaped values. The QA-only schema interpreter fails closed for unknown assertions/formats; it is not a product decoder. Unknown required formats need an explicit harness implementation before that cell is runnable.

The gate does not invent a social WS event, service route, role identity, provider, age claim, admin privilege or browser cache connection. A real WS/cache adapter remains required implementation once the responsible owner supplies its frozen transport and principal/seed lifetime. Neither fixture completion nor HTTP-only results complete original QA04 or IN02/IN03/QA07/FE09 production criteria.
