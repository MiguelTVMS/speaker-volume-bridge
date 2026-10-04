#!/usr/bin/env python3
"""Prepare a verified, reviewable update-catalog change."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import tempfile
from datetime import datetime, timezone
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "validate_update_catalog", SCRIPT_DIR / "validate-update-catalog.py"
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load catalog validator")
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)

TARGET_FIELDS = ("edition", "channel", "os", "architecture")
GITHUB_SOURCE = "github_release"
STORE_SOURCES = {"microsoft_store", "mac_app_store"}
HEX_SHA256 = re.compile(r"^[0-9a-f]{64}$")


class PublicationError(ValueError):
    """Availability evidence cannot safely update the catalog."""


def _object(raw: bytes, context: str) -> dict[str, object]:
    try:
        value = json.loads(raw, object_pairs_hook=VALIDATOR._reject_duplicate_members)
    except (UnicodeDecodeError, json.JSONDecodeError, VALIDATOR.CatalogError) as error:
        raise PublicationError(f"{context} is not valid JSON: {error}") from error
    if not isinstance(value, dict):
        raise PublicationError(f"{context} must be an object")
    return value


def _exact(value: dict[str, object], required: set[str], optional: set[str], context: str) -> None:
    missing = required - value.keys()
    unknown = value.keys() - required - optional
    if missing or unknown:
        details = []
        if missing:
            details.append(f"missing {', '.join(sorted(missing))}")
        if unknown:
            details.append(f"unsupported {', '.join(sorted(unknown))}")
        raise PublicationError(f"{context}: {'; '.join(details)}")


def _target(value: dict[str, object], context: str) -> tuple[object, ...]:
    _exact(value, set(TARGET_FIELDS), set(), context)
    return tuple(value[field] for field in TARGET_FIELDS)


def _semver(value: object):
    match = VALIDATOR.SEMVER.fullmatch(value) if isinstance(value, str) else None
    if match is None:
        raise PublicationError("published version must be a semantic version")
    pre = match.group(4)
    identifiers = tuple((0, int(part)) if part.isdigit() else (1, part) for part in pre.split(".")) if pre else ()
    return (*[int(match.group(index)) for index in range(1, 4)], pre is None, identifiers)


def _verify_source(source: object, entry: dict[str, object]) -> None:
    if not isinstance(source, dict):
        raise PublicationError("source must be an object")
    kind = source.get("kind")
    if kind == GITHUB_SOURCE:
        _exact(
            source,
            {"kind", "draft", "prerelease", "requiredAssets", "availableAssets"},
            {"releaseBody", "tagName", "public"},
            "source",
        )
        classification = entry.get("classification", "GA")
        if entry.get("channel") == "prereleases" or "classification" in entry:
            body = source.get("releaseBody", "")
            markers = re.findall(r"^\*\*Release channel:\*\* (GA|Alpha|Beta)\s*$", body, re.MULTILINE) if isinstance(body, str) else []
            if markers != [classification] or source.get("tagName") != "v" + entry["version"] or source.get("public") is not True:
                raise PublicationError("classification requires independently fetched public release metadata")
        if source["draft"] is not False or source["prerelease"] is not (classification != "GA"):
            raise PublicationError("draft and prerelease GitHub releases are not publishable")
        required = source["requiredAssets"]
        available = source["availableAssets"]
        if (
            not isinstance(required, list)
            or not required
            or not all(isinstance(item, str) and item for item in required)
            or not isinstance(available, list)
            or not all(isinstance(item, str) and item for item in available)
        ):
            raise PublicationError("release asset lists must contain non-empty names")
        missing = sorted(set(required) - set(available))
        if missing:
            raise PublicationError(f"required release assets are unavailable: {', '.join(missing)}")
        if entry.get("edition") in {"microsoft_store", "mac_app_store"}:
            raise PublicationError("Store editions require Store publication evidence")
        return
    if kind in STORE_SOURCES:
        _exact(source, {"kind", "published"}, set(), "source")
        if source["published"] is not True or entry.get("edition") != kind:
            raise PublicationError("Store availability must be explicitly verified for the same edition")
        return
    raise PublicationError("unsupported availability source")


def prepare_catalog(
    catalog_raw: bytes,
    record_raw: bytes,
    generated_at: str,
    expected_sha256: str | None = None,
) -> bytes:
    if expected_sha256 is not None:
        if not HEX_SHA256.fullmatch(expected_sha256):
            raise PublicationError("expected catalog digest must be lowercase SHA-256")
        actual = hashlib.sha256(catalog_raw).hexdigest()
        if actual != expected_sha256:
            raise PublicationError("catalog changed after availability review; retry from the latest develop")

    catalog = VALIDATOR.validate_catalog_bytes(catalog_raw)
    record = _object(record_raw, "availability record")
    operation = record.get("operation")
    entries = list(catalog["entries"])
    changed = False

    if operation == "upsert":
        _exact(record, {"operation", "verifiedAvailable", "source", "entry"}, set(), "record")
        if record["verifiedAvailable"] is not True:
            raise PublicationError("availability must be explicitly verified")
        entry = record["entry"]
        if not isinstance(entry, dict):
            raise PublicationError("entry must be an object")
        _verify_source(record["source"], entry)
        candidate = {"schemaVersion": catalog["schemaVersion"], "generatedAt": generated_at, "entries": [entry]}
        VALIDATOR.validate_catalog_bytes(json.dumps(candidate).encode())
        fields = TARGET_FIELDS + (("classification",) if catalog["schemaVersion"] == 2 else ())
        target = tuple(entry[field] for field in fields)
        matching = [index for index, current in enumerate(entries) if tuple(current[field] for field in fields) == target]
        if matching:
            index = matching[0]
            current = entries[index]
            if current == entry:
                pass
            elif _semver(entry["version"]) <= _semver(current["version"]):
                raise PublicationError("catalog entries cannot be overwritten by the same or an older release")
            else:
                entries[index] = entry
                changed = True
        else:
            entries.append(entry)
            changed = True
    elif operation == "withdraw":
        _exact(record, {"operation", "target", "reason"}, set(), "record")
        if not isinstance(record["reason"], str) or not record["reason"].strip():
            raise PublicationError("withdrawal reason must be non-empty")
        if not isinstance(record["target"], dict):
            raise PublicationError("withdrawal target must be an object")
        fields = TARGET_FIELDS + (("classification",) if catalog["schemaVersion"] == 2 else ())
        _exact(record["target"], set(fields), set(), "target")
        target = tuple(record["target"][field] for field in fields)
        retained = [entry for entry in entries if tuple(entry[field] for field in fields) != target]
        changed = len(retained) != len(entries)
        entries = retained
    else:
        raise PublicationError("operation must be upsert or withdraw")

    if not changed:
        return catalog_raw

    entries.sort(key=lambda entry: tuple(str(entry[field]) for field in TARGET_FIELDS))
    output = dict(catalog, generatedAt=generated_at, entries=entries)
    encoded = (json.dumps(output, indent=2, ensure_ascii=False) + "\n").encode()
    VALIDATOR.validate_catalog_bytes(encoded)
    return encoded


def _write_atomic(path: Path, contents: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(contents)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("record", type=Path)
    parser.add_argument("--catalog", type=Path, default=Path("pages/updates/v1/catalog.json"))
    parser.add_argument("--expected-sha256")
    parser.add_argument("--generated-at")
    args = parser.parse_args()
    generated_at = args.generated_at or datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    try:
        output = prepare_catalog(
            args.catalog.read_bytes(),
            args.record.read_bytes(),
            generated_at,
            args.expected_sha256,
        )
        _write_atomic(args.catalog, output)
    except (OSError, PublicationError, VALIDATOR.CatalogError) as error:
        parser.error(str(error))
    print(f"prepared {args.catalog}; review and merge through the normal release PR flow")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
