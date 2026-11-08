# Release decision — WP-10

Decision date: 2026-11-09. Candidate: master `d2f545a89`.
Evidence: [release-evidence.md](release-evidence.md) and its machine-readable
companion [wp10-gate-report.json](evidence/wp10-gate-report.json).

## Decision

**NOT RELEASABLE.** The profile/social product cannot be released from this
candidate. The integration gate ran end to end and proved the fixture, the
identity verification flow and the durable lifecycle independence of the merged
packages; it could not prove a single product scenario, because the composed
product does not serve any profile-owned operation over HTTP and has no intro post
surface at all.

This is not a quality judgement about the delivered packages. Every merged
package's own tests pass. It is a statement about composition: the parts are
built and the product is not wired.

## What the product does guarantee today

Only what the evidence file shows, and nothing more:

| Guarantee | Evidence |
| --- | --- |
| Seven services start, become ready and answer health checks on an owned private cluster | `wp10-gate-report.json` `stack.readiness` |
| Three real accounts can be created through the real enrollment flow | `wp10-gate-report.json` `identities` |
| The NAVER verification flow works end to end against a configured provider endpoint, and stores a claim | `wp10-gate-report.json` `identities.verification` (`status: VERIFIED`, `claims_present: true`) |
| A room visibility change does not touch the avatar, the profile posts or the media rows | `wp10-gate-report.json` `sweeps.lifecycle.durable.cases.room_visibility` |
| An avatar save does not touch the room, the profile posts or the media rows | `…cases.avatar_save` |
| The repository guards pass (exact bytes, static imports) | `check.sh guards` exit 0, `violations: []` |
| The backend compiles and the frontend typechecks | both exit 0 |

## What the product does NOT guarantee

Every item below is a claim this release cannot make, and none of them is a
formality:

1. **A user cannot publish a profile post, upload a photo, open their room, save
   their room, change room visibility, change their avatar, or read any profile
   page.** All of these are refused at the gateway by the profile producer's
   closed operation set (`403 PERMISSION_DENIED`). The routes exist; the producer
   behind them does not answer.
2. **No intro post exists.** Publishing one, restricting its audience, attaching a
   photo to it, liking it, commenting on it, resolving a shared link to it — none
   of these has a route in the running product.
3. **The audience conditions are not applied to anything**, because there is
   nothing to apply them to. V-01, V-02, V-03, V-04, V-05, V-06 and V-13 are
   `NOT_RUN`.
4. **The six-step acceptance scenario has never run**, in whole or in part.
5. **No screen has been looked at in a browser** at 360, 390 or 430 px.
6. **Nothing has been verified against the real NAVER provider.** See below.

## NAVER console approval and its scope

There is **no NAVER console approval** in this environment, and the release
decision must not imply one.

- **What exists**: a local test application inside the gate script
  (`NaverProvider`), configured into the product through `NAVER_OAUTH_TOKEN_URL`
  and `NAVER_OAUTH_PROFILE_URL`. It issues codes, exchanges them for tokens and
  serves three fixed provider profiles.
- **What that proves**: the product's own flow — state creation, one-use state
  consumption, the callback's closed query parsing, the profile decode, the claim
  write, the status read — works against a provider that speaks NAVER's protocol
  shape.
- **What that does not prove**: that NAVER will issue a token, that the
  console-registered callback matches, that the real `gender` / `birthyear` /
  `birthday` fields arrive in the documented shape, or that the client id and
  secret are valid.
- **Scope of a real approval, when it exists**: one application, one registered
  callback per environment, the `profile` scope, and the client id/secret pair.
  Production already refuses a placeholder client id (`local-` prefix) and will
  not start with one; that refusal is tested, and it is a guard, not an approval.

## Conditions to change this decision

The decision becomes `RELEASABLE` only when all of the following are true, each
with its own artifact:

1. The profile product producer serves the profile post, media, room and avatar
   operations, and the gate's step 1, step 4 and both lifecycle layers pass over
   HTTP.
2. The intro post routes are registered and the audience decision is enforced on
   list, detail, attachment, thumbnail, bytes, shared link, like and comment; the
   cross-surface sweep (T-953) shows eight identical denials.
3. The six-step scenario (T-952) passes end to end with the three identities.
4. A NAVER console application exists for the target environment, and V-12 passes
   against the live provider.
5. The 360/390/430 px captures exist for every screen the scenario touches.
6. QA08 performance is either qualified or explicitly deferred by the user; it is
   deferred today and is not claimed here.

Until then this candidate is an integration checkpoint, not a release.
