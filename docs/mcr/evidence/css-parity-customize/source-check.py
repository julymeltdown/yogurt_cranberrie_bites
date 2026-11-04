"""Bounded foreground ownership check; no browser or geometry claim."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

repo = Path(__file__).resolve().parents[4]
base = "54320d71c4ae0bff3524d1b500cea772e0ba42e0"
css = "frontend/src/features/avatar/whole-character/whole-character.css"
old = subprocess.check_output(["git", "show", f"{base}:{css}"], cwd=repo).decode()
new = (repo / css).read_text()
rules = lambda text: dict(re.findall(r"([^{}]+)\{([^{}]*)\}", text))
a, b = rules(old), rules(new)
assert a.keys() == b.keys(), "No new pseudo-content, decorative or unowned selectors"
expected = {
    ".mcr-whole-choices h2",
    ".mcr-whole-choice-row > button",
    '.mcr-whole-editor .mcr-whole-choice[aria-pressed="true"]',
    ".mcr-whole-editor .concert-action",
}
changed = {selector.strip() for selector in a if a[selector] != b[selector]}
assert changed == expected, changed
for selector in a:
    if any(token in selector for token in ["preview", "canvas", "span", "whole-grid"]):
        assert a[selector] == b[selector], selector
    if selector.strip() not in expected:
        assert a[selector] == b[selector]
# Within changed rules the only background change is an existing Save control fill.
for selector in a:
    oldpaint = re.findall(r"background[^:;]*:[^;]+", a[selector])
    newpaint = re.findall(r"background[^:;]*:[^;]+", b[selector])
    if selector.strip() != ".mcr-whole-editor .concert-action":
        assert oldpaint == newpaint, selector
paths = subprocess.check_output(["git", "diff", "--name-only", base, "HEAD"], cwd=repo).decode().splitlines()
allowed = lambda p: p == css or p == "docs/mcr/frontend/css-parity-customize.md" or p.startswith("docs/mcr/evidence/css-parity-customize/")
assert all(map(allowed, paths)), paths
assert not subprocess.check_output(["git", "diff", base, "HEAD", "--", "docs/source/"], cwd=repo)
reference = Path("/tmp/codex-remote-attachments/01a1a385-9f32-73c2-8715-ff54bd1e0497/AB983BDC-49A0-4F5E-A56F-9F45A5560B6D/1-Photo-1.jpg")
reference_hash = hashlib.sha256(reference.read_bytes()).hexdigest()
assert reference_hash == "2b3988d546f87b813d78ca13bf267730a6dd19118bd6535a2d3c3032b6f12d23"
result = {"result": "PASS", "base": base, "cssSha256": hashlib.sha256(new.encode()).hexdigest(), "changedRules": sorted(changed), "referenceSha256": reference_hash, "paths": paths, "previewCanvasGridDeclarationsUnchanged": True, "decorativeBackgroundDeclarationsUnchanged": True, "docsSourcePreserved": True}
print(json.dumps(result, indent=2))
