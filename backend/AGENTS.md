# Backend lane

- Follow the repository-root working agreement and the assigned task's exact writable paths.
- Own backend domain/application/service code, migrations, and backend tests under `backend/`. Keep UI, browser state, frontend mocks, and frontend tests under `frontend/` with their owner.
- Work in an isolated task worktree/branch. Do not edit the frontend or the integrator's checkout.
- Central Cargo manifests/lockfile, shared HTTP/proto contract inputs, service/router composition, and infrastructure remain integrator-owned even when located under `backend/`. Submit the required change with the handoff.
- Communicate frontend needs through versioned contracts and fixtures. Keep server configuration, credentials, persistence, permissions, capacity, and inventory enforcement on the server side.
- Handoff only the assigned backend patch with its base commit, changed paths, checks, evidence, and limitations. A cross-stack feature needs a separate frontend assignment and commit.
- Preserve strict hexagonal layers: domain/application code has no framework, transport, or persistence imports; DTOs belong at adapter boundaries. Proto sources live under `backend/proto/`.
- Persistence acceptance uses real PostgreSQL in explicitly isolated MCR stores. A successful build or mock cannot certify migrations, recovery, or provider flows. Never start imported services with unreviewed source-default database addresses.
- Retain owning-service boundaries and Post outbox/replay semantics. Contract changes go through the final OpenAPI generator and producer/consumer checks.
