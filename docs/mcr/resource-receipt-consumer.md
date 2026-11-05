# Read-only resource receipt consumer

## Assignment and completion boundary

QA06 owns only `frontend/e2e/resource-control/receipt-consumer.py`,
`frontend/e2e/resource-control/test_receipt_consumer.py`, and this new document.
The objective is a portable standard-library reader for bounded owner receipts,
exact immutable FULL history, and factual process identity projection. Completion
requires implementation, focused tests, developer-owned fixes, this provenance
and limitations record, and a scoped committed handoff to publisher412e.
The original QA06 persistent goal remains BLOCKED with its original metadata;
this assignment neither resets nor completes its mobile/product requirements.
The venue client assignment remains deferred until this handoff.

## Provenance and authority

Newly authored logic follows the producer contract at working revision
`7f9d1095c492867aac70c91c3a9a1b3ab161c687`, published as
`651159ec7bae9ff5e1bbc9d3dd083fc08faa02d9`, specifically
`docs/mcr/evidence/parade-ink-foundation/controller-receipt-contract-v1.md`
and the headless membership example under
`frontend/src/app/__tests__/parade-ink-foundation/`.
No rendering, dependencies, historical evidence, or source files were copied
into this three-file publication.
Producer Native10 proves its stated historical scope; newer receipt shapes here
are separately reconstructed compatibility tests, not retroactive Native10 proof.

Allocator21d clarified active aliases, ordinary/null semantics, retained legacy
claims, exact closure history aliases, and format-aware JSONL handling directly
on 2026-11-05. The current interface and owner receipts are read-only inputs.
Only the owner's own operational index may be atomically updated under the
separate explicit grant. This module never writes any receipt or allocator file.

## Use and trust boundary

Load the hyphenated filename with `importlib.util.spec_from_file_location`, then:

```python
receipts = consumer.load_receipts(owner_directory)
state = consumer.reconcile(receipts, legacy_entries, previous=previous_claims)
projection = consumer.project(
    state, fresh_rows, unknown_candidates=all_relevant_process_roots,
    own_owner=owner_id, ordinary_future_bytes=current_ordinary_upper,
    infrastructure=exact_factual_infrastructure_descriptors,
)
consumer.check_limits(projection, current_offer_and_global_limits)
```

Each existing guard independently supplies its own fresh complete process snapshot,
actual `ps comm`, normalized birth, actual cwd and decoded argv. Rows contain
integer PID keys and `parent`, `birth`, `rss` in bytes, `executable`, `cwd`, `argv`,
and `commandDigest`. Never obtain actual comm by resolving the launcher path.
`argvDigest`, when supplied, is SHA256 of compact JSON argv; `commandDigest` is
caller-measured raw command SHA. These are distinct conventions.

The adapter must enumerate all relevant unknown roots, including opaque shells;
there is no automatic universal macOS process classifier. Ordinary rows require
factual `resourceRole="ordinary"` plus exact descriptor role/identity. Recognized
compiler, Node, module-test, browser and independent script payloads still fail
closed. Exact infrastructure descriptors cover only their named PID, not arbitrary
children. A missing controller reserves the full execution class/caps and creates
no PID. `controlCopies` paths never create process identities.

The caller retains observations across samples and passes `previous` to catch
same-offer acceptance/deadline resets. Absence is never FULL. Current FULL or a
newer ordinary record alone never erases a legacy or previously observed reservation.
Positive aliases `ACTUAL_ACCEPT`, `ACCEPTED_BOOTSTRAP`, `ACCEPTED_RUNNING`,
`ACCEPTED_GUARDS_STARTING`, and `ACCEPTED_INNER_CENSUS` retain full caps. Distinct offers
remain charged; overlapping factual process ownership fails closed. Current and
legacy copies of the same claim retain the higher reservation values.

Closure history aliases are `closedReceipts`, `closedOffers`, `closedHistory`.
Relevant immutable artifacts are bounded, SHA checked and bound to exact nonempty
owner/offer/acceptance epoch and actual FULL epoch with explicit resource zero.
Validation FAIL/PARTIAL_FAIL can still release resources; it does not become PASS.
Missing or unreadable closure evidence fails closed. Irrelevant historical offers
are not read and cannot release another offer. Duplicate JSON keys are rejected.

JSONL is parsed line by line. The packet-independent path requires an explicit
pure format adapter returning exact closure facts. `verify_attested_full` and
`reconcile(..., owner_index_bytes={owner: raw_index_bytes})` provide the separately
authorized `OWNER_INDEX_ATTESTED` path. They require the exact tuple in the bounded
sole-owner index snapshot, validate the immutable packet SHA and compare every
canonical semantic field present; the report retains `ownerIndexSHA256`. The
default ART event-envelope mapper checks one ACCEPT offer and one literal FULL
cleanup event. Other documented formats need the producer-specific pure adapter. A real ART231 QUALIFY01 packet has event ACCEPT time
1793842178.049309, while its owner-index bootstrap epoch is1793842178.045456;
those are not equal and this library never substitutes one for the other. An
adapter must document external owner-attested bindings where packet fields are
absent. The packet-independent default refuses such JSONL rather than guessing a timestamp.
The attested path preserves the exact bootstrap epoch from its authorized tuple
and compares the actual FULL event time exactly; event logging time is separate.

## Adoption and limitations

This is an optional compatibility component for each developer's next already
planned round after their own source freeze and fresh fit. It is not an allocator,
new guard service, approval stage, replay request, native cohort, or mandatory
product gate. No existing peer controller was edited. Existing offers, full-root
WHOLE/nonshrinking history, positive inode growth, OTHER, free floors, finite
WORK/FULL clocks and PID/birth-safe cleanup remain the caller's responsibility.
No universal backend custody, browser/device/multiuser/product acceptance follows.

For a factual current `state` beginning with `Z`, `previous_identities` may carry
the preceding fully verified identity. Terminal association requires exact owner
claim tuple, PID, birth, parent, unchanged descriptor hash and prior snapshot SHA.
The result labels terminal state and preserves observed PID/RSS plus full declared
reservation. An unavailable sleeping `S` process, uncached PID, new birth/parent or
new accepted claim fails closed. This cache behavior is synthetically tested; no
live terminal-process qualification is claimed.

The tests use synthetic reconstructions of reported shapes and failures, not live
peer processes. They do not certify existing peer controllers' adoption.

## Developer validation and handoff

Original PREP32 measured positive inode growth56,705,024B against its32MiB
cap: retained as a raw violation, mostly55,914,496B complete common-Git peer
pack/index replacement and303,104B peer PI07 writes. No deletion credit or baseline
recapture was taken. Fresh PREP64 accepted1793843868.487781. The old unaccepted
16MiB SOURCE family was replaced by distinct SOURCE01/02-0202 sharing256MiB.
SOURCE02 conservatively charged192MiB, including the old actual growth, the
entire additional64MiB prep upper and an additional64MiB reserve for SOURCE01's
failed initial inventory evidence gap. That reserve is not an invented peak.

| Execution | Actual result |
| --- | --- |
| Initial stdin preflight | NOT_ACCEPTED: private argv-bound error, corrected; no SOURCE clock/tests |
| Second preflight | NOT_ACCEPTED: unknown owner state, no state guessed/tests |
| SOURCE01-0202 | Accepted1793844146.450274, FULL1793844147.877162; FAIL before commands on unresolved PID17506, parent49590, birth11:02:27, stateSs |
| SOURCE02-0202 | Accepted1793844299.092567, FULL1793844308.78718; both independent complete initial LIVE samples before commands |
| SOURCE02 unittest | `python3 -B frontend/e2e/resource-control/test_receipt_consumer.py`:35 tests, exit0 |
| SOURCE02 compile | Explicit private-output `py_compile` for both Python files: exit0 |
| SOURCE02 Git intent | exit1: new paths outside sparse definition; overall round remains FAIL |
| Ordinary scoped publication repair | `git add --sparse --intent-to-add --` the three paths: exit0; scoped `git diff --check`: exit0 |
| SOURCE03-0215 | Accepted1793845047.473187, FULL1793845060.2429838; both independent complete initial LIVE samples before commands |
| Final focused reconciliation/identity checks |11 tests, exit0 on the exact final committed Python postimages |
| SOURCE03 remaining compile command | NOT_RUN: the next census failed closed on an ordinary identity mismatch; overall resource round remains FAIL |

SOURCE02's executed source images are preserved privately and byte-verified
against its sealed readset: moduleSHA
`24f49e902c9522627e42a22d96948dd19c7068603e2e2205faf5828d999a4899`,
testSHA `a67784d5f1e4d3af7a3d11d8abf0ea88baf3e43b270de103da8b812690bf14ce`.
The final fix rejects an execution-kind change under the exact same accepted
claim; ordinary overlays continue retaining the original execution reservation.
The final focused command was:

```sh
python3 -B frontend/e2e/resource-control/test_receipt_consumer.py ReconciliationTests ProjectionTests.test_optional_ordinary_executable_uses_exact_digest
```

It passed11 tests on the final image, including unchanged reconciliation cases,
new execution-kind immutability and the missing-measured-comm negative. The
source module and test file were both parsed/imported/executed by that command.
The later explicit `py_compile` command was NOT_RUN after an ordinary identity
census stop; the earlier pre-fix `py_compile` exit0 is preserved separately.
No fourth source run is required solely to obtain a green resource report or
repeat syntax work already exercised by the passing final interpreter run.
The bare private-control error does not establish a particular peer's identity;
no unknown process was exempted or retroactively classified.

Functional helper validation is complete for this bounded assignment, with the
source/control limitations above. No failed resource round is relabeled PASS,
no runtime adoption is certified and no product/native replay is requested.

SOURCE02 controller observed WHOLE6,122,016,768B, positive inode growth74,985,472B,
conservative charged NEW201,326,592B, own RSS peak202,522,624B. The independent
guardian's final sample observed WHOLE6,122,024,960B and own RSS203,620,352B.
These actual higher WHOLE values replace the earlier6,121,287,680B floor for
future owner work. Both clocks closed with active child processes/groups,
ports, dependency links and temporary work0; private evidence/bytecode retained.
Distinct per-guard census/highwater files remain private. SOURCE03 observed
nonshrinking positive growth160,907,264B, charged NEW201,326,592B, WHOLE floor
6,122,024,960B and own RSS204,341,248B. The initial raw-current inventory figure
96,415,744B is not substituted for that higher nondeletion highwater. Ordinary
prep was not continuously sampled; no continuous ordinary-book compliance is
claimed. All observed higher carry remains charged, with no reset/deletion credit.

Private controls and evidence: `/private/tmp/mcr-qa06-receipt-consumer-0145`.
Base revision `16f1ccee3c983e372ab22818713398753e9b3590`. Only the three assigned
new paths are published; no private receipts, logs, process snapshots or fourth
fixture file are included. Source commit `2a2f05d72d3c20cb3810655567bb767a31eaf3ad` descends directly
from that base. Final Python SHA256 values match SOURCE03's executed readset:

- `receipt-consumer.py`: `38ade69ba2249cea44f6f5d7279928929e451831b9c7b5d1008bdf0dfe9780d1`
- `test_receipt_consumer.py`: `78aa34952c6c846717b5cf95479fe729ae31fb91544c6688c6a21b751bcc979f`

A documentation-only child commit completes this evidence record. The owner
sends both immutable commits and exact postimages directly to publisher412e. No king/SL review or acknowledgment
is required. Consumers own their next planned adoption and runtime validation.
