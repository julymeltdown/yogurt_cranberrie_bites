# FE09 scoped integration review

Date: 2026-11-03. Worker base `2a5c8e6`; public core prerequisite `153a3dc`
(`0e2874f` in the worker). Worker commits `983dfb2`, `1a30d8a` and review fix
`831bb5d` integrate as `26cacae`, `3798834` and `a9e130e`.

The integrator reviewed all 19 changed paths against the exact FE09 delegation,
including the four composition files. The boundary guard passes against
`ffe320b` with only router/main as exact composition allowances and the one
assigned handoff document. Import review found no backend implementation,
credential persistence or legacy rendering. The public contract is consumed
read-only, and its frozen artifact is unchanged by wider C04 assembly.

Review finding R1: the controller cleared identity for ordinary protected 403
denials. That would discard drafts/world access for a valid session and turn a
bootstrap denial into unauthenticated. The worker fixed it and added both
permission/consent regressions, actual shell retention on 403 and synchronous
cleanup on 401. A self-review fix also rejects inherited refresh options before
transport, preventing a prototype option from bypassing cookie coordination.
The integrator inspected these fixes; no further actionable issue remained in
this assigned candidate.

Parent checks on integrated frontend candidate `a9e130e`, all exit 0:

- Full frontend suite: **354/354**, including **78 FE09** cases.
- Strict TypeScript/production build and lint; C08 source/import scan: 84 files,
  zero violations. The existing 538 kB / 143 kB gzip main-chunk advisory remains;
  no threshold/configuration change hides it.
- Installed Chrome **156.0.8078.80**, four widths 360/390/430/1280, owned servers
  and browser contexts, native Web Locks/BroadcastChannel and test HttpOnly HTTP
  cookie. Sign-in, neutral invalid credentials, throttling, immediate private
  world release, uncertain server logout/retry, focus/contrast/touch targets and
  overflow checks pass. Peak cookie writers 1; automatic refreshes 0.
- Production entry without approved event configuration makes no API requests,
  uses no default event and excludes fixture identity/globals.
- Contract drift, original planning/provenance and whitespace checks pass.

The parent inspected 390px sign-in and 1280px unconfirmed-logout screenshots;
labels and honest failure states are visible, with no decorative left accents.
Selected parent-run screenshots, the full four-width browser receipt and hashes
are durable in [session-sign-in/](session-sign-in/). Full local parent artifacts
are `/Users/lhs/Desktop/mcr-build/fe09-integrated-a9e130e`.

Status is **MOCK_READY**, not FE09 acceptance or IN01. Production automatic
restore/refresh stays disabled. Secure production cookie/domain/SameSite,
real BE03 browser sessions, revocation/rotation races, two real accounts,
production event, actual Phaser/socket/art bytes and device gates are NOT_RUN.
Completing the delivered increment goal does not close these task gates.
