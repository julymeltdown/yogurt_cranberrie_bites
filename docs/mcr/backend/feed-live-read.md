# Canonical event feed reads

Status: IN_PROGRESS. No live read acceptance is claimed.

## Objective and completion criteria

BE-PUBLIC-FEED-READ delivers event-scoped post list/detail and comment reads
through the existing Post owner store, current actor/subject/event authority and
actual C03 public-author projection. Completion requires implementation,
meaningful guarded/native/HTTP validation, documented evidence, committed direct
non-force master publication, remote ancestry verification and concrete review
fixes. Post, comment and reaction writes remain required later work.

The chat has an incompatible unfinished historical BE13/BE14 goal. It remains
truthfully blocked; this objective does not reset or falsely complete it.

## Ownership and source

The exclusive checkout is `be-world-recipient-producer`, branch
`codex/mcr-be-event-feed-read`. Initial base was
`f4b4080897a6b1ad835fa4e5e9372f82d58e489e`; source was refreshed to `b8b5a939`
before edits and `2ce8b60fdc4b14a14da6fc19a6099b26820df0fc` for the first
adapter qualification. Existing Post persistence, commit fences, Concert feed application,
opaque cursor codec and C04 serializer are retained. No source is copied from
historical product UI or inherited `/api/feed` routes.

The frozen API is `contracts/generated/mcr-domain.openapi.json`: listConcertFeed,
getConcertPost and listConcertComments. A detail request has no query parameters;
its initial comment page uses the default limit 20. Subsequent comment pages use
the listConcertComments cursor binding. PublicProfile permits only user_id,
display_name, nullable bio and canonical AvatarState.

## Required producer boundaries

Canonical owner authority owner `01a1a207-f012-7343-a24b-12b9c544973f` supplies real
Auth/Friend exclusion held through the protected future and transaction commit.
Post must additionally receive qualified current event/participant authority.
A supplied digest, JWT, workload credential, participant row or recent observation
is not this authority. Existing `Owner::new` continues to deny access until the
real production constructor is qualified. Mutation handlers stay unavailable.

Public-profile owner `01a1a205-3f89-78c3-aa67-719c4bb40b8d` supplies the canonical
viewer/subject/event projection. World SELF_PROFILE is not reusable author
permission. FeedResources::author returns PublicAuthor with exact subject UUID
and schema-qualified bytes. Missing author, link or POST-media authority fails
closed; no invented author, URL or success-shaped empty response is permitted.

Root composer `01a1a1fc-7f3d-7ff1-9416-e3435c8a6f08` owns product RPC, Gateway
HTTP, original ingress authorization capture, config/TLS and startup registration.
Typed Context carries bindings only. Production composition must retain the
qualified guard across read, enrichment, final fences and response assembly.

## First validated checkpoint

The compiled groundwork adds `feed::live_read::ReadFeed` for canonical page,
detail and comment orchestration; preserves opaque cursor bindings and immutable
upper/after windows; validates exact C03 author identity/allowlist; and clips the
whole read future to the original absolute deadline. Post read-wire conversion
rejects missing/noncanonical bindings, invalid filters/limits and future windows.
The read settlement registry seals admission, drains explicitly finished work,
and refuses positive settlement for cancelled/unknown work or another process.
It is not owner authority or PostgreSQL transaction settlement by itself.

Production author adapter `feed/resources.rs` is pending and unexported until the
canonical guard and public-profile modules land. Its presence is not compilation
or runtime qualification. The additive qualified Post protocol is preserved as
an external pending artifact while the independent adapter checkpoint retains
the legacy proto. Production Post reads remain closed; no feature switch is enabled.

## Validation

Rust 1.88, `--locked --offline -j 2`, development/test debug 0 and incremental 0:

```sh
cargo test --locked --offline -j 2 -p concert-service -p post-service \
  --test feed_live_read --test concert_feed_live_read --test feed_wire \
  --no-run --message-format=json
```

The Cargo JSON-selected binaries and 1,428 source files were frozen outside the
shared target before execution. The 16 new deterministic adapter cases and three
retained wire regressions passed: **19 passed, zero failed/ignored/filtered**.
The standalone wire build first found a stale cross-checkout proto helper;
scoped cleanup of `contracts-build` removed 20 files (816.3 KiB), and the final
three-target compilation and execution passed. No whole-target clean, third
build tree or dependency install was used.

`backend/scripts/mcr/feed_live_read_smoke.py --adapters-only` checks exact source
and binary hashes, executes that exact inventory with a stripped environment,
and always labels live owner/HTTP/native checks NOT_RUN. Its six Python preflight
cases pass, including substituted binaries, changed sources and skipped tests.

Evidence is external at `/private/tmp/mcr-feed-adapter-20261104/`:
`binaries-v2.json`, `rebuild.jsonl`, `source/`, `adapter-report.json` and
`reproduced-adapters.json`. The initial adapter report SHA-256 is
`ab081dc0e81230c1e23d53d062739d66c8423a47c7d513d8cb4677c2bda0db71`.
Pending qualified IDL is `qualified-protocol.pending.proto`; it is not a live
protocol qualification pin. BACKEND was unconditionally released to the named
media owner after frozen copies and a zero-handle check. No target or PG lease
is retained while awaiting the positive integration prerequisites.

Guarded/native PostgreSQL/live HTTP checks: **NOT_RUN**. Canonical owner guard,
Post qualified read admission/transaction settlement, actual public author
producer integration and root HTTP composition remain required. Post/comment/
reaction writes remain later required scope. This checkpoint is not C04 live
feed acceptance, BE31/BE32 completion or IN01 acceptance.

## Unpublished follow-up source

The task branch additionally preserves unexported, uncompiled drafts:
`feed/policy.rs`, `feed/resources.rs`, `feed/author_filter.rs` and
`post-service/src/concert_feed/read_transaction.rs`. They target the real sealed
OwnerGuardScope and PublicProfileReader::policy/read APIs, share one local guard
per request binding and subject, and never open a second pair transaction.
The comment adapter validates the complete owner batch before filtering currently
unavailable authors, preserving the original internal scan boundary. The Post
transaction draft captures identity on the actual SQLx transaction and queries
its owning PostgreSQL store; its terminal status alone cannot release authority.
The producer role requires explicit permission to read pg_control_system.

The dedicated Post read service will use four signed request envelopes, each
wrapping the unchanged legacy query and ReadAuthority. Shared lease producer
settlement handles outcome lookup. ReadAuthority additionally needs the canonical
root scope capability and deferred Post-to-Concert subject callback; those are
not yet frozen or implemented here. Auth/Friend views or a root actor assertion
cannot attest local relationship restrictions for unknown scan authors. The
qualified protocol remains pending outside the published adapter checkpoint.

These drafts are not part of the 19 passing Rust cases or the live acceptance
claim. Export, combined compilation, guarded/native/HTTP qualification and final
publication remain required after the real producer implementations land.
