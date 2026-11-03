# FE29–FE30 introductions and profiles

Status: **IN_PROGRESS**. Isolated source base
`e6fef53d9f0841b3e16ce3baa4db01a834043957`; branch `codex/mcr-fe-FE29`.
Only `features/people`, `features/intro`, `features/profile`, `features/age-status`
and this document are assigned. Root composition remains delivery-owned.
No source archive, backend, shared transport, manifest or artwork edits.

Frozen inputs: C04 social/common schemas, operation register and policy at the
source base, C03 PublicProfile, C06 AvatarConfig. UI and state orchestration are
newly authored; privacy/cancellation follows FE02/FE27 discipline at the same
base, without copying a renderer or old Bigyeting UI. The existing FE13
`decodeAvatar` validates public AvatarState; FE12 reviewed runtime handles all
product thumbnails. No approved binding means unavailable, never test/legacy art.

Initial public API: `createPeopleController({auth,access})`, `People`,
`PublicProfile({controller,userId,avatars?,sharing?})`, and
`ReviewedProfileAvatar`. Canonical auth exposes privacy/request; access supplies
current server-projected availability/read-only state. Composition owns one
controller per shell lease and disposes it before auth. Avatar options retain
**original bootstrap PrivacyScope**, plus a resolver for pinned reviewed assets.
No new game, world connection or identity is allocated.

People uses only `listConcertIntros` (`q`, `limit`, `cursor`). The frozen contract
has no public intro-by-ID operation; public profile entry uses C03
`GET /people/{userId}`. Unauthorized direct IDs share opaque feedback and erase
that author's cached intro rows. Every public decoder is closed: exact ages,
claims, handles and owner conditions reject the entire response. No X identity
is required. Pagination is explicit, rejecting duplicate rows and repeated
cursors; identity/event/privacy changes erase rows, profile and search text.

Initial checks: strict TypeScript and six public-boundary tests exit 0 using
Node24.22.2/retained pnpm11.7.0 (offline frozen install exit0). Editor/age commands,
UI/native360/390/430 evidence and final combined checks are still in progress.
Real service, device and IN03 acceptance are NOT_RUN.
