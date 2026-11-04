# Candidate02 source-only qualification input

Status: **PREPARED / NOT OFFERED / NOT ACCEPTED / NOT RUN**.

This is a new original SVG after the NativeB owner assessment, not a replacement
for b68. Source SHA-256:
`e7b401a8f431651c05b88a6c52d02b8aabc059f697bf960b41454ba59ae95a10`.
The proposed interface remains360×96, native720×192, transparent ink#191817.
Only XML parse, Python AST parse, hashes and whitespace were checked while
preparing; no raster output or browser was run.

Requested source envelope:90 seconds total /60 work /30 independent cleanup;
8MiB WHOLE /1MiB temporary /2MiB entire durable carry;512MiB sampled aggregate
RSS; max4 processes including guard and sampler; one sequential output child;
64KiB log plus64KiB receipt reservation. Read-only reference inputs3449863
bytes count in WHOLE. All existing public assets, draft/history, notes and prior
receipts count in the owned carry; no peer runtime/toolchain/cache is used.
Actual limits must come from a new named Resource21d offer, with explicit ACCEPT,
fresh16GiB acceptance free-space measurement and8GiB live floor.

Resource21d clarified the actual candidate02 offer before ACCEPT: existing own
private Git administration and `.git` pointer count in WHOLE using a fixed2MiB
reserve, conservatively above the observed1647864 bytes. That preexisting Git
baseline does not count in the2MiB source/output durable cap. Shared common Git
objects and foreign checkouts are read-only excluded baseline. No recursive
common-store scan is performed. The guardian verifies private-admin bytes fit
the reserve and includes that reserve in every WHOLE check/peak. This new
clarification does not rewrite prior sourceB receipt accounting.

The existing guardian now has a candidate mode. It verifies all frozen inputs,
measures references and rasterizes this candidate through the retained renderer
in its unique temporary directory. It checks dimensions/alpha/RGB/source SHA,
then retains four bounded new files in the previously absent
`candidate-02/qualified-01/`: preview/native PNG, source-only paper review PNG
and source-validation JSON. It never changes public b68 files or old evidence.
The durable copy is accounted before copying, hashed afterwards and named in
the raw receipt. Temporary files are cleaned; a failed candidate is not promoted.
Prior receipt command hashes preserve the old guardian/renderer revisions.

After the actual offer, use the clean frozen HEAD and actual values:

```sh
python3 docs/mcr/evidence/art-parade-print-v2/qualify-source.py \
  --offer ACTUAL_NAME --head ACTUAL_HEAD \
  --deadline-utc ACTUAL_CLEANUP_DEADLINE --accepted-at ACTUAL_ACCEPT \
  --accept-free-bytes ACTUAL_FREE_BYTES \
  --total-seconds 90 --work-seconds 60 --cleanup-seconds 30 \
  --whole-bytes 8388608 --temp-bytes 1048576 --durable-bytes 2097152 \
  --rss-bytes 536870912 --processes 4 --log-bytes 65536 \
  --git-baseline-reserve-bytes 2097152 \
  --candidate-source docs/mcr/evidence/art-parade-print-v2/candidate-02/parade-print.svg \
  --candidate-output docs/mcr/evidence/art-parade-print-v2/candidate-02/qualified-01
```

These are proposed numbers, not a granted receipt. Source inspection follows
the actual allocated render. New accepted art needs new pins, consumer handoff,
selective retained native evidence and named-owner publication/readback.
