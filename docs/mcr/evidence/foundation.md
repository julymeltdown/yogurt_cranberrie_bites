# Foundation qualification — 2026-11-03

Scope: C01/C02/C08 and the BE01 build slice. Frontend source candidate `605b858`,
backend import `909e2bf`, boundary/origin guards through `2b42fd4`. Source revisions
and every imported file are bound by the [source map](../source-map.md).

| Executed check | Result |
|---|---|
| Initial frozen frontend install in newly created workspace, Node 24.22.2 / pnpm 11.7.0 | PASS; retained 25 direct source resolutions unchanged, Phaser 3.90.0 added. [Lock evidence](frontend-lockfile.json). |
| `scripts/mcr/check.sh frontend` with pinned Node on PATH | PASS; frozen install, strict TypeScript/Vite build and Oxlint. [Log](foundation/frontend-foundation.log). C08 source scan is now also wired into this command. |
| Pinned source: `cargo +1.88.0 build --locked --workspace` | PASS, 93 seconds. [Log](foundation/source-workspace-build.log). |
| Imported `backend/`: same locked full workspace build | PASS, 29.45 seconds. [Log](foundation/imported-workspace-build.log). All six inherited service binaries built. |
| `python3 backend/scripts/docs/generate-openapi.py --check` | PASS; 92 operations, two artifacts from the final assembler. |
| `python3 -B -m unittest discover -s backend/scripts/tests` | PASS; 22 source generator/verification/Apple-form tests. [Log](foundation/backend-openapi-tests.log). |
| `python3 -B -m unittest discover -s scripts/mcr/tests -p test_boundaries.py` | 38 cases: 37 PASS, one filesystem-specific non-UTF8-name case SKIPPED (macOS EILSEQ). |
| `node --test scripts/mcr/tests/ui-origin.test.mjs` | 14 PASS. Current source scan: 11 files, four content files, zero reviewed extracts, zero violations. |
| Actual Chromium preview at 360/390/430 × 844 | PASS for minimal foundation screen: no horizontal overflow, 44px link targets, English language, zero console errors after favicon correction. [390px screenshot](frontend-foundation-390.png). |

Build environment: macOS arm64; explicit Rust 1.88.0; Node 24.22.2; pnpm 11.7.0.
Backend builds used `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/backend`,
`CARGO_BUILD_JOBS=2`, `CARGO_PROFILE_DEV_DEBUG=0`. The target directory is outside
the repository. No global compiler default was changed. Node archive SHA-256,
pnpm tarball integrity and the Node image digest are in
[`infra/mcr/toolchain.json`](../../../infra/mcr/toolchain.json).

The MSW install hook is explicitly allowed with pnpm 11's `allowBuilds`; the
reviewed hook updates a configured worker directory, and none is configured in
this foundation. No worker or network mocking is claimed by the install result.

Docker daemon execution, backend service startup, migration/DB restart, HTTP
auth/cookies/CSRF, OAuth providers, production build serving, real devices,
multiplayer and release are **NOT_RUN** in this evidence. A later isolated
migration report must not be read as full service restart or login acceptance.
