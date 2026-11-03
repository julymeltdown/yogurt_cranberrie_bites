# C04 recovery reads, version 0.1.1

This additive interface revision closes required mural result recovery and full inbox unread-count gaps. C04 remains REVIEW_CANDIDATE; generated transport metadata and OpenAPI are contract projections, not evidence of enabled routes or live acceptance. The existing common receipt, notification list and C05 wire shapes are unchanged.

## Mural original result

`getConcertMuralOwnResult`: authenticated `GET /api/concert/events/{eventId}/mural/commands/{key}/result`, no body or query, returns the existing `mural.PixelOperation` with `Cache-Control: no-store`. Both path identifiers are canonical UUIDs. Actor selection comes only from the canonical session.

The owner must reauthorize current event/mural access and locate the current actor's retained, unexpired `SUCCEEDED` `placeConcertPixel` receipt for exactly the event and key. The durable original operation must match the receipt's actor/event/resource ID/revision. Missing, foreign, expired, retired, pending, failed or inconsistent results all return `404 RESOURCE_UNAVAILABLE`, without proving nonexecution or granting replay. A dependency or policy outage fails closed. This read never spends cooldown or executes a command.

The consumer first verifies the original status receipt against captured actor/event/operation/key, then checks the returned operation ID/revision/event against that receipt and its x/y/color against the original command. The original `next_allowed_at` controls cooldown recovery. Public snapshots/deltas cannot establish an own acknowledgement; they gain no author or cooldown fields. HTTP snapshot/delta reads and the existing world epoch observer remain the synchronization interface; this revision adds no socket or C05 message.

The BE34/35 producer owns result lookup and persistence qualification. FE33 owns its consumer and negative fixtures. The delivery integrator owns authenticated public routing and the shared contract projections. IN04 must use the real qualified producer/consumer and deployed candidate; offline fixtures do not certify that gate.

## Full unread count

`getConcertNotificationUnreadCount`: authenticated `GET /api/concert/events/{eventId}/me/notifications/unread-count`, no body or query, returns `feed.NotificationUnreadCount`: `{actor_id,event_id,unread_total,as_of}`. The total is a safe nonnegative integer; `as_of` is the server UTC observation timestamp. Responses and errors are not cached.

The notification-service owner counts the complete currently authorized retained unread inbox in one consistent snapshot, independent of list cursor, limit or filter. Current block/deletion/moderation and owning-target visibility apply; unavailable policy fails closed. Counts convey no target authority. The consumer validates current actor/event, clears the observation on scope/policy change, and explicitly refreshes after read/read-all or scoped invalidation. Loaded unread rows never substitute for the total; unavailable remains unavailable.

BE33 owns durable creation/deduplication/read state and this total producer. FE35's existing bounded handoff retains its honest `unread shown` label until the total consumer is implemented and qualified. Root composition and real IN03 remain separate requirements.

## Evidence and remaining work

The revision includes closed positive/negative fixtures, offline captured-target/result binding regressions and generated OpenAPI/transport metadata. Native authorization, expiry, concurrency, cross-user denial, original cooldown preservation, real HTTP routing and live two-client gates remain required. This document records an interface decision, not task acceptance or a scope cut.
