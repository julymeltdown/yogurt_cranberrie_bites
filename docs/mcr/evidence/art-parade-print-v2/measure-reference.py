#!/usr/bin/env python3
"""Measure manually inspected bottom-band regions without modifying the references."""
import hashlib
import json
import argparse
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = Path(__file__).resolve().parent
REFERENCES = [
    (
        "canonical",
        ROOT / "docs/mcr/design/reference-target-2026-11-04.png",
        [
            ("meet", [316, 894, 592, 1011]),
            ("gifts", [620, 927, 906, 1011]),
            ("profile", [1233, 925, 1523, 1011]),
        ],
    ),
    (
        "supplied-jpeg",
        Path("/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg"),
        [
            ("meet", [262, 742, 505, 839]),
            ("gifts", [520, 772, 756, 839]),
            ("profile", [1030, 774, 1269, 839]),
        ],
    ),
]


def main(report_dir=EVIDENCE):
    assert report_dir.is_relative_to(EVIDENCE)
    report_dir.mkdir(parents=True, exist_ok=True)
    result = {
        "task": "ART-PARADE-PRINT-02",
        "method": "Direct visual region selection; exclusive right/bottom bounds. Dark-ink proxy is all RGB channels below95, including photographic grain and control overlap. No tracing, resampling or copying reference pixels into art.",
        "normalization": "372px illustrative inner width at390px viewport, not measured app layout",
        "limits": [
            "Manually inspected regions with approximately3px edge uncertainty; not semantic figure segmentation.",
            "Meet includes the overlapping Create Party control and is taller than the other two bands. Its dark fraction is not independent ornament density.",
            "A shared96-unit source is justified by Gifts/Profile figure and ground extents, not a claim to reproduce every route's occupied height.",
            "Reference measurements are source preparation; actual placed boxes and CTA/scroll clearance require retained native evidence.",
        ],
        "references": [],
    }
    for label, path, regions in REFERENCES:
        raw = path.read_bytes()
        image = Image.open(path).convert("RGB")
        entry = {
            "id": label,
            "path": str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path),
            "bytes": len(raw),
            "sha256": hashlib.sha256(raw).hexdigest(),
            "size": list(image.size),
            "regions": [],
        }
        for route, box in regions:
            crop = image.crop(box)
            rows = [sum(max(crop.getpixel((x, y))) < 95 for x in range(crop.width)) / crop.width for y in range(crop.height)]
            entry["regions"].append({
                "route": route,
                "region": box,
                "size": [crop.width, crop.height],
                "heightAt372pxWidth": crop.height * 372 / crop.width,
                "darkInkProxyFraction": sum(rows) / crop.height,
                "rowDarkFraction": [{"y": i, "fraction": rows[i]} for i in range(0, crop.height, 8)],
            })
        result["references"].append(entry)
    (report_dir / "reference-measurements.json").write_text(json.dumps(result, indent=2) + "\n")
    print("PASS: both references hashed and six inspected regions measured; originals untouched")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report-dir", type=Path, default=EVIDENCE)
    args = parser.parse_args()
    main(args.report_dir.resolve())
