# ART-PARADE-PRINT-02 source revalidation offer input

Status: **PREPARED; NOT OFFERED / NOT ACCEPTED / NOT RUN**. This is an exact
small-run proposal for Resource21d, not an allocation receipt. No export was
rerun while preparing it. Original authoring results remain immutable input.

Proposed envelope:

| Item | Requested bound / accounted set |
| --- | --- |
| Total / work / cleanup | 60 / 30 / 20 seconds, with10 seconds reserved for final receipt |
| Whole | 8388608 bytes; all exclusive asset/doc/evidence files, job temporary files, durable logs/receipt, plus counted3449863 bytes of read-only reference inputs |
| Temporary | 1048576 bytes; one unique `.qualification-work/<offer>-*` directory |
| Durable | 524288 bytes; existing owned carry plus bounded new run log/receipt; no new copies of references |
| Log / receipt reservation | 65536 bytes each; reserve both before launch |
| RSS | 268435456 bytes, sampled aggregate guard+descendants; OS child high-water also recorded and checked |
| Process count | At most4 including guard, one sequential Python export/measurement child and short-lived `ps` sampler |
| Free space | Actual fresh acceptance observation at least16GiB; running floor8GiB, with no new per-child16GiB gate |
| Failure carry | Original frozen source/draft unchanged; bounded durable raw log, failure, partial-output hashes and release receipt; temporary exports removed |

Reference inputs are canonical PNG2967126 bytes and supplied JPEG482737 bytes.
Both are read in place; their bytes count toward the proposed whole envelope.
Git metadata and other writers' checkouts are excluded and untouched. The exact
owned carry will be recalculated from the clean frozen HEAD at acceptance;
the guard inventories every owned input's bytes and hash before starting.

Expected identical PNG exports are61660 bytes total: preview11240,
native2x25153, paper-review25267. Two fresh small JSON reports are bounded by
16384 additional bytes; log and receipt have separate ceilings. These are
expected/proposed bounds, not measured peaks. No browser, server, build,
dependency install, image service or native matrix is launched.

The new `--output-dir` / `--report-dir` options on the existing two scripts send
all fresh exports into the job's own temporary directory. The guard compares
their pixels/files through exact pinned PNG SHA values and the unchanged
reference report. It checks the entire frozen carry for accidental changes.
It never overwrites the qualified public files, first draft, prior validation
JSON or earlier run receipts.

After an actual named offer, explicit ACCEPT, fresh free-space observation and
frozen clean HEAD, invoke this exact command template with the actual values:

```sh
python3 docs/mcr/evidence/art-parade-print-v2/qualify-source.py \
  --offer ACTUAL_OFFER_NAME --head ACTUAL_CLEAN_HEAD \
  --deadline-utc ACTUAL_JOB_DEADLINE_UTC --accepted-at ACTUAL_ACCEPT_TIME_UTC \
  --accept-free-bytes ACTUAL_FRESH_ACCEPT_FREE_BYTES \
  --total-seconds 60 --work-seconds 30 --cleanup-seconds 20 \
  --whole-bytes 8388608 --temp-bytes 1048576 --durable-bytes 524288 \
  --rss-bytes 268435456 --processes 4 --log-bytes 65536
```

These are proposal numbers; execution must match the actual granted offer.
The accepted free-space value is the real observation reported with ACCEPT,
not a constant or a new gate at each child launch. The runner records actual
source free space, sampled minimum, timing, command-file hashes, child exit
codes, sampled RSS/process count and disk peaks. Child `ru_maxrss` is recorded
separately: it is not mislabeled as a continuous aggregate measurement.

The guard traps cancellation, deadline, resource-cap and command failures,
signals only the new session it owns, waits for its child, hashes remaining
temporary outputs, removes its own unique directory, and verifies no live
descendant remains. Durable `runs/<offer>.json` / `.log` record FULL release
only when process and temporary-file release are actually confirmed. An
incomplete cleanup is a failed receipt requiring owner action, not permission
to start a duplicate job. The peak measurements are sampled, not a global
machine profiler.

The preparation check is limited to `ast.parse` of the three Python sources,
existing pin readback and `git diff --check`. It executes no exporter or guard.
The actual committed HEAD, command SHA-256 values, fresh owned carry and source
free observation are sent directly to21d/Root after committing this preparation.
Native acceptance remains NOT_RUN and belongs to the one213 integrated round.
