# Frontend lane

- Follow the repository-root working agreement and the assigned task's exact writable paths.
- No decorative left accent lines or colored left borders, including error/toast/skeleton variants or pseudo-element substitutes. Avoid generic AI-styled card decoration. Use clear labels, spacing and typography; preserve accessible focus indicators.
- Own newly authored browser UI, client state, Phaser runtime, headless client logic, and frontend tests under `frontend/`. Backend services, migrations, and server tests belong to their backend owner.
- Work in an isolated task worktree/branch. Do not edit the backend or the integrator's checkout.
- Package manifests/lockfile, root router composition, and shared/generated contracts remain integrator-owned even when located under `frontend/`. Submit the required change with the handoff.
- Use agreed HTTP/WebSocket contracts and generated public types; do not import backend implementation files or server configuration. Use deterministic client mocks until the real service is ready, and identify mock evidence accurately.
- Handoff only the assigned frontend patch with its base commit, changed paths, checks, evidence, and limitations. A cross-stack feature needs a separate backend assignment and commit.
