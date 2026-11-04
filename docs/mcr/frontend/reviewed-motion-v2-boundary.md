# Reviewed motion v2 frontend boundary

Status: CONTRACT_READY; implementation and local qualification complete, final
publication and the publisher's shared build receipt pending. This bounded
AR03/FE12 contribution connects the canonical
reviewed2 contract to generated wire types, the existing realtime schema registry,
and Auth bundle validation. It does not complete AR03, FE12, IN05, device or
multiuser qualification, or activate a default release.

## Ownership and immutable inputs

World grant `30c6fba3d0e12f3ab48105c8909c5765b23b81ec` allocated only the schema
registries in `world/network/__tests__/generate-types.mjs` and
`world/network/transport/contract.ts`, generator-produced
`world/network/transport/wire-types.ts`, and the reviewed2 branch in
`features/auth/model/bootstrap.ts`. New tests live only under the two allocated
`reviewed-motion-v2/` prefixes. Documentation is this file and
`docs/mcr/evidence/avatar-motion-boundary/`.

The clean published base was `54e1325fce20e3abc98da50c68de0bd72dfd1a33`.
Producer checkpoint `0dd9f443945e6d6232728280d03e83933ac0eef2`, parent
`ec75dcabf19cb92481e23ec222222a625ee53fb8`, was consumed locally as dependency
`f61e087b`; it is not an authored frontend handoff or a publication receipt.
The 53-path source comparison is recorded in `producer-provenance.json`.
The domain OpenAPI merge preserves concurrent published additions; the reviewed
config, manifest, core schema and core OpenAPI match the producer bytes exactly.

| Public input | Definition | SHA256 |
| --- | --- | --- |
| `contracts/assets/v2/reviewed/avatar-config.schema.json` | `ReviewedMotionAvatarConfig` | `8d4b5772c763a32e58dd49bf5115894703bba603c6e2c01333cef5a86c988c2e` |
| `contracts/assets/v2/reviewed/avatar-manifest.schema.json` | `ReviewedMotionAvatarManifest` | `3fd9bfd589a7f4d2749081ef76057f8bbaf488fbf7385c4cc024ffd01790ccbf` |
| `contracts/core/v1/core.schema.json` | `AvatarState.config` union | `e9f9618548d255ba53ea9f46efe6fb52ede468acab636d43f0c487eeab779fba` |
| `contracts/generated/mcr-core.openapi.json` | public HTTP mirror | `5b62b6e545f2ac5c367bd189bed6c509b76b4383f47b016d7b075c88f2742978` |

Each reviewed schema has its exact path under
`https://contracts.mcr.invalid/` as `$id`, one unprefixed public `$defs` member,
and a top-level `$ref` to that member. The reviewed manifest is the closed const
for `mcr_selected_motion_reviewed_20261104`, revision 2, contract 2.0.0. Its
content hash is
`sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018`.
Tests use that public const rather than importing backend implementation or
materializing the sparse backend tree.

## Behavior and dependency handoff

The generator algorithm is unchanged. Adding both reviewed documents to its
registry makes the existing core reference produce
`AvatarState['config'] = AvatarConfig | ReviewedMotionAvatarConfig`.
`AvatarConfig` retains its legacy 1.0/1.1 meaning. WorldPlayer inherits the
complete state union. The generated file records all six source hashes.

The runtime registry adds the same two documents without decoder changes.
Closed schemas still reject extra/private fields, mixed representations,
unreviewed candidate IDs, unknown document IDs and missing definitions.
Reviewed outfit/accessory keys remain mandatory and nullable; body/eyes/hair
are required closed IDs. There is no independent skin identifier.

Auth adds only a reviewed2 `validateBundle` branch. It first retains the existing
complete public HTTP validation, owned immutable clone, and canonical bundle
hash checks; it delegates reviewed manifest and config admission to the single
whole-admission parser and retains existing theme/map binding checks. The
legacy branches are unchanged. No local parallel parser, casts, widened union,
Auth contract evaluator or legacy avatar schema changes are introduced here.

Motion83 exclusively owns the public parser and its reviewed helpers. The first
frontend handoff is `7dc84e84358275d23edf41b592bfae4ef381430b`, with exactly
the two registries, generated wire types and new network test. Root153 owns
publication. Producer schemas, registries, helpers, parser and bootstrap must
be integrated coherently: publishing new core refs before their runtime
registry would break legacy frame compilation. This is a concrete dependency,
not a backend PostgreSQL, full v10 or management approval gate.

The first coherent source batch was independently fetched and verified at remote
`a67fc1342b351750ce766a783052c470dc9a9907`: producer `0dd9f443` maps to
`d839bc20`, Motion helpers `24c573fb` to `fe6a7a1d`, and the first boundary
`7dc84e84` to `a67fc134`. All four boundary payloads are byte-identical.
`first-publication.json` records this observation, not publication of later Auth
work. The Auth follow-up is `789b84c307674812a81e67ad59badc83fab8f532`.

## Validation and remaining work

The qualified composition includes Motion helpers `24c573fb` (locally
`0cb1f2b9`) and public parser `34ef8fbbd4fd59f2f806abd1c93308567cce2008`
(locally `4cda0cecaf1d59c62a61b3634d539cf8dc868e99`). These are consumed
dependencies; this owner did not edit their files. Motion's targeted review of
the six-line Auth branch found no ordering or runtime-authority bug.

| Check | Result |
| --- | --- |
| Pinned Node 24.22.2 generation followed by `--check` | Exit 0; byte-identical checked-in output |
| Relevant combined Vitest suites, cache disabled, runner config loader, two workers | Exit 0; 558 tests in 18 suites |
| Full application `tsc --project tsconfig.json --incremental false --pretty false` | Exit 0; zero diagnostics; no shared build-info write |
| Targeted lint | Exit 0 |
| Planning provenance check | Exit 0; planning scope only |
| UI-origin check | Exit 0; static source/import scope only |
| Changed-path and whitespace checks | PASS |

The final combined suite contains all 58 new network cases and 45 Auth cases,
including 15 valid reviewed configurations, 16 malformed configurations,
private/unknown-reference and cross-bundle rejection, full const/hash tampering,
legacy 1.0/1.1, and clone/freeze behavior. It also exercises Motion's public parser
and the actual Root registry's continued staged-release rejection. Exact commands
and exit codes are in `qualification.json` and the command JSON files; logs are
in the same evidence directory. Generation has its own source-hash record.

Intermediate runs remain clearly separate: initial tests used the shell's Node
26.7.0, while all final tests/types/generation use the selected Node 24.22.2.
The first Auth run passed 29 cases and failed 16 valid reviewed cases against
the old parser. The initial type run also exposed a sparse-backend fixture import;
the fixture now comes from the public schema const. Both that lookup and the
old-parser type gap are resolved by the final passing composition. These early
logs are diagnostic history, not final qualification.

The resource coordinator requested Root153's existing shared Vite slot rather
than duplicate output-heavy work. Its actual candidate, command, exit and log
pin must be recorded when returned. Final publication must be independently
verified with fresh fetch/remote refs, ancestry and every delivered payload.
No self push is authorized.

`production_eligible: true` and `VISUALLY_REVIEWED` are staged contract fields.
Structural Auth validation produces no runtime admission authority. Candidate
revision 1 remains denied, and production registry/resource/native/PostgreSQL
qualification remains with its owners. No live multiuser, device, load or
release acceptance is established by these unit or type checks.
