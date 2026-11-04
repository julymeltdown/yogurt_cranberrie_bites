# FE12/FE13 avatar integration continuity

Status: IN_PROGRESS. The explicit Cancel action and focused source/client
qualification are authored; execution and canonical publication are NOT_RUN.
This is a new integration goal. Completed AR03 artwork and C06 remain preserved.

## Scope and behavior

Base: `a10e4f409f5727cb25b6cc37d75f499a73e9bb70`; exclusive existing
`art-runtime` worktree, branch `codex/mcr-fe-FE12-FE13-avatar-continuity`.
Root153 stopped and transferred the new integration-continuity source/test and
documentation/evidence prefixes. Additional exact grants cover only the
`MotionEditor.tsx` footer Reset-to-Cancel label and its two existing selector
consumers (`reviewed-production/owner.test.tsx` and default-entry `native.mjs`).

FE13 explicitly requires draft/save/cancel. The functioning editor already
discarded drafts through `controller.reset()`, but displayed only Reset. Its
footer now displays Cancel with accessible name Cancel changes. It invokes the
same existing operation: discard the draft, retain persisted configuration,
perform no write. During reconciliation it stays disabled until the user chooses
which authoritative outfit to adopt. No controller or persistence change was
needed; the existing reset also invalidates pending selections via editEpoch.

The seven new production-owner tests exercise actual canonical HTTP adapters and
minted reviewed authority, with deterministic HTTP producers and mocked Canvas:
Cancel without a mutation/publication; newer reload preserving a draft; stale
same-version changed receipt rejection and recovery; initial read failure/retry;
optimistic conflict then Keep/Cancel; uncertain committed-save readback without
duplicate writes; and out-of-catalog rejection. They borrow the retained fixture
and actual runtime, editor, controller and persistence code.

The new narrow 390px native journey mounts the actual `main.tsx` via the retained
default-entry fixture. It uses real login and editor controls, natural browser
rendering, and controlled HTTP/C05 producers. It checks that Cancel restores
the persisted pixels without a PUT, conflict preserves draft pixels, a normal
latest-server read requires explicit reconciliation, and Keep followed by Cancel
adopts the newer persisted outfit without another PUT. It is not live backend,
database, two-account or physical-device evidence.
The preview must match the exact corresponding admitted preset thumbnail's
nonempty pixels, including the server-selected preset after reconciliation.

## Retained evidence and provenance

The published persistence/motion continuation already covers a real document
reload and canonical UI login after Save, identical saved Profile/World pixels,
actual joystick input, incumbent scene animation, an authoritative peer outfit
and movement update, and sign-out cleanup. Its source `Socket.peerMotion` changes
the config to preset 2; it is not merely an unchanged peer animation proof.
Reuse `docs/mcr/evidence/reviewed-persistence-motion/browser-retry04/native/`
`2026-11-04T16-46-52-781Z-native.json` and its original qualification. Do not
rerun or relabel the accepted Save/Profile/self/peer/sign-out matrix.

Catalog and independent layer controls reuse the accepted all-three-width source
and native evidence in `frontend-combined-adoption/reviewed-round01` and
`avatar-part-controls`; this change adds invalid-catalog continuity coverage.
Artwork is the retained selected pack revision 3, reviewed manifest
`mcr_selected_motion_reviewed_20261104` revision 2, content hash
`sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018`.
No originals, derivative images, asset contracts, backend code, generated files,
manifest, lockfile, router or shared transport changes are included.

New fixtures borrow the published reviewed-production fixtures and default-entry
preview at the pinned base. The bounded supervisor is adapted from the retained
`reviewed-persistence-motion/qualify.py`. Dependencies remain the retained Node
24.22.2 installation and frontend lockfile resolutions. Root153 publishes the
committed validated handoff; there is no king/SL review or acknowledgement gate.

## Qualification

Fresh allocator offer and explicit acceptance are required before execution.
Source/browser jobs, command exits, source pins, original failed attempts, byte
and process bounds, cleanup and exact publication ancestry will be recorded in
`docs/mcr/evidence/avatar-integration-continuity/`. Execution remains NOT_RUN
until those actual results exist.

The source round also checks the full application with the pinned retained type
helper (SHA256 `4cbcb57e175101a3fabbcbd67604c223d2604e24732f9d6e3331583a0d624aa1`).
As in the retained qualification, its no-emit invocation enables
`allowImportingTsExtensions` for existing test-config imports while preserving
the entire file set and diagnostics. No tracked compiler config changes.
