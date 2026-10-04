#!/usr/bin/env python3
"""Stamp packaging provenance independently of the user's update preference."""
import json
import os
from pathlib import Path


def stamp(directory, classification):
    if classification not in {"GA", "Alpha", "Beta"}:
        raise ValueError("unsupported publication classification")
    for path in directory.glob("*.json"):
        value = json.loads(path.read_text())
        value["releaseClassification"] = classification
        path.write_text(json.dumps(value, indent=2) + "\n")


if __name__ == "__main__":
    stamp(Path("src-tauri/distribution"), os.environ["RELEASE_CLASSIFICATION"])
