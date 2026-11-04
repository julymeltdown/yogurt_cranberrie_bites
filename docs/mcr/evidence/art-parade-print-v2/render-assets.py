#!/usr/bin/env python3
"""Rasterize the owned editable SVG; this does not launch an app or browser."""
import hashlib
import json
import argparse
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

import fitz
from PIL import Image


ROOT = Path(__file__).resolve().parents[4]
ASSETS = ROOT / "frontend/public/reference-parade-print-v2"
EVIDENCE = Path(__file__).resolve().parent
SOURCE = ASSETS / "parade-print.svg"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main(output_dir=ASSETS, report_dir=EVIDENCE, source_path=SOURCE):
    assert output_dir.is_relative_to(ASSETS) or output_dir.is_relative_to(EVIDENCE)
    assert report_dir.is_relative_to(EVIDENCE)
    assert source_path.is_relative_to(ASSETS) or source_path.is_relative_to(EVIDENCE)
    output_dir.mkdir(parents=True, exist_ok=True)
    report_dir.mkdir(parents=True, exist_ok=True)
    source = source_path.read_bytes()
    tree = ET.fromstring(source)
    assert tree.attrib["viewBox"] == "0 0 360 96"
    assert tree.attrib["width"] == "360" and tree.attrib["height"] == "96"
    assert tree.attrib["id"] == "parade-print-v2"
    ids = []
    path_count = 0
    for node in tree.iter():
        tag = node.tag.split("}")[-1]
        assert tag in {"svg", "title", "desc", "g", "path"}, tag
        if "id" in node.attrib:
            ids.append(node.attrib["id"])
        path_count += tag == "path"
        for key, value in node.attrib.items():
            assert not key.lower().startswith("on"), key
            assert key.split("}")[-1] not in {"href", "style"}, key
            assert "url(" not in value and "http" not in value, value
    assert ids == ["parade-print-v2", "parade-print-ink"]
    assert path_count >= 12
    result = {
        "task": "ART-PARADE-PRINT-02",
        "scope": "Source/asset validation only; native integration NOT_RUN",
        "source": {
            "path": str(source_path.relative_to(ROOT)),
            "bytes": len(source),
            "sha256": sha(source),
            "viewBox": [0, 0, 360, 96],
            "pathCount": path_count,
            "externalReferences": 0,
            "activeContent": False,
        },
        "renderer": {"name": "PyMuPDF", "version": fitz.VersionBind},
        "python": sys.version.split()[0],
        "outputs": [],
    }
    # MuPDF does not resolve SVG's inherited color for currentColor here.
    # Resolve that one paint token for raster export; leave editable source intact.
    assert source.count(b'fill="currentColor"') == 1
    render_source = source.replace(b'fill="currentColor"', b'fill="#191817"')
    result["rasterExportColorResolution"] = {
        "from": "currentColor",
        "to": "#191817",
        "reason": "MuPDF inherited SVG color is not resolved; one paint token only",
        "renderInputSha256": sha(render_source),
        "editableSourceChanged": False,
        "browserColorAcceptance": "NOT_RUN",
    }
    document = fitz.open(stream=render_source, filetype="svg")
    for scale, name in [(1, "parade-print-preview.png"), (2, "parade-print@2x.png")]:
        pixmap = document[0].get_pixmap(matrix=fitz.Matrix(scale, scale), alpha=True)
        path = output_dir / name
        pixmap.save(path)
        raster = Image.open(path).convert("RGBA")
        assert raster.size == (360 * scale, 96 * scale)
        alpha = raster.getchannel("A")
        histogram = alpha.histogram()
        assert histogram[0] > raster.width * raster.height * .35
        assert histogram[255] > raster.width * raster.height * .20
        assert alpha.getpixel((0, 0)) == 0
        assert alpha.getpixel((180 * scale, 95 * scale)) == 255
        bounds = alpha.getbbox()
        assert bounds[0] == 0 and bounds[2:] == (raster.width, raster.height)
        raw = raster.tobytes()
        ink_rgb = {(r, g, b) for r, g, b, a in zip(raw[0::4], raw[1::4], raw[2::4], raw[3::4]) if a == 255}
        assert ink_rgb == {(25, 24, 23)}, ink_rgb
        payload = path.read_bytes()
        result["outputs"].append({
            "path": str(path.relative_to(ROOT)),
            "bytes": len(payload),
            "sha256": sha(payload),
            "size": list(raster.size),
            "mode": "RGBA",
            "nonzeroAlphaBounds": list(bounds),
            "fullyTransparentFraction": histogram[0] / (raster.width * raster.height),
            "opaqueInkFraction": histogram[255] / (raster.width * raster.height),
            "alphaWeightedInkFraction": sum(i * count for i, count in enumerate(histogram)) / (255 * raster.width * raster.height),
            "opaqueInkRgb": [25, 24, 23],
        })
    document.close()
    review_source = render_source.replace(
        b'<g id="parade-print-ink"',
        b'<path d="M0 0h360v96H0Z" fill="#e9e1d4"/><g id="parade-print-ink"',
    )
    review = fitz.open(stream=review_source, filetype="svg")
    review_path = report_dir / "source-review-paper.png"
    review[0].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False).save(review_path)
    review.close()
    result["sourceReview"] = {
        "path": str(review_path.relative_to(ROOT)),
        "sha256": sha(review_path.read_bytes()),
        "description": "Source render on #e9e1d4 paper for inspection, not an app capture or public asset",
    }
    result["status"] = "PASS_SOURCE_ONLY"
    (report_dir / "source-validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ASSETS)
    parser.add_argument("--report-dir", type=Path, default=EVIDENCE)
    parser.add_argument("--source", type=Path, default=SOURCE)
    args = parser.parse_args()
    main(args.output_dir.resolve(), args.report_dir.resolve(), args.source.resolve())
