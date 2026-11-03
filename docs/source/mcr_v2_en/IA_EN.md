# MCR Concert World — English Implementation Ledger

Canonical full specification: MCR_IMPLEMENTATION_PLAN_V2_EN.md in julymeltdown/bigyeting, master.
All paths and behaviors below describe the target implementation, not completed code.

## Information architecture

| IA | Route | Label | Behavior and specific states | Implementation tasks |
|---|---|---|---|---|
| A01 | `/` | ENTER THE WORLD | New landing; signed-in redirect; no old homepage. | FE08, FE09 |
| A02 | `/login` | SIGN IN | Email/provider entry; invalid credentials, throttling, provider unavailable. | FE09, BE03 |
| A03 | `/signup` | CREATE ACCOUNT | Eligibility/consent, signup; duplicate and throttled outcomes without unsafe enumeration. | FE10, BE03 |
| A04 | `/verify-email` | VERIFY EMAIL | Verify and resend; expiry and resend cooldown. | FE10, BE03 |
| A05 | `/reset-password` | RESET PASSWORD | Request and complete reset; expired link and neutral account-existence response. | FE11, BE03 |
| A06 | `/auth/callback` | CONNECTING | Callback completion, denial, invalid state, safe return target. | FE11, BE03 |
| A07 | `/legal/:document` | TERMS / PRIVACY / SAFETY | Versioned notices, operator contact, independent fan-service disclosure. | FE08, BE38 |
| A08 | `/account/deleted` | ACCOUNT DELETED | Completion or pending deletion; clear private client state. | FE36, BE42 |
| W01 | `/world` | WORLD | Movement, camera, interaction; loading, live, reconnecting, low power, full room. | FE07, FE12, FE14, FE15, FE16, FE17, BE10, BE11, BE12, BE13, BE14 |
| W02 | `/world/map` | MAP | Overview, camera and place legend; incompatible map version. | FE19, AR02, BE08 |
| W03 | `/world/places/:placeId` | PLACE | Photo and instructions; verified, concept-only, closed, removed. | FE19, BE08 |
| W04 | `/world/rooms` | ROOMS | Capacity, room switch, party invitation; full and denied. | FE18, BE11 |
| W05 | `/world/nearby` | NEARBY IN WORLD | Accessible nearby-user list; block-filtered presence. | FE18, BE13, BE37 |
| W06 | `/world/theme` | CHOOSE AN ERA | Skin selection; loading, fallback, persisted selection. | FE06, BE07, AR04 |
| W07 | `/world/check-in` | REAL-WORLD CHECK-IN | Explicit place/audience/expiry selection; active, expired, revoked. | FE20, BE09 |
| W08 | `/world/help` | HOW TO PLAY | Controls, virtual/physical distinction, meeting safety. | FE15, FE19 |
| F01 | `/feed` | FEED | Latest-first event feed, filters, cursor loading; stale and read-only. | FE27, BE31 |
| F02 | `/feed/new` | NEW POST | Text and image draft; upload failure, retry, published receipt. | FE28, FE04, BE24, BE31 |
| F03 | `/feed/:postId` | THREAD | Post, comments, reaction, report, linked entity; deleted/unavailable. | FE28, BE32 |
| F04 | `/feed/:postId/edit` | EDIT POST | Owner editing and deletion; version conflict. | FE28, BE31 |
| F05 | `/notices/:noticeId` | NOTICE | Source and confirmation time; superseded or withdrawn. | FE27, BE38 |
| M01 | `/meet` | MEET | PARTIES / PEOPLE / MY PLAN; filters and restricted states. | FE21, FE29 |
| M02 | `/meet/parties/new` | CREATE PARTY | Capacity, ticket section, time, place, requirements; validation. | FE22, BE16 |
| M03 | `/meet/parties/:partyId` | PARTY | Apply/cancel; requested, accepted, full, cancelled, closed. | FE21, BE17 |
| M04 | `/meet/parties/:partyId/manage` | MANAGE PARTY | Review applications, remove member, edit meeting, close/cancel. | FE23, BE18 |
| M05 | `/meet/parties/:partyId/rally` | RENDEZVOUS | Approved-members-only gathering state; moved, expired, cancelled. | FE23, BE19 |
| M06 | `/meet/beacons/:beaconId` | FIND EACH OTHER | Physical meeting description, identification, status; stale/revoked. | FE20, BE09 |
| M07 | `/meet/people` | PEOPLE | Introduction discovery; no results and verification-required. | FE29, BE27 |
| M08 | `/meet/intro/edit` | YOUR INTRO | Intro draft, private eligibility conditions, publish/close. | FE30, BE27 |
| M09 | `/people/:userId` | PROFILE | Large avatar, public profile, request sharing, report/block; private/unavailable. | FE30, BE04, BE30 |
| M10 | `/meet/parties/:partyId/chat` | PARTY CHAT | P1: party-scoped messages; removed-member and reconnect states. | EX01 |
| G01 | `/gifts` | GIFTS | Discover by time/place; remaining count and no-stock state. | FE24, BE20 |
| G02 | `/gifts/new` | CREATE GIVEAWAY | Item, photo, quantity, pickup window, place; invalid/upload-failed. | FE25, BE20, BE24 |
| G03 | `/gifts/:giveawayId` | GIVEAWAY | Owner and public details; open, paused, sold-out, completed, cancelled. | FE24, BE20 |
| G04 | `/gifts/:giveawayId/manage` | MANAGE GIVEAWAY | Owner reservations and stock, pause, confirm pickup; version conflict. | FE25, BE20, BE23 |
| G05 | `/gifts/:giveawayId/reservation` | RESERVE | Atomic reservation confirmation; held, exhausted, duplicate, expired. | FE26, BE21 |
| G06 | `/gifts/pickups/:reservationId` | MY PICKUP | Authorized place, time, confirmation code, status; claimed/expired/cancelled. | FE26, BE22, BE23 |
| G07 | `/gifts/mine` | MY GIFTS | GIVING / RECEIVING and history. | FE24, BE20, BE21 |
| R01 | `/wall` | WALL | Pan/zoom, pixel preview, palette, explicit commit; cooldown/paused/frozen. | FE32, FE33, BE34, BE35 |
| R02 | `/wall/history` | WALL HISTORY | P1: snapshots and time-lapse. | EX02 |
| U01 | `/me` | ME | Avatar, profile completion, party and giveaway activity. | FE34, BE06 |
| U02 | `/me/avatar` | DRESS UP | Layer selections, presets, preview and save; invalid/version-conflict. | FE12, FE13, BE07 |
| U03 | `/me/profile` | EDIT PROFILE | Nickname, bio, optional ticket section, visibility; validation. | FE34, BE04 |
| U04 | `/me/social` | SOCIAL LINKS | Private-by-default handles; configure, replace, remove. | FE31, BE28 |
| U05 | `/me/requests` | REQUESTS | Sharing requests; pending, approved, rejected, revoked, expired. | FE31, BE29, BE30 |
| U06 | `/me/age` | AGE & PRIVACY | Unknown, verified, expired, unsupported, provider unavailable. | FE30, BE26 |
| U07 | `/me/settings` | SETTINGS | Theme, reduced motion, low power, sound off by default. | FE34, FE06 |
| U08 | `/me/blocks` | BLOCKED USERS | List and unblock; prior sharing grants do not return automatically. | FE36, BE37 |
| U09 | `/me/account` | ACCOUNT | Logout, deletion request and status, failure recovery. | FE36, BE42 |
| U10 | `/notifications` | INBOX | Read state and authorized deep links; stale target. | FE35, BE33 |
| X01 | `/archive` | AFTER THE SHOW | Read-only permitted posts and final wall; no private locations or handles. | FE40, BE41 |
| X02 | `/archive/souvenir` | OUR PARADE | P1: opt-in avatar souvenir export. | EX02 |
| S01 | `/report?targetType=:type&targetId=:id` | REPORT | Reason, evidence, receipt; reporter identity hidden from target. | FE36, BE36 |
| S02 | `route-error-boundary` | UNAVAILABLE | Safe 401/403/404/410/429/503 states; return to world. | FE07, FE41 |
| S03 | `connection-and-maintenance-overlay` | RECONNECTING / MAINTENANCE | Offline/stale/version mismatch; no silent offline reservation replay. | FE17, FE41 |
| O01 | `/admin` | CONTROL ROOM | Operational summary and kill switches; degraded state. | FE38, BE38 |
| O02 | `/admin/event` | EVENT SETTINGS | Event phase, feature flags, notices; publish confirmation. | FE38, BE38 |
| O03 | `/admin/places` | VENUE & PLACES | Source review, verification, closure, accessibility notes, map version. | FE38, BE08, AR01 |
| O04 | `/admin/reports` | REPORT QUEUE | Review, action, status; moderator access only. | FE37, BE36, BE39 |
| O05 | `/admin/users/:userId` | MEMBER REVIEW | Minimal necessary data, suspension, connection termination. | FE37, BE39 |
| O06 | `/admin/content` | CONTENT REVIEW | Hide/restore posts, parties, giveaways, and comments. | FE37, BE39 |
| O07 | `/admin/mural` | WALL CONTROL | Pause, region lock, restore, freeze; revision reset. | FE39, BE40 |
| O08 | `/admin/audit` | AUDIT LOG | Search actions and outcomes; redact sensitive contents. | FE38, BE39 |
| O09 | `/admin/archive` | CLOSE THE EVENT | Freeze, export, expire grants, cleanup; partial-failure recovery. | FE39, BE41 |

## Development tasks

