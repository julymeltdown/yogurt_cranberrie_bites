# Intro native resolver integration

Status: IN_PROGRESS. Source is implemented; fresh combined native qualification
is NOT_RUN. This does not activate a Product route or complete IN03.

`config/native_resolver.rs` selects the existing Intro native witness adapter for
exactly the six operations accepted by `intro_live/protocol.rs`. Parent RPC,
operation, event, method and target must match. List queries also pass the actual
closed Intro query validator. Wrong method, foreign operation, unsupported or
duplicate query keys, invalid limit, bad encoding and noncanonical detail IDs
cannot select the committed witness adapter. The selected adapter still checks
the exact original transaction and durable witness; UNKNOWN/error never tries a
second adapter. All concrete adapters use the same configured pool and instance.

The existing Intro canonical fixture now uses the actual union for its
Auth/Friend-authenticated settlement service and exact native-witness case. Its
Concert store uses `mcr_concert_intro`, within the unchanged dedicated-store
policy. No production pool admission is widened. The seven native tests retain
real canonical fencing, publication/privacy behavior and expected UNKNOWN
quarantine. The native-witness case checks committed witnesses, foreign instance,
wrong witness, in-progress, aborted and unwitnessed committed SQL. The existing
union route table includes all six Intro routes and cross-domain relabeling.

Root retains the resolver union. CF02 owns only canonical and Participant factory
files in its isolated worktree. Production mounting, shared runtime supervision,
real HTTP and the complete IN03 journey remain open.
