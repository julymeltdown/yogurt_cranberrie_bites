# Coherent mains round 0035: guard failure before Cargo

Status: `FAILED_BEFORE_CARGO`; both initial samplers failed. No build,
executable copy, application test, service, database or browser ran in this round.
New Concert/Gateway mains and managed-runtime qualification remain `NOT_RUN`.

The frozen application source is `1c8f87675d73f9cfb932beae1ff56a50851a498a`.
The finite offer was `ROOT-COHERENT-CONCERT-GATEWAY-MAINS01-20261105T0035Z`.
Actual acceptance was Unix epoch `1793839036.078345`; actual target and full
release completed at `1793839037.3330271`. Original fixed clocks were not reset.
The immutable full-release receipt SHA-256 is
`ba58745b57717db3d732a5c524002a72706bba2881910622d2e18483672259ed`.
Its exit code is 1 and failure is `RuntimeError: global workflow cap`.

Both initial guards refused before the builder started. They grouped foreign
process groups as workflows, although a controller, independent guardian and
owner inspection helper can use separate process groups in one workflow.
The guard additionally reserved an old PI05 source workflow while PI05 had
already accepted its native round. This classification is unqualified; the
failed sampling did not preserve its complete census before throwing. Later
read-only process metadata supports these defects but cannot reconstruct a
qualified global-capacity sample at the failed acceptance instant.

Pre-acceptance accounting measured the complete shared target at 19,626,471,424
bytes and fresh free space at 15,463,849,984 bytes. Root's 40 GiB retained-carry
floor includes its own complete historical roots and conservative non-target
bound. It is not a 40 GiB measurement of the shared target or a blanket carry
floor for a different owner's footprint. The full 2 GiB growth allowance fit
within this round's 48 GiB WHOLE cap before acceptance. No Cargo target growth
was produced.

Cleanup reaped the guardian, found no remaining target handles, preserved the
clean frozen source and released workflow, owned-child and remaining-growth
claims to zero. Literal release was sent directly to Auth, Post and the allocator
before this documentation. No guard or resource pass is claimed.

The round is permanently closed. A separate retry requires a fresh finite
offer and literal predecessor release. Drafted guard changes group actual
controller ancestry and output-root identity, retain accepted workflows despite
missing PIDs, and persist diagnostics before refusal. Those changes remain
unexecuted and must pass complete initial samples in both guards before Cargo.
The retry does not change the application source or authorize runtime work.
