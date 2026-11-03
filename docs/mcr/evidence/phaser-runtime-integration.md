# FE14 runtime integration review

Date: 2026-11-03. Status: **MOCK_READY** for the reviewed runtime foundation.
Whole FE14, production C06 resources, protected-world integration and IN01 remain
unaccepted. Required-task acceptance remains 11/112; total acceptance is 11/118.

## Candidate and ownership

The worker started at `244d35a85dd884655e270f7c6af6a90657fabfbd` on its isolated
`codex/mcr-fe-FE14` checkout. Its four commits were integrated serially:

| Worker | Integrated |
| --- | --- |
| `a7dcb792d7804c1328f53dcbb586b58cd9a8f0bc` | `1a65e5b` |
| `a1243ffd5cfabd4b194868251562efec7403a6c7` | `9b4e9c8` |
| `1ffff588d44ce7a7aa82a574aaae163cbcd1248c` | `26f5e4f` |
| `9b9307c922facb7c8786595b2d0a78c6086e084a` | `77ae6bdf0c8b47d9468727befcea75a9af64234c` |

All fifteen assigned files match the final worker bytes. The patch contains the
runtime owner, map/resource/schema consumers, Phaser scene, feature tests and
synthetic fixtures, and one frontend document. It changes no backend, central
contract, dependency, application router, network implementation or character
art. The existing central C06 format correction is present on the parent branch.

The independent QA report commits `6480707` and `5bf0708` were integrated as
`d09a1d5` and `b1a19c4`; each changes only the report. The final report is
[phaser-runtime-review.md](../qa/phaser-runtime-review.md).

## Review findings and correction

The initial `26f5e4f` candidate passed its suites but source review and independent
probes found three P2 defects. The original [review record](phaser-runtime/initial-review.json)
is retained alongside the before/after probes.

- R1: cancelling an engine-less replacement released the host while an earlier
  engine's close promise remained pending. Cleanup now retains the claim through
  inherited teardown; stop/dispose/abort all block a competing load or engine.
- R2: WHATWG IPv6 parsing and uppercase IPvFuture acceptance diverged from the
  pinned Python checker. A bounded independent parser now matches those reviewed
  literals, including the checker's documented compatibility quirks.
- R3: a synchronous visibility callback could stop or dispose the owner before a
  stale ready publication. A generation check now fences that callback boundary.

Independent QA cleared all three on exact worker `9b9307c`: 134 targeted cases,
the same before/after lifecycle probes, seven fully rehashed manifest URI cases
and a 655-case scalar IP-literal matrix. Its finite matrix is not exhaustive RFC
certification. Its report distinguishes rerun tests from inspected worker logs.

## Parent verification

Parent reran the combined checks at exact implementation candidate `77ae6bd`:

| Check | Result |
| --- | --- |
| Frontend Vitest | 871/871 PASS, 21 files |
| Strict TypeScript/Vite build | PASS; existing 544.82 kB entry-chunk warning retained |
| ESLint | PASS |
| Real Chrome 156.0.8078.80 | PASS: four widths, reduced viewport, resource failures, lifecycle and review regressions |
| C06 golden validation, C03 generated-source check, C08 origin gate | PASS |
| Exact frontend/doc boundary and whitespace | PASS |

The browser runner verifies the actual installed Phaser 3.90 renderer, owned
listener/RAF/canvas/blob cleanup, pixel colors/layers, late work, hidden teardown,
and production public-entry isolation. No unexpected page error occurred; the
single deliberately injected `FE14_SYNC_START_THROW` is separately recorded.
The native inherited-close case deliberately holds a lease's close contract
after native destruction; it does not claim the destroyed native engine lives on.

The parent visually inspected the stored 390px synthetic map, invalid-map state
and public privacy page. Text and controls are legible, the map/error distinction
is visible, and focus outlines are symmetric. No decorative left accent or
character artwork appears. These are controlled screenshots, not device or
production-world acceptance.

Exact commands, exit codes, source hashes and artifact hashes are in
[result.json](phaser-runtime/result.json). Logs, native reports, three screenshots,
QA probes and before/after results are stored beside it. All owned browser and
server processes exited after the runner. A preliminary parent harness setup
used the wrong receipt key and exited before any test; the corrected invocation
generated the recorded nine passing commands. The archived raw Vite log contains
a trailing space and the Vitest log an extra EOF newline. Their original bytes
and hashes are preserved; the publication whitespace check excludes exactly
those two raw logs and passes for code and documentation. See
[publication-checks.json](phaser-runtime/publication-checks.json).

## Completion and remaining work

Both bounded FE14 implementation and independent review goals were marked
COMPLETE only after final correction and parent disposition; subsequent goal
reads confirmed COMPLETE. Their old checkouts are stopped. The frontend chat is
reused for FE15 on a fresh checkout, while QA reviews the separate Auth client.

The normalized Tiled subset, pinned date-time checker's leap-second limitation,
synthetic-only resources and input-free scene are documented in
[the implementation handoff](../frontend/phaser-runtime.md). Production map and
user-art derivatives, avatar composition, protected-shell wiring, movement,
replication, authenticated world admission, real multiuser/device/load evidence
and full release gates remain required. This checkpoint enables the next
movement implementation; it does not waive those requirements.
