# WP-09 — Three-section navigation and profile screens

Status: **INTEGRATED**. Tasks T-901 … T-908 of
[task-breakdown.md §14](../integration/task-breakdown.md#14-wp-09--navigation-and-screens)
are implemented, tested, and committed on `codex/yc-wp09-nav` (base `02c0b3216`).

The concert-first tab bar is replaced by three primary sections — **Discover**,
**Activity**, **My Profile** — and the concert screens move to a secondary menu
(product-plan.md §7, charter decision D-10). No concert URL moves: the screens
keep their paths and leave the *navigation*.

## 1. What was built

| Task | Deliverable | Path |
| --- | --- | --- |
| T-901 | The section contract and the redirect table | `frontend/src/app/sections.ts` |
| T-901 | The tab bar and the concert secondary menu | `frontend/src/app/section-navigation.tsx`, `.css` |
| T-901 | Section routes in the panel table | `frontend/src/app/routes.ts` |
| T-902 | The owner profile page | `frontend/src/features/profile/ProfilePage.tsx`, `frontend/src/app/section-entry/MyProfilePanel.tsx` |
| T-903 | The visitor profile page | `frontend/src/features/profile/PublicProfilePage.tsx`, `frontend/src/app/section-entry/VisitorProfilePanel.tsx` |
| T-904 | Router registration, lazy loading, redirects, not-found | `frontend/src/app/router.tsx` |
| T-905 | Shell and tab bar | `frontend/src/app/WorldShell.tsx`, `frontend/src/app/shell-chrome/index.tsx` |
| T-906 | Identity settings screen | `frontend/src/features/identity/IdentitySettings.tsx`, `frontend/src/app/section-entry/IdentitySettingsPanel.tsx` |
| T-907 | Accessibility and responsive pass | `frontend/src/app/__tests__/accessibility.test.tsx` |
| T-908 | Package verification | `docs/mcr/integration/evidence/ui-wp09-navigation.json` |

The section screens themselves live in `frontend/src/app/section-entry/`:
`DiscoverPanel`, `ActivityPanel`, `PhotoComposerPanel`, `IntroSlot`.

## 2. Hard constraints and the test that proves each

| Constraint | Test |
| --- | --- |
| Every old path still resolves (directly or by redirect) | `navigation.test.tsx` — the six old tab paths are live routes, the three changed paths redirect, the query string survives, and every section route resolves to a registered panel |
| Primary navigation has exactly three entries | `navigation.test.tsx::renders_exactly_three_links_in_the_tab_bar…`, `shell-sections.test.tsx::renders_three_labelled_links_and_no_concert_tab` |
| No age rendered for any visitor state | `PublicProfilePage.test.tsx` — four states walked for age-shaped text, plus a payload that carries an age is refused by the decoder |
| Shared interests omitted when empty | `PublicProfilePage.test.tsx` — no heading, no filler for an empty or blank list |
| Room opens only from its entry point; no room preview inline | `ProfilePage.test.tsx::offers_the_room_only_through_its_entry_with_no_inline_preview`, `PublicProfilePage.test.tsx::offers_no_room_preview_inline` |
| A not-found route renders no profile or post data | `router.test.tsx` — no field, no id, no user-entered segment in the not-found output |
| Keyboard reachable, every control has an accessible name, no keyboard trap | `accessibility.test.tsx` — 8 routes and 3 component variants checked for names; no `aria-modal`, no `inert` body, focus can leave the shell |
| NO decorative left accent bars anywhere | `shell-sections.test.tsx` — every CSS file under `src/app` and `src/shared/styles` is read for `border-left`, left pseudo-element bars, and the rendered markup is walked for an inline one |

## 3. Verification commands and exit codes

| # | Command | Exit |
| --- | --- | --- |
| 1 | `cd frontend && corepack pnpm@11.7.0 run typecheck` | **0** |
| 2 | `cd frontend && corepack pnpm@11.7.0 exec vitest run src/app src/features/profile` | **1** — 1617 passed, 1 failed |
| 3 | `cd frontend && corepack pnpm@11.7.0 run build` | **0** |

Command 2's single failure is
`src/app/world-entry/__tests__/map-presentation-v1/input-readiness.test.ts::pins_the_frozen_public_capability_schema…`.
It fails identically on the untouched base commit `02c0b3216` (verified by
stashing this branch's changes and re-running): it compares a frozen contract
fixture against the current `contracts/core/v1/core.schema.json`, which drifted
before WP-09. It is not a WP-09 regression and is not fixed here because the
fixture belongs to another package.

The new WP-09 suites, run individually, all exit 0:

| Suite | Tests |
| --- | --- |
| `src/app/__tests__/navigation.test.tsx` | 19 |
| `src/app/__tests__/router.test.tsx` | 19 |
| `src/app/__tests__/shell-sections.test.tsx` | 12 |
| `src/app/__tests__/accessibility.test.tsx` | 26 |
| `src/features/profile/__tests__/ProfilePage.test.tsx` | 10 |
| `src/features/profile/__tests__/PublicProfilePage.test.tsx` | 17 |
| `src/features/identity/__tests__/IdentitySettings.test.tsx` | 9 |

## 4. Design notes

### The redirect table is data

`legacyRedirects` is a closed list, and `legacyRedirect` matches on whole
segments. A test iterates the table, so a path that is neither a live route nor a
redirect fails in CI rather than in a user's browser. A redirect is registered as
a real route *before* the panel routes it would otherwise match, so
`/meet/people` lands on `/discover/people` rather than rendering the concert
People panel, and a legacy path is not also registered as a panel (the ranking
would otherwise depend on which pattern React Router scores higher).

### Three sections, one active decision

`sectionForPath` matches whole segments. `/notifications` activates Activity
because the inbox is where likes, comments, replies and follows live;
`/notifications-settings` does not, because it is not a section path. A concert
screen leaves every section inactive, which is the honest state: the user is not
in a section.

### No age has no path to the DOM

The visitor view model has no `age` field. `decodeVisitorProfile` is a closed
schema, so a payload that carries one throws. `admitGenderBucket` accepts only
the three closed buckets. The test walks the rendered text for four age-shaped
patterns in four states. There is no place for an age to enter.

### The room is a link, never a preview

Both profile pages render exactly one `RoomEntry` and no scene, canvas,
thumbnail, poster or layout data. A test counts the room links (exactly one) and
asserts no room element exists.

### No left accent bar

The user's prohibition is explicit and absolute. The test reads every stylesheet
the shell and the sections ship — `src/app/**/*.css` and
`src/shared/styles/**/*.css` — and fails on a `border-left` that names a colour or
a width, or a left pseudo-element bar. Separation is done with spacing, heading
weight and the existing rule colour.

## 5. Deviations from the task document

1. **`/me` is the My Profile section home, not the concert MeHub.** The section
   home is the owner profile page. The concert MeHub moved to `/me/profile`,
   which the page links to as "Edit profile". This is what "My Profile" means in
   the product plan; the concert account panel keeps its URL and its behaviour.
2. **`/discover/people/:profileId` mounts the concert `PublicProfile`.** The
   concert profile owns the sharing and block flows the product still needs, and
   the profile-service read that would fill the WP-09 visitor view model is not
   wired yet. The WP-09 visitor page (`PublicProfilePage`) is implemented,
   decoded and fully tested against its view model; switching the route to it is
   the integration step once that read exists. This is recorded in the evidence
   `limitations`.
3. **The mutuals-intro screens are a reserved slot.** `features/intros` is being
   written in another worktree, so this package does not import it. `/discover/intro`
   and `/discover/intro/:introId` are registered and render an explicit
   placeholder that says the screens are not part of this build yet — never an
   empty list, which would read as "nobody posted".
4. **`socialRoute` recognises the Discover paths.** The social owner's route
   parser is a concert-path table; the Discover paths are primary-section routes
   and are not in the shell's return-target table, so they are matched directly
   before it. The legacy `/people/{id}` spelling is still recognised so an
   in-flight share is not cancelled by a redirect.
5. **`panelKind` and the reference-flow list.** The panel's `data-mcr-panel`
   attribute names the section first, then the concert screens that still have a
   distinct presentation. `/me/profile` joins the reference-flow list so the
   concert account panel keeps its existing chrome.
6. **Existing tests updated, not deleted.** The shell, chrome, routes,
   default-shell, world-presentation, account-entry, reference-chrome and
   social-composition suites asserted six tabs or the old people paths. Each was
   updated to the new structure with a comment naming the WP-09 change; no
   assertion was removed without a replacement.

## 6. Not done here

- UI-360-390-430 pixel captures. The layout contract is verified in the
  stylesheets; a real capture at the three widths needs a browser run.
- The shell wiring that fills the visitor profile view model from a server read
  (see deviation 2).
- The intro composer and intro detail screens (WP-05).
