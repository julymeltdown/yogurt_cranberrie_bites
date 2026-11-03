# AR01 venue source qualification

Research date: 2026-11-04 KST. Status: **IN_PROGRESS**; physical publication and
operator verification remain **NOT_RUN**. Base: `a25b85592b906209f4a9d20f9c7279ced8abab02`.
Checkout: `/Users/lhs/.codex/worktrees/a519/mcr`, branch `codex/mcr-art-AR01-AR02`.

The [source pack](../../frontend/public/world/places/source-pack.json) records
five exact URLs, UTC retrieval times, retrieved-byte SHA-256 values, response
headers, supported facts, rights, two physical candidate IDs and external gates.
HTML hashes identify observations, not immutable publisher revisions; these
pages can change. The official map was inspected temporarily and is not shipped.

| Reference | Supported use | Limit |
| --- | --- | --- |
| [Venue overview](https://www.p-city.com/front/culturePark/overview?language=EN) | Culture Park lawn/event-space character; Station Plaza is separately named | General description does not establish concert access |
| [Venue location and legend](https://www.p-city.com/front/contents/pcity/location?language=EN) | Studio Paradise and Culture Park identities | No event-day path approval |
| [Official diagram](https://www.p-city.com/pcPub/static/images/map/img_paradise_map01.jpg) | Studio10 appears above Park11, Chroma6 to their right in the diagram | No compass, survey, exact footprints, walking distance or current gates derived |
| [Venue event14396](https://www.p-city.com/front/event/eventView?P_CTNT_TY=imageTy&P_SEQ=14396) | Published November7,2026 19:00 concert at Culture Park | Schedule listing does not verify operations |
| [Ticketlink notice63744](https://www.ticketlink.co.kr/global/en/help/notice/63744) | Published revised date and venue corroboration | Not a route or closure plan |

The supplied `approved_mobile_world.png` was visually inspected and its SHA-256
matches `7abdfbd492216b84317fe31161b65345af5a8e6fa8de7deaed239c49d3db23e1`.
Its explicit “CONCEPT MAP - VERIFY ON SITE” warning remains authoritative.
It supplies design inspiration only. No characters, source pixels, logos,
photographs or map artwork are extracted into this pack.

## Physical candidates and truthful unknowns

Culture Park is candidate `0b6f634a-994f-49a1-9150-000000000001`;
Studio Paradise is candidate `0b6f634a-994f-49a1-9150-000000000002`.
Both are **UNVERIFIED**, closure **UNKNOWN**, photo permission **UNREVIEWED**,
and physical meeting publication **UNAVAILABLE**. No public accessibility claim
or blanket photo license follows from public website access. No raster source
asset is redistributed, so no reuse permission is assumed.

C06 v1 has only `OPEN`/`CLOSED`, so neither unknown physical candidate is emitted
as a product `Place` or selectable MapManifest destination. Unknown is not encoded
as either open or closed. This representation gap was reported to Delivery;
changing schemas is outside this assignment. AR02 uses distinct explicitly
virtual identities. Those identities never authorize physical beacons or arrival.

## Required operator evidence

Delivery must obtain a dated event-day layout with temporary fences, gates,
queues and closures; accessible-route and safe meeting-point review; a named
operator reviewer; publication revision/expiry; and a closure-update owner.
Photo or map-art reuse additionally needs explicit permission and scope. No
operator was contacted and no messages were sent to external parties.

On closure uncertainty or stale qualification, withhold the physical destination.
Later verified publication must use its own current server record, review,
closure revision and expiry, and reauthorize new meetings server-side. Virtual
movement remains unrelated to physical arrival, route safety, distance or admission.

## Validation and delivery

Official pages fetched successfully (exit0); the official diagram bytes hash to
`f4bd424e7528e9734ce963790aca23a8f693557c3daa0846a057fc059d3904f2`.
The preserved source-image hash check passed (exit0). Source pack JSON parses;
all physical candidates carry source IDs, unknown closure, explicit photo state
and denied publication. Source originals and `docs/source/` remain unchanged.

New metadata and summaries are authored for MCR; no headless code or third-party
rendering was copied. Dependencies for this research were Python3 standard
library HTTP/JSON/hash support and visual inspection. AR01 factual research is a
committed bounded handoff; whole AR01 acceptance is not claimed. The containing
Git commit supplies exact ancestry and is sent directly to Delivery and World SL.
