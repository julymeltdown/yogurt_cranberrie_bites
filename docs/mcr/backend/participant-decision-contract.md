# Participant decision query contract

Status: CONTRACT_READY for the additive internal IDL only. No live coordinator,
Profile prepare/reconcile, native decision service, or public route is accepted.

Source: participant-owned protocol checkpoint `3bb9c295`, isolated onto published
master without its unqualified Rust implementation, SQL migration or avatar hook.
Only `backend/proto/concert/v1/participant.proto` and this evidence are delivered.
The existing three participant methods and their field numbers are unchanged.
There are no new dependencies, imports, public HTTP headers or browser DTOs.

New method: `/concert.participant.v1.ConcertParticipantDecisionService/ResolveDecision`.
Issuer/caller is Profile, audience Concert, actor protobuf path `[1]`; the exact
policy reservation and dedicated shared key helpers were compiled and published
by the platform owner in `888cb6bd`. Gateway and parity keys are disallowed by the
dedicated verifier. The actual server remains separately owned participant work.

The ten-field query binds actor, event, session family, operation UUID, original
parent/body/guard-binding digests, deterministic Profile prepared GID, Profile
effect digest and bounded query expiry. Responses echo that whole query and carry
UNRESOLVED, COMMIT or ABORT plus the registered Concert native transaction identity.
Only COMMIT has a nonzero committed participant revision. A fresh recovery query
may inspect an existing exact durable operation; it cannot create or decide one.
Unknown status, timeout, missing row or caller assertions never establish ABORT.

Validation: vendored protoc 3.2.0 invocation with the current proto root,
`--descriptor_set_out`, and `--include_imports`: exit 0. `git diff --check`: exit 0.
IDL SHA256: `8f5df61f1cdce1ed7e386da17a7ed8e3ea914eb24a117a39a85ec17cae26d6b9`.
Descriptor SHA256: `639b449a2f27eee0d221cef74435afd14cc1389f6b77a1e91a445130cf7ae2e0`.
The source and destination IDL bytes match the stated source checkpoint.

Rust client/server compilation, native resolver execution, response delivery,
owner-store races, crash recovery and signed-in participant flows are NOT_RUN
for this contract-only delivery. They remain required in BE-PARTICIPANT-LIVE.
