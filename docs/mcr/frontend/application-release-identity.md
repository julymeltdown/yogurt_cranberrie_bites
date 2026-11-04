# Application release identity

Status: IN_PROGRESS. Task APPLICATION-RELEASE-IDENTITY; sole implementation owner 50a8. Root153 owns runtime integration/publication; FE41 consumes the public leaf; BE38 owns trusted server catalog and admission binding.

The deployment owner explicitly declares development applicationVersion `0.1.0`. This is a development release declaration, not a human-approved production release number. Package, map, theme and API versions are independent and never substitute for it.

`frontend/src/shared/release/application.ts` exposes `ApplicationRelease`, `applicationVersionTuple`, `readApplicationRelease` and `capturedApplicationRelease`. A release has exactly version=1, applicationVersion, buildId and sourceRevision. Missing or malformed metadata returns null. The captured object is an immutable compatibility claim, not authentication or admission. Capture once per bootstrap attempt; the policy consumer owns attempt freshness and post-consumption continuity.

The Root adapter builds a canonical manifest from exact tracked production source/public asset Git blobs, dependency declarations, compiler configuration, five adapter assembly inputs, the actual Node binary and six installed package metadata files. It streams source bytes without creating another checkout or copying public assets. The selected revision must be an ancestor of origin/master. Installed package metadata is not an attestation of every node_modules byte.

`buildId` is SHA-256 of the canonical APPLICATION_BUILD_INPUT_IDENTITY. It binds the explicit version, source revision, assembly, toolchain and selected mode. Emitted outputs are explicitly UNAVAILABLE / NOT_BUILT_BY_THIS_PUBLISHER with an empty file list: this publisher does not produce or certify output bundle bytes. Vite compile injection uses the four-field public release; absolute paths, credentials and event/actor settings are excluded.

The receiving runtime owner must generate a fresh manifest for its actual published candidate, actual assembly and actual mode, and retain the readset. Do not reuse a qualified sample ID for another candidate or register arbitrary caller-provided pairs. The BE38 server catalog admits only exact publisher-owned (buildId, declared applicationVersion) entries; absent or unrecognized claims remain typed UNAVAILABLE. SourceRevision is provenance, never a buildId fallback.

Provenance: Root draft 0d1205851f4401662e380aeefdad15142ef7052a (parent4466796ef88bd78015c7c1df136397dde8f2756f), selected canonical baseline80de53f327c90db21955eb63d3b74aa41ec62514, reused as cee5e421d85f3bd2ff4bc1062fc4ce3c43f96691; owner fixes1fa005001. Only the helper import, manifest assembly/cache write, compile define and return property belong to this owner in server.mjs. All other server/proxy/relay seams remain Root-owned.

Qualification, exact commands/exit codes, sampled and full-input hashes, limits and publication receipts belong in docs/mcr/evidence/application-release-identity and the independent identity phase of docs/mcr/evidence/c07-be38-public-wire. Source checks do not certify Vite startup, browser consumption, catalog wiring, authenticated admission or a live multiuser flow. Those remain runtime-owner obligations.
