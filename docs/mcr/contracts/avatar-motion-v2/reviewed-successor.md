# Canonical reviewed successor work

This is an explicitly assigned extension of C06-MOTION-V2, not completion of the
original candidate handoff or a change to supplied artwork. The existing goal
API remains historically BLOCKED after the build-allocation wait; the owner
explicitly authorized source work to continue without resetting that goal.

## Ownership and base

Implementation checkout `be-avatar-motion-v2`, base
`980fc4d85020f297b36fee8195e1ca92079ff33f`; pre-extension clean source
`ec75dcabf19cb92481e23ec222222a625ee53fb8`. Root independently published the exact
35-path candidate packet as `819d30c3cb32915862281d30cddbe5b7c9bd53cb`.
Candidate schemas, manifest and 31 configs remain byte identical to `ae41db9d`.

Root transferred only the avatar slices of core/realtime refs, validators,
generator-only four OpenAPI mirrors, domain selection/parser/catalog, application
validation, repository representation/parser/serialization and bootstrap avatar
version/eligibility/hash binding. New migration 0044 and the named new runtime/wire
test paths are reserved. No old migration, manifest, lockfile, proto, frontend,
root main/product registry or spatial/map change is granted.

The two new `contracts/assets/v2/reviewed/avatar-{config,manifest}.schema.json`
paths were explicitly granted separately. Root then granted only exact reviewed
schema registration in `generate-mcr-core.py::load_schemas` and
`generate-mcr-domain.py::add_schemas`; no other generator behavior is changed.
Root remains the integrator. FE153 is the separate generated/public parser consumer.

## Artifact and hash rules

The reserved successor is `mcr_selected_motion_reviewed_20261104`, manifest revision
2, contract 2.0.0. The closed motion IDs and pixel mapping are retained. The original
candidate is still `mcr_selected_motion_20261103`, revision 1, with false eligibility
and fallback NONE. No candidate Boolean is flipped or treated as review evidence.

`build_reviewed.py` derives a separate staged manifest and versioned schemas. Its
`production_eligible:true` is an intended release field, not a claim that the staged
release has passed typed/runtime admission or has been published. The builder
reports BUILT_PENDING_RUNTIME_QUALIFICATION. Actual runtime qualification and the
qualified handoff are still required before the integrator admits this release.

The `content_hash` preimage is the entire successor JSON object with only its
root `content_hash` member removed. It includes all resource/source pins, candidate
manifest pin and actual visual-review receipt pin. Objects use sorted Unicode
code-point key order; arrays preserve order. Encoding is compact ASCII JSON,
using lowercase `\uXXXX` escapes and surrogate pairs for non-BMP Unicode, standard
JSON control escapes, integers only, booleans/null preserved, no whitespace/BOM.
Invalid Unicode and floats are rejected. SHA256 yields lowercase hex prefixed
`sha256:`. This matches existing C06 canonical hashing and Rust's retained place
canonical encoder. Duplicate raw keys are rejected before hashing or parsing.

The review receipt binds the candidate/atlas/source/consumer revision, actual
inspected resources, source preservation and the independent 4800-case seam
report/checker. It records concrete visual decisions and limits. It does not
claim all 4800 cases were visually inspected, turn fixture parity into identity
review, or certify live sockets, devices or public HTTP/WS flows.

## Current evidence and remaining work

- Visual inspection and independently computed 4800 composition cases: PASS;
  actual receipt `review/visual-review.json`, bounds result `review/seams.json`.
- Seven new wire tests: PASS using the existing contracts-validation venv. They
  validate real C03 AvatarState and C05 WorldPlayer refs, all 31 successor config
  outcomes, candidate rejection including forged flags, review/resource bytes,
  canonical hash negatives and protected-source equality.
- Existing 29 asset tests plus core 33-fixture and realtime 24-fixture checks: PASS.
- Both OpenAPI mirrors are produced by their actual generators, not hand edited.
- Typed source and canonical backend persistence/bootstrap integration are still
  IN_PROGRESS. Cargo execution and actual runtime/PG evidence are NOT_RUN.
- Initial wire-test run on system Python failed because its URI/date-time format
  plugins were absent. The existing validation venv supplies those pinned plugins;
  no dependency was installed. The first venv run caught an incorrect test baseline
  for candidate fixtures (they did not exist at 980fc4d8); that assertion now compares
  them to their actual immutable ae41 source. Both failures are retained here.

The offered media target was immediately released without Cargo after a fresh
preflight found 7.33 GiB available, below the mandatory 8 GiB floor. No target is
held. The expanded backend source must receive a fresh bounded allocation after
it is ready; no cache cleanup or extra target is implied.
