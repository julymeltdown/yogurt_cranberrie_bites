# Account Private Data field inventory

Status: **IN_PROGRESS**. The bounded core leaf covers participant preferences
and avatar selection only. This inventory is the activation input for the
remaining owner steps; it does not turn an unimplemented owner into a success.

| Category | Actual storage/writer | Disposition | Current owner |
| --- | --- | --- | --- |
| Participant personal settings | `concert_participants.theme`, `low_power`, `reduced_motion`, `sound_enabled`; participant repositories and coordinator | Existing rows are set to canonical neutral `PARADE`, all effect flags false, with revision increment. The key remains for aggregate references. Every later personal write or delete requires the common user-first lock and is rejected by the permanent fence. | This core leaf |
| Avatar selection | `concert_avatar_settings` (including layered, whole-character and reviewed-motion config) | Existing rows are removed in the same fenced transaction. Later inserts and updates are rejected by the permanent fence. World projections that require a selection fail closed. | This core leaf |
| Profile identity and avatar profile fields | Profile service `profiles`/`avatars` | Owning Profile deletion and tombstone policy. No foreign database write. | Profile owner / Auth deletion saga |
| Social handles, requests, grants, publication | `concert_social_requests`, bindings and publication surfaces | Withdraw requests/grants and invalidate publication under the Sharing owner receipt. | Sharing owner |
| Notifications | Notification service and Concert target evidence | Suppress future delivery and retain supplied strong receipt. | Notification owner |
| Intro text and private author input | `concert_intros` and intro native witness | Remove/neutralize personal text while retaining only required audit/native receipt fields under a dedicated typed receipt. | Intro owner (step not implemented) |
| Age claims and verification | `concert_age_owners`, `concert_age_verifications`, `concert_age_claims`, runtime witness | Erase age/birthday evidence through the age owner while retaining the minimum required saga/audit proof. | Age owner (step not implemented) |
| Media bytes and metadata | `concert_media`, `concert_meeting_beacon_photos` plus staged/sanitized bytes | Remove personal bytes and metadata; retain opaque non-readable tombstones only where constraints require them. | Media owner (step not implemented) |
| Precise beacons and rally state | meeting beacon, rally and private membership rows | Expire/revoke precise location and audience, and prevent recreation. | Beacon/Rally owners (steps not implemented) |
| Party and Gift private author input | Party application/member private fields and Gift private child intents | Neutralize personal input while retaining aggregate stock/claim/audit references. | Party/Gift owners (steps not implemented) |
| Private Mural metadata | Mural operations/history and private author association | Remove private association/history where another user's public artwork does not require it; generation fences still deny future private writes. | Mural owner (step not implemented) |
| Post and Feed content | Post service tables | Owning Post deletion and public/private projection policy. No foreign database write. | Post owner |
| Friend relationships | Friend service tables | Owning Friend relationship deletion and request/grant withdrawal. No foreign database write. | Friend owner |

## Canonical constraints

The core leaf preserves the participant key because foreign keys, admission,
feed joins and private relationship state refer to it. The personal selection
columns are neutralized and the separate avatar row is deleted. This is the
default owner-neutral state permitted by the existing schema, not an invented
identity. The exact writer fences reject direct SQL recreation even when an
application path is bypassed.

The avatar row is removed with a `BEFORE DELETE` guard. PostgreSQL requires the
guard to return `OLD` for a delete and `NEW` for insert/update; returning `NEW`
would cancel the delete. The participant mutation guard follows the same rule,
so direct SQL cannot bypass the permanent fence or suppress the controlled
erasure transaction's neutralization.

Deleted-user evidence may retain an opaque user UUID and immutable transaction,
receipt and audit references. It must not retain personal preference values,
avatar choices, text, age evidence, image bytes, precise locations, handles, or
private audience data.

The companion orchestrator must report each unimplemented owner as
`INCOMPLETE`/retryable. A successful core receipt is never a whole PrivateData
acknowledgement.
