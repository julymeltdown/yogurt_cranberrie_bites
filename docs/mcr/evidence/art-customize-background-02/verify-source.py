#!/usr/bin/env python3
"""Bounded stdlib source/scope proof; does not render or launch application code."""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
from xml.etree import ElementTree as ET

BASE = "55132a2f06948fced1025dd70c583b3848d32724"
ASSET = "frontend/public/backgrounds/cute-entry-v1/customize-playful-lower.svg"
CSS = "frontend/src/shared/styles/cozy-identity-backgrounds-v1.css"
DOC = "docs/mcr/art-customize-background-02.md"
EVIDENCE = "docs/mcr/evidence/art-customize-background-02/"
SELECTOR = ":root .shell-panel[data-mcr-panel='customize'] > .shell-panel-body"
ROOT = Path(__file__).resolve().parents[4]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def pin(path):
    raw = (ROOT / path).read_bytes()
    return {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def allowed(path):
    return path in (ASSET, CSS, DOC) or path.startswith(EVIDENCE)


def luminance(color):
    values = [int(color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4 for v in values]
    return sum(a * b for a, b in zip(linear, (.2126, .7152, .0722)))


def main():
    args = argparse.ArgumentParser()
    args.add_argument("--write", action="store_true")
    write = args.parse_args().write
    old = git("show", BASE + ":" + CSS).decode()
    new = (ROOT / CSS).read_text()
    pattern = re.escape(SELECTOR) + r" \{[^}]*\}"
    before, after = re.findall(pattern, old), re.findall(pattern, new)
    assert len(before) == len(after) == 1, "Exactly one existing selector required"
    editable = r"^  background-(?:image|position|size):[^;\n]+;\n"
    strip = lambda block: re.sub(editable, "", block, flags=re.M)
    assert strip(before[0]) == strip(after[0]), "Unleased declarations changed"
    assert old.replace(before[0], "<leased-rule>") == new.replace(after[0], "<leased-rule>"), "Other CSS bytes changed"
    assert "background-image: url('/backgrounds/cute-entry-v1/customize-playful-lower.svg');" in after[0]
    assert "background-position: center bottom;" in after[0]
    assert "background-size: 100% 100%;" in after[0]
    # The unchanged grouped inner editor rule still uses the original paper.
    assert "background-image: url('/backgrounds/cute-entry-v1/customize-paper.svg');" in new

    raw = (ROOT / ASSET).read_bytes()
    assert len(raw) <= 16384, "Small original vector only"
    assert not re.search(rb"<!DOCTYPE|<!ENTITY|<\?xml-stylesheet", raw, re.I)
    svg = ET.fromstring(raw)
    assert svg.tag == "{http://www.w3.org/2000/svg}svg"
    assert svg.attrib == {"width": "390", "height": "780", "viewBox": "0 0 390 780", "fill": "none"}
    tags = {"svg", "g", "rect", "path", "circle", "ellipse"}
    attributes = {"width", "height", "viewBox", "fill", "d", "cx", "cy", "r", "rx", "ry", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin"}
    for node in svg.iter():
        assert node.tag.removeprefix("{http://www.w3.org/2000/svg}") in tags
        assert set(node.attrib) <= attributes, "Only closed inert geometry is allowed"
        assert not (node.text or "").strip() and not (node.tail or "").strip()
        for key in ("fill", "stroke"):
            if key in node.attrib:
                assert re.fullmatch(r"none|#[0-9a-f]{6}", node.attrib[key])
    assert svg[0].tag.endswith("rect") and svg[0].attrib == {"width": "390", "height": "780", "fill": "#fbf2df"}

    changed = git("diff", "--name-only", BASE, "--").decode().splitlines()
    untracked = git("ls-files", "--others", "--exclude-standard").decode().splitlines()
    assert all(allowed(p) for p in changed + untracked), "Write scope exceeded"
    original_paths = git("ls-tree", "-r", "--name-only", BASE, "--", "frontend/public/backgrounds/cute-entry-v1/").decode().splitlines()
    for path in original_paths:
        assert git("diff", BASE, "--", path) == b"", "Existing source/manifest changed"
    colors = sorted(set(re.findall(r'#[0-9a-f]{6}', raw.decode())))
    ink = luminance("#191817")
    ratios = {c: round((luminance(c) + .05) / (ink + .05), 3) for c in colors}
    result = {
        "task": "UI-BG-CUSTOMIZE-02", "base": BASE,
        "status": "SOURCE_PASS_RENDER_PENDING", "dependencies_added": [],
        "scope": "One new SVG; only image/position/size in existing outer Customize selector; own new doc/evidence",
        "svg": {"dimensions": [390, 780], "opaque_base": "#fbf2df", "elements": sum(1 for _ in svg.iter()), "external_references": 0, "text_or_characters": False},
        "pins": {p: pin(p) for p in [ASSET, CSS, EVIDENCE + "verify-source.py"]},
        "existing_asset_files_unchanged": len(original_paths),
        "nonleased_css_bytes_unchanged": True,
        "observed_color_palette_dark_ink_ratios": ratios,
        "minimum_flat_palette_dark_ink_contrast": min(ratios.values()),
        "limits": "Source/scope proof only. Flat palette samples do not certify composed contrast or focus. Root owns one allocated default-App three-width pair cohort; actual pixel/control/character proof and owner inspection remain required. No live-service/device/feature acceptance."
    }
    if write:
        (ROOT / EVIDENCE / "source-validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
