# FE10 integrated signup and verification review

Date: 2026-11-03. Worker base `e725ae4`; implementation `6a8e827`,
documentation `d1b67f9` and disposal correction `e45e87d` integrated serially as
`f825b31`, `f498194` and `7bfea96`. All thirteen assigned paths match the final
worker bytes: [inventory](signup-verification/hashes.json).

The existing public routes now use the canonical session/HTTP/privacy owner for
signup, verification and resend. Responses never install login authority. Raw
password bytes, exact six-digit codes, neutral resend outcomes, duplicate and
generation fences, URL/secret cleanup and server-only Retry-After hints are
covered. Anonymous verification has explicit event-independent capability
discovery; sign-in and world readiness retain their event gate.

Parent review found and resolved three concrete issues: an interval continuing
after its wait expired; anonymous verification inheriting the unrelated event
configuration gate; and a reserved capability microtask dispatching after
synchronous disposal. The final source includes regression coverage for each.
The parent reviewed the two minimal composition exceptions and cross-directory
imports; no remaining bounded finding was identified.

Combined candidate `7bfea96` emitted passing results for:

- **737 frontend tests** across twenty files, including 68 new enrollment cases;
  strict TypeScript/production build and lint.
- Exact thirteen-path frontend boundary, frozen C03 generation, C08 origin and
  planning/provenance checks. No dependency, cookie policy or artwork changed.
- Installed Chrome 156.0.8078.80 at 360, 390, 430 and 1280px, plus reduced height:
  forms, duplicate/uncertain outcomes, server wait hints, privacy/URL/storage
  hygiene, keyboard focus, targets, contrast and overflow.
- Production anonymous verification: explicit capability GET followed by exact
  resend/verify requests without an event; cold public entry makes no requests.
- Retained four-width sign-in and recovery suites, native Web Locks and peer
  invalidation; peak cookie writers one and automatic refreshes zero.

The [retained logs and reports](signup-verification/) bind these results to the
source inventory. Parent visually inspected the production 390px availability
form and 1280px verified result, retained alongside the reports. The heading's
symmetric keyboard-focus outline is an accessibility indicator; no decorative
left accent is present. The existing >500kB root-bundle advisory remains
(544.82kB / 144.67kB gzip); no threshold was changed.

Status: **MOCK_READY**, with the bounded implementation/review goal approved for
completion. Production signup remains unavailable: approved eligibility,
publication and atomic consent persistence are still missing. Synthetic choices
do not record consent. Real Auth/mail, expiry/quota transactions, production
Secure cookies, physical devices, vertical integration and release remain
**NOT_RUN**. Application retry zero does not certify one physical TCP arrival.
No required task or IA acceptance count changes. See the
[worker record](../frontend/signup-verification.md) for commands, source pins
and the remaining complete FE10 criteria.
