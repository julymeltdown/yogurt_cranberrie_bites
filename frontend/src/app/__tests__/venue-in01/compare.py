#!/usr/bin/env python3
"""Task 28 comparison builder.

Reads the deterministic captures (states.json), the stable case register
(cases.json) and the supplied reference photographs. Produces per-case
reference crops, component-aligned overlays, diffs, the machine-readable
report and the evidence hash list. It never regenerates or edits a reference.
"""
from __future__ import annotations

import hashlib
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[4]
REPO = ROOT.parent
EVIDENCE = REPO / ".omo/evidence/mcr-finish-project/task-28-venu-in01"
REFERENCE = REPO / "docs/mcr/design/venue-redesign-2026-11-05"
WIDTHS = (360, 390, 430)

# Developer-measured source pixels inside the supplied composites.
FRAME = {"tab": 37, "friendRow": 82, "friendPortrait": 55, "primary": 36, "name": 15}
FRIEND_COMPONENT = {"selectedPortrait": 58, "primary": 36, "name": 22, "source": 11}
NOTES_FRAME = {"tab": 37, "noteCard": 122, "notePortrait": 48, "primary": 40}
NOTES_COMPONENT = {"notePortrait": 48, "name": 15, "body": 13, "primary": 36}
# The user-facing 44px target is intentional where the reference study is smaller.
ACCEPTED_44 = {"primary", "tab"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def family(case_id: str) -> dict[str, int]:
    suffix = case_id.split("-", 1)[1]
    if suffix in {"map-overview", "friends-list"}:
        return FRAME
    if suffix in {"notes-map-list", "composer"}:
        return NOTES_FRAME
    if suffix in {"selected-friend", "friend-freshness", "freshness-formats", "marker-taxonomy", "sharing-control"}:
        return FRIEND_COMPONENT
    if suffix in {"place-note-card", "audience-selector", "expiry-selector", "inline-reply"}:
        return NOTES_COMPONENT
    return {}


def region_for(case: dict) -> str:
    suffix = case["caseId"].split("-", 1)[1]
    roi_width = case["roi"][2] - case["roi"][0]
    if suffix == "whole-plan":
        return "canvas"
    if suffix in {"notes-map-list", "map-overview", "friends-list", "composer"}:
        return "tabPanel"
    if suffix == "selected-friend":
        return "sheet" if roi_width > 300 else "friendDetail"
    if suffix in {"friend-freshness", "freshness-formats"}:
        return "friends"
    if suffix == "sharing-control":
        return "sharing"
    if suffix in {"place-note-card", "inline-reply"}:
        return "noteCard"
    if suffix in {"audience-selector", "expiry-selector"}:
        return "composer"
    if suffix == "place-card":
        return "sheet"
    if suffix == "marker-taxonomy":
        return "canvas"
    return "tabPanel"


def app_metric(metrics: dict, key: str):
    if key == "tab":
        return (metrics.get("tab") or {}).get("height")
    if key == "friendRow":
        rows = metrics.get("friendRows") or []
        return rows[0].get("height") if rows else None
    if key == "friendPortrait":
        return (metrics.get("friendPortrait") or {}).get("width")
    if key == "selectedPortrait":
        return (metrics.get("selectedPortrait") or {}).get("width")
    if key == "noteCard":
        cards = metrics.get("noteCards") or []
        return cards[0].get("height") if cards else None
    if key == "notePortrait":
        return (metrics.get("notePortrait") or {}).get("width")
    if key == "primary":
        return (metrics.get("primary") or {}).get("height")
    fonts = metrics.get("fonts") or {}
    if key in {"name", "body", "source"}:
        return float(fonts.get(key)[:-2]) if fonts.get(key) else None
    return None


def deltas(case: dict, metrics: dict) -> dict:
    table = family(case["caseId"])
    roi_width = case["roi"][2] - case["roi"][0]
    if not table or roi_width <= 0:
        return {}
    scale = 390 / roi_width
    out = {}
    for key, source_px in table.items():
        actual = app_metric(metrics, key)
        if actual is None:
            continue
        reference = source_px * scale
        delta = actual - reference
        accepted = key in ACCEPTED_44 and delta > 0
        out[key] = {
            "referenceSourcePixels": source_px,
            "referenceCSSAt390": round(reference, 1),
            "actualCSS": round(actual, 1),
            "deltaCSS": round(delta, 1),
            "material": abs(delta) > (2.0 if key in {"name", "body", "source"} else 6.0) and not accepted,
            "disposition": "ACCEPTED_44PX_TARGET" if accepted else None,
        }
    return out


def overlay(app_path: Path, ref_path: Path, out_overlay: Path, out_diff: Path, region: dict | None) -> dict:
    app = Image.open(app_path).convert("RGB")
    if region:
        left, top = round(region["x"]), round(region["y"])
        right, bottom = round(region["x"] + region["width"]), round(region["y"] + region["height"])
        app = app.crop((left, top, right, bottom))
    ref = Image.open(ref_path).convert("RGB")
    scaled = ref.resize((app.width, round(ref.height * app.width / ref.width)), Image.LANCZOS)
    padded = Image.new("RGB", (app.width, app.height), "white")
    padded.paste(scaled.crop((0, 0, app.width, min(app.height, scaled.height))), (0, 0))
    Image.blend(app, padded, 0.5).save(out_overlay)
    common = (app.width, min(app.height, scaled.height))
    diff = ImageChops.difference(app.crop((0, 0, *common)), scaled.crop((0, 0, *common))).convert("L")
    canvas = Image.new("L", (app.width, app.height), 255)
    canvas.paste(diff, (0, 0))
    canvas.save(out_diff)
    return {"uniformScale": round(app.width / ref.width, 5), "scaledReferenceHeightCSS": scaled.height,
            "comparedHeightCSS": common[1], "actualRegion": region}


def main() -> None:
    definitions = json.loads((EVIDENCE / "cases.json").read_text())
    states = json.loads((EVIDENCE / "states.json").read_text())
    if states.get("result") != "PASS":
        raise SystemExit("captures did not pass: " + json.dumps(states.get("errors")))
    manifest = json.loads((REFERENCE / "reference-manifest.json").read_text())
    manifest_sha = {row["photo"]: row["sha256"] for row in manifest["files"]}
    head = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    pins = {name: sha256(path) for name, path in {
        "reference-manifest.json": REFERENCE / "reference-manifest.json",
        "reference-state-map.json": REFERENCE / "reference-state-map.json",
        "policy-freeze.md": REPO / "docs/mcr/contracts/paradise-city-v1/policy-freeze.md",
        "venue-root/production.ts": ROOT / "src/app/venue-root/production.ts",
        "paradise-city-map/MapCanvas.tsx": ROOT / "src/features/paradise-city-map/ui/MapCanvas.tsx",
        "venue-in01/fixture.tsx": ROOT / "src/app/__tests__/venue-in01/browser/fixture.tsx",
        "venue-in01/native.mjs": ROOT / "src/app/__tests__/venue-in01/browser/native.mjs",
    }.items()}
    captured = datetime.now(timezone.utc).isoformat(timespec="seconds")
    for directory in ("captures", "fixtures", "reference", "overlay", "diff", "ia", "ia-live"):
        (EVIDENCE / directory).mkdir(parents=True, exist_ok=True)

    cases = []
    for case in definitions["cases"]:
        case_id = case["caseId"]
        verdict = case["verdict"]
        photo_path = REFERENCE / case["sourcePath"]
        if sha256(photo_path) != manifest_sha[case["photo"]]:
            raise SystemExit(f"{case_id}: reference bytes changed")
        roi = case["roi"]
        crop = Image.open(photo_path).convert("RGB").crop(tuple(roi))
        crop_path = EVIDENCE / "reference" / f"{case_id}.png"
        crop.save(crop_path)
        entry = {
            "caseId": case_id,
            "photo": case["photo"],
            "originalPhotoSHA256": manifest_sha[case["photo"]],
            "referenceRole": case["referenceRole"],
            "targetState": case.get("targetState"),
            "developerMeasuredSourceROI": {"x": roi[0], "y": roi[1], "width": roi[2] - roi[0], "height": roi[3] - roi[1], "units": "source JPEG pixels"},
            "roiStatus": "DEVELOPER_MEASURED",
            "implementationCommit": head,
            "captureTimeWithTimezone": captured,
            "fixtureOrRealServiceDeviceLabel": "CONTROLLED_FIXTURE - real VenueShellFeatures production sources and frozen clients over a deterministic transport; no live backend, device, physical venue or character runtime",
            "producerContractSourcePins": pins,
            "declaredVariableDataMasks": case.get("masks", []),
            "verdict": verdict,
            "materialDiscrepancies": [],
            "adaptations": [],
            "fixDisposition": "NO_FIX_REQUIRED" if verdict == "PASS" else ("RECORDED" if verdict == "DIFFER" else "BLOCKED_PRODUCER"),
            "actualCapturePath": None,
            "fixtureCapturePath": None,
            "referenceCropPath": str(crop_path.relative_to(EVIDENCE)),
            "overlayPath": None,
            "diffPath": None,
            "captures": {},
            "measured": {},
        }
        if verdict == "NOT_REACHABLE":
            entry["blockingProducer"] = case["differences"][0]
            entry["materialDiscrepancies"] = case["differences"]
            entry["comparisonStatus"] = "NOT_COMPARABLE_PRODUCER_UNAVAILABLE"
            for width in WIDTHS:
                live = EVIDENCE / "captures" / "states" / f"unavailable-{width}.png"
                fixture = EVIDENCE / "captures" / "states" / f"{case['state']}-{width}.png"
                actual = EVIDENCE / "captures" / case_id / f"{width}.png"
                fixture_out = EVIDENCE / "fixtures" / case_id / f"{width}.png"
                actual.parent.mkdir(parents=True, exist_ok=True)
                fixture_out.parent.mkdir(parents=True, exist_ok=True)
                actual.write_bytes(live.read_bytes())
                fixture_out.write_bytes(fixture.read_bytes())
                entry["captures"][str(width)] = str(actual.relative_to(EVIDENCE))
                entry["actualCapturePath"] = entry["captures"].get("390")
                entry["fixtureCapturePath"] = str(Path("fixtures") / case_id / "390.png")
                metrics = states["states"].get(f"unavailable-{width}", {})
                entry["measured"][str(width)] = {
                    "actualViewportCSSWidth": width,
                    "actualViewportCSSHeight": metrics.get("viewport", {}).get("height"),
                    "devicePixelRatio": metrics.get("viewport", {}).get("dpr"),
                    "comparisonStatus": "NOT_COMPARABLE_PRODUCER_UNAVAILABLE",
                }
        else:
            entry["materialDiscrepancies"] = case["differences"]
            entry["comparisonStatus"] = "COMPARED"
            for width in WIDTHS:
                source = EVIDENCE / "captures" / "states" / f"{case['state']}-{width}.png"
                actual = EVIDENCE / "captures" / case_id / f"{width}.png"
                actual.parent.mkdir(parents=True, exist_ok=True)
                actual.write_bytes(source.read_bytes())
                entry["captures"][str(width)] = str(actual.relative_to(EVIDENCE))
                entry["actualCapturePath"] = entry["captures"].get("390")
                metrics = states["states"].get(f"{case['state']}-{width}", {})
                measured = {
                    "actualViewportCSSWidth": width,
                    "actualViewportCSSHeight": metrics.get("viewport", {}).get("height"),
                    "devicePixelRatio": metrics.get("viewport", {}).get("dpr"),
                    "deltas": deltas(case, metrics),
                    "metrics": metrics,
                }
                entry["measured"][str(width)] = measured
                if verdict in {"PASS", "DIFFER"}:
                    transform = overlay(actual, crop_path, EVIDENCE / "overlay" / f"{case_id}-{width}.png", EVIDENCE / "diff" / f"{case_id}-{width}.png",
                                        (metrics.get("regions") or {}).get(region_for(case)))
                    measured["uniformScaleAndCropTransform"] = transform
                    measured["overlayPath"] = str(Path("overlay") / f"{case_id}-{width}.png")
                    measured["diffPath"] = str(Path("diff") / f"{case_id}-{width}.png")
                    if width == 390:
                        entry["overlayPath"] = measured["overlayPath"]
                        entry["diffPath"] = measured["diffPath"]
            entry["adaptations"] = sorted({f"{key}:{value['deltaCSS']}px" for measured in entry["measured"].values()
                                           for key, value in measured.get("deltas", {}).items() if value.get("disposition")})
        cases.append(entry)

    ia = []
    for row in definitions["ia"]:
        entry = {
            "iaId": row["iaId"],
            "state": row["state"],
            "verdict": row["verdict"],
            "liveStatus": row["live"],
            "blockingProducer": row.get("blocker"),
            "note": row.get("note"),
            "captures": {},
            "liveCaptures": {},
        }
        for width in WIDTHS:
            source = EVIDENCE / "captures" / "states" / f"{row['state']}-{width}.png"
            target = EVIDENCE / "ia" / row["iaId"] / f"{width}.png"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(source.read_bytes())
            entry["captures"][str(width)] = str(target.relative_to(EVIDENCE))
            if row["live"] == "blocked":
                live = EVIDENCE / "captures" / "states" / f"unavailable-{width}.png"
                live_target = EVIDENCE / "ia-live" / row["iaId"] / f"{width}.png"
                live_target.parent.mkdir(parents=True, exist_ok=True)
                live_target.write_bytes(live.read_bytes())
                entry["liveCaptures"][str(width)] = str(live_target.relative_to(EVIDENCE))
        ia.append(entry)

    counts = {verdict: sum(1 for item in cases if item["verdict"] == verdict) for verdict in ("PASS", "DIFFER", "NOT_REACHABLE")}
    ia_counts = {verdict: sum(1 for item in ia if item["verdict"] == verdict) for verdict in ("PASS", "DIFFER", "NOT_REACHABLE")}
    report = {
        "schemaVersion": 1,
        "task": "VENU-IN01 row 28",
        "qualification": "CONTROLLED_FIXTURE",
        "implementationBaseCommit": "13d2ee25f",
        "captureHead": head,
        "generatedAt": captured,
        "referenceSet": "venue-redesign-2026-11-05",
        "referenceManifestSHA256": pins["reference-manifest.json"],
        "referenceStateMapSHA256": pins["reference-state-map.json"],
        "widthsCSS": list(WIDTHS),
        "counts": {"cases": counts, "total": len(cases), "ia": ia_counts, "iaTotal": len(ia)},
        "caveat": "Photo 5, Photo 9 and Photo 10 are PROVISIONAL primary roles. This report does not approve that provisional variant and does not claim pixel identity, live service, device or physical-venue acceptance.",
        "producerAvailability": {
            "VENUE_FRIEND_V1": "NOT_REACHABLE: PgVenueLocationAuthority::friend_in returns VenueLocationError::PolicyUnavailable (503). All friend-location, sharing-audience, meeting and invitation cases are recorded against the truthful unavailable state.",
            "notes": "REACHABLE in the integrated composition; deterministic rows are CONTROLLED_FIXTURE, not live producer evidence.",
            "landmarks": "REACHABLE (L12 reviewed landmark tuple); deterministic grants are CONTROLLED_FIXTURE.",
        },
        "comparisonBoundary": "Venue panel surface: MAP/FRIENDS/NOTES tabs, siteplan, sheets, sharing footer and meeting leaf. Shell header and bottom navigation are FE shell scope and are excluded from this comparison.",
        "limitations": [
            "No character runtime is mounted in the deterministic fixture; portrait slots show the product's truthful 'Character unavailable' fallback instead of the reference character art.",
            "No live backend, GPS, device, physical venue or two-client flow was exercised.",
            "The reference composites are design studies; phone-frame heights are not handset CSS viewports. Only uniform width scaling is used; heights are never stretched.",
            "PROVISIONAL Photo 5 / Photo 9 / Photo 10 selection remains unapproved.",
            "The bilateral friend authority returns 503, so friend-dependent reference cases are NOT_REACHABLE, not matches.",
        ],
        "cases": cases,
        "ia": ia,
    }
    (EVIDENCE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    files = sorted(path for path in EVIDENCE.rglob("*") if path.is_file() and path.name != "sha256.txt")
    (EVIDENCE / "sha256.txt").write_text("".join(f"{sha256(path)}  {path.relative_to(EVIDENCE)}\n" for path in files))
    print(json.dumps({"counts": counts, "ia": ia_counts, "files": len(files)}))


if __name__ == "__main__":
    main()
