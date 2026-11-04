# Build-script worktree paths

Status: source correction committed for qualification; combined build and native rerun NOT_RUN.

The first Root fourteen-artifact attempt at `c15cb5cfaa6b3cb2c6fb7238bac6449ed3ee521a` failed before any test or database fixture ran. Its first locked/offline Rust 1.88 Cargo command exited 101 after 1.35 seconds. Friend and Notification reported that current-checkout protocol inputs were shadowed by include directories under a previously used resolver worktree.

Auth, Friend, Notification and Concert build scripts still used compile-time `env!("CARGO_MANIFEST_DIR")` for some local protocol paths. A reused build-script executable therefore retained its earlier checkout path. Other inputs came from the retained `contracts_build::proto_root()` helper, which correctly resolves Cargo's current build-script environment at execution time. Mixing the two roots caused the observed `protoc` rejection.

The integrator replaces those eleven local path expressions with the existing runtime `contracts_build::proto_root()` helper. Protocol bytes, generated type configuration, service dependencies, manifests and lockfile remain unchanged. This follows the existing helper's shared-target contract. No cache clean, protocol waiver or old-worktree redirection was performed.

The failed attempt and actual release are preserved privately under the owned Root qualification output. Guarded acceptance was `2026-11-04T06:42:35.165878Z`; actual zero-handle target release was `2026-11-04T06:42:38.280490Z`, before any copied test or PG run. Sampled additional allocation was 864,256 bytes; target growth was zero. These facts establish cleanup, not application qualification.

Validation must rerun the combined build at the corrected clean commit. The prior five-peer source-equivalence claim must also be refreshed: Auth, Friend and Notification build inputs changed, so their production main executables require new qualified builds before a fresh runtime. Post and Profile reuse requires an independent comparison of their complete compile inputs and actual baked migration paths. Full browser IN01 remains unfinished.
