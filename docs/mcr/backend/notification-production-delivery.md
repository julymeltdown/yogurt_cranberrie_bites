# BE33 production delivery and Notification startup

Status: `IN_PROGRESS` for original BE33. The bounded production delivery and
Notification startup checks below passed against source
`7e15b1962ea48a59ae0802b0e2b118fc1e0a56b1`. Real canonical target/inbox policy
and public IN03 acceptance remain open.
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
| New routing tests (4) | PASS | Reuse owned NB03 SQL fixture; protected SQL setup rolls back; additional actual Concert-client case uses the copied live ingress, not public IN03 |
| New ingress tests (3) | PASS | Actual mTLS and Notification PG; concurrent repeats, immutable fanout, purge retry, caller/certificate denial and malformed input |
| New runtime tests (4) | PASS | Pure opt-in checks, actual pre-DB refusal, live process mounting and repeated delivery across graceful restart |
| Real canonical target/inbox/native policy | NOT_RUN | Separate owner 207's provider and Root combined composition required |
| Public Gateway / original IN03 | NOT_RUN | Required downstream vertical-flow gate, no acceptance inferred |

The developer accepted `BE33-DELIVERY-MEDIA-202611041106Z` for the existing
MEDIA target. Both build commands exited 0 with Rust 1.88, two jobs, debug
information disabled, incremental disabled and the offered contracts-build dev
optimization level 2:

```text
cargo +1.88.0 test --locked --offline -p notification-service --test concert_delivery_ingress --test concert_runtime --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2
cargo +1.88.0 test --locked --offline -p concert-service --test notification_projection_routing --no-run --message-format=json --config profile.dev.package.contracts-build.opt-level=2
```

The actual Notification binary was emitted automatically. Automatic Concert main
was recorded as metadata/growth only and was neither copied nor run. Four exact
artifacts (94,420,048 bytes total) were copied and hash-pinned outside the target:

| Artifact | SHA-256 |
| --- | --- |
| `concert_delivery_ingress` | `100fe59cab02b932b0995a222d1dc405b479fb679ed24207be8e1925e2584132` |
| `concert_runtime` | `99dcec420fd384e40230432ffd803dc7026a2cb1efc048bfb49227c84114a311` |
| `notification_projection_routing` | `6309aa3c83de295ba5082f1da7a33368b5edafa6d6b3a3c8ef53de0b6991620c` |
| `notification-service` | `9c92b2e0141503752af7ff99be60173426c7030da3f66be5c862cbf382450d0b` |

The target was released unconditionally after Cargo finished and full `lsof`
returned exit 0 with zero target handles. No target access occurred afterward.
Each copied test binary ran with `--test-threads=1 --nocapture`, exiting 0:

- `concert_delivery_ingress`: 3 passed, 0 failed, 0 ignored.
- `concert_runtime`: 4 passed, 0 failed, 0 ignored.
- `notification_projection_routing`: 4 passed, 0 failed, 0 ignored.

The actual Concert client delivered concurrently and repeatedly through the copied
production Notification main, producing exactly one notification. The actual main
startup check repeated delivery across two gracefully terminated service processes
against the same Notification database and still observed exactly one notification.
The Inbox service was mounted and rejected a missing canonical scope. This is a
mounting/fail-closed result, not a real positive target-policy or public-flow grant.

The original owned-cluster and PKI helpers were reused for exactly these PG16
databases: `mcr_concert_notification`, `mcr_concert_notification_source`, and
`mcr_notification_target_policy`. No real Auth/Friend/Post service was started.
Protected SQL routing metadata remained uncommitted and rolled back. Prepared
transactions were zero and protected fixture rows committed were zero. All owned
main/test processes, listeners, database, PKI, temporary fixture and artifact handles
were released before the original absolute stop; full `lsof` returned exit 0 with
zero owned artifact handles. The source worktree remained clean throughout testing.

Private evidence is retained at `/private/tmp/mcr-be33-delivery-7e15b196/`:

| Evidence | SHA-256 |
| --- | --- |
| `source-1.tar.gz` (full tracked backend/contracts source) | `debaa37fd1d7ef1566ac667ba4e73465226a78140bc6b2e7a02fca51c0186b2a` |
| `artifact-receipt.json` (source/runtime pins and exact Cargo artifacts) | `522c41a9fe9a01e763873259b1918820bc5481b3a4ad72bdb2a1cc14ca8e46d4` |
| `target-release.json` | `d6e901a1a71108753af459ca80e71a0a3911b2570f25a8968e08ee5970e411af` |
| `qualify-result-1793790780.json` | `4704abdc2a921d3d4662b2de7468279ddd33ea1423109721e248f8ef530a358e` |
| `full-release.json` | `b2a412996681fc8745b9826bc7f27e11e063210cd17be459debb04baf4d1d830` |

The first copied orchestration attempt stopped at `fixture-pki` before database or
tests; its failure report and executed controller are preserved. The helper then
used the owned explicit OpenSSL request configuration; the successful attempt ran
all eleven cases. No Rust source fix, rebuild or repeat of a passing case occurred.
The first wrapper retained the failed phase/exit result, not the full OpenSSL
stderr; the limitation is not hidden by the successful retry.

Full-release accounting: prior retained carry 416,411,648 bytes; retained target
growth 156,639,232 bytes; private output 102,137,856 bytes; whole retained growth
675,188,736 bytes. Peak whole usage was 761,757,696 bytes, below the offered
2.5 GiB cap, and minimum observed free space was 22,418,092,032 bytes. No resources
remain held. Existing CF03's eleven qualified cases were unchanged and not rerun.

## Developer ownership and publication

The latest human working agreement applies to this resumed assignment: the
implementation developer owns validation and fixes; there is no separate king,
SL or QA review/acknowledgment gate. The named publishing developer receives this
validated handoff directly. Publication routing does not transfer shared-file or
publishing ownership. Canonical target provider work remains with owner 207 and
Concert main/shared composition remains with the designated Root developer.

Source handoff commits are `b54d88d4` (shared private wire/build), `93830287`
(Concert projection/client), and `e86952b5` (Notification ingress/startup), with
`7e15b196` the exact clean qualified source pin. Subsequent changes to this document
are documentation-only. Normal publication, delivered ancestry and this owner's
remote proof remain required. Original BE33 is unfinished while real canonical
inbox/native policy and the actual public Gateway/IN03 flow remain unqualified;
no old review clause is treated as an unfinished product requirement.
