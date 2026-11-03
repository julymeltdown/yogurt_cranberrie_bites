# Strict Gateway access context integration

Worker `ed2f166917f17a9d6d488655fdaf98292f6e408f`, based on published
`b16a679`, is integrated unchanged as clean candidate
`e7d208ec000efccaacb12df1fa9010b104fe007a`. All four assigned path hashes match
the worker receipt. Parent reviewed the full context, tests and minimal shared
decoder extraction; existing legacy signatures, projection, validation settings,
key source and rotation behavior are preserved. No existing test was weakened.

The new process-local context requires actually verified canonical user/family,
signed integer expiry and a trusted original monotonic request stop. It retains
signed wall bounds and captured monotonic bounds, and rechecks them at identity
and deadline access. It has no public constructor, Clone, serialization, raw
token, roles or provider/Profile/event assertions. Future live expiry is normal;
no iat requirement, arbitrary JWT lifetime or product revocation SLO is invented.

On this combined candidate, parent reran Rust 1.88.0 with locked/offline
dependencies, two jobs and disabled debug information: **13 strict context +
209 retained Gateway unit + 13 verification transport tests**, **three
compile-fail doctests**, the library check and Gateway binary build all pass,
exit 0. Parent rustfmt, exact four-path backend boundary and whitespace checks
pass. Owning source, retained lock/tool fixtures, library-source and worker log
pins are verified. No PostgreSQL, Docker or global service is required or started.

[Machine evidence](gateway-concert-access/result.json) records the exact
candidate, commands, exits, source/log hashes and limits. Logs and the worker
handoff are retained beside it. The corrected development compile failure is
preserved separately and is not counted as a passing invocation. The worker's
historical handoff document remains byte-identical; this record supplies the
parent integration disposition.

This qualifies a bounded BE04 prerequisite. Trusted HTTP deadline capture,
unknown-key refresh budget propagation, current owner/Profile/event reads,
key withdrawal across awaits, revocation, signed Concert admission and stream
invalidation remain unqualified. No route or accepting policy is enabled;
whole BE04 and project acceptance remain open. The earlier unchanged Auth
Docker-only privilege/schema fixture failure remains separate evidence.
