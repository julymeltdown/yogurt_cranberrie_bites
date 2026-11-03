# FE-ROOT-SELECTED-CHARACTER

Status: MOCK_READY for the selected-character root and media-provider wrapper. Public Profile portrait composition remains assigned to its existing media developer; the root task goal stays active through that follow-up. Implementation owner validates and publishes this scope directly; there is no king/SL technical review or QA approval gate. Assignment base `d2a5fbddb781bcd1ac87e830a45405fd3c39c0f8`; isolated branch `codex/mcr-fe-root-selected-character` in the reused checkout. Only stopped-author drafts `cf9940ac` and `e21cb5b6` were cherry-picked, as `414283c9` and `d8b7c36d`. Their earlier checks are not final evidence for this candidate.

## Runtime and consumer bindings

`createProductionPorts(auth, worldOptions, social)` constructs one root character registry and passes it to the actual default World composition. `ports.characters` is its public port. `get(originalPrivacyScope, validatedBootstrap)` admits one selected runtime for the exact manifest pin; `read(originalPrivacyScope, pin)` only borrows an already admitted runtime. Registry/production-port teardown owns disposal; World/view/editor teardown releases only its own leases.

`CharacterProvider` mounts above account, community and World consumers, retaining the registry-owned runtime across routes. `useCharacterRuntime()` returns only a current borrowed runtime. The factory retains the original scope instead of recapturing authority for old inputs. The account owner rejects a borrowed runtime from any different user, event, session generation, policy generation or gate. Whole editor dispatch uses the actual published whole-character controller; verified commits call existing `me.refreshAfterAvatar(scope, revision)`, preserving dirty profile text and retaining server replication as the only World appearance authority.

Header reads already-authorized Bootstrap/Me identity. Me's renderer receives its actual loaded avatar state. Whole `1.1.0` values use `WholeCharacterView`; layered inputs retain their truthful legacy preview/unavailable behavior. The default resolver dispatches whole manifests to the published pure validator and keeps the supplied selected manifest unchanged. It does not invent the layered `provenance` field. Map/theme/source/hash/room checks remain required.

`useLoadedProfileActivity(subjectUserId)` observes an existing authorized, ready feed page from the canonical social owner. It starts no feed query, supplies no service readiness, and returns no page after scope/service loss. Existing activity presentation labels this as a partial view. Public Profile/other portrait callbacks remain with the media composition owner until its explicit handoff; this implementation does not acquire their files implicitly.

## Evidence and current dependencies

Pinned Node 24.22.2, retained dependency link only; no install, dependency, contract, backend, art or source archive edits. New source and tests are authored against the published canonical union, whole runtime/editor/World/Profile ports. Initial developer-run affected checks: **238 tests across 18 files**, full TypeScript and scoped lint pass (exit 0). These include exact selected-manifest resolver admission, borrowed lifetime/privacy, real controller GET/PUT-to-Me refresh, dirty-text preservation, and StrictMode/provider ordering. Final combined affected checks pass **229 tests in 17 files** (exit 0); the earlier 238-test selection also included whole feature tests. Full TypeScript, product build, scoped lint, planning and UI-origin checks pass (exit 0). Product build retains the existing Phaser chunk-size warning.

The independent fixture author owns only `frontend/e2e/reference-integrated/**` and its QA document. Assistance supplies concrete fixture/evidence source; final root integration validation remains this developer's responsibility. The genuine fixture must use `createProductionPorts` without a custom `load`, exact selected Bootstrap and six whole-character C05 values, actual PNG decoding, GET/PUT/save-to-replication/Header/Me propagation, warm navigation, privacy cleanup and singleton measurements. HTTP/socket producers are deterministic; live signed-in backend, physical devices, multiuser, IN01/IN05 and full character animation remain separate evidence boundaries.

## Developer-run default-loader qualification

Receipt: [`native.json`](../../../frontend/src/app/character-entry/__tests__/evidence/native.json). Source candidate `a3182b31` (before documentation/receipt and source-only peer merge); the receipt pins exact consumer, renderer, controller, fixture and manifest SHA-256 values. QA source commits `af158006`, `1cc8c2b7`, `8d7268db` were consumed unchanged and reconciled with their published master ancestry. No custom load, engine factory or renderer was supplied. Actual retained CANVAS pool ownership and scene sprites were observed read-only; there is no Phaser debug-global dependency.

Command: `PATH=<pinned-node-24.22.2>/bin:$PATH MCR_WHOLE_ROOT_OUTPUT=<new-absolute-output-directory> node frontend/e2e/reference-integrated/whole/native.mjs` — **exit 0; 360/390/430 px all PASS**. The run checks all six visible static sprite pixel hashes against independently acquired approved PNG regions; exact selected manifest and downloaded source-release/atlas response-body hashes match supplied bytes. It verifies initial Header/Me identity; actual selection, reset, one explicit PUT with expected revision 7, verified revision 8 GET/Me/Header update; and a server C05 delta before World appearance update.

Each width retains exactly one observed game, camera, character runtime and World canvas through World → Me → Customize → World, repeated warm visits and all three theme selections. Concurrent socket peak is one (covered panels can reconnect transport; this is not a one-total-ticket claim). One compact character atlas remains decoded; warm Customize adds no image decode (five total decode calls both first and warm Customize; the total includes map resources). Theme changes legitimately decode their own resources. No loader-call count is claimed.

Terminal `suspended` privacy invalidation disposes the original character runtime, with zero leases, pending requests, decoded atlases, cleanup failures, private queries, active sockets and World canvases. An earlier diagnostic using `block` correctly cleared the old runtime but reopened a fresh generation: block and consent revocation are invalidation/revalidation events, not terminal gates. That diagnostic is not counted as a complete-teardown PASS.

Captures under the receipt directory include saved World at all three widths, selected draft and saved Me. The developer inspected actual World, Customize and Me captures. These are deterministic Auth/C05 fixture receipts, **MOCK_READY**, not signed-in live-service, physical-device, real-multiuser, complete animation, IN01/IN05 or full visual acceptance. Round3 scene proposals remain unadmitted and are not activated by this task.

## Reproduction commands

Use retained dependencies and pinned Node 24.22.2. No installation or lockfile change was performed. From `frontend/`, each command exited 0:

```sh
node node_modules/vitest/vitest.mjs run src/app/character-entry/__tests__ src/app/account-entry/__tests__ src/app/world-entry/__tests__ src/app/media-entry/__tests__ src/app/social-composition/__tests__ src/app/shell-chrome/__tests__ src/app/__tests__/reference-root.test.tsx
node node_modules/typescript/bin/tsc -b --pretty false
node node_modules/vite/bin/vite.js build
```

From the repository root, `python3 scripts/mcr/check-plan.py`, `node scripts/mcr/check-ui-origin.mjs`, and `git diff --check` exited 0. Scoped frontend boundaries passed against the fresh peer master with exact documentation grants for this file and `docs/mcr/frontend/character-runtime-ownership.md`; the published delta contains only assigned root frontend paths, tests/captures and those documents. Fixture source was published separately by its assigned owner. Native consumer/source hashes were checked unchanged after reconciliation. A concurrent backend publication caused one ordinary non-fast-forward rejection; its disjoint paths were fetched and merged before the successful non-force push. No unrelated work was overwritten.
