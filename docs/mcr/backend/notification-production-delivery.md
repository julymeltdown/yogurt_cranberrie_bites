# BE33 production delivery and Notification startup

Status: `IN_PROGRESS`. Build, PostgreSQL, mTLS and actual-process checks are
`NOT_RUN` for this source revision until a fresh finite resource offer is accepted.
This continues original BE33; it creates no new required task and does not accept
IN03 or reduce other notification families from specification scope.

## Source and ownership

- Base: `145a8e939fb7bede7e46931d6559adb2c51f62a7`, delivery repository
  `julymeltdown/mcr`, isolated branch `codex/mcr-be-BE33-production-delivery`.
- Existing BE33 application/domain ports and Notification repository are reused.
  No inherited rendering or additional dependency is imported; lockfiles and
  `docs/source/` remain unchanged.
- Target callback dependency: canonical owner 207's source
  `50a5003370b4dfab9cb6483c8689a8378446f9a0`, taken into this branch as
  `f7e215eb`. Its append-only inbox proto and new `concert/target_client.rs` remain
  that owner's sole-writer scope. This dependency supplies a wire/client, not
  proof that the real Root target provider is complete or qualified.
- Root explicitly transferred the new delivery proto, both service build files,
  Concert notification adapter paths, Notification delivery/module paths, and
  Notification main/config/runtime tests to this writer. Root retains Concert
  main, canonical composition, retention, recovery and publication.
- Original `notification_target_policy_support/fixture.rs` and
  `concert_inbox_support/mod.rs` are included read-only by new tests. Their
  owned-cluster validation is reused; no duplicate fixture runner is introduced.

## Behavior

`SocialProjectionRouting` resolves the actual event-scoped social aggregate for
create/decide/cancel/revoke commands, validates original actor direction and
historical resource revision, and selects only the other participant. It locks
existing event/settings/admissions/participants/normalized block-pair metadata.
Missing/corrupt sources or policy are unavailable. Explicit closed event phase,
disabled social setting, suspension or local block suppresses fanout. Unsupported
command families remain unavailable rather than silently acknowledging no work.
An expired/revoked sharing request remains neutral metadata; routing selects no
handle, consent payload, introduction text, location or free-text summary.

`PgProjectionOwner` still verifies the immutable committed receipt/outbox identity
before routing, and bounds the whole database operation to the shorter of the
original deadline and three seconds. Its source snapshot is not an inbox read
authorization or retained Root exclusion.

`NotificationDeliveryClient::connect(endpoint, credential, &TlsMaterial)` requires
HTTPS and presents the Concert workload identity to Notification. The private
`ConcertDeliveryService.Deliver` wire contains only the immutable five-field
envelope and at most 1000 closed recipients. The handler requires a verified
Concert caller, an actual peer certificate, and an original finite deadline.
Unknown enums, noncanonical/nil IDs, duplicate recipients and invalid revisions
fail before ingestion. The client accepts success only for the exact echoed
envelope after the existing Notification-owned transaction commits.

The existing durable delivery receipt freezes first committed fanout, preserves
deletion suppression and makes retries safe after lost acknowledgements or
purging. Background delivery is a system operation; it does not fabricate a
Gateway delegation or an original user's Auth session. Inbox/read/count effects
continue through actual canonical Auth/Friend/Root target policy and the existing
native transaction tracking machinery.

## Notification startup

`NOTIFICATION_CONCERT_MODE` defaults to `off`; only explicit `live` mounts the
Concert inbox, its existing producer settlement, and delivery ingress on the
existing mTLS gRPC listener and service supervisor. Settings parse before database
or listeners. Inherited service settings and inherited RPC caller roles remain
unchanged.

Live serving requires:

- Existing `NOTIFICATION_GRPC_TLS_CERT_PATH`, `_KEY_PATH`, `_CA_PATH` material.
- HTTPS service origins `NOTIFICATION_CONCERT_AUTH_GRPC_ENDPOINT`,
  `NOTIFICATION_CONCERT_FRIEND_GRPC_ENDPOINT`, and
  `NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT`.
- Explicit distinct `NOTIFICATION_SERVICE_CREDENTIAL`,
  `CONCERT_SERVICE_CREDENTIAL`, `AUTH_SERVICE_CREDENTIAL`, and
  `FRIEND_SERVICE_CREDENTIAL`. Existing previous-caller rotation variables are
  supported and cannot overlap another configured workload credential.

Concert alone invokes inbox/delivery. Auth/Friend alone invoke their existing
settlement methods. The additional registry is separate from inherited methods;
this does not grant Concert inherited notification RPC access. Inbox composition
always supplies the real `RootTargetClient`; no unavailable/positive mock fallback
is installed. Post/Report target authority still requires its actual owner policy
and remains unavailable when absent.

## Validation ledger

| Check | Status | Evidence / limit |
| --- | --- | --- |
| Explicit-path Rust 1.88 rustfmt | PASS | Exit 0; default stable rustfmt failed to load its driver, so installed 1.88 formatter was used without installation |
| `git diff --check` | PASS | Exit 0 |
| New routing tests (4) | NOT_RUN | Reuse owned NB03 SQL fixture; protected SQL setup rolls back; additional actual Concert-client case uses the copied live ingress, not public IN03 |
| New ingress tests (3) | NOT_RUN | Actual mTLS and Notification PG; concurrent repeats, immutable fanout, purge retry, caller/certificate denial and malformed input |
| New runtime tests (4) | NOT_RUN | Pure opt-in checks, actual pre-DB refusal, live process mounting and repeated delivery across graceful restart |
| Real canonical target/inbox/native policy | NOT_RUN | Separate owner 207's provider and Root combined composition required |
| Public Gateway / original IN03 | NOT_RUN | Required downstream vertical-flow gate, no acceptance inferred |

Planned finite build commands use Rust 1.88, `--locked --offline`, `-j 2`, debug
information disabled and incremental disabled, in the coordinator-selected existing
target only. The exact test targets are Notification `concert_delivery_ingress`,
Notification `concert_runtime`, and Concert `notification_projection_routing`, plus
the actual `notification-service` binary. Test execution uses copied, hash-pinned
artifacts only after explicit unconditional target release. The accepted offer,
source/artifact hashes, commands/exit codes, native evidence, cleanup and subsequent
publication ancestry must be appended before this work is reported accepted.

Existing CF03's 11 qualified cases are unchanged and are not repeated by this
stage. No Cargo, PostgreSQL or application process was started during source prep.

Source handoff commits: `b54d88d4` (shared private wire/build), `93830287`
(Concert projection/client), `e86952b5` (Notification ingress/startup). The
pre-qualification code pin is `e86952b5`; subsequent evidence documentation does
not substitute for executed checks. Resource request sent to coordinator 21d;
no named offer has yet been accepted.
