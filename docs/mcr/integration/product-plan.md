# Product plan — profiles, mutuals-intro posts, photos, and personal rooms

Status: **PLANNING_COMPLETE / IMPLEMENTATION_NOT_STARTED**. This document is the
product authority for the integrated application. It restates the user's supplied
plan in engineering terms and adds the decisions that the plan leaves to
implementation.

## 1. Product thesis

The profile is the center of a user's identity. Music — emo above all — is the
entry point: users pick favorite bands and artists (K-pop excluded), plus anime
and games, and those shared picks are how they find each other. Photographs,
artwork, a short introduction, and optional mutuals-intro posts carry the rest of
the getting-to-know-you. A personal room is a separate space where a user
expresses themselves; it is not a step in any introduction flow.

The service does **not** claim that decorating a room or owning an avatar is the
differentiation. Personal-space decoration already exists elsewhere. The
differentiation is discovering people through music taste, real photographs,
creative work, and honest introductions, with a heart as the only signal of
interest.

## 2. Entities and ownership

| Entity | Purpose | Owns | Must never reference |
| --- | --- | --- | --- |
| `profile` | Durable identity: nickname, introduction, profile image, emo character, favorites | `profile_rooms`, `profile_avatars`, `intro_posts`, `profile_posts`, `media_assets` | An external account ID used as identity |
| `profile_room` | One personal space per profile: layout, furniture, posters, wall/floor style, visibility | Its layout revision | `intro_id`, `event_id` |
| `profile_avatar` | The emo character configuration for the profile | Config + revision | Concert participant key |
| `intro_post` | An optional introduction post for finding people | Body, imported favorites snapshot, attachments, `audience`, likes, comments | `room_id`, room visibility |
| `intro_like` | A heart on an intro post | `(intro_id, actor_profile_id, created_at)` | Any approval, application, or reward state |
| `profile_post` | An ordinary selfie/photo/artwork post | Description, attachments, visibility | `intro_id` |
| `media_asset` | An uploaded image file plus its transformed derivatives | File ownership and status | — (viewing authorization comes from the attached post) |
| `identity_claim` | The verified NAVER age and gender attributes for one account | Provider, verification time, expiry, derived age | Any user-editable field; any event |

Hard rules, each of which has a verification case in
[verification-plan.md](verification-plan.md):

1. A room is a subordinate feature of a profile, never a component of an intro
   post. No room preview inside an intro post; no room visit required to publish
   one.
2. Publishing an intro post requires no room decoration. Deleting or privatizing
   an intro post leaves the room untouched. Room visibility and intro audience
   are independent settings that never modify each other.
3. Attachments on a restricted intro post never appear in the profile gallery or
   in room posters automatically. Each publication is an explicit, separate act.
4. A poster's depicted band or game never becomes a favorite automatically.
   Favorites drive discovery; posters are self-expression.
5. Ordinary photographs are never pixel-art converted. Conversion exists only for
   room posters.
6. Viewing a profile or a room never generates a like or a visit notification.
   Only deliberate actions (like, comment, follow) notify.
7. Age and gender are verified provider attributes, never user-entered fields.
   The user can connect, inspect, and disconnect the verification; they cannot
   type a different value.

## 2.1 Age and gender verification

An age or gender restriction is only meaningful if the attribute is real.
The service therefore uses the NAVER Login verification the donor already
implements: the user completes a NAVER authorization, the server reads the
profile API, and the returned `birthyear`, `birthday`, and `gender` become a
verified claim with a fixed lifetime.

| Aspect | Behavior |
| --- | --- |
| Obtained from | NAVER Login profile API during a dedicated verification flow, not from a form |
| Stored | Provider, subject hash, gender bucket, age at verification, next birthday, verification time, expiry |
| Lifetime | At most 90 days; the user is prompted to re-verify after expiry |
| Gender values | `female`, `male`, and the provider's explicit `unknown` |
| Missing provider field | Verification fails; no claim is written; the user stays unverified |
| User control | Connect, see the verification date and expiry, disconnect |
| Required for | Viewing or being matched by a post that carries an age or gender restriction |
| Not required for | Profile, photos, room, favorites, unrestricted intro posts |

A user who never verifies can use the whole product except restricted posts. The
interface must state that in one line rather than showing a dead end.

## 3. First-time experience

1. Registration creates a profile, one default emo character, and one personal
   room in its default state. Choosing a nickname is the only mandatory step.
2. Interest selection offers three categories in this order: **bands and artists**
   (K-pop excluded), **anime**, **games**. Up to five per category. No category is
   mandatory; skipping all of them is allowed and the profile, photos, and room
   still work.
3. Before the first publication of each content type, the interface explains that
   visibility for that type. Profile visibility, photo publication, room
   visibility, and intro-post publication are four separate controls, never one
   "publish everything" button.
4. A profile with an undecorated room and no intro post is still discoverable.

## 4. Discovery

Two separate lists, never merged:

- **People** — profiles whose selected catalog items overlap with the viewer's.
  Default tab is music; tabs for anime and games. A card shows: profile image or
  emo character, nickname, public introduction, the **names of the actual shared
  items** ("You both love MCR and NANA"), an optional one-line interaction-style
  description, an open-profile action, and an optional follow action. Opening the
  room is a separate action inside the profile.
- **Mutuals-intro posts** — post cards with their own heart button. Each card is
  filtered by its own audience before it is included.

Rules:

- Overlap is computed from catalog item identifiers, never from image analysis.
- No compatibility score, closeness score, or ranking by favorite count or like
  count.
- If two profiles share nothing, they may still browse each other's public
  profiles, but the card must not claim shared interests.
- A viewer's own search preferences never grant access to someone else's post.

## 5. Interaction model

### Hearts on intro posts

| Action | Meaning | Result |
| --- | --- | --- |
| Tap heart | Interest in or agreement with the introduction | Toggle like state and count; notify the author once |
| Author opens likers | See who expressed interest | Author-only paginated list with profile summary; one tap to the profile |
| Follow | Want to see someone's public activity again | Independent follow edge; no approval, no effect on likes |
| Comment / reply | Discuss the post | Visible only within the post's audience |

- No song choice or greeting is required before leaving a heart.
- Liking never creates a follow, a friend request, a pending approval, or any
  contact disclosure.
- Mutual likes and mutual follows are not an application and approval.
- Authors have no obligation to respond; the service shows no unanswered-request
  indicator and no acceptance deadline.
- The liker list is author-only. Ordinary viewers see the count and their own
  state. Inaccessible accounts (blocked, deleted, restricted) are omitted.

### Notification content

Beneath "rin liked your mutuals-intro post", show the liker's profile image,
nickname, shared interests, and one-line introduction. Tapping the entry opens
that profile directly — never an acceptance screen, never the room.

### Conversations

Short comments on intro posts and photo posts, with replies. A song title or link
may be included optionally. Guestbook messages are not a required step. Comments
are visible only to users authorized to view the parent post. The initial release
adds **no** direct messaging, voice chat, or real-time chat.

## 6. Personal room

Every profile has exactly one room with a persisted layout. Authorized visitors
see the saved room while the owner is offline. A character standing in a room
never implies the owner is online.

| Element | Initial scope |
| --- | --- |
| Character | Emo presets and the existing MCR customization |
| Space | One fixed room; a small set of wall and floor styles |
| Furniture | Small default set, grid-aligned placement |
| Posters | Pixel-art posters generated from user images, up to four |
| Persistence | Layout saved, restored on return, shown to authorized visitors |

### Poster flow

Exactly three steps: **import image → crop and preview the pixel conversion →
place on the wall**.

- The default conversion must be immediately usable on a phone-sized screen.
- At most two presets ("Fine pixels", "Chunky pixels"). No color-count, dithering,
  or layer controls. No separate poster workshop or pixel editor.
- Faces, logos, and album artwork must remain recognizable at mobile size.
- The conversion happens in the browser; the server stores the resulting poster
  image and the placement, not the source photo.

## 7. Navigation and screens

Three primary sections replace the concert-first tab bar for the integrated
product. MCR's existing world, feed, meet, gifts, and wall screens remain
reachable but are not the primary navigation.

| Section | Contents |
| --- | --- |
| Discover | People (shared interests) and Mutuals-intro posts; compose entry |
| Activity | Likes received, comments, replies, follow notifications |
| My Profile | Introduction, favorites, profile image, emo character, photo/artwork posts, room entry |

Subordinate screens: `Profile` (visitor view), `Room` (subordinate to profile),
`Intro composer` (independent post-composition screen), `Photo composer`,
`Likers` (author-only), `Notification detail`.

The audience controls live under the heading **"Who can view my mutuals-intro
post"** inside the intro composer and editor — never in a global settings page,
never labelled as a search filter.

## 8. Product decisions

The user answered the three blocking questions on 2026-11-08 (charter §7). The
remaining items were decided by the lead under the user's delegation.

| ID | Question | Decision | Status |
| --- | --- | --- | --- |
| P-01 | Gender source | Verified NAVER Login claim, same flow the donor already implements. Not self-declared. | DECIDED by user |
| P-02 | Concert world in the primary navigation | Removed from primary tabs; reachable from a secondary menu. | DECIDED |
| P-03 | Room default visibility | `public`, owner toggle to `private`. | DECIDED |
| P-04 | Photo post default visibility | `public`, owner toggle to `private`. | DECIDED |
| P-05 | Comments on photo posts | In scope, sharing one implementation with intro comments. | DECIDED |
| P-06 | Maximum favorites per category | 5 | DECIDED |
| P-07 | Follow edge source | Existing `friend-service` follow, exposed through a profile-shaped operation. | DECIDED |
| P-08 | What happens to a user whose claim expired while browsing | The restricted post disappears on the next request; the UI offers re-verification without revealing the hidden content. | DECIDED |
| P-09 | Should the audience control offer `unknown` | Yes. It addresses users whose provider reported no gender, and it is never a wildcard. | DECIDED |
| P-10 | Can a user hide their verified age/gender from their own profile display | Yes. The claim is used for access decisions; showing it on the public profile is optional and off by default. | DECIDED |

## 9. Acceptance criteria for the integrated release

The end-to-end scenario that must work:

1. User A selects MCR and NANA on the profile and publishes one selfie and one
   original artwork as ordinary profile posts. The room stays in its default
   state.
2. Independently of the room, A publishes an intro post with audience
   "Women · Ages 18–29". User B satisfies it and can view the post, its
   attachments, and leave a heart. Whether B ever published an intro post is
   irrelevant.
3. A reaches B's profile from the like notification or the likers list. After
   viewing B's introduction, favorite music, photos, and artwork, A may follow B
   or comment. No acceptance screen, no room visit.
4. Later, A places a pixel-art poster in the room. The published intro post and
   its like records are unchanged. Deleting the intro post leaves the room and the
   photo posts intact.
5. User C, who fails the audience conditions or whose attributes cannot be
   evaluated, cannot view A's restricted post or its attachments. Whether C can
   view A's profile or room depends on those resources' own visibility.

Completion criteria:

- [ ] Profiles and ordinary photo/artwork posts work without any intro post or
      room decoration.
- [ ] Character, furniture, and poster placements persist across sessions.
- [ ] The image-to-pixel-art-to-wall flow completes without a separate creation
      screen or external editor.
- [ ] Heart toggle, notification, and author-only liker list work, with one-tap
      navigation to the profile.
- [ ] Hearts trigger no application, acceptance, automatic follow, or contact
      disclosure.
- [ ] Ordinary photographs are never pixel-art converted; poster creation is
      explicitly initiated.
- [ ] Audience conditions apply consistently to lists, details, attachments,
      shared links, and like creation, and are re-evaluated on every new request.

## 10. Questions to ask during user testing

1. "Does this filter decide which people I want to see, or who is allowed to see
   my post?"
2. "What happens when I tap the heart, and where can the author find my profile?"
3. "Can I publish an intro post and exchange hearts without ever opening my
   room?"
4. "Can I tell whether this photograph is public on my profile or attached only
   to a restricted intro post?"

Observe whether likes lead to profile visits and real conversations, and whether
photos and artwork help people get to know each other. Do not read like or
follower counts as closeness. If users mistake the direction of the audience
control or where a photo will be published, fix that interface first.
