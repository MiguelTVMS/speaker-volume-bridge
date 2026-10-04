"""Validate the Markdown-backed website guide and its published routes."""
from importlib.util import module_from_spec, spec_from_file_location
import json
import re
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
GUIDE = ROOT / "pages" / "guide"
WIKI_ORIGIN = "https://github.com/MiguelTVMS/speaker-volume-bridge/wiki"


class WebsiteGuideTests(unittest.TestCase):
    def setUp(self):
        self.sources = sorted(GUIDE.glob("*.md"))

    def test_every_guide_page_uses_the_shared_layout(self):
        self.assertGreater(len(self.sources), 20)
        for source in self.sources:
            with self.subTest(page=source.name):
                self.assertTrue(source.read_text().startswith("---\nlayout: guide\n---\n"))

    def test_internal_guide_links_resolve(self):
        routes = {"/guide/": GUIDE / "index.md"}
        routes.update({f"/guide/{source.stem}.html": source for source in self.sources if source.stem != "index"})
        for source in self.sources:
            for target in re.findall(r"\[[^]]+\]\((/guide/[^)]*)\)", source.read_text()):
                with self.subTest(page=source.name, target=target):
                    self.assertIn(target, routes)

    def test_public_content_no_longer_links_to_the_github_wiki(self):
        paths = [ROOT / "README.md", ROOT / "pages" / "llms.txt"]
        paths.extend((ROOT / "pages").glob("*.html"))
        paths.extend(self.sources)
        for path in paths:
            with self.subTest(path=path.name):
                self.assertNotIn(WIKI_ORIGIN, path.read_text())

    def test_deployment_keeps_markdown_next_to_rendered_pages(self):
        workflow = (ROOT / ".github" / "workflows" / "pages.yml").read_text()
        layout = (ROOT / "pages" / "_layouts" / "guide.html").read_text()
        self.assertIn("actions/jekyll-build-pages@v1", workflow)
        self.assertIn('sudo chown -R "$(id -u):$(id -g)" _site', workflow)
        self.assertIn("python3 scripts/publish-website-markdown.py --destination _site", workflow)
        self.assertIn('rel="alternate" type="text/markdown"', layout)
        self.assertIn('"@type": "WebPage"', layout)
        self.assertIn('"@type": "BreadcrumbList"', layout)
        self.assertIn('property="og:image:width"', layout)

    def test_markdown_alternates_cannot_overwrite_html_pages(self):
        config = (ROOT / "pages" / "_config.yml").read_text()
        self.assertNotIn("permalink: pretty", config)
        for name in ("index.md", "privacy.md", "upgrade.md"):
            with self.subTest(page=name):
                self.assertIn(f"  - {name}\n", config)

    def test_github_only_admonitions_are_not_used(self):
        for source in self.sources:
            with self.subTest(page=source.name):
                self.assertNotRegex(source.read_text(), r"^> \[!\w+\]", msg="Use a styled guide-callout instead")

    def test_breadcrumbs_remain_linked_and_unadorned(self):
        breadcrumbs = (ROOT / "pages" / "_includes" / "breadcrumbs.html").read_text()
        styles = (ROOT / "pages" / "styles.css").read_text()
        self.assertIn('<a href="/guide/" aria-current="page">Guide</a>', breadcrumbs)
        self.assertIn('<a href="{{ page.url }}" aria-current="page">{{ guide_title }}</a>', breadcrumbs)
        self.assertRegex(styles, r"\.breadcrumbs a \{[^}]*text-decoration: none;", msg="Breadcrumb links must not be underlined")

    def test_current_release_version_has_one_source(self):
        release = json.loads((ROOT / "pages" / "_data" / "release.json").read_text())["version"]
        self.assertRegex(release, r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")
        for source in self.sources:
            with self.subTest(page=source.name):
                self.assertNotIn(release, source.read_text())
        self.assertIn("{{ site.data.release.version }}", (GUIDE / "index.md").read_text())

    def test_published_markdown_resolves_release_version(self):
        script = ROOT / "scripts" / "publish-website-markdown.py"
        spec = spec_from_file_location("publish_website_markdown", script)
        module = module_from_spec(spec)
        spec.loader.exec_module(module)
        version = json.loads((ROOT / "pages" / "_data" / "release.json").read_text())["version"]
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            module.publish(destination)
            guide = (destination / "guide" / "index.md").read_text()
            self.assertIn(f"v{version}", guide)
            self.assertNotIn("{{ site.data.", guide)


if __name__ == "__main__":
    unittest.main()
