# Social policy metadata on the existing World stream

Contract revision: `SOCIAL_POLICY_V1`. Status: CONTRACT_READY for schema and
offline oracle only. Production sender, frontend adapter, real HTTP/WS/cache
qualification and original IN03: NOT_RUN.

The existing World snapshot and movement shapes remain unchanged. Two explicit
server-only messages extend the `1.0.0` server-message union:

- `policy.baseline`: `protocolVersion`, `type`, `streamId`, `eventId`,
  `scope: "SOCIAL"`, and positive safe-integer `policyRevision`.
- `policy.invalidate`: the same binding fields plus a closed `reason`:
  `SOURCE_CHANGED`, `CONSENT_CHANGED`, `BLOCK_CHANGED`, `EXPIRED`, or
  `CANONICAL_POLICY_CHANGED`.

No handle, private content, resource identifier, actor identifier, owner secret,
or completion capability is transmitted. The authenticated current World stream
already binds the actor. Policy revisions are monotone per canonical actor/event,
start at one, and are unrelated to the movement sequence. These messages never
grant service access, consent, age eligibility, or authority to read private data.

Before a matching explicit baseline arrives after the actual current World
snapshot, every private Social availability value remains false. World movement
can remain available independently. No client zero/default revision, World
availability flag, or C03 mutation capability substitutes for this baseline.

The production source must gate stream registration, obtain the Social owner's
held database baseline, install the exact actor/event/stream revision, arm
publication under the same local gate, then release the held database guard.
Generation writers cannot commit across that held baseline. Failure at any step
withdraws the private view. There must be no network or user wait under the
database guard. Publisher acknowledgement requires real current-stream handoff
or durable stream withdrawal, not an additional application queue.

Consumers keep the transport and its baseline at shell identity lifetime,
outside disposable Social controllers. Reconnect, room/stream change, identity
or event change, invalidation, malformed metadata, gap, and overflow withdraw
private views synchronously before refetch. Pending reads and media URLs are
cancelled/released, and late completions cannot refill a newer privacy tuple.
An invalidation that advances the revision withdraws the full private tuple;
even a contiguous advance is not permission to read again. A jump also requires
a fresh bound server baseline. Old/duplicate revisions are ignored without
rolling the floor back or restoring withdrawn authority. A duplicate baseline
at the withdrawn revision cannot reopen a private view.

Recovery requires a fresh current stream baseline and fresh server bootstrap,
availability and HTTP results bound to that same identity/event/lifetime tuple.
Neither a baseline nor a client refresh restores revoked consent. Unblocking
does not restore consent. Actual expiry comes from the server's durable deadline
producer; a frontend clock or fixture cannot certify it.

The original Social owner implements the sealed source/audience/generation,
deduplication and expiry port. The backend integrator owns this public schema and
the existing World sender/composition. The frontend integrator owns generated
wire types, the strict decoder and the single production policy adapter; the
original IN03 harness borrows that adapter. A C05 World revoke is not evidence
for this Social policy path. No second socket or duplicate harness is admitted.

Validation commands (exit 0):

```
python3 -B -m unittest discover -s scripts/mcr/tests -p test_realtime.py
python3 -B scripts/mcr/validate-realtime.py
```

The oracle passed 37 cases and all 26 indexed wire/HTTP fixtures. It verifies
metadata shape, current stream/event binding, missing baseline denial,
withdrawal, revision floors, duplicate behavior and private-field refusal.
Those are contract fixtures, not live multiuser or privacy acceptance evidence.
