# FE02 and BE02 integration — 2026-11-03

This checkpoint accepts FE02's headless cache/privacy criteria and BE02's relocated
native tooling criteria. It does not accept a browser login, real consent flow,
container image or release gate.

## Frontend review and execution

Worker base `d76cbb5ca7ff419cf8c4e6ac1d587e4c69c42644`, implementation
`f12e476a902ee946ea149bbb47dd58083b3e2a53` and handoff `1b8d981` integrate as
`4207792` and `45085d7`. All five paths match the assigned query source, tests and
lane documentation; no dependency, backend, router, style or contract changed.

Parent reviewed canonical keys, session/policy generations, query cancellation
and eviction, active observer scrubbing, post-queryFn commit fencing, successive
same-session revocations, stale recovery compare-and-set, explicit reveal behavior,
cleanup/disposal and public-only dehydration. Mutation cache removal is explicitly
not cancellation of server effects: owners must guard callbacks and clear their
drafts/observers/subscriptions through the documented ports. This limitation is
tested rather than hidden behind a generic cache-clear claim.

On integrated candidate `45085d7`, pinned Node 24.22.2/pnpm 11.7.0 commands:

- `pnpm --dir frontend test src/shared/query src/shared/api`: exit 0, **189 tests**
  (45 query/privacy plus 144 transport/error), including the FE01→FE02 delayed-A
  response/account-B composition and a post-queryFn/pre-cache-commit race.
- `pnpm --dir frontend run build`: exit 0, strict TypeScript/Vite production build.
- `pnpm --dir frontend run lint`: exit 0, scoped Oxlint.

The modules are headless and not yet wired into a product shell. Actual event
adapters, consent expiry, cross-tab behavior, UI owner cleanup and real-server
privacy flows remain FE09/FE31/IN03 obligations.

## Backend review, correction and execution

Worker base `0fb22af06fbc139a8f9b6740d8cf780eaa55557a`; implementation `6645639`
and handoff `e3492ab` integrate as `441efa7` and `072d39e`. Parent found that an
idle/partial-header mail HTTP connection could block synchronous shutdown because
the timeout was applied after parsing. Worker correction
`f2bdf94ef6d30b3fc45235fa8ebed8ec31d30aed`, integrated as `e9e8b3c`, moves timeout
setup before parsing. Two actual-socket regressions fail without that correction.
All seven paths remain inside the explicitly assigned tooling, lane doc and
`infra/mcr/backend/` subtree. Existing runtime/migration helpers are unchanged.

Parent commands on the corrected integrated code:

- `python3 -m unittest discover -s backend/scripts/mcr -p test_dev_stack.py`:
  exit 0, **16 tests**, including accepted idle/partial-header sockets and bounded
  cleanup. Existing helper tests were already qualified in the earlier
  [55-test integration](core-foundation-integration.md).
- `python3 backend/scripts/mcr/dev_stack.py start --bin-dir
  /Users/lhs/Desktop/mcr-build/backend/debug --ready-file
  /Users/lhs/Desktop/mcr-build/be02-dev-integrated-01-ready.json --report
  /Users/lhs/Desktop/mcr-build/be02-dev-integrated-01.json --duration-seconds 180`:
  exit 0, six services ready, actual native PostgreSQL/Redis/private mail.
- A separate process ran `python3 infra/mcr/backend/verify_local_auth.py` against
  that owned ready descriptor, with a new report path: exit 0. Signup, verify,
  login, refresh, reset-request and reset-confirm returned 200; logout returned
  204. **Two actual Auth outbox deliveries** reached the private inbox.
- A separate `dev_stack.py stop --ready-file ...` returned STOPPING, exit 0;
  the foreground owner then returned **PASS / cleanup PASS**, exit 0.

The exact redacted [stack receipt](be02-dev-integrated.json) binds source
`e9e8b3ce0f358acd338b2811758b9a0b1e2de063`, script/helper hashes, six binary hashes,
84 migrations and health results. Eight service/Redis/mail resources stop cleanly;
PostgreSQL cleanup is separately owned, and the private root no longer exists.
The [auth seam receipt](be02-auth-seam-integrated.json) records statuses and verifier
hash without credentials, reset links or verification codes. Parent verified the
current launcher hash and all six binaries match the executed report.

## Acceptance boundary

BE02's fresh migration and restart/persistence requirement is established by the
earlier [six-service runtime receipt](be02-runtime-integrated.json). This increment
adds usable foreground ownership, status/stop, private mail delivery, repaired
image build context and runtime SQL paths. That satisfies the bounded tooling
task; Docker image build/run remains **NOT_RUN**, as the daemon is unavailable.
Mutable image tags, Linux dynamic linking/nonroot execution and production runtime
roles must be qualified before packaging/deployment and BE44 acceptance.

Local HTTP uses an explicit fixture-only insecure-cookie override; production
Secure-cookie policy is unchanged. The auth seam still uses inherited `/auth/*`
routes. C03 `/api/auth/*` ingress, Origin/intent enforcement, capability/provider
exposure, response protections and expired/reused-credential cases remain BE03.
No real browser, external provider, Concert, production mail or release behavior
is claimed. The next independent lane assignments are BE03 and FE07.
