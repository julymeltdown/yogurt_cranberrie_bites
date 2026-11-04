# Feed reference media fixture derivatives

Status: **QUALIFIED fixture assets; publication/readback pending**.
Native extraction, pixel identity checks and direct crop inspection passed. No browser, build, export,
new artwork or dependency installation is needed.

## Authorization and ownership

The human/Main input relayed by Root153 permits only the visible media in the
Feed panel of `docs/mcr/design/reference-target-2026-11-04.png` to be extracted
as fixture derivatives. Writable paths are exactly
`frontend/public/reference-feed-media-v1/{source-provenance.json,reference-card-1.png,reference-card-2.png,extract.py}`
and this new document. Root is the sole publisher; the d51f Feed CSS owner
consumes the qualified PNGs with fixture identity in artifact/test metadata
only, without unsolicited product navigation labels or badges. This changes no real
post, backend response, contract, character or application code.

Work reuses `/Users/lhs/Desktop/mcr-worktrees/fe-customize-background-02`, branch
`codex/mcr-ui-bg-customize-02`. Its existing committed Customize work at
`719abad6c8ca2339fbc6f559106a1cdbff276e5b` is preserved. The actual persistent
goal is still **BLOCKED** on that different assignment's Root-only capture and
publication dependency. It is not overwritten, reset, or falsely completed for
this separate Feed asset task.

## Read-only inspection and crop plan

The approved original was directly viewed in place at the publisher's
`docs/mcr/design/reference-target-2026-11-04.png`. It is an RGB PNG, 1536x1024,
2,967,126 bytes, SHA-256
`d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617`.
The repository source commit is
`cdcd834f6551941c3b0693552370fd01c45f0f1d` and its Git blob is
`feeeb7ea637db1ef94d1cbfad970d405e28e1c99`.
The full original remains unchanged; it is not copied into the fixture asset
directory. The crop coordinates use that full native reference resolution.
This expressly approved canonical PNG is distinct from the historical
1280x853 JPEG attachment with SHA-256
`2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23`.
They depict the same visual reference; their file bytes and dimensions differ.
Only the canonical 1536x1024 PNG is used for this extraction.

The first Feed card shows a short, wide concert-media thumbnail; the second
shows a short, wide sticker/band-logo thumbnail. These are the only authorized
regions. The planned crops are inset within the visible media rectangles to
exclude their interface borders, surrounding avatars, text, counts and controls.

| Asset | Source box, exclusive right/bottom | Native output |
| --- | --- | --- |
| `reference-card-1.png` | `(80,704)..(277,771)` | 197x67 RGB |
| `reference-card-2.png` | `(80,885)..(277,931)` | 197x46 RGB |

The crop plan was returned to Root before executable extraction. It preserves
the wide media shape; it does not make square crops, reconstruct hidden
portions, enlarge, sharpen, filter, recolor, inpaint, synthesize or reuse
characters or venue/map artwork. No screenshot UI is used as application UI.

## Reproduction and limits

Within the fresh accepted resource window, run:

```sh
python3 frontend/public/reference-feed-media-v1/extract.py --source /absolute/path/to/docs/mcr/design/reference-target-2026-11-04.png
```

The script uses the already available Pillow library without installation.
It pins the entire input file, mode and dimensions, performs only integer
rectangular crops, writes lossless PNGs, reopens them to verify exact RGB pixel
equality, confirms the original remains unchanged and records full source/crop/
output hashes in `source-provenance.json`. Both emitted PNGs were directly inspected and qualified as the authorized
visible media only; no surrounding post text, avatars or controls are included.

Only screenshot thumbnails are available. The original photographs, their
native resolution, photographer, venue, date and real-post provenance were not
supplied. A screenshot extraction is not an original concert photo. Each native crop is the complete fixture media input for both card and detail.
Use the actual 197x67 and 197x46 dimensions; fit down only on a smaller viewport.
Do not upscale the assets or their display, invent a higher-resolution detail
source, or add fixture badges to normal product navigation. Keep provenance
identity in artifacts/test metadata only. They do not
establish permission or provenance for unrelated uses, real posts or media
outside this exact human-authorized fixture scope.

Publication and exact immutable PNG/source readback remain pending. This asset
handoff does not resolve the separate Customize goal's rendering requirement.

## Executed extraction and owner disposition

Allocation `FEED-REFERENCE-CROPS-202611041012Z` accepted at
2026-11-04 10:13:24.090633 UTC; deadline 10:14:24.090633 UTC with 15 seconds
reserved for cleanup. Fresh free space was 20,117,819,392 bytes, charged carry
171,877 bytes, temp zero. Latest human native-dimension/display policy updated
the tentative crop bounds before acceptance; the accepted exact source pin is
`3b7fb08cdba9bbd1397c7dba6c445382f5265b58e4d8c030d6b6f71e3483bb15`.
The absolute Python 3.14 interpreter ran the recipe with
`PYTHONDONTWRITEBYTECODE=1`; extraction and native RGB identity assertions
exited 0. No dependency install, resampling, reconstruction or other export ran.

Owner directly viewed both emitted PNGs. **PASS**: card 1 contains only the
visible wide concert thumbnail, card 2 only the wide sticker/band-logo
thumbnail. Their different native aspect ratios are intentional. No UI avatars,
post copy, counters, buttons or neighboring panels are included. These are
complete fixture inputs for card and detail; a consumer may fit them down only.
The full source file was read again and its original SHA remained unchanged.

| File | Native dimensions | Bytes | PNG SHA-256 |
| --- | --- | --- | --- |
| `reference-card-1.png` | 197x67 RGB | 25007 | `a28db3162d4fe822b17d3228ad82866d4eeb0e2524fb606d469ccd925fd7385d` |
| `reference-card-2.png` | 197x46 RGB | 20007 | `2c5863585bce002be0ea0b057f4927d67c6f99e97cc7f5edee6ba9c47589ffdd` |

Exact source pixel RGB hashes and exclusive crop boxes are retained in
`source-provenance.json`; those identity checks cover the original RGB pixels,
not merely dimensions or visual similarity. The two PNGs total 45,014 bytes.
The post-extraction entire checkout carry was 220,036 bytes (under 256KiB);
new outputs remain under 128KiB and no temp or long-lived process was created.
`git diff --check` passed. Only the five assigned new paths are committed in
this handoff; Customize source and its blocked goal remain unchanged.
Root and the existing d51f consumer receive these exact immutable asset pins;
normal Root publication and owned byte readback are still required.

### Resource timing exception

The extraction and direct image inspection passed, but the complete output
workflow did **not** meet its 60-second allocation. Final documentation,
accounting and commit finished at 10:14:29.260476 UTC, approximately 65.17
seconds after the stated acceptance clock and 5.17 seconds after its hard
deadline. The required 15-second cleanup reserve was not maintained for the
whole workflow. This is recorded as a timing exception, not an in-budget pass.
Final charged carry was 222,248 bytes, new assigned files 59,489 bytes,
temporary output zero, free space 20,114,354,176 bytes, and owned live handles
zero. Explicit FULLRELEASE and the overrun were reported to 21d, Root and the
consumer. No extraction, view or check was repeated after release. This timing
record was added through the separately permitted source/documentation and
normal committed-handoff path; the PNGs and their validated pixels are unchanged.
