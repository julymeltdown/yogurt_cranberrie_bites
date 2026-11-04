# FE12/FE13 avatar integration continuity

Status: IN_PROGRESS, waiting for final publication. Developer implementation and
source/native validation are delivered. Root published the source unit as
`c3c60842a0900e1625633e293092509fe59d341f`; the native harness repairs and
completed evidence are the remaining publication handoff.
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
and process bounds, cleanup and exact publication ancestry are recorded in
`docs/mcr/evidence/avatar-integration-continuity/`.

The source round also checks the full application with the pinned retained type
helper (SHA256 `4cbcb57e175101a3fabbcbd67604c223d2604e24732f9d6e3331583a0d624aa1`).
As in the retained qualification, its no-emit invocation enables
`allowImportingTsExtensions` for existing test-config imports while preserving
the entire file set and diagnostics. No tracked compiler config changes.

The first finite source offer, `AVATAR-CANCEL-SOURCE-20261104T1955Z`, became
predecessor-eligible after QA10's literal release. Admission then failed before
ACCEPT: free space was 12,774,301,696 bytes, below the offer's explicit 12 GiB
start floor. Projected free space after all outstanding/proposed growth passed
the separate 9 GiB reserve. No worker, guardian, temporary root, dependency link
or application process started. The declined offer is not reused or weakened;
the same candidate was requeued for a fresh eligible offer. The exact declined
disposition was sent directly to 21d and the named FE34 resource successor.

The fresh `AVATAR-CANCEL-SOURCE-20261104T2002Z` offer was actually accepted at
20:01:07.408862 UTC on November 4 (05:01:07 KST on November 5), source
`73542e304f999d8bb8743fe1d36a1b3b4c583493`. Its explicitly offered start floor
was 11 GiB, with 9 GiB after all outstanding/proposed growth and 8 GiB live.
Fresh free space was 12,872,318,976 bytes; conservative after-ALL free space
was 9,982,443,520 bytes. The original work/FULL clocks were 420/480 seconds;
no clock, footprint, process or ownership limit was extended.

All four sequential commands exited 0: 35 tests across three files (the seven
new cases plus 28 affected retained cases), scoped lint with zero warnings,
full application types across 1,036 files with zero diagnostics, and the new
native driver's JavaScript syntax. Test output retains React act warnings from
the affected retained owner tests; native rendering was qualified separately.

Supervisor and independent guardian both reported no breach. Peak aggregate
usage was six processes, 1,748,238,336 bytes RSS, 5,931,963 bytes inclusive NEW,
and 93,085,611 bytes WHOLE; temporary peak was 3,557,320 bytes. The fixed caps
were eight processes/2.5 GiB RSS, 68 MiB inclusive NEW, 224 MiB WHOLE,
48 MiB temporary and 20 MiB durable. Independent final process/path checks
proved all owned PIDs ended, private temporary files were removed, no dependency
link existed, and no application/browser/target resources were held. Literal
FULL was sent directly to the FE34 successor and 21d at 20:01:59.665985 UTC.
A separate browser offer was required; its completed round follows.

## Native result and developer fixes

`AVATAR-CANCEL-390-NATIVE-20261104T2010Z` was accepted at 05:08:55.070330 KST
on November 5 (20:08:55.070330 UTC November 4), after QA06's literal release.
Its original work/FULL deadlines stayed 05:15:55/05:16:55 KST. Fresh free space
was 19,082,252,288 bytes; after all conservatively charged growth it was
16,074,936,320 bytes. The two guards retained 11/9/8 GiB start/reserved/live
floors, 48 processes/4 GiB RSS, 116 MiB inclusive NEW, 288 MiB WHOLE,
96 MiB temporary and 20 MiB durable. Each retry retained the higher prior
WHOLE maximum; no failure, Git, image or other proof bytes were excluded.

The final one-390px actual-main journey passed at source
`4d6ff4dd62e0d253097f0f899969e7d70ea4f0c5`, Chrome 156.0.8078.80:

- Choose preset 2, then Cancel: the preview exactly returns to preset 1 pixels,
  Save disables, and no PUT occurs.
- A controlled producer changes the saved outfit to preset 5 at revision 10.
  The actual optimistic PUT at expected revision 7 receives a conflict. Draft
  preset 2 pixels remain; Save disables until an explicit latest-state read.
- That explicit read produces the reconciliation controls while preserving the
  draft. Keep retains preset 2; Cancel then restores the exact admitted preset 5
  pixels and disables Save. There is one rejected PUT and no duplicate write.
  One additional legitimate participant read refreshes the committed consumer.

The five failed attempts remain unchanged with their raw logs and images:
CommonJS Playwright exports; a retained dependency subpath; an overbroad package
alias; the older fixture's missing current Auth admission response; and an overly
strict read-count assertion that conflated reconciliation with Profile refresh.
All repairs are confined to this owner's test harness/control paths. The fixture
now reuses the canonical Auth admission helper through the real protected request
path. It does not replace Auth, admission logic, controllers, rendering or clocks.
Package aliases match exact names, and locator/navigation waits are bounded.
The original seven product cases were not rerun; product code did not change.

`native-retry05/native/native.json` records the passing checks, exact preset pixel
digests, controlled requests and three unaltered captures. Developer visual
inspection confirms the original, retained-draft/reconciliation and latest-server
outfits with visible Cancel controls and selected preset state. This qualifies the
changed flow at 390px; full-reference, physical-device and real-service acceptance
remain separate and are not claimed.

Across all six attempts, both guards reported no breach. Peak usage was 14
processes, 2,685,861,888 bytes RSS, 33,548,098 bytes inclusive NEW,
230,740,437 bytes WHOLE, 30,316,369 bytes temporary and 785,845 bytes durable.
Independent final checks found no owned PIDs, temporary roots, dependency link,
Vite/Chrome or other held resources. Literal FULL at 05:13:16.661171 KST was sent
directly to Map26e and 21d before documentation or publication work.

The source publication receipt binds `54685f7df` to `c3c60842a` with all 25
scoped modes/blobs exact in remote `7804fbae`. Native completion is handed to
the same Root153 publisher, without a king/SL review or acknowledgement stage.
