#!/usr/bin/env python3
"""Map4 PARADE ground presentation only; immutable World sources remain inputs.

Run only under a fresh accepted source-output allocation. No browser, network,
source generation service, child process or temporary output is used.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import platform
from pathlib import Path
import xml.etree.ElementTree as ET

from PIL import Image, ImageChops, ImageDraw, ImageFilter, __version__ as PILLOW_VERSION

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
SIZE = (768, 1296)
GEOMETRY_HASH = "sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f"
ALLOCATION = "MAP4-FIRSTVIEW-PARADE-ART-202611040944Z"
BASE = "20682b5736838a4acbdc79e000350834cc553961"
PLAZA_FILL = "#9fba96"
SOURCE = "frontend/public/world/reference-scene-v2/quality-v6/ground.png"
LAYOUT = "frontend/public/world/reference-scene-v2/quality-v6/ground-layout.svg"
PROPOSAL = "frontend/public/world/reference-scene-v2/spatial-v3/geometry-proposal.json"
RUNTIME_SOURCE = "frontend/src/world/reference-scene/spatial-v3/source-inputs.json"
BINDING = "frontend/src/app/world-entry/__tests__/reference-scene-root/source-inputs/source-binding.json"
INPUT_HASHES = {
    SOURCE: "e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795",
    LAYOUT: "8b4cd3f33dcad57e980c4c0fc9067fa35fb1f0e8aaf8b72d992dedad26195187",
    PROPOSAL: "373dfdb9ee119c5c2ae39b9b5843732e9fb6ca3d7030c2a26f026adc40a1e8dc",
    RUNTIME_SOURCE: "847d9c92a4ddfa22b8187bb6afe9a9fc2e6e09b1c7fe8e5f85c5a7f5c0d846a9",
    BINDING: "0f15f5614d5502442a4e58cc995041f7ee1f2f906e21d4b630db7b13af65ee6b",
}
# Exact channel transform: clamp(floor((dot(integer coefficients, RGB)+offset+50)/100)).
# This applies at the original pixel, without resampling or spatial displacement.
RGB = ((88, 0, 15, 3700), (0, 85, -25, 9100), (0, 0, 42, 12000))
PALETTE = {"rose": "#edb3be", "lilac": "#c9b9dc", "cream": "#fff0cb",
           "center": "#dca761", "leaf": "#719d78"}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=True) + "\n").encode()


def pin(path: Path, data: bytes | None = None) -> dict:
    raw = path.read_bytes() if data is None else data
    return {"path": str(path.relative_to(ROOT)), "bytes": len(raw), "sha256": "sha256:" + digest(raw)}


def inputs():
    pins = []
    for name, expected in INPUT_HASHES.items():
        path = ROOT / name
        raw = path.read_bytes()
        assert digest(raw) == expected, name
        pins.append(pin(path, raw))
    proposal = json.loads((ROOT / PROPOSAL).read_bytes())
    runtime = json.loads((ROOT / RUNTIME_SOURCE).read_bytes())
    binding = json.loads((ROOT / BINDING).read_bytes())
    assert proposal["geometry_hash"] == runtime["geometry_hash"] == binding["geometry_hash"] == GEOMETRY_HASH
    assert binding["map_id"] == "mcr_reference_scene_r3" and binding["map_revision"] == 4
    assert len(binding["scenes"]) == 3
    shared_ground = [scene["runtime_resources"][0] for scene in binding["scenes"]]
    assert all(ground == shared_ground[0] for ground in shared_ground)
    assert shared_ground[0]["sha256"] == "sha256:" + INPUT_HASHES[SOURCE]
    assert proposal["geometry"] == runtime["geometry"]
    assert proposal["plaza_parcel"] == runtime["plaza_parcel"]
    xml = ET.parse(ROOT / LAYOUT).getroot()
    polygons = [e for e in xml if e.get("id", "").startswith("paving-")]
    assert len(polygons) == len(proposal["paving_polygons"]) == 47
    for element, coordinates in zip(polygons, proposal["paving_polygons"]):
        actual = [[float(n) for n in point.split(",")] for point in element.attrib["points"].split()]
        assert actual == coordinates, element.attrib["id"]
    with Image.open(ROOT / SOURCE) as image:
        assert image.size == SIZE and image.mode == "RGB"
        original = image.copy()
    return pins, proposal, original


def channel(pixel, row):
    return max(0, min(255, (sum(a * b for a, b in zip(pixel, row[:3])) + row[3] + 50) // 100))


def transfer(original: Image.Image) -> Image.Image:
    # Integer implementation makes the product PNG recipe independent of SVG filter engines.
    out = bytearray(SIZE[0] * SIZE[1] * 3)
    for i, pixel in enumerate(original.get_flattened_data()):
        for c, row in enumerate(RGB):
            out[i * 3 + c] = channel(pixel, row)
    return Image.frombytes("RGB", SIZE, bytes(out))


def ground_masks(proposal):
    paving = Image.new("L", SIZE)
    draw = ImageDraw.Draw(paving)
    for polygon in proposal["paving_polygons"]:
        draw.polygon([tuple(point) for point in polygon], fill=255)
    # The existing Map-only plaza is painted after ground by the consumer; avoid its edge too.
    draw.polygon([tuple(point) for point in proposal["plaza_parcel"]], fill=255)
    interior = ImageChops.invert(paving.filter(ImageFilter.MaxFilter(21)))
    nearby = paving.filter(ImageFilter.MaxFilter(65))
    edge_lawn = ImageChops.multiply(interior, nearby)
    return paving, interior, edge_lawn


def flowers(edge_lawn, interior):
    centers = []
    # Deterministic sparse edging. These are flat ground pixels, never objects or obstacles.
    for y in range(24, SIZE[1] - 24, 14):
        for x in range(20, SIZE[0] - 20, 14):
            if not edge_lawn.getpixel((x, y)) or (x * 17 + y * 31) % 11 > 3:
                continue
            if any((x - px) ** 2 + (y - py) ** 2 < 45 ** 2 for px, py in centers):
                continue
            if all(interior.getpixel((x + dx, y + dy)) for dx in (-8, 0, 8) for dy in (-8, 0, 8)):
                centers.append((x, y))
    return centers[:96]


def flower_shapes(centers):
    shapes = []
    for i, (x, y) in enumerate(centers):
        petal = [PALETTE["rose"], PALETTE["lilac"], PALETTE["cream"]][i % 3]
        shapes.extend([
            ([x - 7, y + 3, x, y + 7], PALETTE["leaf"]),
            ([x, y + 2, x + 7, y + 6], PALETTE["leaf"]),
            ([x - 3, y - 7, x + 3, y - 1], petal),
            ([x - 7, y - 3, x - 1, y + 3], petal),
            ([x + 1, y - 3, x + 7, y + 3], petal),
            ([x - 3, y + 1, x + 3, y + 7], petal),
            ([x - 2, y - 2, x + 2, y + 2], PALETTE["center"]),
        ])
    return shapes


def render_png(original, shapes, interior):
    painted = transfer(original)
    decoration = Image.new("RGB", SIZE)
    mask = Image.new("L", SIZE)
    d, m = ImageDraw.Draw(decoration), ImageDraw.Draw(mask)
    for box, color in shapes:
        d.ellipse(box, fill=color)
        m.ellipse(box, fill=255)
    # Clip even the rasterized ellipse fringe to the safe lawn interior.
    mask = ImageChops.multiply(mask, interior)
    painted.paste(decoration, mask=mask)
    output = io.BytesIO()
    painted.save(output, format="PNG", optimize=True)
    return output.getvalue(), mask


def render_svg(original_bytes, proposal, shapes):
    # The editable source retains exact original bytes, affine color paint, original
    # paving coordinates as exclusion geometry, and individually editable flat flowers.
    matrix = " ".join(str(n / 100) for row in RGB for n in (*row[:3], 0, row[3] / 255))
    matrix += " 0 0 0 1 0"
    polygons = "".join('<polygon points="' + " ".join(f"{x},{y}" for x, y in points) + '" fill="black"/>'
                       for points in [*proposal["paving_polygons"], proposal["plaza_parcel"]])
    ellipses = "".join(f'<ellipse cx="{(a+c)/2}" cy="{(b+d)/2}" rx="{(c-a)/2}" ry="{(d-b)/2}" fill="{color}"/>'
                       for (a, b, c, d), color in shapes)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="768" height="1296" viewBox="0 0 768 1296">'
            '<title>Map4 PARADE-only warm ground; original geometry and World frozen</title>'
            '<metadata>paint.py is the exact integer raster authority; SVG filters may round differently across engines.</metadata>'
            '<defs><filter id="warm-paint" x="0" y="0" width="100%" height="100%" color-interpolation-filters="sRGB">'
            f'<feColorMatrix type="matrix" values="{matrix}"/></filter>'
            '<mask id="lawn-only" maskUnits="userSpaceOnUse" x="0" y="0" width="768" height="1296">'
            f'<rect width="768" height="1296" fill="white"/>{polygons}</mask></defs>'
            f'<image id="immutable-map4-ground" width="768" height="1296" href="data:image/png;base64,{base64.b64encode(original_bytes).decode()}" filter="url(#warm-paint)"/>'
            f'<g id="editable-flat-floral-edging" mask="url(#lawn-only)">{ellipses}</g></svg>\n').encode()


def build():
    assert ALLOCATION != "PENDING_FRESH_RESOURCE_OFFER", "Fresh accepted allocation required"
    pins, proposal, original = inputs()
    paving, interior, edge_lawn = ground_masks(proposal)
    centers = flowers(edge_lawn, interior)
    shapes = flower_shapes(centers)
    png, mask = render_png(original, shapes, interior)
    svg = render_svg((ROOT / SOURCE).read_bytes(), proposal, shapes)
    paths = {"svg": HERE / "parade-ground.svg", "png": HERE / "parade-ground.png"}
    for key, data in [("svg", svg), ("png", png)]:
        paths[key].write_bytes(data)
    source = {
        "format": "MCR_MAP4_FIRSTVIEW_GROUND_V1", "status": "SOURCE_VALIDATED_NATIVE_NOT_RUN",
        "allocation": ALLOCATION, "base_revision": BASE,
        "scope": "PARADE-authored shared Map4 ground; reusable only under each exact supported original theme tuple; not World/C03 asset replacement or map admission",
        "shared_ground_role": {"supported_themes": ["REVENGE", "PARADE", "DANGER"],
                              "basis": "All three pinned map4 scene runtime_resources use the identical original ground descriptor",
                              "era_specific_props_redrawn": False, "separate_era_exports": 0,
                              "consumer_adoption_and_native_all_era_validation": "REQUIRED_SEPARATELY"},
        "map_id": "mcr_reference_scene_r3", "map_revision": 4, "geometry_hash": GEOMETRY_HASH,
        "dimensions": {"width": 768, "height": 1296}, "inputs": pins,
        "output": pin(paths["png"], png), "editable_source": pin(paths["svg"], svg),
        "recipe": pin(HERE / "paint.py"),
        "rgb_integer_rows": RGB, "rounding": "clamp((dot(RGB,row)+offset+50)//100)",
        "palette": PALETTE,
        "plaza": {"fill": PLAZA_FILL, "points": proposal["plaza_parcel"],
                  "stroke": "#b4a27d", "stroke_width": 6, "geometry_change": False},
        "geometry": {"paving_polygons": 47, "paving_coordinates_sha256": digest(encoded(proposal["paving_polygons"])),
                     "plaza_coordinates_sha256": digest(encoded(proposal["plaza_parcel"])),
                     "exclusion_mask_sha256": digest(paving.tobytes()), "safe_lawn_mask_sha256": digest(interior.tobytes())},
        "flat_detail": {"kind": "small flat petal and leaf ellipses; no props, targets or obstacles",
                        "centers": centers, "count": len(centers), "painted_pixels": mask.histogram()[255],
                        "mask_sha256": digest(mask.tobytes()), "minimum_path_clearance_px": 10},
        "preservation": {"original_bytes": "exact pinned input, retained immutable",
                         "spatial_resampling": False, "world_changes": False, "character_changes": False,
                         "props_atlases_signage_changes": False, "geometry_camera_changes": False,
                         "new_navigation_or_landmarks": False},
        "runtime": {"python": platform.python_version(), "pillow": PILLOW_VERSION,
                    "temporary_files": 0, "child_processes": 0, "browser_or_generation_calls": 0},
        "limits": "Source only. No native actual-default Map4 comparison, all-era, human, live/device or parent acceptance."
    }
    (HERE / "source-provenance.json").write_bytes(encoded(source))
    print(json.dumps({"status": "SOURCE_READY", "png": source["output"], "flowers": len(centers), "plazaFill": PLAZA_FILL}))


def verify():
    pins, proposal, original = inputs()
    record = json.loads((HERE / "source-provenance.json").read_bytes())
    assert record["inputs"] == pins and record["allocation"] == ALLOCATION
    for key in ["output", "editable_source", "recipe"]:
        expected = record[key]
        assert pin(ROOT / expected["path"]) == expected, key
    paving, interior, edge_lawn = ground_masks(proposal)
    centers = flowers(edge_lawn, interior)
    shapes = flower_shapes(centers)
    expected, mask = render_png(original, shapes, interior)
    assert expected == (HERE / "parade-ground.png").read_bytes()
    assert render_svg((ROOT / SOURCE).read_bytes(), proposal, shapes) == (HERE / "parade-ground.svg").read_bytes()
    assert ImageChops.multiply(mask, paving).getbbox() is None
    assert record["plaza"]["points"] == proposal["plaza_parcel"] and record["plaza"]["fill"] == PLAZA_FILL
    assert record["geometry"]["paving_coordinates_sha256"] == digest(encoded(proposal["paving_polygons"]))
    assert centers and len(centers) <= 96
    with Image.open(HERE / "parade-ground.png") as image:
        assert image.size == SIZE and image.mode == "RGB"
    total = sum(path.stat().st_size for path in HERE.iterdir() if path.is_file())
    assert total < 3 * 1024 * 1024 and len(list(HERE.iterdir())) == 4
    print(json.dumps({"status": "PASS_SOURCE_ONLY_NATIVE_NOT_RUN", "paving_polygons": 47,
                      "original_input_pins": len(pins), "flowers": len(centers), "durable_bytes": total,
                      "temporary_bytes": 0, "owned_children": 0}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true", help="Read-only checks; no output files")
    arguments = parser.parse_args()
    verify() if arguments.verify else build()
