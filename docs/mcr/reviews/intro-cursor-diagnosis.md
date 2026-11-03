# BE27 second-page cursor diagnosis

Disposition: **verified serialization-width defect; minimal correction already
authored by the sole BE owner, native fix qualification pending**. This bounded
diagnosis does not accept BE27 or introduce another approval gate.

QA branch `codex/mcr-qa-intro-cursor`, base
`8d328cb39b6b62832e82878f169945a0fdceeca3`. Sole writable path is this report.
The source checkout `/Users/lhs/Desktop/mcr-worktrees/be-intro-discovery` was
read-only throughout. No application, test, proto, manifest or dependency edits;
no Cargo, binary execution, server, cluster, database or process lease.

## Verified cause and exact transition

The failing encoder emitted the row-position timestamp with ordinary decimal
`timestamp_micros()` formatting, while the decoder accepts **exactly 20 ASCII
decimal digits**. Contemporary positive epoch-microsecond values contain 16
digits. The encoder padded the upper fence and deadline but omitted padding on
the position. Authentication succeeds over those original bytes; parsing then
rejects the authenticated position.

Direct evidence is `/private/tmp/mcr-be27-native-72.json`: nine token fields,
upper width 20, position timestamp width 16, deadline width 20,
`MACS equal=true`, and `after <= upper < deadline`. The timestamp transition is
`1793738970526168` to `00001793738970526168`. No recorded token, key, actor/event
identity or profile payload is reproduced here. Receipt 72 reports the same
`second discover: Invalid` failure. This establishes the fault from actual
server-generated input, independently of guessing about a bad MAC.

Newest owner receipt inspected: `/private/tmp/mcr-be27-native-78.json`, four
native tests executed, three passed, pagination failed at `second discover:
Invalid`; its recorded binding tags and MACs match. Its immutable candidate is
`/private/tmp/mcr-be27-intro-00ecd45-20261104/intro-tests-candidate-78`, whose
SHA256 matches the receipt. Receipt 77 also records equal MACs. Receipt 78 does
not log the position field, so the direct width observation is attributed to
72, rather than invented for 78. The owner explicitly confirmed that candidate
78 retained the unpadded position and that the working source changed afterward.
No historical candidate-78 source snapshot was supplied; the binary hash is not
misrepresented as a source hash.

Current source references below identify the corrected working snapshot at
commit `00ecd45999dacf29addf8ede9532060112eb9a03`, not committed BE27 code:

| Transition | Exact path and lines | Finding |
| --- | --- | --- |
| First request / second request | `backend/services/concert-service/tests/intro.rs:598–614` | Same fixture viewer/event, limit 1 and absent filter; the second request clones the first returned token unchanged. Fresh session/request IDs are not cursor-binding fields. |
| Scope and filter binding | `backend/services/concert-service/src/adapters/out/repository/intro.rs:463–477` | Actor, event, operation, collection and optional query hash; no profile revision or session lease is serialized in this binding. |
| Fence and last returned row | Same repository file, `561–576` | Seals a window using the first page clock and the returned row's `(created_at, intro_id)`. |
| Token serialization | `backend/crates/concert-application/src/intro/cursor.rs:51–59`, `123–132` | Upper/deadline use `{:020}`. The owner has now also changed the position to `{:020}:UUID`. |
| Decode sequence | Same cursor file, `70–96`, `135–150` | Nine fields; deadline/tag; MAC; upper; position. `decode_position` invokes `parse_micros(time)`, which rejects length other than 20 at lines 145–147. |
| SQL ordering | Same repository file, `484–501` | Descending `(created_at, intro_id)` with a strict tuple continuation; failure occurs in cursor opening before this second-page query. |

The representation is delimited ASCII, not JSON. The demonstrated fault does
not require floating-point precision loss, JSON object canonicalization, UUID
ordering, wrong fixture IDs, a changed filter, or failed profile projection.
The authenticated old bytes are internally intact but violate the decoder's
width contract. PostgreSQL timestamp precision does not add leading zeroes to
ordinary integer formatting.

## Dependency-free reproduction

Executed Python 3 standard-library checks in memory, exit 0; no files or
dependencies created. One check privately extracted receipt 72's token and
asserted the widths, field count, logged successful MAC comparison and temporal
ordering above. The following independently reproducible synthetic case reseals
both variants, rather than mutating an already authenticated token:

```python
import hashlib
import hmac

key = bytes(range(32))  # synthetic reproduction only
domain = b"mcr-intro-cursor-v1:"
actor = "00000000-0000-4000-8000-000000000001"
event = "00000000-0000-4000-8000-000000000002"
row = "00000000-0000-4000-8000-000000000003"
upper, after = 1793739088527182, 1793738970526168
deadline = upper + 900000000
binding = f"{actor}|{event}|listConcertIntros|intros|-"
valid = lambda s: len(s) == 20 and s.isascii() and s.isdecimal()

for name, position in [("old", str(after)), ("padded", f"{after:020}")]:
    body = f"{upper:020}|{position}:{row}|{binding}|{deadline:020}"
    sign = lambda b: hmac.new(key, domain + b.encode(), hashlib.sha256).hexdigest()
    fields = f"{sign(body)}|{body}".split("|")
    assert len(fields) == 9
    assert fields[3:8] == binding.split("|")
    assert hmac.compare_digest(fields[0], sign("|".join(fields[1:])))
    assert int(fields[1]) <= upper + 1000 < int(fields[8])
    assert valid(fields[1]) and valid(fields[8])
    accepted = valid(fields[2].split(":", 1)[0])
    assert accepted == (name == "padded")
    print(name, "position_width", len(position), "parser_accepts", accepted)
```

Observed: `old position_width 16 parser_accepts False`; `padded position_width
20 parser_accepts True`. Both variants authenticate and retain the same binding
and lifetime. This executes a faithful model of the source's width guard, not a
claim to have compiled or executed the Rust decoder or rerun native PostgreSQL.

## Minimal recommendation and ownership

Retain the owner's one-field correction in `encode_position`:
`format!("{:020}:{}", p.created_at.timestamp_micros(), p.intro_id)`.
Keep strict parsing, HMAC verification, request binding and final authorization.
Do not pad an incoming token before MAC verification or weaken authentication
to accommodate an encoder defect. New tokens must be sealed over the padded
bytes. Existing malformed tokens may continue to fail closed.

The same BE author owns implementation and qualification. A focused codec
regression should round-trip an actual 16-digit epoch-microsecond row position,
assert encoded width 20, and preserve tamper/wrong-binding/expiry rejection.
The existing real-PG pagination test should then reach its second/third-page and
filtered continuation assertions without weakening them. The owner is rebuilding
and qualifying the correction; QA holds no build/database slot and requires no
additional approval before that work proceeds.

## Source and evidence pins

Two read-only hash passes before report authoring found the same owner HEAD and
all eight working-file hashes below. These pins describe an already corrected,
unbuilt source snapshot. They do not certify candidate 78's source ancestry.

| Working file relative to owner checkout | SHA256 |
| --- | --- |
| `backend/crates/concert-application/src/intro/cursor.rs` | `c79de139e6006035f7bddb93098c3ac06a8e0608ccf5300206fe8c3454cc0dc3` |
| `backend/crates/concert-application/src/intro/mod.rs` | `d3f518330e277242a16ff8173b6c7b62ff7a3a805e4a0aaf61548ec3c44ba3d4` |
| `backend/crates/concert-application/src/intro/wire.rs` | `718564160548e0d2516ff3811150685ef2857bc7c89e6102cdf9b2bad1bd634c` |
| `backend/services/concert-service/src/adapters/out/repository/intro.rs` | `7ede1115df450a4e23f07fa3350ebd6d76a869dbe7ff47ce5bbbf95fb4648439` |
| `backend/services/concert-service/tests/intro.rs` | `07df1963ecf96f12c8f9d5201b6d13101b5ce9240ac669e3843b1315b5e7f4cf` |
| `backend/services/concert-service/tests/intro_support/native_smoke.py` | `44496569017d384fa0a66c7b9737db44e1fb9e7537eb32084a52dcec9ba3c24d` |
| `backend/services/concert-service/tests/intro_support/owned.rs` | `7bbd0b08401716dae3311f6315bdc4fb10cbfa71165ce92a7a6059c3b7df7879` |
| `backend/services/concert-service/src/adapters/out/repository/intro_schema_v1.sql` | `fa42cf9028f7eb6caaf20e4b8c34c0ad192396e867d813dfdf917cb1d3f6448c` |

| Artifact | SHA256 |
| --- | --- |
| Native receipt 72 | `6b10ccf104644c9e40bf37bf2e70de6705a71ccdeba65a0e76b5f75d9d11489a` |
| Native receipt 77 | `40e55a81404b31cdbfdd13606e5ff0383e0b8fe7189528d31ead22003e353efe` |
| Native receipt 78 | `b7f4649e28a7c6348534f19c1fcca9a314b1ef1f4047411b71b57ff55694bbf9` |
| Immutable candidate 78 binary | `04e98ac2645a217f21c04d2c8038a6b68483b983564f1f0d4f655aad14b020d9` |

QA verification: source/path/line reads, immutable candidate/receipt hash match,
in-memory recorded-transition checks and synthetic before/after reproduction
all exited 0. Final staged whitespace and sole-path checks are recorded with
the committed handoff. Temporary owner receipts can disappear; the redacted
transition, reproduction and source pins above preserve the diagnosis.

Scope remains targeted failure analysis. Native success for the corrected
snapshot, real-service eligibility/authorization acceptance and full BE27
acceptance are **NOT_RUN by this QA assignment**. Only the sole report is handed
to the BE owner and Main; no source ownership transfers to QA.
