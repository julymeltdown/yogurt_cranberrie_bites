# Feed CSS foreground parity

Task: CSS-FEED, related F01/FE27 and shared F03/FE28 styling. Bounded CSS status:
MOCK_READY. Full foreground/photo parity: IN_PROGRESS with exact constraints below.
Owner: `01a1a5d9-332b-7822-a8f7-412e295fa211`. Persistent goal: ACTIVE.
Delivery goes directly to Root153 (`01a1a153-1227-7e72-801f-a2a6b91981e1`),
who integrates and publishes. No king/SL technical review stage applies.

## Scope and provenance

The isolated checkout is `/Users/lhs/.codex/worktrees/d51f/mcr`, branch
`codex/mcr-fe-css-parity-feed`, fast-forwarded before source edits to
`54320d71c4ae0bff3524d1b500cea772e0ba42e0` from `origin/master`.
Root153 explicitly granted exclusive ownership after the former community
art writer stopped. Product changes are confined to
`frontend/src/features/event-feed/feed.css`. Documentation and evidence stay in
this file and `docs/mcr/evidence/css-parity-feed/`.

The new human reference is the supplied `1-Photo-1.jpg`, SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
The Feed panel occupies rectangle `(0,446)-(255,853)` in that 1280×853 photo.
The evidence crop is normalized to 390×623 for inspection; it is an evidence
derivative, never application content. The original is untouched.

No external logic, dependency, contract, asset, or rendering code is imported.
Existing C04 Feed data, authorization and route callbacks remain unchanged.
There are no database migrations. Retained dependency lock hash matches the
read-only delivery tree: `47ca44459819ad0e887d2f4f93394fdf3b1053b8f45edc9ceb0236981feee1f6`.

## CSS intent

The reference has a compact four-tab strip, bold author/time hierarchy,
compact post text, narrow media strips aligned with the text, a single action
row and a centered compose control. The final pass adjusts tab proportions,
17px condensed bold post text, 16px card inset, media-column alignment, 2.85:1
media strip geometry, action text/alignment and a 216px maximum compose width.
Every new selector is scoped to `.feed-browse`; generic Thread/composer rules
and shared post-content selectors remain unchanged.

Selected-character CSS, 48px portrait boxes and canvas/image rendering remain
byte-identical. Decorative page/card/footer backgrounds, shared styles,
shell/chrome, product TSX/JS, assets, manifests and lockfiles are unchanged.
No generated text, fake controls, identities, posts, counts, or media are added.
No decorative left rails are introduced. Existing 44px targets and focus remain
required in the browser checks.

## Validation and iteration

The named `CSS-PARITY-FEED-202611040938Z` round was offered and explicitly accepted
after an own fresh 16.114 GiB free-space/port/runtime/lock preflight. Source
checkpoint `b16133bd1285dd7ba4fef812bee6c4517cf4fc24` preserved the first pass and
driver. The first driver attempt failed because dynamic import of Playwright's
CJS entry did not expose `chromium`; Chrome never launched. That report and exit
1 are preserved in `failed-before-1/`, `failed-before-1.log` and
`supervisor-attempt1.json`. The evidence-only driver now uses its CJS require
entry. Repairs retained the original resource deadline.

The evidence-only [driver](../evidence/css-parity-feed/native.mjs) reuses the existing EventFeed reference fixture
for two explicit synthetic media rows and its actual controller/privacy behavior.
It compares the saved baseline CSS at 390px with the changed CSS, then checks
360/390/430 geometry, tabs/keyboard, notice source, thread callbacks, compose,
options/media filtering, refresh, privacy revocation, empty, read-only,
unavailable, pending and error states. It also consumes the existing cozy
default-root fixture with canonical Auth, router, selected-character renderer
and Phaser over synthetic HTTP/C05. That fixture is not live admission evidence.
The script writes only owned evidence and private temporary caches.

The [comparison index](../evidence/css-parity-feed/comparison.html) shows the
human crop, fresh baseline, first and final iterations, and current default app.
The baseline and both candidate rounds passed their relevant assertions, exit 0.
First iteration copy used 16px Courier New. Actual 390px comparison showed
serifs/heavy glyphs and extra truncation; the second source iteration changed it
to 17px bold condensed sans-serif and reran the same browser checks at all three
widths. The final CSS SHA-256 is
`60a55f5e4f3493dfe2ffad49f26772e8703e5ea21d49c5f7036ace950edba178`;
the final browser report and [input pins](../evidence/css-parity-feed/final-input-pins.json)
bind the tested source and existing fixtures. No source CSS changed afterward.

At 390px, the first iteration placed the first card at `(14,108)`, width 362px,
height 238.70px, with media at `(99,210.48)`, 260×91.22px. These align closely
with the normalized reference's first-card location and roughly 90px media strip.
The final typography revision retains the layout structure. Both candidate
rounds record all exact dimensions; the photo remains an image comparison, not a
claim that different text/media populations are pixel-identical.

| Check | Result |
| --- | --- |
| Native baseline at 390px | PASS, exit 0 |
| Iteration 1 and final iteration at 360/390/430px | PASS, exit 0 each |
| All visible Feed controls >=44px; document width equals viewport | PASS in every candidate case |
| Keyboard tab navigation, notices/source, all three post actions, compose, options/filter, refresh | PASS |
| Normal and forced-colors visible keyboard focus | PASS; final captures retained |
| Empty/read-only/unavailable/pending/error and privacy media/blob revocation | PASS |
| Actual default cozy root, selected-character canvas, compose route and textarea | PASS at 390px |
| Default root single Phaser canvas/socket and zero after disposal | PASS |
| Existing Feed/FeedThread controller/presentation/UI regressions | PASS, 37 tests in 4 files, exit 0 |
| `git diff --check` | PASS, exit 0 |
| `python3 scripts/mcr/check-plan.py` | PASS, exit 0; planning only |
| Background and portrait declaration comparison against saved baseline | PASS, exit 0 |

Browser: installed Chrome version recorded in each report. Node: absolute
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node` (24.22.2),
matching bin first in every child PATH. The exact single-worker Vitest command
and exits are in `supervisor.json`/`tests.log`. No install, build or type compiler
ran. Source changes are CSS only; no TypeScript contract or application code changed.

Full resource release occurred 243.91 seconds after the original supervisor
start. Last browser/test activity ended about 168.31 seconds after that start;
more than 60 seconds remained for whole owned cleanup. Measured peak whole
output was 22,851,772 bytes against 193,273,528; durable evidence at release was
3,240,390 bytes against 20 MiB. Lowest measured free space was 16,797,978,624
bytes, above the 8 GiB stop floor. Every browser/server/context and private
cache closed, port 5292 was freshly bindable, and the own temporary dependency
link and slot TMP/profile directory were removed. Resource21d received the full
release directly. [Release receipt](../evidence/css-parity-feed/release.json).

Broad build, live-service, device, real multiuser, full QA06 and IN03 gates are
NOT_RUN. No browser/resource reservation remains during documentation/publication.

## Exact remaining constraints and requests

- At normalized 390px, the photo portrait frame is about 60px; current approved
  selected-character slots are 48px. Root153 explicitly excluded character
  source/rendering/UV/crop/framing/scale changes. This discrepancy was sent
  directly to Root153; this worker cannot resize or reframe selected art.
- Reference identities, post wording, reaction/comment counts and both media
  images are supplied content. Existing fixtures explicitly identify fictional
  authors and venue illustrations. CSS cannot reproduce the reference content;
  no fabrication or image substitution is authorized.
- The photo's second attachment is shallower than its first. Current Post DTO
  exposes media IDs and the renderer supplies media. Exact varied media geometry
  cannot be inferred safely from post order; no `nth-child` fixture styling is
  added. A genuine media aspect policy belongs to the media/content owner. In
  the final comparison, the two current fixture media strips have equal height,
  leaving the second card and compose row lower than the photo. This remains
  material; neither the green checks nor the close first-card dimensions close it.
- Shared titlebar, close control and full-shell composition are Root153's scope.
  Current cute background artwork has a separate owner and is preserved.

Full target parity remains open until the browser comparisons and all material
owner/content differences are resolved. A passing fixture run alone does not
close that requirement or the original persistent goal.

## Handoff

Source checkpoint: `b16133bd1285dd7ba4fef812bee6c4517cf4fc24`.
Final CSS/evidence handoff: the follow-up commit carrying this final record.
Root153 alone publishes master. Remote ancestry remains pending until the bounded
validated handoff is integrated and published. The original goal stays ACTIVE
because full parity constraints and remote delivery are unfinished; no separate
king/SL review or acknowledgment is required.
