# FE12/FE13 reviewed body palette

Status: MOCK_READY for source qualification. Source tests, application types and
lint pass at `3e299cd76ae55380124b7e29e120ea48370291e1`. Actual reviewed
Root-App integration/native qualification remains NOT_RUN for this increment. The aligned original AR03 goal was
freshly read as BLOCKED and unfinished; it is reused without reset or a scope cut.

## Ownership and source base

Dedicated checkout: `/Users/lhs/Desktop/mcr-worktrees/fe-ref-map`, branch
`codex/mcr-fe-FE12-part-controls`, base
`654974128a27c948a449fa9e58f466b8e612361d`. The prior production-adapter branch
ending at `ba426e06` remains intact. New source writes are confined to
`frontend/src/features/avatar/part-controls/**` and
`frontend/src/features/avatar/wardrobe-options/**`, this document and
`docs/mcr/evidence/avatar-part-controls/**`.

Root153 confirmed that the current MotionEditor already supplies preset/category
selection, all reviewed items, nullable wardrobe slots, Random/Reset/Save and
pose preview. Recreating those controls would duplicate delivered behavior.
The concrete missing reference feature is a compact BODY/SKIN color palette.
This increment supplies only that view and its necessary pure option adapter.
Root retains the existing MotionEditor import/BODY render hunk until its shared
adoption source qualification finishes. No existing editor, screen CSS, shared
runtime/registry/authority, public contract, asset pack or supplied pixel changes.

## Exact consumer contract

Import `MotionBodyPalette` from
`features/avatar/part-controls/MotionBodyPalette` and render
`<MotionBodyPalette controller={owner.controller} />` in the existing BODY
category. The generic prop is the same `MotionEditorController<C>` already
borrowed from the Account-owned reviewed production editor. The leaf subscribes
to its state only. It never starts/stops/disposes a controller, creates a runtime,
loads art, creates an Auth/cache/player/socket, or calls Save/Reset/Random.
The existing owner continues to enforce lifecycle, persistence, capabilities,
conflict/uncertainty reconciliation and server receipt rules.

`motionBodyOptions(items)` in `wardrobe-options/body-options.ts` preserves each
catalog body key in its verified order and derives an exact representative RGB
triplet. `motionBodySelection(selection, key, options)` replaces only `body` in
an existing wardrobe selection and preserves the other five fields. A preset
never becomes a fabricated wardrobe. Every click reads fresh controller/catalog
state and passes the next selection to the existing `controller.choose`.

The view shows selected chips with named buttons and pressed state. Every chip
has a minimum 44×44 target, visible keyboard focus and a symmetric control border.
Four columns retain all eight choices in two rows at narrow widths. This is a
truthful accessible palette for eight reviewed tones, not a claim of exact parity
with the reference's seven smaller one-row chips. The original preview rectangle
and source pixels are untouched. Existing Save/Reset/Random remain in Root's
editor; local tone changes do not issue a server mutation by themselves.

## Qualified data and unavailable states

The actual reviewed manifest is `mcr_selected_motion_reviewed_20261104` revision2,
canonical hash
`sha256:7f2891c81ffa1d5850107118898a7ef9377b222ab8639b3f2c9daf7908cf9018`.
Its eight body items and the pinned art descriptor each contain four RGB triplets
per body. Reviewed `composition.hand_palette_color_index` is2; the unchanged
compositor uses that same exact index for body/hand color. Chips read this triplet
directly from `MotionItem.skinPalette.colors[2]`. No shade is averaged, guessed
from an ID or sampled from a screenshot, and no sprite is recolored by this leaf.
The descriptor and manifest palettes were checked directly before authoring.

Missing, malformed, non-integer, out-of-range or incomplete palette metadata
keeps the real body key but renders its option unavailable without a fallback
color. Duplicate/empty keys fail closed. A missing body collection or withdrawn
runtime shows an unavailable message. Preset mode keeps chips disabled and
requires the existing Dress up action; busy controller phases disable selection.
Draft selection follows existing owner behavior while Save is unavailable or
reconciliation is pending; this leaf never bypasses mutation gating.

Separate trousers, independently dyed hair, new accessory subslots, arbitrary
RGB persistence and new wardrobe combinations are outside this increment and
are not fabricated. The existing reviewed catalog/slot vocabulary is unchanged.

## Validation and integration boundaries

New tests cover exact eight reviewed RGB triplets, metadata negatives, all-key
body changes, preservation of every other coordinate, actual canonical
Auth/privacy/HTTP owner integration, pressed/disabled states, no automatic save,
no lifecycle ownership, stale-click races, preset non-conversion, capability and
conflict gating, and synchronous private withdrawal. Image decode is mocked in
component tests; native and pixel proof cannot be inferred from unit results.

Resource allocation is required before executable checks. The completed source
round used retained Node24.22.2 and dependencies read-only, runner config loading,
private TMP/cache, one Node child at a time, 2.5GiB aggregate RSS and 128MiB whole
charged output/carry, 16GiB start /8GiB stop floors, at most600seconds including
60seconds cleanup. Source work does not reserve that shared execution slot.

Native qualification must use the actual reviewed default App with Root's shared
character qualification at360/390/430. No standalone palette browser matrix will
be presented as integration. Original full FE12/FE13/AR03 acceptance remains open
until the actual route, self/peer/Profile behavior and server persistence pass.
Root153 publishes the committed scoped handoff through normal serial publication;
no king/SL review or acknowledgment is a prerequisite.

## Delivered source evidence

Source base: `654974128a27c948a449fa9e58f466b8e612361d`. Ordered commits:
`bfaf7318869971564be3de671e18898c82a67bd4` adds the isolated palette and tests;
`6a3976d7937cda99d1798609c7d6584deacce1b3` freezes process/Git accounting and
runner configuration; `3e299cd76ae55380124b7e29e120ea48370291e1` caches the
negative-fixture external-store snapshot and narrows its public resource path.
No product implementation changed after the first source commit.

Allocation `FE12-BODY-PALETTE-SOURCE-B-202611041355Z` retained its original
13:57:24.883099Z admission and14:07:24.883099Z deadline. Full release was measured
at14:00:58.186306Z (23:00:58 KST, November4). One initial negative-fixture test
failed because its read function manufactured a fresh snapshot per call; the
failure log and checks receipt remain preserved. The snapshot was cached, the
fixture path comparison was explicitly narrowed, and the full source round passed:

- Retained Node24.22.2: exit0.
- Vitest runner loader, one worker:384 tests /18 files, exit0.
- Full application TypeScript:886 files /0 diagnostics, exit0.
- Scoped Oxlint:0 warnings/errors, exit0.
- `git diff --check 654974128a27c948a449fa9e58f466b8e612361d`: exit0.
- `python3 scripts/mcr/check-plan.py`: exit0.

See `docs/mcr/evidence/avatar-part-controls/checks-*.json` for absolute argv,
source SHA256 pins, each attempt and exit codes. `unit-attempt1.log` preserves
the failure; `unit.log` and other named logs contain the passing run. A mistyped
source hash in one retry command was rejected by the supervisor before admission
or child creation; the next command read the actual committed HEAD.

Measured peaks were3 workflow processes,1,535,049,728B aggregate RSS and
1,955,472B whole charged carry. Full release retained1,957,461B, including
1,762,221B conservative source-Git accounting; free disk was29,587,091,456B.
Owned validation processes, temporary/cache paths, local dependency links,
borrowed `.vite-temp` and evidence handles were all zero. No dependency install,
browser, native capture, server, build or database ran. Documentation receipts
are added after resource release and remain below the same128MiB carry cap.

These are deterministic transport/component checks with mocked image decode.
Root must wire the exact export in its existing BODY hunk and include it in the
one actual default-App360/390/430 reviewed all-layers/shared-player journey.
No screenshot, actual server persistence, live peer delivery or pixel fidelity is
certified here. The original BLOCKED goal remains unfinished; this source handoff
does not reset, complete or narrow its full product acceptance requirements.
