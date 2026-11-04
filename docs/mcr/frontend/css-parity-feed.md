# Feed CSS foreground parity

Task: CSS-FEED, related F01/FE27 and shared F03/FE28 styling. Status: IN_PROGRESS.
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
typewriter-like post text, narrow media strips aligned with the text, a single
action row and a centered compose control. The prepared pass adjusts tab
proportions, 16px post text, 16px card inset, media-column alignment, 2.85:1 media
strip geometry, action text/alignment and a 216px maximum compose width.
Every new selector is scoped to `.feed-browse`; generic Thread/composer rules
and shared post-content selectors remain unchanged.

Selected-character CSS, 48px portrait boxes and canvas/image rendering remain
byte-identical. Decorative page/card/footer backgrounds, shared styles,
shell/chrome, product TSX/JS, assets, manifests and lockfiles are unchanged.
No generated text, fake controls, identities, posts, counts, or media are added.
No decorative left rails are introduced. Existing 44px targets and focus remain
required in the browser checks.

## Validation and iteration

Native allocation: next ready cosmetic round in resource21d's queue; no output
lease yet. Its fresh approximately 14 GiB free-space reading is below the 16 GiB
start threshold. Browser/test results: NOT_RUN. Source authoring is independent
of that resource allocation.

The evidence-only `native.mjs` reuses the existing EventFeed reference fixture
for two explicit synthetic media rows and its actual controller/privacy behavior.
It compares the saved baseline CSS at 390px with the changed CSS, then checks
360/390/430 geometry, tabs/keyboard, notice source, thread callbacks, compose,
options/media filtering, refresh, privacy revocation, empty, read-only,
unavailable, pending and error states. It also consumes the existing cozy
default-root fixture with canonical Auth, router, selected-character renderer
and Phaser over synthetic HTTP/C05. That fixture is not live admission evidence.
The script writes only owned evidence and private temporary caches.

Focused existing Feed and FeedThread regressions are planned; broad build,
live-service, device, real multiuser, full QA06 and IN03 gates are NOT_RUN.

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
  added. A genuine media aspect policy belongs to the media/content owner.
- Shared titlebar, close control and full-shell composition are Root153's scope.
  Current cute background artwork has a separate owner and is preserved.

Full target parity remains open until the browser comparisons and all material
owner/content differences are resolved. A passing fixture run alone does not
close that requirement or the original persistent goal.

## Handoff

Source checkpoint is prepared for committed preservation, with native validation
and screenshots still pending the capacity gate. It is not a validated delivery
or publication request. Root153 alone publishes master. Remote ancestry remains
pending until the validated handoff is integrated and published.
