# World producer continuation

All three former writers were verified terminal before new isolated checkouts
were created at `1b7caec`. Existing selected models are retained. Communication
is English only. No additional Main or scrum-lead review gate is introduced.

| Chat | New checkout suffix | Assigned implementation |
| --- | --- | --- |
| `01a1a1fd-6b5d-75b0-8d42-01ade79b9393` | `be-world-projection-context` | Sealed projection context, inherited deadline/cancellation, per-owner attempts and typed relationship binding |
| `01a19f83-12cc-7f21-8ad3-55749f012d9b` | `be-world-profile-producer` | Dedicated Concert-only minimal current Profile RPC and Concert client |
| `01a1a21a-a088-7c31-83e3-19de119ba873` | `be-world-recipient-producer` | Current Auth/Friend Presence observations plus local recipient policy |

Each checkout lives under the existing `mcr-worktrees` workspace and uses the
matching `codex/mcr-…` branch. Old checkouts remain read-only. Each owner must
inspect its actual goal, reuse aligned unfinished work, and only create a new
unbudgeted goal when none remains unfinished. A blocked legacy goal with no
resume API is reported honestly while explicitly authorized manual work proceeds.
Completion requires implementation, relevant native validation, documentation,
clean committed handoff and concrete fixes; a sent assignment is not goal proof.

The context owner exclusively owns
`backend/services/concert-service/src/adapters/out/relationship_authority/world_auth.rs`,
its tests, new `world_projection.rs`/support, and only the new module export and
sealed `RequestBinding::from_world` conversion in the parent module. Its lane
document is `docs/mcr/backend/world-projection-context.md`.

The Profile owner owns new `profile-service/.../grpc/world_identity.rs`,
`profile-service/tests/world_identity_rpc.rs` and its support, new Concert
`adapters/out/world_profile.rs` and support, and its lane document. Exact shared
grants cover adding the dedicated `profile.v1.ConcertWorldProfileService.ReadCurrent`
to the existing Profile proto; generation and module exports in the two services;
registration in Profile main; optional Concert caller configuration; the exact
method-policy admission and dedicated signer/verifier Profile-audience matches
in platform. Existing Gateway-only Profile methods retain their policies. A
cycle-free Profile service dev-dependency may be added to Concert with only its
corresponding lockfile dependency edge; all retained resolutions/checksums stay
fixed. These shared files have one writer, the Profile owner.

The recipient owner writes only new Concert `world/recipient_owners.rs`/support,
its dedicated tests and lane document, plus the module export in `world/mod.rs`.
It does not edit runtime, Auth context, Profile, shared policies or manifests.

All reads must inherit the original absolute and monotonic stop, shared
cancellation and immutable credential/source expiry. Self Profile means
actor equals subject; recipient scope additionally binds an authoritative
server-owned candidate membership. No browser identity constructor, renewed
two-second budget, packed revision vector or local observation substitutes for
a distributed publication fence. The runtime writer remains stopped until
concrete qualified producer interfaces and integration ownership are available.

BE41 released the existing `media-combined-fa4b4b3/target` after native checks and
fixture cleanup. Main checked that no process retained it and granted it only
to the context owner. BE27 retains `mcr-build/backend`; Profile and recipient
proceed with source/fixtures while awaiting an actual release. No third target
or installation is authorized. WorldSL coordinates exact signatures and source
pins, with no approval queue or repeated unchanged status reports.
