# Canonical Concert self Profile producer

Status: IN_PROGRESS. Base: `8f89d1905a86143d34ff77d2951fc60651ab2497`.
BE-PROFILE-SELF owns only newly assigned Profile SELF files and the explicitly
allocated new migration `0008_concert_self_profile.sql`. Public Profile retains
all four existing registration files; ParticipantLive owns client/orchestration;
canonical authority owns actual Auth/Friend guard and settlement contracts.

The canonical stored content remains `profiles.nickname` and `profiles.bio`.
Display names are 1..40 Unicode scalars, nonblank and control-free; bio is at most
160 scalars and control-free. Omitted bio preserves it; explicit null clears it;
empty stored bio projects as absent. Username, avatar, counts and privacy do not
belong to this producer. Private self content is not a public profile DTO.

The dedicated `profile.self.v1.ConcertSelfProfileService` takes both actual owner
GuardView capabilities and exact GuardBinding. No bounded Auth observation is
converted into mutation authority. PrepareUpdate consumes exact signed PATCH
bytes and derives selected content fields. `operation_id` is the existing request
UUID; no browser idempotency header is added. Profile epoch/revision is internal
and distinct from the C03 event participant `expected_revision`.

Cross-store writes require durable preparation, participant CAS and an actual
coordinator decision. A proposed value is not committed content. Unknown native
transaction outcomes retain exclusion and prepared effects; deadline expiry is
not permission to release them or guess rollback. ReadOperation and Reconcile
cannot originate another mutation. Full coordinator/guard interface and native
qualification are in progress; nothing here certifies a live PATCH flow yet.

Source authored for this task; no rendering or dependency upgrade. Validation,
registration, migrations and native multi-store checks: NOT_RUN.
