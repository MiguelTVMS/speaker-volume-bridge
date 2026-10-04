#!/usr/bin/env python3
"""Resolve repository build metadata for the Jekyll website."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[1]


def npm_version():
    package = json.loads((ROOT / "package.json").read_text())
    value = package.get("version")
    return value.strip() if isinstance(value, str) and value.strip() else None


def cargo_version():
    source = (ROOT / "Cargo.toml").read_text()
    workspace = re.search(r"(?ms)^\[workspace\.package\]\s*(.*?)(?=^\[|\Z)", source)
    if not workspace:
        return None
    version = re.search(r'^version\s*=\s*"([^"]+)"', workspace.group(1), re.MULTILINE)
    return version.group(1) if version else None


def git_value(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def resolve(ref=None, revision=None, version=None):
    package_version = npm_version()
    resolved_version = version or package_version or cargo_version()
    if not resolved_version:
        raise SystemExit("No project version found in package.json or Cargo.toml")
    resolved_ref = ref or os.getenv("GITHUB_HEAD_REF") or os.getenv("GITHUB_REF_NAME")
    if not resolved_ref:
        resolved_ref = git_value("branch", "--show-current") or "detached"
    resolved_revision = revision or os.getenv("GITHUB_SHA") or git_value("rev-parse", "HEAD")
    return {
        "version": resolved_version,
        "version_source": "package.json" if version or package_version else "Cargo.toml",
        "ref": resolved_ref,
        "revision": resolved_revision[:12],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "pages" / "_data" / "build.json")
    parser.add_argument("--ref")
    parser.add_argument("--revision")
    parser.add_argument("--version")
    args = parser.parse_args()
    data = resolve(args.ref, args.revision, args.version)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, indent=2) + "\n")
    print(f"Website build: {data['ref']} v{data['version']} ({data['revision']})")


if __name__ == "__main__":
    main()
