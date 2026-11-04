# Hosted contract CI installation recovery

Original obligations: C02 / C07 / QA10. Status: ACCEPTED for this bounded hosted contract-CI recovery. Both repaired hosted runs passed; whole-release acceptance remains separate.

The existing `MCR contract generation` workflow failed before QA01/C07 could run. On [run 41880082996](https://github.com/julymeltdown/mcr/actions/runs/41880082996), candidate `dcccb88283a7212915c35cf7a4f9b2ca8bd84e29`, job `126749969067`, the install step invoked Corepack from the repository root. It selected pnpm 12.16.1, while `frontend/package.json` specifies `pnpm@11.7.0`. pnpm rejected the mismatch with `ERR_PNPM_BAD_PM_VERSION`; both contract-check steps were skipped. Earlier local C07 evidence is preserved and does not establish hosted success.

The minimal repair runs the dependency-install step with `working-directory: frontend`. Corepack can then resolve the frontend's existing package-manager pin before invoking pnpm. The step prints its selected version and retains `install --frozen-lockfile`. The [Corepack documentation](https://github.com/nodejs/corepack/blob/main/README.md) describes local project package-manager selection and the fallback when no package manager is configured. The old pnpm `--dir frontend` argument did not set Corepack's own working directory.

The workflow keeps the same pinned checkout/Python/Node actions, Python 3.14.3, Node 24.22.2, Ubuntu 24.04, 15-minute timeout, read-only repository permission, push/pull-request triggers, Python requirements, complete QA01 coverage/negative probes and complete C07 reproducibility/compatibility checks. No check is skipped or allowed to fail. Manifests, lockfile resolutions, generators, contracts and application code remain unchanged. No new service, runner upgrade, performance job or local Rust target is introduced.

## Source and custody

The exact working base is published `b97608d4235eb7f82cba29c4807e219f7f606e3c`. Root transferred the sole existing workflow path and new documentation/evidence prefixes; World stopped all writes and loaned the existing `sl-world` checkout exclusively. The new branch is `codex/mcr-ci-installation-recovery`. World `c277f4697155508f11687c9a6f993ffbf700d8aa`, FE16 `26688e8f9c4df4a476cb05ab4c9405f899b5ada5` and shared CSS `390493282882413b85c7e08f91368548c570e38a` remain preserved. No new checkout or dependency installation is needed for local authoring.

Base Git blobs: workflow `95af1aca8fa5c6bf372033fcb0a1822addc4b70d`; frontend manifest `d3c1adc42639ce34c38c62de7b02d40d55fa9b42`; lockfile `7717408b8c7e5dca744e85aa55ea792e09ab5c5c`. Supplied `docs/source/` remains byte for byte unchanged. The owner's new unbudgeted persistent goal remains active until normal publication and actual hosted QA01/C07 success, including any necessary fixes.

## Qualification and delivery

Implementation commits `a14dc4df` and `db35904c2701c85f7edeeb7c2f6a6835f0e6d96b` were delivered through Root's normal serialized publisher. Root's canonical receipt records exact five-path publication at `28a3716ed60ca208c3e98d26b8835e9505d3f930`, included in independently verified remote master `a9c04eb256e7b50e6efc99bfd076eca9a8293100`, with scoped modes/blobs and normal non-force push. No repeated publisher readback or proof-only successor was required.

Developer source inspection and `git diff b97608d4235eb7f82cba29c4807e219f7f606e3c HEAD --check` passed (exit 0). A packet-wide whitespace check initially identified two timestamp-only blank lines in the original hosted log. The full log bytes were then preserved losslessly as a JSON-escaped string with their SHA-256, rather than trimming the diagnostic. The optional local Ruby YAML/bash preflight remained NOT_RUN; its unaccepted source request was withdrawn so it held no resource and did not gate actual hosted qualification. No local dependency install, Corepack invocation, app test replay, browser, Rust build, temporary runtime tree, dependency link or port was created.

| Hosted run | Actual candidate | Install | QA01 | C07 | Overall |
| --- | --- | --- | --- | --- | --- |
| [Original failure 41880082996](https://github.com/julymeltdown/mcr/actions/runs/41880082996) | `dcccb88283a7212915c35cf7a4f9b2ca8bd84e29` | FAILED, pnpm 12.16.1 mismatch | SKIPPED | SKIPPED | FAILURE |
| [Repaired push 41883941337](https://github.com/julymeltdown/mcr/actions/runs/41883941337) | `a9c04eb256e7b50e6efc99bfd076eca9a8293100` | SUCCESS | SUCCESS | SUCCESS | SUCCESS |
| [Published descendant 41883753649](https://github.com/julymeltdown/mcr/actions/runs/41883753649) | `1be2c6873d482477a817005c0829f6af8d3c5128` | SUCCESS | SUCCESS | SUCCESS | SUCCESS |

The actual hosted logs confirm Node `24.22.2`, pnpm `11.7.0`, an up-to-date frozen lockfile with the resolution step skipped, and successful installation. QA01 passed with 193 fixtures, 1,591 responses and 140 operations, followed by all 13 coverage/negative-probe regression tests. C07 reported `PASS` for all eight original checks: drift, deliberate schema mismatch, missing operation, output drift, inventory regressions, motion regressions, strict TypeScript and runtime compatibility. Both complete `contracts` jobs succeeded; none of these steps was skipped, made optional or replaced with an echo/local historical claim.

The push run appeared after a metadata delay. Workflow activity, enabled Actions, the fixed source and both eventual push events were observed; no token-suppression diagnosis was established. No manual trigger, workflow dispatch change, empty commit or rerun of an old unfixed candidate was needed. Both qualified runs were ordinary existing-workflow push runs.

See [repaired-push evidence](evidence/ci-installation-recovery/hosted-success-41883941337.json) and [descendant evidence](evidence/ci-installation-recovery/hosted-success-41883753649.json). Each preserves full job metadata, the losslessly encoded decoded job log, log SHA-256, 14 exact source/blob/SHA-256 pins and four canonical input-tree pins. Both runs have identical contract readsets. The qualified workflow blob is `0ff97ca3a559351a0ee669613b218dfe62a9a534`, SHA-256 `a872b1c0137d631423d4806dfbb8a71f0bd75e80c64e83f76eee428cb4b6eb10`. The prior [local C07 record](c07-generation.md) and its original evidence remain unchanged; its historical hosted-NOT_RUN statement is superseded only for the exact hosted candidates recorded here.

This bounded contract workflow establishes hosted setup and the existing offline contract checks. Full frontend/backend application builds, real servers, devices, multiuser flows, deployment, performance and whole QA10/IN05 acceptance are not established by it. No original C02/C07/QA10 product obligation is silently removed. The final evidence/documentation handoff is published normally; the clean checkout loan is returned with preserved refs and no owned runtime resources.
