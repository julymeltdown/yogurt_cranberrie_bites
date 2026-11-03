# Combined world rooms and ticket producer checkpoint

The signed Gateway rooms/ticket routes and Concert RPC producer are integrated at
`5baf5bf1c5d90b358cde8ac5732377b4970c92ad`. The four-commit author range
`cc3bb84046cc96388c4e34186b4eae399ec02642..3a9ba8615290ca3d3f135b85dda0c4280af52aa6`
changes exactly **24 paths**. Migration `0005_world_admission.sql` is an unchanged
prerequisite, not an additional changed path. All incoming bytes match the
stopped author's clean handoff; no newer published startup changes were replaced.
The separate runtime worker's pending checkout remains untouched.

The internal protobuf, generated-code build registrations, signed request expiry
fields, Gateway client/state/routes and Concert service registration move together.
They use the existing Concert endpoint and current canonical authority; no second
endpoint, public WebSocket implementation or new frontend implementation is added.

Fresh combined validation used Rust 1.88.0, the retained lockfile, offline Cargo,
two build jobs, and disabled development/test debuginfo. The author explicitly
released the existing backend target before this run; BE26's distinct target was
not used. No package installation or lockfile change occurred.

| Check | Result |
| --- | --- |
| Gateway `concert_identity_composition` | Exit 0, 21 passed, 0 failed |
| Concert `world_rpc_authority` | Exit 0, 4 passed, 0 failed |
| Both focused no-run JSON artifact commands | Exit 0; exact executables copied and hashed |
| Backend and contract input stability | 1,454 tracked file hashes unchanged during qualification |
| Authored input equality | All 24 changed paths and 13 native receipt source bindings match |

Full commands, exits, log hashes and binary hashes are in
[the qualification record](world-producer-combined.json). Raw logs, all input pins,
author executable copies and the original native receipt are retained under
`/private/tmp/mcr-world-producer-combined-3a9ba86-20261104/`. These copies preserve
the previous evidence before any reuse of the mutable build target.

The author's independent source-bound PostgreSQL 16.14 admission gate passed four
native tests, actual restart persistence, expected migration-reapply refusal and
cleanup. Its receipt labels `f745169` as the last prior commit, while explicit
hashes bind the final tested inputs. This report preserves that distinction and
does not relabel it a new combined database run. The unchanged admission sources
and migration match those hashes. The former full Gateway suite's two
Docker-dependent environmental failures remain separate; no full-suite pass is
claimed.

This is an integrated producer checkpoint. Public WebSocket handshake, deployed
Gateway/Concert/database journey, two-account World, BE12 simulation, BE13 privacy
projection, full SQLx startup ledger, load, IN01 and release acceptance remain
**NOT_RUN** here. The author performed the implementation review; Main checked
scope, evidence binding and combined execution without adding another approval
round.
