# BE-PARTICIPANT-LIVE

Status: IN_PROGRESS. This extends Phase A at `0313afa5`; it does not replace its
bounded CONTRACT_READY assessment or accept BE07/IN01.

## Ownership and source

The participant owner writes `src/participant/**`, participant RPC definitions,
dedicated participant tests/harnesses, and this evidence. Resource SL assigned
only `migrations/0027_participant_profile_coordinator.sql` as an additional shared
path; World retains 0026. Product startup loads the migration directory through
migration-runtime, without a numbered compile-time registry. The root composer
retains startup, routing, manifests and readiness probes. Canonical avatar
application/repository transaction hooks remain assigned to their original owner.

The current Auth adapter calls the existing Auth-owned
`AuthConcertRelationshipService.ReadCurrent` with a dedicated Concert read key,
explicit workload credentials, exact self actor/event/session/request binding,
SAFETY_SELF purpose, and the original two-second wall/monotonic deadline. It
requires a fresh exact no-store owner response and active account/session facts.
It implements no mutation capability. Network fixtures exercise the real guards
and client transport, but deliberately synthetic owner replies are not native
Auth or public-route acceptance.

The participant context retains the platform-sealed original parent when the
platform captures it. It checks the exact RPC literal, actor and complete unary
protobuf. Access for live guard acquisition refuses a missing parent. The guard
owner must add the three exact Gateway participant RPC literals to parent capture;
this participant patch does not modify platform allowlists.

The full C03 parser preserves absent versus explicit-null bio, permits selected
profile fields and/or preferences, and rejects duplicate and unknown properties.
It is not yet wired into the preference-only Phase A handler. No public header
or expected_revision key is changed.

## Cross-store protocol under implementation

The Profile SELF owner supplies canonical content plus epoch/revision, prepares
only selected nickname/bio fields, and retains the actual prepared transaction.
Its deterministic GID is `mcr_sp_` plus the request UUID's lowercase simple form.
The planned effect digest is SHA256 over ASCII `mcr profile self effect v1\0`,
the 32-byte guard-binding digest, the 16-byte operation UUID, the 16-byte Profile
epoch, its big-endian i64 revision, and SHA256 of the exact raw PATCH body.

Migration 0027 is a draft durable operation registry. It fixes the original
binding, expected participant revision, Profile version/GID/effect, and actual
Concert transaction identity before either store's protected effect. Only the
actual Concert participant CAS transaction may write its COMMIT decision.
Profile reconciliation must independently query the authoritative committed
decision. A registered aborted Concert transaction can establish ABORT; timeout,
absence, an unknown transaction, or a caller boolean cannot. Unknowns retain
prepared effects and owner exclusion. Journal/recovery code, settlement RPC,
native crash/race proofs, and cross-store read visibility remain unfinished.

## Validation

New live code: formatting and `git diff --check` PASS. Compile, focused tests,
0027 SQL/native migration, actual live owner mutation, root/Gateway composition,
browser and full signed-in multiuser acceptance: NOT_RUN at this source checkpoint.
Phase A validation in `participant-producer.md` does not qualify these new changes.

Required completion remains implementation, meaningful native and public-route
validation, committed non-force delivery, documentation, and concrete review fixes.
