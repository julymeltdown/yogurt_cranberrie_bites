# Canonical contract boundary for the managed runtime

Status: source intake complete; new mains and actual runtime `NOT_RUN`.

The compiled Concert source `7d74b7f` and Gateway source `1ab1cc5` carry different
contract snapshots. Neither snapshot matches the current published frontend's
domain and realtime boundary. The qualified managed-profile correction therefore
refuses to identify the backend helper revision as a published frontend candidate.

The Product/API integrator copied the exact published contract snapshot at
`d58d1b24de5bb2458fa1a479f266f7eb40234d23` into its existing isolated checkout.
The C07 producer's qualification remains the published `83f55f9` result; this
intake neither reruns nor relabels that qualification. The active pin remains
`147be6549c1f4c1d1307415ff2350b67bc2bdeace47f503050bfdf4da66198c5`.
All 243 pinned filesystem inputs now match their published hashes.

Commit `30c1dcdc2` contains 45 server/shared contract, fixture, generator and pin
paths. Separate commit `537149338d5407ab1b8052385502c053b1b634f7` contains the one
already-published generated frontend contract type required by the same pin.
No frontend application implementation or supplied source archive was changed.
The adjacent JSON records each source revision, destination path, mode, immutable
blob, before/after hash and dependency boundary.

The before-copy projection was 87,879,680 bytes, including complete postimages,
Git/index, directory and proof allowances, within the distinct 128 MiB ordinary
preparation booking. Actual positive postimage allocation was 61,665,280 bytes;
overwritten files were not credited as reclaimed space. Earlier preparation
budgets and retained WHOLE history were preserved. No target was read, and no
compiler, generator, service, database or browser was launched by this intake.

The full scoped Git whitespace check exited 2 on exactly the already-published
`contracts/generated/api-client/THIRD_PARTY_NOTICES.txt:22` EOF blank line.
Its canonical bytes are preserved. This historical artifact exception is neither
a fresh generator defect nor a full diff-check pass.

Next: preserve the actually qualified Gateway Enrollment adapter/mount closure,
freeze the complete source and include inputs, and build fresh actual Concert
and Gateway mains under a separate finite allocation. Only their actual serving
contract boundary can qualify the managed profile against the selected published
App. Enrollment, signed roles, Settings, three-principal attachment, restart,
default App and all multiuser journeys remain `NOT_RUN`.
