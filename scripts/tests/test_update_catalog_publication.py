import hashlib
import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("publication", ROOT / "scripts/prepare-update-catalog.py")
publication = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publication)

STAMP = "2026-10-04T12:00:00Z"


def catalog(entries=None):
    return json.dumps({"schemaVersion": 1, "generatedAt": STAMP, "entries": entries or []}).encode()


def entry(version="1.8.0", edition="direct_windows", architecture="x86_64"):
    return {
        "edition": edition,
        "channel": "stable",
        "os": "windows" if "windows" in edition or edition == "microsoft_store" else "macos",
        "architecture": architecture,
        "version": version,
        "publishedAt": STAMP,
        "releaseNotes": "Verified stable release.",
        "action": {"type": "open_url", "url": "https://svb.miguel.ms/upgrade.html"},
    }


def direct_record(value=None):
    return {
        "operation": "upsert",
        "verifiedAvailable": True,
        "source": {
            "kind": "github_release",
            "draft": False,
            "prerelease": False,
            "requiredAssets": ["SpeakerVolumeBridge-windows-x64.exe"],
            "availableAssets": ["SpeakerVolumeBridge-windows-x64.exe"],
        },
        "entry": value or entry(),
    }


class PublicationTests(unittest.TestCase):
    def prepare(self, existing, record, digest=None):
        return json.loads(
            publication.prepare_catalog(
                catalog(existing), json.dumps(record).encode(), STAMP, digest
            )
        )

    def test_published_release_is_added_and_repeat_is_idempotent(self):
        first = self.prepare([], direct_record())
        first_raw = json.dumps(first).encode()
        second = publication.prepare_catalog(
            first_raw, json.dumps(direct_record()).encode(), STAMP
        )
        self.assertEqual(second, first_raw)

    def test_draft_prerelease_and_missing_assets_are_rejected(self):
        for field in ("draft", "prerelease"):
            record = direct_record()
            record["source"][field] = True
            with self.assertRaisesRegex(publication.PublicationError, "draft and prerelease"):
                self.prepare([], record)
        record = direct_record()
        record["source"]["availableAssets"] = []
        with self.assertRaisesRegex(publication.PublicationError, "unavailable"):
            self.prepare([], record)

    def test_store_waits_for_explicit_matching_publication(self):
        store_entry = entry(edition="microsoft_store")
        record = direct_record(store_entry)
        with self.assertRaisesRegex(publication.PublicationError, "Store editions"):
            self.prepare([], record)
        record["source"] = {"kind": "microsoft_store", "published": False}
        with self.assertRaisesRegex(publication.PublicationError, "explicitly verified"):
            self.prepare([], record)
        record["source"]["published"] = True
        self.assertEqual(len(self.prepare([], record)["entries"]), 1)

    def test_partial_availability_only_changes_the_verified_target(self):
        other = entry(version="1.7.0", edition="direct_macos", architecture="aarch64")
        output = self.prepare([other], direct_record())
        self.assertEqual({value["edition"] for value in output["entries"]}, {"direct_macos", "direct_windows"})

    def test_older_and_same_version_changes_cannot_overwrite(self):
        current = entry("2.0.0")
        for version in ("1.9.9", "2.0.0"):
            replacement = entry(version)
            replacement["releaseNotes"] = "Different"
            with self.assertRaisesRegex(publication.PublicationError, "cannot be overwritten"):
                self.prepare([current], direct_record(replacement))

    def test_withdrawal_removes_only_the_target(self):
        left = entry(edition="direct_macos", architecture="aarch64")
        right = entry()
        record = {
            "operation": "withdraw",
            "target": {field: right[field] for field in publication.TARGET_FIELDS},
            "reason": "Public asset was withdrawn.",
        }
        self.assertEqual(self.prepare([left, right], record)["entries"], [left])

    def test_stale_concurrent_input_is_rejected(self):
        original = catalog()
        digest = hashlib.sha256(original).hexdigest()
        changed = catalog([entry()])
        with self.assertRaisesRegex(publication.PublicationError, "changed after availability review"):
            publication.prepare_catalog(changed, json.dumps(direct_record()).encode(), STAMP, digest)

    def test_invalid_existing_catalog_and_candidate_fail_closed(self):
        with self.assertRaises(publication.VALIDATOR.CatalogError):
            publication.prepare_catalog(b"{}", json.dumps(direct_record()).encode(), STAMP)
        record = direct_record()
        record["entry"]["unexpected"] = True
        with self.assertRaises(publication.VALIDATOR.CatalogError):
            self.prepare([], record)


if __name__ == "__main__":
    unittest.main()
