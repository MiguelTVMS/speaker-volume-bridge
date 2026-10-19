"""Generate the canonical website sitemap from static and guide sources."""
import argparse
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PAGES = ROOT / "pages"
ORIGIN = "https://svb.miguel.ms"


def urls():
    static = sorted(path for path in PAGES.glob("*.html"))
    result = [ORIGIN + ("/" if path.name == "index.html" else f"/{path.name}") for path in static]
    for path in sorted((PAGES / "guide").glob("*.md")):
        route = "/guide/" if path.name == "index.md" else f"/guide/{path.stem}.html"
        result.append(ORIGIN + route)
    return result


def render():
    entries = "\n".join(f"  <url><loc>{url}</loc></url>" for url in urls())
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{entries}
</urlset>
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    target = PAGES / "sitemap.xml"
    content = render()
    if args.check:
        if not target.exists() or target.read_text() != content:
            raise SystemExit("Website sitemap is stale; run scripts/generate-sitemap.py")
    else:
        target.write_text(content)


if __name__ == "__main__":
    main()
