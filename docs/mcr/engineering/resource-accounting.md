# Bounded filesystem resource accounting (TOOLS-FS-01)

This stdlib-only helper gives existing controllers a complete allocated-block
measurement without treating ordinary child rename/removal as an immediate fatal
error. It does not allocate capacity, replace controller ownership, certify product
runtime behavior, or add an approval gate. Controller owners adopt it explicitly.

## API and custody

Load `scripts/mcr/resource-accounting/accounting.py` with the controller's normal
module loader. Capture `RootIdentity` for each declared scope while holding existing
custody. `collect(roots, deadline=absolute_monotonic_deadline, checkpoint=live_guard)`
returns `complete`, allocated `bytes`, per-scope complete samples, and attempts.
Allocated bytes are `st_blocks * 512`; `(st_dev, st_ino)` is counted once across all
roots, including overlapping directories and cross-scope hardlinks. Declaration
order assigns a shared inode to its first scope. Keep that order stable for the
ledger's lifetime. Scope attribution can change when files move; retained maxima
are conservative and may over-reserve, never provide deletion credit.

Descendant symlinks are counted as symlink inodes and are not followed. A retained
external tree must be an explicit scope. Unexpected root symlinks are rejected.
For an existing legitimate read-only alias, use
`RootIdentity.capture(alias, read_only_target=declared_real_directory)`. This pins
both the link and target identities and exact link text; it grants no write rights.
The caller still verifies read-only custody, ancestor/path ownership, source HEAD,
readset hashes, target handles, exclusive-use offers and forbidden entrypoints.
A root alias accounts for the target tree; include the alias's parent scope if its
link inode also belongs in the accounting set. No manifest, dependency installation,
link creation or shared controller is changed by importing the helper.

## Completeness and uncertainty

Every attempt opens pinned directories with `O_NOFOLLOW`, compares opened identity,
walks without following child symlinks and closes descriptors on every exit.
Child `ENOENT` records the exact observed path/errno and known subtotal, then starts
a completely fresh scan. Defaults are three attempts and 30 ms between attempts;
at most five attempts and 50 ms are accepted. The caller supplies one absolute
monotonic deadline; retries do not renew it. Entry, depth and race-evidence bounds
also fail closed. The live checkpoint runs throughout traversal and retry waits.

Only a fresh attempt with no unresolved missing entries returns complete samples.
Persistent churn raises `AccountingError`, retaining every incomplete attempt's
`knownBytes`, scopes and races. EACCES, EIO, unknown errors, root loss/replacement,
timeout, bound exhaustion and a failing checkpoint cannot become success, zero
usage or free capacity. A successful filesystem walk is a sequential observation,
not an atomic filesystem snapshot. Keep both guards running and retain conservative
growth reservations for writes between samples. Continuous unresolvable churn may
still require a stop; the helper does not hide an unsafe underscan behind a prior
successful sample.

`complete_allocated` is a compatibility adapter for existing single-root `du -sk`
controllers. It accepts only one successful numeric total with the exact root and
empty stderr. It retries only exit 1 where every nonempty diagnostic names a child
of that declared root and ends with the C-locale ENOENT diagnostic. It preserves
raw stdout/stderr/exit/PID/reaping details, but never admits a failed subtotal.
It terminates/reaps its owned measurement child on timeout or live-check failure.
Use `collect` for global inode deduplication across multiple roots; summing separate
`du` results does not establish that property. Output is bounded to 64 KiB of
retained diagnostics; a producer that exceeds it is a fatal diagnostic overflow.

## Conservative ledger and owner integration

Capture a complete baseline before work. Initialize `Carry` with those stable
scope keys, previous retained NEW, previous WHOLE high water and any conservative
reservation. Pass each complete `collect(...)["scopes"]` to `Carry.observe`.
NEW retains previous usage plus the sum of each scope's positive high-water growth;
WHOLE retains its previous maximum and the current deduplicated union plus
reservation. Neither deletion nor smaller subsequent samples reduces a retained
maximum. An incomplete sample is rejected. On an accounting exception, preserve the
receipt, stop owned work and run the owner's existing bounded cleanup path.

The importing controller keeps its existing numeric caps and admission calculation:
fresh free space, all outstanding reservations, worst-case new growth, fixed work
and FULL deadlines, PID/RSS closure, independent guardian readiness before children,
private process groups, source/custody checks and actual child reaping. Include
controller, guardian, measurement and test descendants in live limits. Throttle
expensive live measurements to a short bounded interval inside the checkpoint;
do not spawn a process per filesystem entry. `limit_reason` is a small example for
existing free/NEW/WHOLE/process/RSS/deadline branches, not an allocator replacement.
Any guard uncertainty must stop work rather than release capacity early.

For an existing owner controller, the integration point is small (substitute its
already owned roots, pinned source checkout and unchanged live check):

```python
import importlib.util
import sys

spec = importlib.util.spec_from_file_location(
    "mcr_resource_accounting", source_root / "scripts/mcr/resource-accounting/accounting.py")
accounting = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = accounting
spec.loader.exec_module(accounting)

roots = {name: accounting.RootIdentity.capture(path) for name, path in owned_scopes.items()}
baseline = accounting.collect(roots, deadline=fixed_work_deadline, checkpoint=live_check)
ledger = accounting.Carry(
    {name: value["bytes"] for name, value in baseline["scopes"].items()},
    retained_new=previous_new_highwater, prior_whole=previous_whole_highwater,
    prior_new=previous_new_highwater, reservation=remaining_growth_reservation)
try:
    snapshot = accounting.collect(roots, deadline=fixed_scan_deadline, checkpoint=live_check)
    charged = ledger.observe(snapshot["scopes"])
    enforce_existing_new_and_whole_caps(charged)
except accounting.AccountingError as error:
    preserve_failure(error.receipt())
    stop_owned_work_and_run_existing_cleanup()
    raise
```

The example's baseline must also stay inside the owner's failure/cleanup handler.
`fixed_scan_deadline` cannot exceed the original fixed work deadline. Start the
independent guardian before workload children; it measures independently with the
same frozen custody/readset and unchanged cap values. It must not share a mutable
ledger with the primary process.

## Provenance and functional qualification

`docs/mcr/evidence/resource-accounting/provenance.json` pins the inspected source
and historical distinctions. Root2040 retained a RuntimeError class without the
underlying diagnostic: ENOENT there is an inference. BE38's complete fresh scans
and Root2050's null guardian failure are distinct from QA07's private Chrome-TMP
partial-subtotal rule. That private rule is not authoritative whole-tree policy.
Root2050's compiler failure is a separate implementation issue, not a guard pass
for this helper or a reason to replay SQL/browser/product journeys.

The new tests cover fresh retry, persistent disappearance, raw diagnostics,
permission/I/O/unknown failures, root custody, explicit alias support, inode union,
actual heartbeat rename/removal, live limit stops, deadlines and subprocess reaping.
The finite `qualify.py` runner adds independent pre-child readiness and both live
resource guards around these source tests. Its controlled faults use small real
files/processes and deliberately low test caps; they do not fill the host disk or
claim a product/build/browser result. Qualification requires a fresh 21d offer.
The fresh `TOOLS-FS-01-RETRY-SOURCE-20261104T2130Z` run qualified source
`fd66380a17199e71ceeb4761e750ce5a47b79575`: **25/25 source cases passed** in
0.944 seconds. Primary and independent guards each made five complete samples;
both finished without failure. Actual FULL was 2026-11-04 21:06:51.248403 UTC,
with no owned children, workflow or remaining growth reservation. Peak total NEW
was 45,495,729 bytes including the retained 41,158,065-byte prior charge and 4 MiB
reservation; WHOLE was 2,625,007,616 bytes, processes seven and RSS 153,255,936
bytes. The minimum free space across both guards was 14,574,551,040 bytes.

The first run's eight-second baseline timeout is preserved alongside its exact
incomplete subtotal. It admitted no workload. The runner fix moves its expensive
process-measurement throttle timestamp to measurement completion, avoiding an
accidental process measurement per filesystem entry when `ps` is slow. No scan
bound or capacity cap was enlarged for the successful fresh retry.

Thirty actual `du` samples and thirty native samples completed during atomic
heartbeat rename/removal churn. Those concurrent samples did not naturally hit
ENOENT; do not claim that they did. A separate deterministic real unlink before
the real `stat` syscall exercised incomplete-attempt evidence and fresh retry.
Controlled persistent/error faults and deliberately low resource thresholds are
identified separately in the raw observations. No product/runtime/SQL/browser
acceptance or controller-owner deployment is implied by these source results.

See [qualification](../evidence/resource-accounting/qualification.json), the
[exact artifact index](../evidence/resource-accounting/artifact-index.json),
[retained first failure](../evidence/resource-accounting/failed-01-FULL.json)
and [successful FULL](../evidence/resource-accounting/qualified-02/FULL.json).
