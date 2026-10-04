#!/usr/bin/env python3
"""Publish raw Markdown alternates with website data tokens resolved."""
import argparse
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
PAGES = ROOT / "pages"
RELEASE_TOKEN = "{{ site.data.release.version }}"


def release_version():
    data = json.loads((PAGES / "_data" / "release.json").read_text())
    version = data.get("version")
    if not isinstance(version, str) or not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version):
        raise SystemExit("pages/_data/release.json must contain a semantic version")
    return version


def markdown_sources():
    yield from (PAGES / name for name in ("index.md", "privacy.md", "upgrade.md"))
    yield from sorted((PAGES / "guide").glob("*.md"))


def build_metadata():
    path = PAGES / "_data" / "build.json"
    if not path.exists():
        raise SystemExit("Generate pages/_data/build.json before publishing Markdown")
    return json.loads(path.read_text())


def render(source, version, build):
    replacements = {
        RELEASE_TOKEN: version,
        "{{ site.data.build.version }}": str(build["version"]),
        "{{ site.data.build.ref }}": str(build["ref"]),
        "{{ site.data.build.revision }}": str(build["revision"]),
        "{% if site.data.build %}": "",
        "{% endif %}": "",
    }
    content = source.read_text()
    for token, value in replacements.items():
        content = content.replace(token, value)
    if "{{ site.data." in content:
        raise SystemExit(f"Unresolved site data token in {source.relative_to(ROOT)}")
    return content


def publish(destination):
    version = release_version()
    build = build_metadata()
    for source in markdown_sources():
        relative = source.relative_to(PAGES)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(render(source, version, build))
    return version


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--destination", type=Path, default=ROOT / "_site")
    args = parser.parse_args()
    version = publish(args.destination)
    print(f"Published Markdown alternates with release version {version}")


if __name__ == "__main__":
    main()
