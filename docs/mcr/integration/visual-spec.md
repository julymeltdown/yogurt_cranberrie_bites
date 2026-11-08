# Visual specification — profile and social surfaces

Status: **INTEGRATED** on `codex/yc-wp10-art`. Presentation only; no behavior,
controller, contract or server authority changed.

This document is the written contract for the visual layer the new
profile/social screens share. It records the token meanings, the primitive
inventory and the per-screen composition rules, so a later screen can be built
on the same system instead of inventing a fourth one.

## 1. Why this exists

The product had real screens but no coherent visual identity. Before this
package, the six profile/social surfaces each carried their own presentation
system:

| Surface | Previous presentation |
| --- | --- |
| `features/favorites` | Local `--pi-*` fallbacks, underline-rail tabs, ad-hoc row borders |
| `features/discovery` | Local `--pi-*` fallbacks, underline-rail tabs, boxed cards |
| `features/profile-media` | A separate `--color-*` palette, 12px card radii, 999px pill buttons |
| `features/profile-room` | Hardcoded hex values, no tokens at all, 4px radii |
| `features/identity` | A separate dark `--mcr-*` palette (`#f4f1e8` on `#201e25`) |
| `features/profile-avatar` | `--pi-*` with literal fallbacks and a duplicated rule block |

Four incompatible palettes, three radius systems and two different tab
treatments across six screens that sit next to each other in the product. This
package replaces all of them with the one existing contract — it does **not**
introduce a second token system.

The reference direction is
[reference-target-2026-11-04.png](../design/reference-target-2026-11-04.png),
which the user designated an exact component/design target. The parts of it this
package implements on the profile/social screens:

- black title bars and black selected tabs with white condensed uppercase text;
- aged-ivory paper surfaces with strong **symmetric** 1–2px rules;
- deep crimson as the single accent, reserved for primary actions;
- dense content rows with a small avatar chip, a name, a metadata line and a
  compact action, rather than spacious boxed cards;
- compact uppercase display typography over readable body text.

## 2. Tokens

All tokens live in `frontend/src/shared/styles/parade-ink/tokens.css` under the
existing `pi01-photo2-v1` contract. The WP-10 extension is a clearly marked
second block in that one file. Every added value either derives from an existing
token or is a scale step; **no new hue is introduced**.

### 2.1 Colour roles (pre-existing)

| Token | Value | Meaning |
| --- | --- | --- |
| `--pi-ink` | `#171717` | Primary text, title bars, selected tab surface |
| `--pi-paper` | `#f9f9f8` | Page and card surface |
| `--pi-surface` | `#f9f8f6` | Secondary surface |
| `--pi-muted` | `#545454` | Metadata and secondary text |
| `--pi-rule` | `#3c3c3b` | Ordinary symmetric control and card border |
| `--pi-soft-rule` | `#777774` | Disabled and dashed borders |
| `--pi-crimson` | `#922532` | The single accent: primary action, focus |
| `--pi-on-dark` | `#ffffff` | Text on ink |
| `--pi-disabled` | `#e3e3e0` | Disabled field |
| `--pi-focus` | `#922532` | Focus ring |

### 2.2 Colour roles (WP-10 extension)

| Token | Value | Meaning |
| --- | --- | --- |
| `--pi-field` | `#efeeea` | Inset field: media tile, avatar frame, quiet panel |
| `--pi-card` | `var(--pi-paper)` | Card surface |
| `--pi-card-rule` | `var(--pi-rule)` | Card border |
| `--pi-divider` | `#d8d7d3` | The rule **between** list rows |
| `--pi-inverse-surface` | `var(--pi-ink)` | Inverse surface |
| `--pi-inverse-rule` | `#4a4a48` | Rule on an inverse surface |
| `--pi-accent` | `var(--pi-crimson)` | Accent alias |
| `--pi-accent-ink` | `var(--pi-on-dark)` | Text on accent |
| `--pi-accent-quiet` | `#f0e2e4` | Accent-tinted field |
| `--pi-selected-surface` | `var(--pi-ink)` | Selected tab / pressed control |
| `--pi-selected-ink` | `var(--pi-on-dark)` | Text on a selected surface |
| `--pi-status-ok` | `#1f5c38` | Success text |
| `--pi-status-ok-field` | `#e6efe8` | Success field |
| `--pi-status-warn` | `#6b4a10` | Warning text |
| `--pi-status-warn-field` | `#f6eeda` | Warning field |
| `--pi-status-danger` | `var(--pi-crimson)` | Error text and border |
| `--pi-status-danger-field` | `var(--pi-accent-quiet)` | Error field |
| `--pi-status-neutral` | `var(--pi-muted)` | Neutral status |
| `--pi-status-neutral-field` | `var(--pi-field)` | Neutral field |

Every status pair (text on its own field) keeps at least 4.5:1 contrast, and the
focus ring keeps at least 3:1 against paper. The status colour is **never** the
only signal: the badge, empty state and inline error all carry a word.

### 2.3 Spacing scale

| Token | Value | Use |
| --- | --- | --- |
| `--pi-space-1` | 4px | Tight inline gap |
| `--pi-space-2` | 8px | Row gap, control gap |
| `--pi-space-3` | 12px | Card padding, screen padding |
| `--pi-space-4` | 16px | Section padding |
| `--pi-space-5` | 24px | Empty-state padding, screen bottom |
| `--pi-space-6` | 32px | Large separation |

The pre-existing `--pi-content-inset` (6px) and `--pi-row-gap` (8px) remain and
are steps of the same scale.

### 2.4 Type scale

| Token | Value | Use |
| --- | --- | --- |
| `--pi-caption-size` / `-leading` | 12 / 16 | Badge, step chip, caption |
| `--pi-meta-size` / `-leading` | 14 / 18 | Metadata line, hint |
| `--pi-body-size` / `-leading` | 18 / 22 | Body copy |
| `--pi-control-size` / `-leading` | 18 / 22 | Button and control label |
| `--pi-heading-size` / `-leading` | 20 / 26 | Section heading |
| `--pi-title-size` / `-leading` | 26 / 32 | Screen title |
| `--pi-display-size` / `-leading` | 32 / 38 | Reserved for a hero title |
| `--pi-weight-body` | 400 | Body |
| `--pi-weight-strong` | 700 | Headings, labels, emphasis |

Display text is `--pi-font-display` (Oswald), body text is `--pi-font-body`
(Roboto). Screen titles, section headings and tab labels are uppercase display;
body and metadata are sentence case.

### 2.5 Geometry

| Token | Value | Meaning |
| --- | --- | --- |
| `--pi-focus-ring` | 3px | Focus outline width |
| `--pi-focus-offset` | 2px | Focus outline offset |
| `--pi-card-padding` | `var(--pi-space-3)` | Card inset |
| `--pi-card-gap` | `var(--pi-space-2)` | Card internal gap |
| `--pi-chip-size` | 40px | Small avatar chip frame |
| `--pi-chip-size-lg` | 56px | Large avatar chip frame |
| `--pi-media-radius` | `var(--pi-radius)` | Media tile radius |
| `--pi-badge-height` | 22px | Badge minimum height |
| `--pi-section-gap` | `var(--pi-space-5)` | Gap between screen sections |

Pre-existing `--pi-hit` (44px), `--pi-radius` (2px), `--pi-rule-width` (1px),
`--pi-frame-border` (2px) and `--pi-tab-height` (44px) are unchanged and remain
the interaction and frame contract.

## 3. Primitive inventory

CSS lives in `frontend/src/shared/styles/parade-ink/profile-social.css`.
React components live in `frontend/src/shared/ui/profile-social.tsx` and are
exported from `frontend/src/shared/ui/index.ts` in the existing barrel style.

| Primitive | Class | React | Purpose |
| --- | --- | --- | --- |
| Section header | `.pi-section__*` | `SectionHeader`, `Section` | Screen/card section title with count, description and trailing action |
| Card | `.pi-card*` | `Card` | A distinct object with its own field and symmetric border |
| List | `.pi-list` | `List` | Dense list container with divider-separated rows |
| Row | `.pi-row*` | `Row` | Media + title + metadata + actions, in one dense line |
| Row link | `.pi-row-link` | `RowLink` | A row whose whole surface is one target |
| Media tile | `.pi-media-tile*` | `MediaTile` | Square/4:3 media cell that reserves its ratio |
| Media grid | `.pi-media-grid` | `MediaGrid` | Labelled responsive media grid |
| Avatar chip | `.pi-avatar-chip*` | `AvatarChip` | Small identity mark: frame + name + metadata |
| Badge | `.pi-badge*` | `Badge` | Short status word with a semantic tone |
| Empty state | `.pi-empty*` | `EmptyState` | What is missing and what to do next |
| Inline error | `.pi-inline-error*` | `InlineError` | A refusal or failure attached to a region |
| Screen frame | `.pi-screen*` | `Screen` | Bounded column, one `h1`, consistent rhythm |
| Tab strip | `.pi-tabs*` | `TabStrip` | Compact equal-cell tab row with a black selected state |

Each primitive is presentation only: it owns no data, starts no request, and
keeps whatever accessible role its content needs. `SectionHeader` takes a
`level` so the document outline stays correct without a visual change.

## 4. Composition rules

These are the rules a new profile/social screen follows. They are what makes the
six screens read as one product.

### 4.1 Frame

- Use one `.pi-screen` (or the feature's equivalent root) per screen: bounded to
  42rem, centred, `--pi-space-3` padding, `--pi-space-5` bottom (at least the
  safe-area inset).
- Exactly **one** `h1` per screen, uppercase display at `--pi-title-size`.
- Sections are separated by `--pi-section-gap`. Do not add redundant
  introductory paragraphs before a list; the first useful row appears promptly.

### 4.2 Lists versus cards

- A **list** of similar things is `.pi-list` + `.pi-row`. Rows are separated by
  a single `--pi-divider` rule and carry **no** per-row border box and **no**
  per-row colour.
- A **card** is for a genuinely distinct object (a room, a claim block, a
  composer fieldset), never as a wrapper around each item of a list. This is
  what prevents the "repetitive accented cards" failure mode.

### 4.3 Tabs

- Tab rows are `.pi-tabs` (or the feature's tab classes styled identically):
  equal cells, `--pi-tab-height`, symmetric rule, and the **selected** tab is
  black surface with white text.
- There is no underline rail and no coloured bottom edge. Selection is a
  surface change, not an edge change.

### 4.4 Status, errors and empty states

- A status word is a `.pi-badge` whose tone maps to a status token. The word
  always carries the meaning; the tone is redundant.
- A refusal or failure attached to a region is `.pi-inline-error`: a full
  symmetric danger border, a tinted field, an uppercase title and an optional
  action. It is `role="alert"` by default.
- Transient messages stay in the existing `ToastRegion`, not in an inline error.
- An absence is `.pi-empty`: a dashed symmetric border, a title and one line of
  guidance. It never implies a visibility restriction it cannot prove.

### 4.5 Media and identity

- Media uses `.pi-media-tile`, which reserves its aspect ratio so a grid never
  jumps. A missing or failed image shows an explicit text state; it never
  substitutes another image.
- Identity uses `.pi-avatar-chip`. When no character is available the chip shows
  the initial inside an `aria-hidden` frame; it never shows substitute character
  art. This preserves the character-source rule: only the user-selected pack and
  its reviewed derivatives may render as a character.

### 4.6 Controls and focus

- Controls are `--pi-hit` (44px) minimum, with a symmetric `--pi-rule-width`
  border and `--pi-radius`.
- Focus is a `--pi-focus-ring` outline at `--pi-focus-offset`, on every
  interactive element, in every restyled screen. It is never removed.
- Disabled controls use `--pi-disabled` with `--pi-muted` text.

## 5. Per-screen application

| Screen | Root | Applied |
| --- | --- | --- |
| Favorites | `.favorites-picker` | Screen frame, uppercase title, shared tab strip, dense result rows with dividers, status summary, inline error |
| Discovery | `.discovery-surface` | Screen frame, shared tab strip, category chips, dense people rows with avatar chip frame |
| Profile media | `.profile-media-composer`, `.profile-media-gallery` | Screen frame, section-style fieldsets, symmetric buttons, ratio-preserving tiles, explicit empty/unavailable states |
| Profile room | `.room-screen` | Screen frame, symmetric controls, dense placed-item list, conflict as an inline error, import steps as chips. **The Phaser canvas (`.room-stage`) keeps its own dark field and pixelated scaling** — it is a rendered scene, not chrome |
| Identity | `.identity-surface` | Screen frame, status as a badge mapped to status tokens, claims as a divided definition list, report/message as a symmetric panel |
| Profile avatar | `.mcr-profile-avatar` | Screen frame, single action-row rule set, shared control and selected states, canvas preview keeps `image-rendering: pixelated` |

## 6. Prohibitions, and how they are enforced

The product rule forbids decorative left accent lines, coloured left borders,
inset left rails and pseudo-element left bars in product UI, notices, error
panels, toasts, skeletons and documentation-style panels.

`frontend/src/shared/ui/__tests__/profile-social-primitives.test.tsx` reads the
**actual stylesheets** of every surface above and fails if any of them declares:

- `border-left`, `border-inline-start`, or their `-width`/`-style`/`-color`
  longhands;
- a `::before` or `::after` pseudo-element;
- an inset left `box-shadow` rail or a left-positioned absolute bar;
- a raw colour literal or a `rgb()`/`hsl()` value (a second palette);
- a `border-radius` outside the shared radius, the deliberate badge pill, or 0.

It also asserts that every restyled surface keeps its `:focus-visible` rule, that
the shared controls keep a symmetric border, and that status/error panels carry
their state on the whole border and in text rather than on one edge.

The pre-existing markup guard in
`frontend/src/features/identity/__tests__/IdentityStatus.test.tsx` is unchanged
and still passes.

## 7. Verification

| Command | Exit | Result |
| --- | --- | --- |
| `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** | — |
| `cd frontend && corepack pnpm@11.7.0 exec vitest run src/features/favorites src/features/discovery src/features/profile-media src/features/profile-room src/features/identity src/features/profile-avatar src/shared` | **1** | 728 passed, 2 failed, 14 skipped |
| `cd frontend && corepack pnpm@11.7.0 run build` | **0** | — |
| `cd frontend && corepack pnpm@11.7.0 run lint` | **0** | 48 warnings, 0 errors |

The 2 failures are pre-existing and unrelated to this package. They are the same
2 that fail at the base commit `02c0b3216` before any WP-10 change:

- `src/shared/api/__tests__/c07-social-client/compatibility.test.ts`
- `src/shared/api/paradise-city-v1/__tests__/location-registration.test.ts`

The scoped run went from 700 passed / 2 failed at baseline to 728 passed / 2
failed: the 28 new cases are the primitive and prohibition guard tests.

## 8. Limitations

- This is a source-level visual layer. No browser captures were taken for this
  package; the existing native-capture harnesses belong to their own feature
  owners. Visual acceptance against the reference remains open.
- The concert screens were deliberately not restyled. This package is scoped to
  the new profile/social surfaces.
- The primitives are adopted by these six screens. Other profile/social screens
  added later should adopt them rather than adding local CSS.
