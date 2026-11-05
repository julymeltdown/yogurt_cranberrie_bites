# IN04 Root managed profile

Status: `IN_PROGRESS`. The real seven-service flow, operator policy execution,
role application, Settings HTTP, Wall restart and App convergence are `NOT_RUN`.
Source and protocol checks below cannot accept those runtime gates.

The Root profile is `backend/scripts/mcr/in04_wall_runtime/launch.py`. Run it as
`python3 -B -m in04_wall_runtime.launch` with `backend/scripts/mcr` on `PYTHONPATH`,
the existing finite launcher allocation arguments, the actual seven-service
`--inputs`, and a private `--in04-managed-inputs` file. Import starts no job.

The closed managed file has six fields:

```json
{
  "version": 1,
  "frontend_candidate": "<full published frontend commit; independent of helper/backend revisions>",
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

The frontend candidate must be an exact commit in the recorded `origin/master`
history with a frontend package. Its domain and realtime contract bytes must
equal the backend/helper contract source. Contract drift fails closed; a newer
frontend does not relabel an older backend. The attachment's candidate and fault
bindings use this frontend revision. The private descriptor keeps
`source.helperRevision`, per-service `backendRevisions` and `source.frontendRevision`
separately. The actual frontend builder supplies `applicationRelease` only when it
builds and starts the selected published App; the backend never synthesizes it.

The revision-separation correction is source-qualified at
`d47d36ec5ec37cf921b597cc7af6d30885c5edb2`. Under the distinct finite
`ROOT-IN04-FRONTEND-BINDING-SOURCE01-20261105T0012Z` allocation, seven real-local-Git
binding cases, the new attachment mismatch case and the changed attachment
construction case passed: nine tests, exit 0. Five changed Python files and three
private controls passed AST parsing; private JSON and immutable pins passed.
These checks start no product service and certify no real frontend deployment.

```sh
PYTHONPATH=backend/scripts/mcr python3 -B -m unittest -v \
  in04_wall_runtime.test_frontend_binding \
  in04_wall_runtime.test_attachment.AttachmentCases.test_actual_objects_sessions_and_observations_precede_descriptors \
  in04_wall_runtime.test_attachment.AttachmentCases.test_frontend_candidate_cannot_be_replaced_by_a_helper_revision
```

Retained test-log SHA256: `f99c0aa50bf10db138cc8f9e3b18704da39c75d1a1ad6f10578aa182eef60b10`.
Full-release SHA256: `564fa4b3569d93319fd9f63198e21802b7d683896f8b492c40f109c08a94ffbd`.

Both guards were live and fully sampled before tests. Actual acceptance was
`1793837614.767688`; actual full release was `1793837617.510829`. Both guards
reported no failure; all children, workflows, source jobs and remaining growth
were zero, with no target access. The allocator and PI01 successor received
literal release before this evidence update. Peaks: 7 processes, 103,268,352 bytes
RSS, 409,600 bytes NEW, 34,435,354,624 bytes WHOLE; minimum free space
16,490,512,384 bytes. The retained local receipt directory is identified by
basename `mcr-root-in04-managed-sfcb4p33`; machine-local paths are not published.

The current canonical frontend contract boundary at `8b47e0e40e8182d2401565fbb079533efffb41e7`
differs from the compiled Concert `7d74b7f` domain and realtime contracts. The new
gate correctly requires a coherent producer before runtime admission. The new
Auth19 main compilation closes only its own missing executable input; genuine
Enrollment, roles, Settings, private attachment and IN04 remain `NOT_RUN`.

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
