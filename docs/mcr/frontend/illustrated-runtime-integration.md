# Illustrated runtime integration — 2026-11-04

The existing Phaser runtime can now consume the original illustrated scenery
through the optional C06 binding, retain player-foot depth, replace eras
atomically and dispose owned textures and decoded images. This is consumer
implementation, not a production Bootstrap/avatar release or a claim that all
ten reference screens are visually complete.

## Ancestry and scope

- Integration base: `800b969a2d92990593d77731d0e71a0fdd357668`.
- Author: `2afb527a03b13411f92fa8d0b0db636a868d45ea`, parent
  `595f0dc9e2ceed9bdb6ee28a7de670d8ea781eba`.
- Delivered consumer: `3eefd4ef`. Only this author commit was cherry-picked.
  Contract `d682bb4` was already delivered as `0fd471d`; `595f0dc` is the local
  copy of Main's `d434630` generated-schema/semantic dependency and was not
  integrated again.
- All 91 changed paths match their exact grants and author bytes. Existing-file
  preimages match, with no new-path collisions or cross-lane implementation
  imports. All 105 delivery pins and 79 immutable-input pins match.
- Character originals, scenery originals, map geometry, `docs/source/`, backend,
  root routes and lockfiles are unchanged by the consumer patch. The author
  handed off a clean stopped writer and reported its actual goal complete.

## Qualification

Main's fresh combined checks on `3eefd4ef` passed: full strict frontend TypeScript
(exit 0, 4.88 seconds), 649 tests in 23 affected world/resolver/default-shell files
(exit 0, 5.33 seconds) and whitespace checks (exit 0). The installed dependency
link was temporary and removed after the checks. No dependencies were installed.
The first shell attempt to check `HEAD^` was rejected by zsh glob expansion;
the corrected argument-based invocation passed without changing source.

The author's source-bound evidence remains valid: 51 native captures across
360/390/430 widths; all 45 era/landmark views match an independently assembled
composition of the bound scenery with zero channel differences. Six captures
show an explicitly TEST ONLY depth marker, with 81 pixels visible behind a tree
and 392 in front. Those exact comparisons do not measure similarity to the user's
collage. Native cancellation, rejection, atomic replacement and disposal checks,
types, lint, build and deterministic exports passed within the recorded limits.

Main inspected recorded 390px scenery, compact shell and Customize captures.
The first shows actual illustrated terrain; the others still use synthetic
resources. No new live-browser, approved-avatar, physical-device or multiplayer
test was run by Main. Production release binding and full visual acceptance
remain open. The peer/author handoff was used without another general review.

The machine-readable [integration receipt](../evidence/illustrated-runtime-integration-2026-11-04.json)
records exact paths, hashes and commands. The [author handoff](illustrated-world-runtime.md)
contains detailed resource, memory and lifecycle boundaries. The
[UI reference audit](../design/ui-ux-reference-review.md) retains the screen-level
remaining work.
