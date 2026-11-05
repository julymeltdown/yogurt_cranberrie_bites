# Isolated avatar part previews

Status: implementation and developer validation complete; canonical publication
pending this source seal. Owner: 230. Consumer: 534 (MotionEditor call site only).

Complete objective: implement the scoped isolated-part preview producer, validate
pixels/UVs, authority and lifetime bounds, repair validation failures, document
the results, commit an immutable scoped handoff directly to publisher 412e and
deliver the authorized scope through its canonical publication receipt.

Completion requires a typed `acquirePartPreview` lease, transparent intrinsic
64 by 64 raw down/idle item cells, at most four pending or active reservations,
no additional atlas load, synchronous withdrawal before notifications, abort
and unmount cleanup, and unchanged full-outfit behavior without the optional
MotionOption part prop. Relevant source, pixel, UI, type and lint checks must
actually run under fresh finite resource allocations. Consumer integration and
whole-product/device acceptance remain separately owned.

The incompatible historical persistent goal remains BLOCKED and is neither
reset nor falsely completed. This document records the new full objective.
Original task mapping: scoped FE12/FE13 and PI07 presentation delta; no new
original task closure is claimed. Historical FE37 and Reports delivery remains
complete and unchanged.

Preserved source base: `7ef98ff71a8914087aa158d4bd8280bcdeaddfc9`.
Publication baseline: `1907016b6aa41c17998acab691a83e99d0a80076`.
Only exact scoped postimages/hunks are for publication; private ancestry is not.

The dependency is reviewed family `mcr_selected_motion_reviewed_20261104`,
revision 2, contract 2.0.0, source `0dd9f443945e6d6232728280d03e83933ac0eef2`.
Content hash: `sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018`.
Manifest: `backend/releases/selected-character-motion-v2/reviewed/manifest.json`,
SHA-256 `1342f2b640a8135c2f128713fe1f59adc0b6da8b925a0ac84808b2d8d1ec884d`.
Six presets and 28 admitted independent items are distinct from the selected
20 original sheets and their 314 reference groups.

Preparation allocation PARTPREVIEW-PREP32-0147 was actually accepted through
2026-11-05 02:30 UTC: additional 32 MiB (8 source, 16 Git, 8 controls/evidence),
including shared Git growth. Private controls/evidence use
`/private/tmp/mcr-partpreview230-0147`. Retain all historical evidence and the
nonshrinking whole-footprint maximum of at least 4,683,662,502 bytes.
This preparation allocation permits source and scoped Git operations, not test
or runtime execution. The separately allocated executions are recorded below.

## Implemented contract

`runtime.acquirePartPreview({ kind, key }, { signal })` returns a
`MotionPartLease`. Its `read()` returns the selected pair, borrowed canvas,
`representation: 'isolated-part'`, `direction: 'down'`, `phase: 0` and
`composition: 'raw-item-cell'`, or null after withdrawal. Subscribe and dispose
follow the existing scoped lease pattern. Dispose is idempotent.

The API borrows an already existing atlas flight and fails closed before one
exists. It does not start a load, add a deadline timer, cache, art owner, network
request, RAF or scene clock. The verified descriptor must contain the exact
kind/key pair, and the current authority must accept its configuration roundtrip.
Four pending/active reservations bound async races; the compositor independently
bounds active isolated canvases to four. Each preview draws exactly the selected
raw down/idle cell, with no body additions, clipping, hand recoloring or other
composition. Release zeros the canvas; owner withdrawal clears every part canvas
and retained runtime reference before notifying observers.

MotionOption accepts optional typed `part`. With it, the narrow view attaches
the lease canvas itself at 56 CSS pixels and uses layout cleanup for category,
key, runtime and unmount changes. Without it, the existing choose/MotionView
full-outfit path and button semantics remain. MotionEditor is unchanged here;
534 owns its later call-site adoption. Body RGB swatches and separate None
actions are unchanged.

## Executed source validation

Frozen candidate: `a71313a11ff3fc4f4f1733d5db05229c3fff72ce`.
PARTPREVIEW-SOURCE01-0202 actually completed with PASS and literal outer FULL at
2026-11-05 01:58:34.176473 UTC; all processes and temporary output were released
before result inspection. No second source execution was needed or accepted.

- Vitest: 61/61 passed, exit 0. New runtime tests: 18; new UI tests: 8.
  Existing renderer: 9; reviewed production authority: 24; resource: 2.
- Full application TypeScript with private no-emit config: exit 0.
- Exact changed/new producer and test paths, oxlint --deny-warnings: exit 0.

The 28-item source case verifies exactly one approved UV draw per item; it is
not a native pixel comparison. Other cases cover invalid kind/key/shape,
authority refusal, cold-owner refusal, no additional load/timer/clock, four
active and pending slots, abort before/during decode and synchronous draw,
whole-owner withdrawal before callbacks, listener removal, late cleanup,
button semantics, category/runtime changes and absent-part full-outfit behavior.

Raw commands, outputs, source/readset hashes and independent guard records:
`docs/mcr/evidence/parade-ink-part-preview/runs/source-01/`.
Measured primary peaks: WHOLE 4,683,662,502 bytes, cumulative NEW 120,414,208
bytes, temporary 1,380,352 bytes, RSS 2,093,465,600 bytes. The independent outer
record remains authoritative for the full process tree and carry maximum.
Historical raw failures, source and evidence remain preserved.

At the SOURCE01 checkpoint, native RGBA/transparent-cell comparison and
representative cards were NOT_RUN, awaiting a separate finite browser allocation.
Real backend/provider journeys,
physical devices, consumer MotionEditor integration and whole-PI07/reference
parity are not established by this producer source validation.

## Preserved native attempts

NATIVE01-0208 stopped before the primary or browser at 02:03:14.687672 UTC:
the preserved checkout does not materialize the reviewed backend manifest.
The actual immutable producer blob at `0dd9f443` was then read directly from
Git: 97,407 bytes, exact SHA-256 `1342f2b640a8135c2f128713fe1f59adc0b6da8b925a0ac84808b2d8d1ec884d`.
Its JSON exactly equals the local reviewed schema const. The corrected inventory
charges complete containing Git inodes; no file or authority was fabricated.

NATIVE02-0208 reached Chrome and generated the three representative captures,
but its independent outer guard failed when `du` raced an atomically replaced
Chrome preference file. It is FAIL, not accepted native proof. Outer FULL was
02:04:11.864334 UTC, all processes and temporary output released; strict port
5373 was successfully rebound and closed after release. The retained failed-run
maxima are WHOLE 5,533,209,766 bytes and cumulative NEW 126,062,592 bytes.
The three images were inspected after FULL and show isolated hair, outfit and
eye cells; the failed guard still prevents a native acceptance claim.

Own guards now retry at most three literal vanished-file races, retaining the
largest partial allocation and requiring a complete successful census. Every
other error remains fail-closed. The bootstrap receipt is written before its
canonical ACCEPT pointer, and the primary points to its already-written actual
identity receipt before expensive census work. A peer exposed the former
pointer-order issue; no peer record was modified.

Both raw failed attempts remain under `runs/native-01/` and `runs/native-02/`.
The next native execution required a fresh distinct allocation; closed rounds
were not reopened. The later notification fix required only its affected
source checks, as recorded below.

## Lifetime follow-up

A final source pass found that a withdrawal observer could reenter dispose while
notifications were still being iterated. The producer now clears the listener
set before callbacks and returns immediately from an already closed lease.
The added regression also verifies that one throwing observer cannot prevent
another observer from receiving exactly one notification.

SOURCE02 stopped before commands because its strict clean-tree check rejected
the outer guard's newly sealed bootstrap receipt. Its raw failure remains
preserved. The primary now permits exactly that single expected untracked
receipt, using `--untracked-files=all`; all other mutations still fail admission.

SOURCE03-0215 qualified candidate `b69640b5bf551e90246d035d3273d5481adb5ec8`:
four affected notification/reentrant lifetime cases passed, full application
no-emit TypeScript passed, and changed runtime/test lint passed, all exit 0.
The other 23 cases in the two selected files were intentionally not rerun; the
35 unchanged renderer/authority/resource cases were not selected. Retain the
earlier 61-case source result as historical coverage, not a fresh 62-case run.
Actual outer FULL: 2026-11-05 02:17:42.093908 UTC, all resources released before
inspection. Whole-tree guard maxima: WHOLE 5,533,209,766 bytes, cumulative NEW
130,646,016 bytes, temporary 1,388,544 bytes, RSS 2,146,729,984 bytes, five PIDs.

Receipt-first publication was exercised successfully in SOURCE03: outer and
primary factual acceptance files existed before their canonical pointers and
before expensive census/workload. Both independent complete initial LIVE
samples were recorded before commands. All source/failure/guard evidence is
retained under `runs/source-02/` and `runs/source-03/`.

## Completed native product validation

NATIVE03-0215 ran candidate `71204892b0b3fc50dde880a0995ab18258315a2e`;
its four production postimages are identical to SOURCE03's qualified source.
Chrome application metadata version: 156.0.8078.80, retained local executable.
The controlled protected-bootstrap fixture minted a reviewed production authority;
the real runtime loaded and verified the actual selected descriptor and atlas.
This is native rendering with a controlled authorization fixture, not a live
backend/provider or physical-device journey.

`runs/native-03/native.json` is PASS: all 28 down/idle raw item cells have zero
RGBA mismatches against independent crops from the one observed approved atlas
response. Every cell is 64 by 64 and retains both transparent and visible pixels.
The pixel hashes, approved frame indices and per-cell alpha counts are recorded.
The native run also verifies four displayed hair, outfit and eye options, no
full-outfit composition or clock, one descriptor request, exactly one atlas
request, synchronous erasure/removal of all four canvases on withdrawal, and
zero remaining part reservations. The three 390px PNGs were inspected after
literal FULL; they show only the requested parts and legible option labels.
Their combined size is 47,662 bytes. Browser errors: zero.

Native product and primary execution are PASS. The independent outer guard hit
`PermissionError: [Errno 1] Operation not permitted` in its final `os.killpg`
cleanup loop and exited before writing its final receipt. Its status remains
FAIL_CLEANUP_REPORTING; no outer PASS or unsupported failure cause is inferred.
Both independent complete initial LIVE samples preceded browser work, and the
primary retained its actual measurements and successful cleanup record.

Recovery completed within the original fixed lease, with no new workload:
the original controller/guard and own runtime/profile commands were absent,
the primary had released all children and its temporary root, the browser and
server were closed, and strict port 5373 was rebound and closed. Every remaining
outer temporary file was archived before removing that exact owned directory.
The canonical owner receipt was atomically marked literal FULL at
2026-11-05 02:22:17.887690 UTC, resources zero, before pixel/log/image inspection
and this documentation. Raw outer state, census and primary output remain in
`runs/native-03/outer-recovery/`; the recovered outer receipt records the failure
and actual closure separately from the product result. No duplicate browser job
was run solely to produce a green helper/reporting receipt.

Primary maxima: WHOLE 5,543,919,440 bytes, cumulative NEW 162,013,184 bytes,
temporary 30,199,808 bytes, RSS 1,872,969,728 bytes, 10 PIDs. Retain all historical
higher maxima and failed attempts; cleanup gives no deletion credit.
Native result SHA-256: `9b75fa0a0e5e3ad7eadec487ea1607b54679240471f775a380bdca4fc6005fa4`.
Primary release SHA-256: `641b34c2d4679f80aa63a465c12a4c3d6d905f99480f656b8ffd72800f28e430`.
Recovered outer release SHA-256: `de7053776e36da515851510791c7199f6b1a19faa3cef5ef047877ef187b6b2c`.

## Scoped delivery

The publisher consumes only the exact authorized changed postimages/hunks in
`docs/mcr/evidence/parade-ink-part-preview/handoff.json`, from the preserved base
`7ef98ff7` and canonical baseline `1907016b`. The three granted shared-file
preimages match both bases exactly. No private branch ancestry is for integration.
The immutable source seal is sent directly to publisher 412e, Root153 for outcome
accounting and consumer 534 for call-site ownership, without review/ACK gates.
The publisher's one canonical receipt establishes delivery; this source document
does not predict its future publication commit or require a proof-only successor.

Only this isolated producer is qualified. MotionEditor call-site integration,
Profile/World/default-preset product acceptance, whole PI07/reference parity,
real backend/provider journeys and physical devices remain with their existing
owners and evidence. No original task closure or new admitted inventory is
claimed, and the incompatible historical persistent goal remains BLOCKED.
