# Participant native effect witness

Status: CONTRACT_READY for the frozen ABI; implementation/native qualification
of the new repository hook and settlement resolver is NOT_RUN.

Root explicitly assigned migration
`backend/services/concert-service/migrations/0035_participant_effect_witness.sql`
and participant-owned resolver/tests to the Participant owner. The Avatar owner
alone edits the application trait, avatar repository, existing avatar tests and
avatar documentation. Frozen migration SHA256:
`113900167fdc2806a9f5126c508b8ae737bcc9ca718ce7dbf408e913d1c3bebc`.

`AvatarEffectFence::witness_binding()` returns immutable correlation, never
authority or a commit assertion. Its default is Unavailable. The descriptor has
four UUIDs (`producer_instance_id`, `scope_id`, `operation_id`,
`session_family_id`), four exact nonzero `[u8;32]` digests (`parent_digest`,
`guard_binding_digest`, `body_digest`, `effect_binding_digest`), and a 64 lowercase
hex `release_digest` equal to the execute-time release. The held sealed scope
supplies these values. No boolean, new scope, SQLx type or private Profile content
crosses the application boundary.

The repository captures the actual writer tuple and registers/confirms it with
the existing held scope before effects. It validates the descriptor once before
effects. The actual successful UPDATE is followed by the witness INSERT on that
same SQLx transaction, with authority/fence checks before and after the await and
immediately before commit. Every successful avatar or preferences mutation
advances revision, including identical values, and requires a witness. Reads and
explicit synthetic `None` fences produce none. No ON CONFLICT success shortcut.

The INSERT has exactly these 20 bound parameters, in this order:

| Position | Column | Rust / SQL type |
| --- | --- | --- |
| 1 | operation_id | UUID / uuid |
| 2 | actor_id | actual repository user UUID / uuid |
| 3 | event_id | actual repository event UUID / uuid |
| 4 | session_family_id | UUID / uuid |
| 5 | producer_instance_id | UUID / uuid |
| 6 | scope_id | UUID / uuid |
| 7 | parent_digest | exact 32 bytes / bytea |
| 8 | guard_binding_digest | exact 32 bytes / bytea |
| 9 | body_digest | exact 32 bytes / bytea |
| 10 | release_digest | 64 lowercase hex String / text |
| 11 | effect_binding_digest | exact 32 bytes / bytea |
| 12 | identity_digest | exact 32 bytes / bytea |
| 13 | mutation_kind | `AVATAR` or `PREFERENCES` / text |
| 14 | expected_revision | actual validated expected i64 / bigint |
| 15 | committed_revision | actual resulting i64 = expected + 1 / bigint |
| 16 | concert_system_identifier | actual canonical decimal String / text |
| 17 | concert_database_oid | actual u32 bound as i64 / bigint |
| 18 | concert_full_xid | actual canonical decimal String / text |
| 19 | concert_backend_pid | actual i32 / integer |
| 20 | concert_backend_started | actual DateTime UTC / timestamptz |

`created_at` uses the database default. The table is
`public.concert_participant_effect_witnesses`. Identity hashing uses the existing
`platform::actor_delegation::owner_binding::identity_digest` with producer
`CONCERT`, the actual five-part writer tuple, the descriptor effect digest,
producer instance UUID, and `operation_id.as_bytes()` as settlement reference.
The scope UUID is joined separately and does not change that canonical recipe.

The participant avatar effect recipe remains SHA256 of the concatenation:
`mcr participant avatar effect v1\0`, guard digest (32 bytes), release digest
(64 ASCII bytes), raw body SHA256 (32 bytes). It is distinct from the guard
digest. Both avatar and preferences use this recipe. The Profile coordinator
keeps its existing separate expected Profile effect recipe and 0027 journal.

0035 joins the immutable exact root scope/guard and registered identity through
foreign keys. Its INSERT trigger requires the actual executing native tuple,
published ACTIVE scope, confirmed owner tracks, exact immutable root binding and
transaction identity, and target revision/xmin from an actual same-transaction
write. UPDATE and DELETE are refused. Root fixture rows in a component test do
not establish real owner authority. A settlement adapter must still check the
configured actual store, native commit status, full identity and all original
request bindings; raw native COMMITTED without this witness remains UNKNOWN.

This is a C03-specific contract, not a generic receipt or a Social/Post witness.
A pre-effect insertion in an authority check cannot prove a successful domain
effect and must not replace the repository hook.
