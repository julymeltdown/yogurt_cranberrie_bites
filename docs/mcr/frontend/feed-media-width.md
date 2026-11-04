# Feed post media width

Task: `FEED-MEDIA-WIDTH-01`. Bounded source/native status: MOCK_READY / PASS.
Publication: pending Root153's canonical verified receipt.

Root153 transferred temporary sole-writer ownership of the post-media width
rules in `frontend/src/features/event-feed/feed.css`, this document and
`docs/mcr/evidence/feed-media-width/**`. Root retains the remaining stylesheet,
shared media, caller/composer, background and publication ownership. This is a
bounded follow-up to the published post-only cohort at
`7eac13ce30d6dd6be4df30727f75d5df90d3489d`; its existing receipts remain valid
for their original scope.

The reported actual 390px defect is an unnecessarily narrow thumbnail despite
available card width. The candidate changes only the post figure's width from
`min(100%, 228px)` to `min(100%, 298px)`. With the existing symmetric 1px borders
and 3px padding, an uncapped image grows from 220px to 290px. The 100% limit
continues to fit narrower containers. The shared selector covers actual Feed
post and Thread article media without styling surrounding non-post UI.

The qualified baseline is source
`1e07321cede694b66aae06184fd05c624a5a0ccb`, stylesheet SHA-256
`477adc0571c6151d94c2b91208b4c01397e2c3c93b663dc22964b06d91cb5777`.
Candidate stylesheet SHA-256:
`0793335a6ba7007d271cc7afdb54493aa3996cb2f08391b179c8e2144efb5feb`.
An exact byte comparison confirms that the single width declaration is the only
product change. No source is copied from another product and no dependency,
asset, contract, markup or media-reader change is introduced.

Natural aspect ratio, the 18rem image/wrapper ceiling, authorized offscreen
metadata, privacy withdrawal, complete contain rendering, readable failure
states, selected character bytes, 44px actions, keyboard focus and non-post UI
remain unchanged. Expected uncapped fixture dimensions are 290×98.6294px for
197×67 media and 290×67.7157px for 197×46 media. These are calculations, not
runtime observations.

Owner209 and owner230 coordinated one coupled capture after the selected-avatar
Thread source fix became available. That source readiness does not impose a broader
Thread or Wall completion gate. The run must use exact source/hash inputs and
an accepted resource allocation, check responsive Feed widths 360/390/430,
actual Thread media, stable observer exit/return geometry and existing
privacy/focus/control behavior. This CSS owner assesses the same saved images
and resolves any width-scoped failures. No duplicate browser or implicit lease
extension is authorized. New output will be recorded under this task's evidence
directory; old raw results remain untouched.

Completion includes the responsive implementation, relevant developer-owned
validation and fixes, evidence/documentation, committed direct handoff and
Root's canonical verified publication receipt. No king/SL review,
acknowledgment, own consumer readback or proof-only successor commit is needed.
The existing persistent Feed foreground goal remains aligned under the latest
human clarification. Its BLOCKED tool metadata is preserved because no resume
API is exposed; the goal is neither reset nor falsely completed.

Source-boundary evidence: [source-scope.json](../evidence/feed-media-width/source-scope.json).

## Qualified coupled result

Owner209's committed packet is
`449f0a92cb413fd04c91e1eec90e82884ce17c41`, under
`docs/mcr/evidence/feed-root-composition/media-width-round-1/`.
Allocation `FE209-WIDTH-POSTMEDIA-NATIVE-202611041649Z` ran the exact
`ac399c44074a157a7d6bfe9475382672997cc109` stylesheet with shared media
`49b8bfde`, backdrop `375c73df` and selected-author caller `5447e338`.
The tested fixture source is `8eee70df2956fffc37c7b3ef30ef3411fefefdbb`;
its unrelated IN03 source bundle was not executed and is excluded from this
width qualification. Native and supervisor/driver syntax checks exited 0;
six synthetic contexts passed with no errors.

The CSS implementation owner inspected the same six full Feed/Thread PNGs
at 360/390/430px. Their bytes match the immutable committed packet; no browser
rerun or image copy was added. Feed images are 290px wide at all three widths.
Thread uses 268px images inside its narrower 360px layout and 290px at 390/430px.
Images remain complete and proportionate, selected character artwork is visible,
post text remains readable, surrounding non-post controls retain their styling,
and no horizontal clipping appears. No width-scoped correction is required.

Actual first-image height is 98.625px for 290px-wide media and 91.140625px for
268px-wide Thread media. All six heights and per-case scroll extents remain
stable across loaded, observer-outside and return. The image DOM disappears
outside and returns afterward. Visible focus, 44px controls, privacy withdrawal
and zero remaining Blob URLs pass in the responsible owner's result.

FULL release at `2026-11-04T16:53:42.611437+00:00` records no owned processes,
listener, dependency link or cache/temp, with successful port5279 reuse bind.
Prior short-height, negative-state, legacy-author and unchanged source checks
are reused under their original sealed receipts; they were not replayed here.
This synthetic result does not accept live IN03, arbitrary media ratios or the
complete ten-screen visual target.

Exact commands, input/artifact hashes, measured geometry and CSS-owner image
disposition: [native-assessment.json](../evidence/feed-media-width/native-assessment.json).
Root publishes the qualified source and latest owned documents directly; its
canonical receipt closes publication without another consumer proof cycle.
