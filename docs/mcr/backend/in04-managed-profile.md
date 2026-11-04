# IN04 Root managed profile

Status: `IN_PROGRESS`. The real seven-service flow, operator policy execution,
role application, Settings HTTP, Wall restart and App convergence are `NOT_RUN`.
Source and protocol checks below cannot accept those runtime gates.

The Root profile is `backend/scripts/mcr/in04_wall_runtime/launch.py`. Run it as
`python3 -B -m in04_wall_runtime.launch` with `backend/scripts/mcr` on `PYTHONPATH`,
the existing finite launcher allocation arguments, the actual seven-service
`--inputs`, and a private `--in04-managed-inputs` file. Import starts no job.

The closed managed file has five fields:

```json
{
  "version": 1,
  "environment_id": "<fresh canonical nonnil UUID>",
  "window_id": "<one named finite managed window>",
  "operator_packet": {"path": "<absolute private operator receipt>", "sha256": "<exact receipt SHA-256>"},
  "restart_stage": "POSTGRES_CONCERT_AFTER_BASELINE"
}
```

Use `NONE` for a window that does not perform the IN04 PostgreSQL/Concert restart.
The file and operator packet must be owned, private regular files. The actual
operator executable, source, compiled receipt and Enrollment helper are pinned;
an arbitrary executable or a qualification label cannot replace them.

The profile installs the actual Enrollment mail sink before Auth starts. The
existing policy operator publishes only an isolated synthetic policy into the
same owned Auth database; the real public Enrollment/verification/login/consent
flow creates all three identities. Bearers remain in RAM. The real managed role
issuer supplies an initially empty signed-publication file before Concert starts,
then grants only the third actual principal OPERATOR. Public OwnAccess must show
that operator and two genuine members with null access; public Settings must
return the same event. No role/identity fixture SQL is used.

The public mural snapshot operation creates its repository checkpoint. Root
observes exact persisted equality to the current mural before attaching controls.
The concrete deployment closure checks actual pinned executable identities,
source/helper bytes, retained children, current controlled replacement environment,
role publication, migration package and private input custody. It permits only the
three actual owner-restart UUID changes and requires World owner/consumer equality.

The baseline handshake leaves the public RuntimeQualificationPort unchanged.
After its initial real generation observation, the adapter sends exactly one
private `restart-baseline` observation notification. The observer verifies the
same current generation and binding and records it only when explicitly prearmed.
It has no restart method. Unarmed, stale, foreign and duplicate notifications fail
closed. The separate guarded Root profile loop consumes the record, independently
reobserves the incumbent control generation, and queues its own PostgreSQL/Concert
restart. The existing owner controller performs it on a later loop iteration.
There is no setup timer or retry after an uncertain notification.

The initial managed attachment is the owner's 39-case qualified source at
`75809d0c64b0a602f93d7c837495891a1873f1cb`, with final documentation
`d4300acb5af2104b171dcdf78f2736e02af660e0`. The optional Root stage hooks are the separately qualified five-case follow-up
`de79265714c3af94d5f4e4b7ece13d8e8a416f7a`; actual runtime remains NOT_RUN.
The Node adapter's changed bytes require a new frontend embedded artifact and SHA;
the historical adapter SHA remains historical.

The new 7 baseline and 5 independent owner-stage Python cases passed at source
`7d74b7fbf397688f3ae41dd7fa5347d3bcc50aa4`. The first Node protocol
attempt stopped before assertions because its allocation TMPDIR exceeded the macOS
Unix socket path limit. A separate finite retry used a short private TMPDIR and
passed the Node protocol without replaying the Python cases. Both allocations
released all resources; the failed receipt remains preserved. These checks use
synthetic observations/controls and qualify no real provisioning or restart.

The qualified adapter source SHA-256 is
`40fddf11e1520452b73966c8043939cbc35d65ced0e2935515018ab1051cf350`.
The frontend must rebuild its embedded artifact from these bytes. See
[in04-managed-profile-qualification.json](in04-managed-profile-qualification.json)
for exact commands, exit codes, source hashes, failed-attempt provenance and
resource release. Next qualify and run the coherent serving binaries, exact
helper inventory and finite same-stack cohort.
The original Root2040 accounting failure, Root2050 missing-interface failure and
Root2100 three Settings type errors remain preserved in their original receipts.
The Settings source correction is owner `55f58fe3cf9cc2745df24c72d53a0cc5f118f49e`;
its combined compilation and actual Settings HTTP are still `NOT_RUN`.
