# World presentation budget

Status: IN_PROGRESS. This is a bounded original FE06/QA06 presentation correction.
The original ten-screen reference obligations and actual device/live qualification
remain open. No browser or build pass is inferred from source or unit tests.

## Ownership and inputs

The clean source base is published
`d1a5deaae0f6b79b365cb4c451d921711573ab8c`, containing `12a5d394`.
World's pre-edit grant is `e6312ce472a4b00f64d0c874424c0f715b5e4e89`.
Implementation is isolated on `codex/mcr-fe-world-reference-budget` in
`fe-ref-customize`. The prior canonical and mural branches are preserved.
The new goal is `1793766233`, unbudgeted and active until validation, fixes,
documentation, publisher handoff, remote verification and cleanup complete.

Source changes are only the closed-World tools branch/local state in
`frontend/src/app/WorldShell.tsx`, corresponding tools/stage and composition
summary/controls presentation selectors in `frontend/src/app/shell.css`, and the
new `frontend/src/app/world-presentation/WorldTools.tsx` helper. Composition's
existing status/retry/privacy/resource logic needs no changes. Camera, MapScene,
authoritative coordinates, approved art, eight Root-owned MAP files, footer,
panels, global typography, contracts and backend remain outside the assignment.

World additionally allocated three existing route-presence test selectors before edits in
`46d3999555e30295afa2b75330f1c42dcdaea180`: one Map-link sentinel in
`app/__tests__/reference-root/root.test.tsx` and two World-tools-navigation
sentinels in `app/__tests__/default-shell/chrome.test.tsx` now identify the visible
World-tools button. Every original route, focus, input, mount and one-world
assertion remains. New tests and native driver use only
`app/__tests__/world-presentation-budget/`; existing v5 tests are read-only.

QA06's immutable input is
`e8dac180cd20b9135a32b13243f5f05dccd34de2`, parent `e3173efb`.
Its packet distinguishes the approximate existing raster height of 281px at
390×554 from the code-derived extra-row estimate of about 86px. Neither is a
retained 554px DOM/zoom measurement. The six actors' tight formation is explicit
fixture content; it is not a product position defect.

The unchanged reference image SHA256 is
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`;
World crop `(589,3,370,526)` is displayed proportionally. The previous actual raw
World image is `71f4a58478241753e44cf1f3e5a25fcef54823f9548a996df068414c39a031db`,
and QA06's paired World image is
`40a2b62d7f49c72d56eb904e974be678a230256fb75f8625ab882a4ef4ea08a7`.
The driver verifies these immutable input bytes without changing source images.

## Presentation change

The four original World tool destinations move into a compact 44px disclosure
over the scene. Their exact IDs and routes are retained. Disclosure state belongs
to the persistent shell so returning from a panel can restore focus to its
originating link. Escape closes the disclosure and returns focus to its button.
The main/skip target remains `shell-content`; six primary destinations remain.

The scene's grid area spans the full composition, including the compact status
strip. Controls remain in the first grid row, above that strip. Connected status
and the full virtual-position notice remain readable; reconnect/error states
retain their original live text and 44px retry action. Their layout can expand
without reducing the camera viewport or changing lifecycle/authority logic.
No camera formula, server feet, fixture roster, geometry or image is changed.

## Qualification plan and current evidence

Meaningful regressions pass 110 tests across seven suites, including seven new
disclosure/route/focus/same-world cases and all 39 retained shell checks. Full
application types and targeted lint exit 0. Runtime evidence is still NOT_RUN.

The new native driver reuses the read-only published actual Root/default Phaser
fixture. Its baseline loads the exact two pre-change files from `d1a5deaa` through
a test-only Vite source plugin; the candidate loads the authored files normally.
All remaining application, asset and fixture inputs are the same. Before/after
DOM rectangles, canvas dimensions, camera state and authoritative self are
recorded at 360/390/430×554/844; candidate captures include all three eras.
The original reference stays unchanged and is compared at its actual aspect.

The focused interaction case covers four tools/Back focus, six destinations,
skip focus, keyboard/reduced viewport, connection close/reconnect, resource
failure/retry, privacy withdrawal and game/socket/blob cleanup. An observed
fixture Socket constructor is used only to deliver the existing synthetic
server-close event; no normal admission or authoritative position is bypassed.
All fixture and browser limitations must remain explicit in the final report.
The optimized actual application build follows browser closure sequentially,
using retained React/Tailwind configuration and `copyPublicDir:false`; it shares
the same bounded workflow and removes its own temporary output.

The unaccepted `FE26E-WORLD-PRESENTATION-20261104T0429Z` offer was returned
without launch for the time-sensitive IN01 run. A new named sequential slot,
fresh >=8GiB/process guard, one Chrome/harness, <=0.2GiB peak and <=40MiB durable
output are required before runtime work. No install, Cargo, shared build target,
peer interruption or inherited allocation is authorized. Source/no-emit work
continues independently. Root153 remains the sole serialized publisher.
