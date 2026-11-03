# Illustrated scenery contract integration

Status: CONTRACT_READY. This checkpoint allows optional illustrated scenery in
the existing C06 ThemeManifest and embedded C03 Bootstrap. It does not approve
character artwork, publish a product release, or certify the active Phaser patch.

Author `d682bb4ea345eae9bed2132436f141822e9d8f71` was integrated as `0fd471d`.
Its eight paths match the explicit frontend and C06 support grants; the author's
ongoing renderer work stayed in its isolated checkout. No new Bootstrap field
was required: the existing theme-manifest reference includes this optional field.
Both core OpenAPI copies and the domain OpenAPI were regenerated from canonical
inputs, without hand-editing generated schema content.

The Python asset validator and Rust Bootstrap validator now match the frontend's
closed-schema and cross-field rules: unique texture IDs and paths, atlas-only
frame references, exact map-sized ground, intersecting object bounds with a
128-pixel perimeter allowance, depth bounds, at most four textures and 96 objects,
2 MiB aggregate transfer and 2,097,152 aggregate pixels. Legacy manifests without
scenery retain their behavior. The file loader reads one flat bundle directory;
nested positive/negative fixture suites are independent of that release bundle.

Twenty-five shared synthetic cases exercise valid edges and invalid references,
duplicates, geometry and budgets in both Python and the actual Rust embedded
Bootstrap validator. Hashes are recomputed before rejection checks so stale
integrity values cannot mask a semantic failure. The frontend's 12 scenery cases
and 48 retained Auth contract cases also pass, as does full TypeScript checking.

Evidence with exact commands, exits, source/binary hashes and failed cache-repair
attempts is in `evidence/illustrated-contract-integration-2026-11-04.json`.
Python asset tests: 29 passed; core tests: 23 passed; domain fixtures: 47 passed;
both generation checks and the planning provenance check passed. Rust's exact
scenery test passed all 25 shared cases after stale cross-worktree local build
artifacts were rebuilt. No third target or dependency installation was used.

The main build lease was released after the terminal result, copied executable
and zero-open-file check; the Intro owner received the next explicit lease.
The same checks do not constitute rendered-scene, device, real multiuser, source
art approval or operational venue evidence. Those gates remain open.
