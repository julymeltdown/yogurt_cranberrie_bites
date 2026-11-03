# HTTP identity composition checkpoint — 2026-11-03

The strict Gateway HTTP context and configured Auth/Profile reader are integrated.
They preserve one verified actor, session family and original request deadline.
This is a reviewed component boundary; the real HTTP-to-both-owners database
journey is still running in its isolated development chat.

| Scope | Worker chain | Integrated chain |
|---|---|---|
| HTTP extraction and actual timeout correction | `6297fc0`, `66aea57` | `c68594c`, `898779f` |
| Ordered Auth/Profile composition and shorter-stop fix | `1d2e587`, `76d7899`, `9a2c202` | `775d5f1`, `16a37fa`, `4d66ce4` |
| Configured reader, checked HTTP borrow, joint-test dependency | Integrator | `1f6dceb` |
| Existing retained dependencies for BE08 place values | Integrator | `f440df3` |

The HTTP extractor uses the configured strict JWT verifier and actual timeout
middleware. Missing trusted deadlines fail closed. Unknown-key refresh and later
work consume the same original stop. Parent review rejected an earlier test that
injected a deadline without enforcing the production timeout; the corrected six
cases execute that middleware, including an actually stalled handler.

The reader first queries current Auth facts. Inactive accounts, ended sessions,
failed reads or a mismatched requested binding stop before Profile dispatch.
Ordinary email identities require no external social binding. The subsequent
minimal self-Profile read must match the canonical actor. Private observations
are borrowed, nonserializable and checked again at consumption.

Parent review found the combined observation could outlive a shorter enclosing
deadline after leaving its task scope. The correction retains that absolute stop
inside the observation; all accessors reject it after expiry even while the
longer original context remains valid. Production AppState now constructs the
reader from the same configured concrete Auth/Profile clients. Test-double
AppState leaves it absent. No accepting Concert route is enabled.

Parent ran six actual HTTP-context cases, eight controlled signed composition
cases, Gateway library/binary checks and an Auth-service test-target compile:
all exit 0. Worker evidence retains fifteen Profile transport tests and fourteen
compile-fail doctests. The [archive index](http-identity-integration/index.json)
pins final source, original and normalized log hashes, failed attempts, command
results and the complete three-commit composition ancestry. Parent verified all
four final composition paths against the worker receipt.

The Auth test-only Profile dependency creates no production cross-service call.
BE08's four domain dependencies reuse exact existing lockfile resolutions;
offline metadata passes, with its compiled feature validation still pending.
Only the HTTP-owner-chain worker currently owns the shared Rust target.

Whole BE04 remains IN_PROGRESS. Actual joined Auth/Profile/PostgreSQL execution,
signed Concert admission, event participation policy, public product endpoints,
TLS and release gates remain separate obligations. Accepted required tasks stay
11/112; bounded implementation completion does not change that count.
