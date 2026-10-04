#!/usr/bin/env python3
"""Validate the public Speaker Volume Bridge update catalog."""

from __future__ import annotations

import argparse
import json
import re
from datetime import datetime
from pathlib import Path
from urllib.parse import urlsplit

MAX_CATALOG_BYTES = 256 * 1024
MAX_NOTES_BYTES = 16 * 1024
EDITIONS = {
    "direct_macos",
    "direct_windows",
    "microsoft_store",
    "mac_app_store",
    "debian",
}
CHANNELS = {"stable"}
OPERATING_SYSTEMS = {"macos", "windows", "linux"}
ARCHITECTURES = {"aarch64", "x86_64"}
SEMVER = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


class CatalogError(ValueError):
    """The catalog violates the versioned public contract."""


def _reject_duplicate_members(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise CatalogError(f"duplicate object member: {key}")
        result[key] = value
    return result


def _require_keys(value: dict[str, object], required: set[str], context: str) -> None:
    missing = required - value.keys()
    if missing:
        raise CatalogError(f"{context} missing fields: {', '.join(sorted(missing))}")


def _timestamp(value: object, field: str) -> None:
    if not isinstance(value, str):
        raise CatalogError(f"{field} must be a string")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise CatalogError(f"{field} must be an RFC 3339 timestamp") from error
    if parsed.tzinfo is None:
        raise CatalogError(f"{field} must include a timezone")


def _https_url(value: object, field: str) -> None:
    if not isinstance(value, str):
        raise CatalogError(f"{field} must be a string")
    parsed = urlsplit(value)
    if parsed.scheme != "https" or not parsed.netloc or parsed.username or parsed.password:
        raise CatalogError(f"{field} must be an absolute HTTPS URL without credentials")


def validate_catalog_bytes(raw: bytes) -> dict[str, object]:
    if len(raw) > MAX_CATALOG_BYTES:
        raise CatalogError(f"catalog exceeds {MAX_CATALOG_BYTES} bytes")
    try:
        catalog = json.loads(raw, object_pairs_hook=_reject_duplicate_members)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CatalogError("catalog is not valid UTF-8 JSON") from error
    if not isinstance(catalog, dict):
        raise CatalogError("catalog root must be an object")
    _require_keys(catalog, {"schemaVersion", "generatedAt", "entries"}, "catalog")
    if type(catalog["schemaVersion"]) is not int or catalog["schemaVersion"] not in (1, 2):
        raise CatalogError("unsupported schemaVersion")
    _timestamp(catalog["generatedAt"], "generatedAt")
    entries = catalog["entries"]
    if not isinstance(entries, list):
        raise CatalogError("entries must be an array")

    preview = catalog["schemaVersion"] == 2
    targets: set[tuple[object, ...]] = set()
    for index, entry in enumerate(entries):
        context = f"entry {index}"
        if not isinstance(entry, dict):
            raise CatalogError(f"{context} must be an object")
        _require_keys(
            entry,
            {
                "edition",
                "channel",
                "os",
                "architecture",
                "version",
                "publishedAt",
                "releaseNotes",
                "action",
            },
            context,
        )
        for field, allowed in (
            ("edition", EDITIONS),
            ("channel", {"stable", "prereleases"} if preview else CHANNELS),
            ("os", OPERATING_SYSTEMS),
            ("architecture", ARCHITECTURES),
        ):
            if entry[field] not in allowed:
                raise CatalogError(f"{context} has unsupported {field}")
        if not isinstance(entry["version"], str):
            raise CatalogError(f"{context} has invalid semantic version")
        match = SEMVER.fullmatch(entry["version"])
        if match is None:
            raise CatalogError(f"{context} has invalid semantic version")
        if match.group(4) and any(part.isdigit() and len(part) > 1 and part.startswith("0") for part in match.group(4).split(".")):
            raise CatalogError(f"{context} has invalid semantic prerelease version")
        if entry["channel"] == "stable" and match.group(4) is not None:
            raise CatalogError(f"{context} stable entry cannot use a prerelease version")
        classification = entry.get("classification")
        if preview:
            if entry["edition"] not in {"direct_macos", "direct_windows", "debian"}:
                raise CatalogError("preview feed excludes Store editions")
            if classification not in {"GA", "Alpha", "Beta"}:
                raise CatalogError("preview feed requires publisher classification")
            if entry["channel"] != ("stable" if classification == "GA" else "prereleases"):
                raise CatalogError("classification conflicts with channel")
        elif classification is not None and classification != "GA":
            raise CatalogError("stable feed excludes previews")
        expected_os = {"direct_macos": "macos", "mac_app_store": "macos", "direct_windows": "windows", "microsoft_store": "windows", "debian": "linux"}[entry["edition"]]
        if entry["os"] != expected_os:
            raise CatalogError("edition conflicts with platform")
        _timestamp(entry["publishedAt"], f"{context}.publishedAt")
        notes = entry["releaseNotes"]
        if not isinstance(notes, str) or not notes.strip() or len(notes.encode()) > MAX_NOTES_BYTES:
            raise CatalogError(f"{context}.releaseNotes must be 1..{MAX_NOTES_BYTES} UTF-8 bytes")
        action = entry["action"]
        if not isinstance(action, dict):
            raise CatalogError(f"{context}.action must be an object")
        _require_keys(action, {"type", "url"}, f"{context}.action")
        if not isinstance(action.get("type"), str) or action["type"] != "open_url":
            raise CatalogError(f"{context} has unsupported action")
        _https_url(action["url"], f"{context}.action.url")
        if preview and action["url"] != "https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v" + entry["version"]:
            raise CatalogError("preview feed requires exact release page")

        target = (entry["edition"], entry["channel"], entry["os"], entry["architecture"])
        if preview:
            target += (classification,)
        if target in targets:
            raise CatalogError(f"duplicate target: {'/'.join(target)}")
        targets.add(target)
    return catalog


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("catalog", type=Path, nargs="?", default=Path("pages/updates/v1/catalog.json"))
    args = parser.parse_args()
    try:
        validate_catalog_bytes(args.catalog.read_bytes())
    except (OSError, CatalogError) as error:
        parser.error(str(error))
    print(f"validated {args.catalog}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
