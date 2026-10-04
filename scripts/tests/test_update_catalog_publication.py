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
        "action": {"type": "open_url", "url": "https://svb.miguel.ms/guide/Upgrading.html"},
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

    def test_additive_metadata_round_trips_through_publication_validation(self):
        enriched = entry()
        enriched["futureMetadata"] = {"rollout": "gradual"}
        enriched["action"]["autoUpdate"] = {"package": "com.example.app"}
        record = direct_record(enriched)
        output = self.prepare([], record)
        self.assertEqual(output["entries"][0]["action"]["autoUpdate"], {"package": "com.example.app"})

    def test_shared_additive_fixture_is_accepted_and_preserved(self):
        fixture = (ROOT / "tests/fixtures/update-catalog/additive-metadata.json").read_bytes()
        output = publication.prepare_catalog(fixture, json.dumps({"operation": "withdraw", "target": {"edition": "direct_windows", "channel": "stable", "os": "windows", "architecture": "x86_64"}, "reason": "fixture parity"}).encode(), STAMP)
        self.assertEqual(json.loads(output), json.loads(fixture))

    def assert_fixture_mutation(self, original, record, expected_entries):
        raw = json.dumps(original).encode()
        record_raw = json.dumps(record).encode()
        changed_stamp = "2026-10-05T12:00:00Z"
        output = publication.prepare_catalog(raw, record_raw, changed_stamp)
        expected = dict(original, generatedAt=changed_stamp, entries=expected_entries)
        self.assertEqual(json.loads(output), expected)
        self.assertNotEqual(output, raw)
        self.assertEqual(
            publication.prepare_catalog(output, record_raw, "2026-10-06T12:00:00Z"),
            output,
        )

    def additive_fixture(self):
        return json.loads((ROOT / "tests/fixtures/update-catalog/additive-metadata.json").read_bytes())

    def test_shared_additive_fixture_preserved_on_insertion(self):
        original = self.additive_fixture()
        added = entry()
        self.assert_fixture_mutation(original, direct_record(added), original["entries"] + [added])

    def test_shared_additive_fixture_preserved_on_version_update(self):
        original = self.additive_fixture()
        original["entries"].append(entry())
        replacement = entry("1.9.0")
        self.assert_fixture_mutation(original, direct_record(replacement), [original["entries"][0], replacement])

    def test_shared_additive_fixture_preserved_on_withdrawal(self):
        original = self.additive_fixture()
        removed = entry()
        original["entries"].append(removed)
        record = {
            "operation": "withdraw",
            "target": {field: removed[field] for field in publication.TARGET_FIELDS},
            "reason": "Fixture withdrawal.",
        }
        self.assert_fixture_mutation(original, record, original["entries"][:1])

    def test_shared_rejection_fixtures_are_rejected_by_publication(self):
        withdrawal = {
            "operation": "withdraw",
            "target": {
                "edition": "direct_windows",
                "channel": "stable",
                "os": "windows",
                "architecture": "x86_64",
            },
            "reason": "fixture parity",
        }
        for name in (
            "missing-required.json",
            "unsupported-action.json",
            "duplicate-key.json",
            "duplicate-target.json",
        ):
            with self.subTest(name=name), self.assertRaises(publication.VALIDATOR.CatalogError):
                publication.prepare_catalog(
                    (ROOT / "tests/fixtures/update-catalog" / name).read_bytes(),
                    json.dumps(withdrawal).encode(),
                    STAMP,
                )

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
        record["entry"].pop("unexpected", None)
        record["entry"].pop("edition")
        with self.assertRaises(publication.VALIDATOR.CatalogError):
            self.prepare([], record)


if __name__ == "__main__":
    unittest.main()

class ReleasePolicyTests(unittest.TestCase):
    def fixture(self):
        return (ROOT / "tests/fixtures/update-catalog/release-policy.json").read_bytes()

    def record(self, value):
        record = direct_record(value)
        record["source"].update(prerelease=value["classification"] != "GA", releaseBody="**Release channel:** " + value["classification"], tagName="v" + value["version"], public=True)
        return record

    def test_shared_fixture_validator_publisher_and_actual_metadata_preservation(self):
        original = json.loads(self.fixture())
        publication.VALIDATOR.validate_catalog_bytes(self.fixture())
        empty = dict(original, entries=[])
        for value in original["entries"]:
            output = publication.prepare_catalog(json.dumps(empty).encode(), json.dumps(self.record(value)).encode(), STAMP)
            actual = json.loads(output)
            self.assertEqual(actual["futureCatalog"], original["futureCatalog"])
            self.assertEqual(actual["entries"][0], value)
            self.assertEqual(publication.prepare_catalog(output, json.dumps(self.record(value)).encode(), STAMP), output)
        beta = dict(original["entries"][2], version="1.11.0-beta.2")
        beta["action"] = dict(beta["action"], url="https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v1.11.0-beta.2")
        output = publication.prepare_catalog(self.fixture(), json.dumps(self.record(beta)).encode(), STAMP)
        actual = json.loads(output)
        self.assertEqual(actual["futureCatalog"], original["futureCatalog"])
        self.assertIn(original["entries"][0], actual["entries"])
        self.assertIn(original["entries"][1], actual["entries"])
        self.assertIn(beta, actual["entries"])
        with self.assertRaises(publication.PublicationError):
            publication.prepare_catalog(output, json.dumps(self.record(original["entries"][2])).encode(), STAMP)

    def test_classification_requires_matching_public_publisher_evidence(self):
        for classification in ["GA", "Beta", "Alpha"]:
            value = next(entry for entry in json.loads(self.fixture())["entries"] if entry["classification"] == classification)
            for field, wrong in [("draft", True), ("public", False), ("releaseBody", "**Release channel:** Unknown"), ("tagName", "v0.1.0"), ("availableAssets", []), ("prerelease", classification == "GA")]:
                record = self.record(value)
                record["source"][field] = wrong
                with self.subTest(classification=classification, field=field), self.assertRaises(publication.PublicationError):
                    publication.prepare_catalog(self.fixture(), json.dumps(record).encode(), STAMP)

    def test_stable_feed_rejects_numeric_previews_and_preview_feed_rejects_store(self):
        original = json.loads(self.fixture())
        for value in original["entries"][1:]:
            with self.assertRaises((publication.PublicationError, publication.VALIDATOR.CatalogError)):
                publication.prepare_catalog(catalog(), json.dumps(self.record(value)).encode(), STAMP)
        original["entries"][0]["edition"] = "mac_app_store"
        with self.assertRaises(publication.VALIDATOR.CatalogError):
            publication.VALIDATOR.validate_catalog_bytes(json.dumps(original).encode())

    def test_semantic_precedence_and_same_version_promotion(self):
        self.assertLess(publication._semver("1.9.0"), publication._semver("1.10.0"))
        self.assertLess(publication._semver("1.10.0-beta.2"), publication._semver("1.10.0-beta.10"))
        self.assertLess(publication._semver("1.10.0-beta.10"), publication._semver("1.10.0"))
        self.assertEqual(publication._semver("1.10.0+old"), publication._semver("1.10.0+new"))

    def test_independent_verifier_uses_release_body_and_exact_architecture_asset(self):
        spec = importlib.util.spec_from_file_location("verify", ROOT / "scripts/verify-catalog-availability.py")
        verifier = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(verifier)
        value = json.loads(self.fixture())["entries"][2]
        release = {"draft": False, "prerelease": True, "tag_name": "v" + value["version"], "body": "**Release channel:** Beta", "published_at": STAMP, "assets": [{"name": "speaker-volume-bridge-macos.dmg", "state": "uploaded", "size": 100}]}
        verified = verifier.verify(self.record(value), release)
        self.assertEqual(verified["source"]["requiredAssets"], ["speaker-volume-bridge-macos.dmg"])
        release["assets"] = [{"name": "speaker-volume-bridge-windows-arm64-unsigned.exe", "state": "uploaded", "size": 100}]
        with self.assertRaises(verifier.publication.PublicationError):
            verifier.verify(self.record(value), release)
