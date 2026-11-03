# Auth/Friend owner observation integration — 2026-11-04

The previously qualified owner-observation handoff is integrated in the six
commits ending at `d49294ae8073d291d28bf01eebafacea703f310c`, following
`bcb0428d01db2bdfa9970c4cb56cf2d53a45ab67`. The stopped author's clean delivery is
`2ec7ee6adf4d9ab27d89e59af1a37db1e7aa70f8`; its native-qualified code is
`dc7b67e41725a7703d20a1b4571db6a77901c5ca`. Only the six owner commits were
replayed; their already-delivered prerequisite commits were excluded.

All 37 changed paths are backend code/tests or the assigned backend handoff.
Thirty-five files match the author byte for byte. Two additive integration
resolutions preserve existing behavior: Concert build generation retains the
world/post protocols and adds the relationship protocol; the outbound adapter
registry retains existing media modules and adds relationship authority.
All 32 qualified source pins and five immutable author receipts match.

Fresh combined `python3 backend/scripts/mcr/test_owner_relationship_smoke.py`
passes all nine harness tests (exit 0), and `git diff --check` passes (exit 0).
The author's evidence records 118 platform tests, 17 focused tests and three
executed PostgreSQL checks, including owner restart and cleanup. Its native
report SHA-256 is
`9e59b42c4317afe5cecd01bda12ca9f24ba528de83a731a841c31cf30b021072`.
These results remain bound to that author's source and binaries. A fresh
combined Rust build is **NOT_RUN**, queued for a released existing build target;
this integration does not relabel historical binaries as current ones.

The implementation supplies request-bound observations through dedicated
Concert read proofs and independent Auth/Friend revisions. It does not provide
distributed revocation fencing, private-handle authority or protected mutation
permission. Live-world use additionally needs a sealed consumed-ticket/current-
membership binding and an Auth-only signed client. That follow-up reuses this
existing Auth ReadCurrent service; the abandoned separate ReadWorldSession RPC
WIP is explicitly excluded. No task or real multiuser acceptance is advanced.

See [the author handoff](relationship-authority.md) for the protocol, migrations,
commands, limitations and source provenance.
