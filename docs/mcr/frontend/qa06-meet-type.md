# QA06 Meet type hierarchy

Status: **MOCK_READY** for bounded finding `QA06-HUMAN-MEET-TYPE-01`.
The title/place/action correction and source-specific fixture qualification are
complete. Original FE21 closure is retained. Whole QA06, human acceptance of
full-reference fidelity, real multiuser/device/IN02 and release acceptance remain
open; this report does not amend the 112 required tasks or 61 required IA entries.

## Assignment and source

Community grant: `96bd057df1f56568c5f4dae55367ac6965f8e250`.
Assignment base: `4af1c86863698e311d286039a0702d5b2517c1c3`.
Implementation and qualified harness: `e9912f13fc3e055bde4b905d663bf7fc545226b9`.
Isolated checkout: `fe-ref-feed`, branch `codex/mcr-fe-QA06-meet-type`.
Initial goal readback was null, checkout clean, and the three new evidence,
runner and report prefixes were absent. A new unbudgeted persistent goal covers
implementation, validation, committed handoff, fixes, actual Root153 publication,
fresh independent remote ancestry/payload proof, then clean STOP and exact lease
return. At this committed handoff publication remains pending; the goal stays
active until those remaining steps have actually completed. No receipt commit is
required. Root153 alone publishes; no King/main/scrum-lead review gate is imposed.

Only title/place/action typography, spacing and ink in
`frontend/src/features/party-discovery/ui/party-discovery.css` changes product
source. All other delivered paths are in the three explicitly granted new
runner/evidence/report prefixes. No JSX, controller, shared frame/footer, global
type, router, root composition, art, backend, contract, manifest, lockfile or
source original was modified. Shared frame/footer owner 213 retains its scope.

## Diagnosis and correction

The exact human JPEG and saved historical 390×637 Meet frame were visually
inspected. The same three titles/capacities provide a useful comparison:
Area A Queue Group 12/20, After Show Hangout 8/12, and Photo Walk (Parade ver.)
5/8. Actual place/time and absent member portraits remain truthful fixture data;
the reference does not authorize fabricated members, times or creation capability.

Historical capture source: `12a5d394eeabc89e1c7b0012fb51038f737c9a7f`.
Its CSS SHA256 is `6e7c259074016d9f05d90cba8a3355403906a4330a384548302c8abe2b564e54`.
The fresh base CSS SHA256 is
`264b4c5304ada1419b46f73d3cdb0168505234a0d308289560967a495d801350`.
The intervening CSS difference is only the retained title-hit fix
(`pointer-events:none` on static place/time) and its comment. Thus the historical
15/12/13px title/place/JOIN hierarchy was still current before this correction.

Names now use 18px, place/time 14px at weight 600 with ink `#3e3229`, and JOIN
14px. Name tracking changes from -.035em to -.02em; line height 1.15 and weight
800 stay. Place glyph grows from 11×13 to 12×15px. Place margin changes from
-22px 0 7px to 2px 0 6px, keeping metadata below potentially wrapped titles and
retaining the full 44px title target and pointer-events safety. This leaves more
vertical space below single-line names; it is not a claim of exact target spacing.
Portraits, member/capability state, columns, row minimum, filters, Create Party,
unavailable copy, footer, focus rules and behavior are unchanged.

Candidate CSS SHA256:
`0f4b94ffb12cf8890d6e7c4e258e69b68833287dca0e8ad275a69d0700ebdea8`.

| Measurement at 390×637 | Base | Candidate |
| --- | ---: | ---: |
| Name / place / JOIN font | 15 / 12 / 13px | 18 / 14 / 14px |
| Place weight | 400 | 600 |
| Name/action size ratio | 1.154 | 1.286 |
| First name ink width / height | 111.328 / 17px | 138.453 / 21px |
| Each row height | 110px | 111.1875px |
| Create Party y / height | 487.953125 / 46px | unchanged |
| Unavailable copy y / height | 538.953125 / 13.75px | unchanged |

The 1.1875px row growth is about 1.08%. All captured candidate rows avoid
name/place text overlap; controls retain at least 44px nominal targets. At 420px
height the content scrolls normally while Create Party, copy and footer remain
visible. Scrolling and the actual final-row JOIN were exercised.

## Qualification

The new runner imports the existing qualified QA06 Meet CTA fixture read-only.
It uses the actual default app/root/router, canonical Auth and selected
whole-character renderer with synthetic HTTP/C05/test admission. No panel or
renderer override is used. Its baseline phase serves the exact immutable base
CSS through a Vite pre-transform; the candidate phase serves the unchanged
committed CSS. Other runtime inputs stay constant. Both served CSS hashes,
23 source pins, asset reads and detailed measurements are in
[the native report](../evidence/qa06-meet-type/native/report.json).

One named slot `QA06-MEET-TYPE-202611040608Z` was explicitly offered, accepted
and fully released to BackendSL21d and Community. One Chrome process and serial
Vite phases ran; there was no production build, installation, Cargo process,
shared-target deletion or peer cleanup. Start free disk was 31.469276GiB; final
fresh free disk was 35.359058GiB. Retained native output was 873,213 bytes before
log/report sealing. Browser, Vite, profile/cache directories and the temporary
dependency symlink were removed before release; no validation slot remains held.

All eight views passed, exit 0: baseline 390×637; candidate 390×637,
360/390/430×844 and 360/390/430×420. All seven candidate cases exercised actual
title and JOIN touch navigation with Back, filter expansion/input/submission,
People/Back, MY PLAN/Parties, visible keyboard focus, and one canvas/world socket
at a time. No page errors or horizontal overflow occurred. Disabled Create Party
and its copy were initially reachable in every case. Required short-height and
637px content scrolling passed. The existing uncertain-POST flow was not rerun
natively for this CSS change; affected logic suites supply its retained coverage.

All eight PNGs are retained under `../evidence/qa06-meet-type/native/`.
Individually visually inspected: baseline 390×637, candidate 390×637,
360×844, 430×844 and 360×420. The other three were generated and asserted,
without a separate individual visual inspection. No native case failed;
historical captures and failures remain untouched.

Pinned Node: 24.22.2 at
`/Users/lhs/Desktop/mcr-tools/node-v24.22.2-darwin-arm64/bin/node`.
Native browser: Chrome 156.0.8078.80. Commands below use that Node from `frontend/`
except the two root static checks. Logs are retained as gzip in the evidence root.

| Command | Result |
| --- | --- |
| `node node_modules/vitest/vitest.mjs run src/features/party-discovery/__tests__ --cache=false --maxWorkers=1` | PASS, exit 0; 71 tests / 5 files |
| `node node_modules/typescript/bin/tsc --project e2e/community-client/qa06-meet-type/tsconfig.json --pretty false` | PASS, exit 0; strict imported-app noemit |
| `node --check e2e/community-client/qa06-meet-type/native.mjs` | PASS, exit 0 |
| `MCR_NATIVE_SLOT=QA06-MEET-TYPE-202611040608Z node e2e/community-client/qa06-meet-type/native.mjs` | PASS, exit 0; 8 native views |
| `python3 scripts/mcr/check-plan.py` | PASS, exit 0; planning only |
| `node scripts/mcr/check-ui-origin.mjs` | PASS, exit 0; static origin only |
| `git diff --check` | PASS, exit 0 before handoff |

Production build: **NOT_RUN**. The bounded CSS change was compiled through native
Vite, with affected logic and imported-app type checks; another build would not
resolve an additional identified risk. Physical iOS/Android, real HTTP/C05 service,
real multiuser and whole ten-screen/three-era qualification: **NOT_RUN** here.

## Integration drift and limits

All 23 recorded source hashes match the qualified local checkout. Fresh remote
`bfa0193939c3eebd6c80ade85083c6530c6875e1` has independent shared-frame/footer
changes in `frontend/src/app/shell-chrome/parade-footer.tsx`,
`frontend/src/app/shell-chrome/shell-chrome.css`, `frontend/src/app/shell.css`, and
`frontend/src/shared/ui/concert-window/concert-window.css`. Their exact diff was
read: border widths and footer dimensions stay constant; artwork, border style,
backgrounds and scuff treatment change. There is no remote Meet CSS conflict.
These captures therefore retain the older pinned shared appearance and do not
certify owner 213's newly published artwork. Root153 receives this explicit drift
classification with the handoff. No shared file was adopted or rewritten here.

Exact reference SHA256:
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
Historical frame SHA256:
`7cd49e1a0e6348e42f009966f554d52377dd89d90d74fd9f65b7196505d4bb02`.
Read-only source paths/revisions, source-pack/lockfile/contracts and reference
provenance are recorded in
[source-provenance.json](../evidence/qa06-meet-type/source-provenance.json).
Human reference remains the target; the bounded hierarchy improvement does not
claim complete pixel equivalence, new character art, broader QA06 acceptance or
physical arrival from virtual positions.
