#!/usr/bin/env python3
"""Extract approved Feed screenshot media as lossless, native-size fixtures."""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image

SOURCE_PATH = "docs/mcr/design/reference-target-2026-11-04.png"
SOURCE_COMMIT = "cdcd834f6551941c3b0693552370fd01c45f0f1d"
SOURCE_GIT_BLOB = "feeeb7ea637db1ef94d1cbfad970d405e28e1c99"
SOURCE_SHA256 = "d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617"
SOURCE_BYTES = 2967126
SOURCE_SIZE = (1536, 1024)
CROPS = [
    ("reference-card-1.png", (80, 704, 277, 771), "Visible wide concert-media thumbnail in the first Feed reference card"),
    ("reference-card-2.png", (80, 885, 277, 931), "Visible wide sticker and band-logo media thumbnail in the second Feed reference card"),
]


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    args = parser.parse_args()
    original = args.source.read_bytes()
    assert len(original) == SOURCE_BYTES and sha(original) == SOURCE_SHA256, "Approved reference bytes required"
    target = Path(__file__).resolve().parent
    records = []
    with Image.open(args.source) as source:
        assert source.format == "PNG" and source.mode == "RGB" and source.size == SOURCE_SIZE
        source.load()
        source_rgb_sha = sha(source.tobytes())
        for name, box, description in CROPS:
            left, top, right, bottom = box
            assert 0 <= left < right <= source.width and 0 <= top < bottom <= source.height
            crop = source.crop(box)
            # No conversion, filtering, resizing, synthesis, metadata or new pixels.
            assert crop.mode == "RGB" and crop.size == (right - left, bottom - top)
            path = target / name
            crop.save(path, format="PNG", optimize=False, compress_level=9)
            with Image.open(path) as emitted:
                assert emitted.mode == source.mode and emitted.size == crop.size
                assert emitted.tobytes() == crop.tobytes(), "Lossless native pixel equality required"
            records.append({
                "path": "frontend/public/reference-feed-media-v1/" + name,
                "description": description,
                "source_box_ltrb_exclusive_right_bottom": list(box),
                "dimensions": list(crop.size), "mode": crop.mode,
                "bytes": path.stat().st_size, "sha256": sha(path.read_bytes()),
                "rgb_sha256": sha(crop.tobytes()), "native_source_pixel_identity": True,
                "use": "Reference fixture media only; never a real user post or original photograph"
            })
    assert sha(args.source.read_bytes()) == SOURCE_SHA256, "Reference must remain unchanged"
    result = {
        "schema_version": 1, "status": "EXTRACTED_NATIVE_PIXEL_IDENTITY_PASS",
        "authorization": "Human/Main Feed-media-only reference derivative lease relayed by Root153",
        "source": {"repository_path": SOURCE_PATH, "commit": SOURCE_COMMIT, "git_blob": SOURCE_GIT_BLOB,
                   "sha256": SOURCE_SHA256, "bytes": SOURCE_BYTES, "dimensions": list(SOURCE_SIZE),
                   "mode": "RGB", "rgb_sha256": source_rgb_sha, "unchanged_after_extraction": True},
        "method": "Integer rectangular crop only, lossless PNG encoding, original RGB pixels; no resampling/upscale/inpainting/synthesis",
        "extractor_sha256": sha(Path(__file__).read_bytes()), "derivatives": records,
        "fixture_display_policy": {"complete_media_input": True, "card_and_detail_native_dimensions": True, "allow_fit_down": True, "allow_upscale": False, "provenance_labels": "Artifact/test metadata only; no product navigation badges"},
        "limits": "Only tiny visible screenshot thumbnails are available: 197x67 and 197x46. Original photo files, higher resolution, authorship, venue, date and real-post provenance were not supplied. Do not infer them or reuse avatars, characters, venue/map imagery or surrounding interface. Fixture/provenance identity belongs only in artifact and test metadata, not product navigation. Each crop is the complete fixture media input: use its native width/height in card and detail, fit down only, never upscale. No real-post/server-contract claim."
    }
    (target / "source-provenance.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
