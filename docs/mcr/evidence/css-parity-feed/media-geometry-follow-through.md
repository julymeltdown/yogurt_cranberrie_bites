# Feed attachment geometry follow-through

CSS-FEED, same goal, lease, branch and checkout. This is a source/input analysis
of the already captured final iteration, not a new native/browser run.

## What determines the height

`PostContent` in `frontend/src/features/event-feed/EventFeed.tsx` gives each
authorized media ID to the supplied media renderer. The actual `FeedImage`
checks the current privacy scope, matching media ID, POST purpose, read path and
blob URL before rendering. Its `img` has the authorized media width and height
attributes. Therefore CSS **can** use actual intrinsic attachment proportions;
the post DTO containing only media IDs is not itself a CSS limitation.

The current scoped CSS gives every figure the same 2.85:1 preview box and uses
`object-fit: cover`. This produces equal-height cropped previews for equal-width
media columns. The two source images in the existing reference fixture are
`frontend/public/world/illustrated/parade-scene.png` and `revenge-scene.png`.
Their actual PNG IHDR dimensions are both **1152×896 (9:7)**. The qualified
fixture's authorized media metadata is also 1152×896 for both images. These are
explicit synthetic venue illustrations, not the two post attachments in the
human reference.

## Current measured result and the input mismatch

The existing final native report records these dimensions:

| Viewport | Each media column | Each preview box | Each article |
| --- | --- | --- | --- |
| 360px | 230px | 230×80.6875px | 229.078125px high |
| 390px | 260px | 260×91.21875px | 239.609375px high |
| 430px | 300px | 300×105.25px | 253.640625px high |

At normalized 390px, the reference's first attachment is roughly 254×90px and
the second roughly 254×61px. Their proportions differ, approximately 2.8:1 and
4.2:1. The second existing fixture illustration has no corresponding wide
aspect ratio or content. It cannot fill the reference's shorter full-width box
while also preserving its complete 9:7 composition.

At a 260px media-column width, the complete current 9:7 image needs
`260 * 896 / 1152 = 202.2222px` of height. Fitting the complete image into 91px
of height makes its painted width about 117px; fitting it into 61px makes its
painted width about 78px. Stretching it across 260px distorts the composition.
Covering a 260×61px box instead preserves only about 30% of its vertical
composition. Changing only the second figure's height would manufacture that
difference from the row order, with no media-derived reason for it.

## CSS alternatives and disposition

- **Natural aspect ratio (`height: auto`, no forced figure ratio):** can correctly
  produce different heights when the actual authorized attachments have different
  proportions. With the current two 9:7 inputs, both become about 202px high at
  390px; the second-card gap to the photo increases. This does not solve the
  current input mismatch.
- **A shorter uniform aspect ratio or maximum height:** shortens both fixtures,
  losing the close first-card geometry. With `cover` it crops more actual image
  content. With `contain` it leaves most of the wide preview box unpainted and
  reduces the whole venue illustration to the small widths above. Neither
  reproduces the reference's full-width second attachment.
- **`nth-child`, media-ID or fixture-name special cases:** can force the second
  row to be shorter but fabricate a media distinction. They are not a truthful
  product rule and are not added.
- **Natural sizing of genuine wide authorized attachments:** is a viable CSS
  follow-up once the media/content owner supplies the matching approved input
  and confirms the intended full-image versus thumbnail behavior. It does not
  require changing Post DTOs or weakening authorization.

No further product CSS change or deeper crop is made to disguise this mismatch.
The existing 654bbb16 CSS/evidence handoff remains unchanged and bounded
MOCK_READY. Full screen/photo parity remains unproven.

## Exact request to Root153

Route to the responsible media/content owner: provide the approved authorized
attachment bytes and matching width/height metadata for a representative wide
first image and shallower second image, or explicitly identify the intended
general thumbnail policy for real attachment proportions. Do not substitute
reference pixels, fabricated posts/media, arbitrary row heights or selected
character art. With those genuine inputs/policy, this CSS owner can implement
intrinsic sizing if needed and validate 390px plus 360/430px, actions/focus,
privacy states and unchanged portraits in a later named resource round.

Other existing constraints (selected 48px portrait framing versus the reference,
shared chrome, authorized post content, separate background ownership) remain.
The original persistent goal stays active. This request is an input/interface
dependency, not a king/SL technical review or approval gate.
