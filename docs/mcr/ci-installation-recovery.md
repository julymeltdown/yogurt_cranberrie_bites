# Hosted contract CI installation recovery

Original obligations: C02 / C07 / QA10. Status: IN_PROGRESS; repaired hosted result NOT_RUN.

The existing `MCR contract generation` workflow failed before QA01/C07 could run. On [run 41880082996](https://github.com/julymeltdown/mcr/actions/runs/41880082996), candidate `dcccb88283a7212915c35cf7a4f9b2ca8bd84e29`, job `126749969067`, the install step invoked Corepack from the repository root. It selected pnpm 12.16.1, while `frontend/package.json` specifies `pnpm@11.7.0`. pnpm rejected the mismatch with `ERR_PNPM_BAD_PM_VERSION`; both contract-check steps were skipped. Earlier local C07 evidence is preserved and does not establish hosted success.

The minimal repair runs the dependency-install step with `working-directory: frontend`. Corepack can then resolve the frontend's existing package-manager pin before invoking pnpm. The step prints its selected version and retains `install --frozen-lockfile`. The [Corepack documentation](https://github.com/nodejs/corepack/blob/main/README.md) describes local project package-manager selection and the fallback when no package manager is configured. The old pnpm `--dir frontend` argument did not set Corepack's own working directory.

The workflow keeps the same pinned checkout/Python/Node actions, Python 3.14.3, Node 24.22.2, Ubuntu 24.04, 15-minute timeout, read-only repository permission, push/pull-request triggers, Python requirements, complete QA01 coverage/negative probes and complete C07 reproducibility/compatibility checks. No check is skipped or allowed to fail. Manifests, lockfile resolutions, generators, contracts and application code remain unchanged. No new service, runner upgrade, performance job or local Rust target is introduced.

## Source and custody

The exact working base is published `b97608d4235eb7f82cba29c4807e219f7f606e3c`. Root transferred the sole existing workflow path and new documentation/evidence prefixes; World stopped all writes and loaned the existing `sl-world` checkout exclusively. The new branch is `codex/mcr-ci-installation-recovery`. World `c277f4697155508f11687c9a6f993ffbf700d8aa`, FE16 `26688e8f9c4df4a476cb05ab4c9405f899b5ada5` and shared CSS `390493282882413b85c7e08f91368548c570e38a` remain preserved. No new checkout or dependency installation is needed for local authoring.

Base Git blobs: workflow `95af1aca8fa5c6bf372033fcb0a1822addc4b70d`; frontend manifest `d3c1adc42639ce34c38c62de7b02d40d55fa9b42`; lockfile `7717408b8c7e5dca744e85aa55ea792e09ab5c5c`. Supplied `docs/source/` remains byte for byte unchanged. The owner's new unbudgeted persistent goal remains active until normal publication and actual hosted QA01/C07 success, including any necessary fixes.

## Qualification and delivery

Pending developer workflow checks, direct committed publication and actual hosted result. Evidence under `docs/mcr/evidence/ci-installation-recovery/` will bind the failed and repaired runs to their exact commit/workflow/manifests and retain failure history. This bounded contract workflow does not establish full application builds, device, multiuser, backend deployment or whole-release acceptance.
