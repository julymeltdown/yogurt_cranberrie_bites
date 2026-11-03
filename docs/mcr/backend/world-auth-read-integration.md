# Auth-only World read integration

The sealed Auth-only producer is integrated from author commits `508929a`,
`30ca2a4` and `914704b` as `a15f1bb`, `690bfae` and `9420961` respectively.
The author stopped with a clean checkout and completed its bounded goal.
See [the source handoff](world-auth-read.md) for API and validation details.

Integration verified all eight assigned paths and existing-file preimages,
1,308 native source hashes and three frozen executable hashes. The resulting
**entire backend Git tree is identical to qualified source `30ca2a4`**; subsequent
frontend changes did not alter it. No duplicate code review or Cargo build was
added. The original evidence remains source-bound: eight new deterministic
checks, three retained transport checks, two actual Auth/Concert native cases
and four Python evidence guards passed. Owning migrations, reapplication,
restart persistence and fixture cleanup passed. The final native report hash is
`57bdfa656ad81f6497e2cc184936b6bf4d5d11f4144c2deeadf610674b278c51`.

The native build target was explicitly released by this writer. Integration
verified the release receipt, absence of its four fixture directories and no
retaining process before allocating that same target to the World runtime
owner. Native evidence and frozen executables are preserved.

Status remains **CONTRACT_READY for the bounded producer**. Its integration
does not provide the missing Profile/recipient authority, distributed
invalidation fence, production configuration, WebSocket composition proof,
multiuser/device/load evidence or IN01 acceptance. The runtime owner's ongoing
sealed-Auth and room-switch work is a separate candidate. That work also owns
the exact discovered membership-lease milliseconds correction and its native
regression; this Auth producer receipt does not claim that later fix.
