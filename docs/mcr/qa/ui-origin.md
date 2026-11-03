# C08 source-origin guard

This increment implements the automated part of **C08**. Parent integration and
acceptance remain **NOT_RUN** in this worker; **QA02 is not accepted**. Manual
source comparison, route/visual review, and reviewed new snapshots are still
required. No original application source or assets were copied and no upstream
tests, live services, or hardware were accessed.

Worker base: `3c1e2a18a916a3f39b44ac63987f1c05cd4410c5` (the boundary JSON-doc fix,
after `4b81622da318bae5399b3409e848c6b7352366c3`). Exact scope is the gate, its
fixture test, the allowlist, and this evidence document. Shared check/CI wiring
and master publication belong to the integrator.

## Inputs and use

The existing `docs/mcr/frontend/reuse-inventory.json` remains the sole inventory;
it is not duplicated here. The worker read it from the parent checkout.
`docs/mcr/reuse-allowlist.json` pins its exact raw-file SHA-256:
`9d1202211cd136948a7afeb5588238ba9de13613980dab42616011f47762a079`.
Its source is `julymeltdown/bigyeting` commit
`5eecdcaffd1a9d505b9ab2a974f8d5623217e90a`.

The gate requires allowlist/inventory schema version **1**, that exact repository
and source revision, **422** exclusions, **7** candidates, full lowercase
SHA-256/Git-blob hashes, unique exclusion paths, and matching candidate metadata.
An inventory edit, including whitespace, requires explicit review and repinning.
The inventory's proposal status does not itself authorize source extraction.

```sh
node scripts/mcr/check-ui-origin.mjs --root /path/to/integrated/mcr
node --test scripts/mcr/tests/ui-origin.test.mjs
```

Default root is the current directory. A portable `--root` selects the complete
repository, including its inventory and allowlist. For pre-integration review,
`--allowlist /absolute/path/to/reuse-allowlist.json` can select this patch's
allowlist while the root points to the parent's working tree. The command only
reads files. Exit codes are **0** PASS, **1** detected violations, **2** invalid or
missing inputs, unsupported metadata, unsafe file types, or no frontend content
(`NOT_RUN`). Standard Node libraries only; no install or source checkout is needed.

## Checks and review records

The gate hashes all frontend files recursively, including source, public assets,
tests/E2E, and root entrypoints. It skips directories named `node_modules`,
`dist`, `build`, `coverage`, `.git`, `.vite`, `.cache`, `test-results`, and
`playwright-report`. Symlinks and non-regular inputs fail without traversal.
Exact excluded bytes fail under any destination name or extension. Candidate
whole-file byte matches also fail unless that exact destination is reviewed.

JS/TS variants, CSS/SCSS, and HTML are checked for static imports, export-from,
literal dynamic imports/require/require.resolve, CSS imports/quoted URLs, and
HTML `src`/`href`. Forbidden dependencies include inventory entries marked
`EXCLUDE_FROM_FOUNDATION`, Three.js, GSAP, Motion/Framer Motion, and
`@mantine/`, `@ffmpeg/`, and `@react-three/` namespaces. Package manifests are
also checked, including development/optional/peer dependencies. Legacy
components/pages/widgets/styles/layout/theme namespaces, explicit legacy or
Bigyeting namespaces, and inventoried module/asset references are rejected.
Newly authored `src/main` and `src/app/router` imports retain their specification
paths; their bytes still receive the same legacy hash checks. Comments and
unrelated string literals are skipped. Markdown/prose is not import-scanned.

The shipped `reviewed_headless_extracts` array is **empty**. A future record must
name `source_path`, `source_sha256`, `source_revision`, `destination`,
`destination_sha256`, `kind: "headless-extract"`, `symbols`, `reviewed_by`, and
nonempty `tests` containing existing frontend `.test`/`.spec` source paths.
Source metadata must match an inventoried nonvisual `.ts` candidate; destinations
must be `.ts` source with exact reviewed bytes. Excluded source paths/hashes and
excluded destination hashes cannot be approved. Test paths establish linkage;
the gate does not run or certify the tests. The integrator must review selected
symbols, isolate mixed-file logic, dependency adaptations, provenance, and actual
semantic regression evidence before adding a record.

## Evidence and limits

The tests use generated synthetic inventories and content in temporary roots;
they do not duplicate the real inventory or copy upstream controls/assets.
Negative probes cover renamed legacy UI, asset bytes, prohibited imports and
manifest dependencies, visual-source/destination allowlist edits, unreviewed
candidate copies, missing regression references, wrong hashes/revisions/schema/
counts, missing inputs, and symlinks. Positive probes cover original code,
documentation/comments, reviewed headless metadata, and excluded build/dependency
directories. An empty source tree is explicitly `NOT_RUN`.

This is a deterministic guard, not a sandbox or proof of originality. Modified,
reformatted, partial, renamed-symbol, transformed, or re-encoded legacy code/assets
can evade exact hashes and require manual comparison. The lexer is not a full
parser or dependency resolver: computed imports, escapes, alternative aliases,
unquoted CSS URLs, loaders/globs, JSX/regex edge cases, inline script languages,
and transitive dependencies can be missed or misclassified. Namespace checks can
also flag newly authored code at reserved legacy paths. Other source languages,
excluded directories, remote assets, and secrets are not certified. Reviewed
policy and inventory pins are trusted inputs, not authenticated permissions.
Files must remain stable during a run; there is no concurrent-change protection.

Worker checks (Node **26.7.0**):

| Command | Exit | Result |
|---|---|---|
| `node --test scripts/mcr/tests/ui-origin.test.mjs` | 0 | 14 fixtures passed, no skips |
| `node --check scripts/mcr/check-ui-origin.mjs` | 0 | Syntax check passed |
| `node scripts/mcr/check-ui-origin.mjs --root /Users/lhs/Desktop/mcr --allowlist /Users/lhs/Desktop/mcr-worktrees/qa-boundaries/docs/mcr/reuse-allowlist.json` | 0 | Read-only parent smoke check passed |

Whitespace verification and local commit ancestry are included in the handoff. A read-only
parent working-tree smoke run passed with **11 files / 4 content files**, zero
violations, and zero approved extracts. This is a changing working-tree snapshot,
not immutable candidate or whole-application acceptance. The worker's available
Node is **26.7.0**; C02's selected Node 24 runtime qualification is not established
by this check. Source suites, build/browser/device/multiuser/release checks remain
**NOT_RUN**.
