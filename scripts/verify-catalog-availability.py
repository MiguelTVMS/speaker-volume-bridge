#!/usr/bin/env python3
"""Replace operator assertions with independently fetched public GitHub evidence."""
import importlib.util
import json
import re
from pathlib import Path
from urllib.parse import quote
from urllib.request import urlopen
import argparse

SPEC = importlib.util.spec_from_file_location("publication", Path(__file__).with_name("prepare-update-catalog.py"))
publication = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publication)
REPOSITORY = "MiguelTVMS/speaker-volume-bridge"


def verify(record, release):
    entry = record["entry"]
    if release.get("draft") is not False or release.get("tag_name") != "v" + entry["version"]:
        raise publication.PublicationError("release must be public and match package version")
    body = release.get("body", "")
    classifications = re.findall(r"^\*\*Release channel:\*\* (GA|Alpha|Beta)\s*$", body, re.MULTILINE)
    if len(classifications) != 1:
        raise publication.PublicationError("release requires unambiguous publisher classification")
    classification = classifications[0]
    if release.get("prerelease") is not (classification != "GA"):
        raise publication.PublicationError("release classification conflicts with GitHub status")
    if entry.get("classification", "GA") != classification:
        raise publication.PublicationError("requested classification conflicts with publisher")
    suffix = {
        ("direct_macos", "aarch64"): "macos.dmg",
        ("direct_macos", "x86_64"): "macos.dmg",
        ("direct_windows", "aarch64"): "windows-arm64-unsigned.exe",
        ("direct_windows", "x86_64"): "windows-x64-unsigned.exe",
        ("debian", "aarch64"): "linux-arm64.deb",
        ("debian", "x86_64"): "linux-x64.deb",
    }.get((entry["edition"], entry["architecture"]))
    if suffix is None:
        raise publication.PublicationError("unsupported direct release target")
    required = "speaker-volume-bridge-" + suffix
    available = [asset["name"] for asset in release.get("assets", []) if asset.get("size", 0) > 0 and asset.get("state") == "uploaded"]
    if required not in available:
        raise publication.PublicationError("compatible public asset is unavailable")
    record = dict(record, source={"kind": "github_release", "draft": False, "prerelease": release["prerelease"], "public": True, "releaseBody": body, "tagName": release["tag_name"], "requiredAssets": [required], "availableAssets": available})
    record["entry"] = dict(entry, publishedAt=release["published_at"])
    return record


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("record", type=Path)
    args = parser.parse_args()
    record = publication._object(args.record.read_bytes(), "record")
    if record.get("operation") != "upsert" or record.get("source", {}).get("kind") != "github_release":
        return
    tag = "v" + record["entry"]["version"]
    with urlopen("https://api.github.com/repos/" + REPOSITORY + "/releases/tags/" + quote(tag, safe=""), timeout=10) as response:
        raw = response.read(1024 * 1024 + 1)
    if len(raw) > 1024 * 1024:
        raise publication.PublicationError("release metadata too large")
    verified = verify(record, json.loads(raw))
    args.record.write_text(json.dumps(verified))


if __name__ == "__main__":
    main()
