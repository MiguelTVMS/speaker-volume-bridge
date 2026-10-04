# Public website

The user-facing website lives in `pages/`. Jekyll assembles static HTML from the
page sources, shared layouts and includes, with local CSS and no third-party
fonts. Optional tracking is managed through a consent-gated Google Tag Manager
integration. The desktop application and its build remain separate.

Shared site chrome lives in `pages/_includes/`. The Markdown-backed guide lives
in `pages/guide/` and uses `pages/_layouts/guide.html`. Edit a shared header,
footer, consent panel, breadcrumb, or guide navigation once rather than copying
markup between pages.

Before Jekyll runs, `scripts/generate-website-build-data.py` writes branch/ref,
revision, and project version into `pages/_data/build.json`. It prefers a root
`package.json` version when present and otherwise uses the Cargo workspace
version. The shared footer renders this build identity. The file is generated
and ignored rather than committed, so local, PR, and `main` builds each describe
their actual source. This build identity does not replace the separately
verified GA version stated in the user guide.

The documented GA version has one source: `pages/_data/release.json`. Guide
Markdown uses `{{ site.data.release.version }}` wherever it refers to the current
release. Jekyll resolves the token in rendered HTML, and
`scripts/publish-website-markdown.py` resolves it when publishing raw `.md`
alternates for people, crawlers, and language models. Update the JSON value once
when the guide moves to a new GA release; keep historical release and toolchain
versions literal.

## Preview

Use a local Jekyll environment to run `bundle exec jekyll serve --source pages`
from the repository root, then open `http://localhost:4000`. A plain Python file
server does not expand the shared includes or render the guide Markdown. Check
the landing, guide, privacy, and guide upgrade pages at mobile and desktop widths.

## Publish

In repository Settings > Pages, select **GitHub Actions** as the source. The
Website workflow builds `pages/` with the official GitHub Pages Jekyll action
when changes to that directory or the workflow reach `main`. It then publishes
resolved guide Markdown beside the rendered HTML before uploading the artifact.
It does not depend on release creation or application
packaging. Manual dispatch is supported on `main` only. Configure the
`github-pages` environment to allow deployments from `main`.

Changes follow the normal feature branch, approved PR, and Gitflow process.
No deployment runs from feature branches or `develop`.

The machine-readable update catalog is published with the same atomic `pages/`
artifact at `updates/v1/catalog.json`. Validate it with
`python3 scripts/validate-update-catalog.py`. Its entries represent independently
verified public availability per distribution and application architecture; a
build, tag, upload, submission or workspace version is not sufficient evidence.
See [ADR 0019](decisions/0019-distribution-aware-updates.md) for the v1 contract.

### Catalog publication

Catalog entries deliberately lag package creation. First complete the ordinary
`develop` release-candidate flow and publish the stable release through its
approved promotion PR. Confirm each direct-download asset is publicly retrievable,
or separately confirm the exact Store edition is publicly listed. Drafts,
prereleases, submissions and uploads are not availability evidence.

From `develop`, run **Propose update catalog** with a JSON availability record and
the SHA-256 of the catalog that was reviewed. The workflow serializes writers,
revalidates the full document, changes only `pages/updates/v1/catalog.json`, and
opens a focused PR back to `develop`. It never publishes a package or deploys the
site. Repeated evidence is idempotent; older versions, incomplete asset sets,
unverified Store listings and stale catalog digests fail closed. Each architecture
and edition advances independently, so partial availability is expected.

For a withdrawn public package, use a `withdraw` record with its four-field target
and a non-empty reason. Withdrawal removes that target and never exposes an older
release as a downgrade. Retry a failed or stale proposal from current `develop`.
After review, the catalog follows the normal release path from `develop` to
`release/<version>` and then `main`; only the `main` Pages deployment serves it.
That deployment fetches the public catalog, validates the served bytes, and
requires a cache-control header before completing.

Operator cases:

- **Initial bootstrap:** start from the checked-in empty valid catalog, calculate
  its SHA-256, and propose only the first independently verified target. An empty
  catalog remains a valid normal state until then.
- **Store lag:** the release operator who observes the exact version publicly
  obtainable in Microsoft Store or Mac App Store supplies the explicit Store
  record. Do not reuse build, submission or approval status.
- **Retry or conflict:** fetch current `develop`, review its new digest and rerun.
  The concurrency group prevents overlapping writers; a PR conflict is resolved
  by closing the stale proposal and generating a new one, never by hand-merging
  unverified entries.
- **Withdrawal:** use the explicit target and reason. Do not republish a prior
  version as a substitute.
- **Failure:** leave the current catalog untouched. Fix the availability evidence,
  complete asset publication, or wait for Store visibility before retrying. A
  release without a catalog entry is supported and appears unavailable to clients.

Example direct-release evidence:

```json
{
  "operation": "upsert",
  "verifiedAvailable": true,
  "source": {
    "kind": "github_release",
    "draft": false,
    "prerelease": false,
    "requiredAssets": ["SpeakerVolumeBridge-windows-x64.exe"],
    "availableAssets": ["SpeakerVolumeBridge-windows-x64.exe"]
  },
  "entry": {
    "edition": "direct_windows",
    "channel": "stable",
    "os": "windows",
    "architecture": "x86_64",
    "version": "1.8.0",
    "publishedAt": "2026-10-04T12:00:00Z",
    "releaseNotes": "Stable release notes.",
    "action": {"type": "open_url", "url": "https://svb.miguel.ms/guide/Upgrading.html"}
  }
}
```

## Content maintenance

Keep feature descriptions aligned with README.md and the website guide. The website
privacy page reproduces PRIVACY.md and adds a separate website hosting notice.
When the application policy changes, update both copies together, preserving
its effective date. The policy currently names Windows and macOS audio outputs;
review the source policy for Ubuntu coverage before changing the legal text.
Keep `pages/license.txt` identical to LICENSE. Do not add analytics, remote fonts,
or third-party embeds without reviewing the website privacy notice.

## Permanent installer links

Download buttons use GitHub's `releases/latest/download/` URLs. Release CI publishes
one fixed-name file per package: macOS DMG, Windows
x64/ARM64 installers, and Ubuntu x64/ARM64 DEBs. Versioned build filenames
are renamed before upload; the release tag supplies the version. `scripts/prepare-release-downloads.sh` validates all
release assets before renaming them. Windows and Linux filenames explicitly identify x64 and ARM64. The old architecture-free
Windows URL and macOS ZIP are not published in future releases. Prereleases also receive these assets,
but the latest stable URLs do not select prereleases.

The website does not need to be rebuilt when a release is published. Before
first deploying these links, publish a stable release with the updated workflow
or upload the matching fixed-name assets to the existing latest stable release.
Older releases do not gain these files automatically.

## Search and assistant discovery

The canonical origin is `https://svb.miguel.ms`. All static and generated guide
pages declare canonical, Open Graph, social-card, Markdown alternate, and JSON-LD
metadata. Guide pages also publish breadcrumb markup and structured data. The
sitemap lists canonical HTML pages and is generated from the source inventory.
robots.txt allows crawling and links to the sitemap. llms.txt is a
curated project guide, an emerging convention rather than a ranking guarantee.
Keep its claims consistent with the visible page and privacy policy. No ratings,
reviews, or unverified compatibility claims are added to structured data.

The application schema retains the former product name as `alternateName` for
the rebrand. Social images include their actual dimensions and accessible text.
Run `python3 scripts/generate-sitemap.py` after adding or removing a page. Run
the website SEO and guide unit tests to check metadata consistency, sitemap
coverage, shared rendering rules, image dimensions, and local links and
fragments. The website validation workflow also performs the real Jekyll build
and checks its guide entry point before merge.

After deployment, submit the sitemap in Google Search Console and Bing Webmaster
Tools. Validate structured data with their inspection tools. Updating these files
does not itself submit the site or guarantee indexing or rich results.

## Markdown versions

Every hand-authored HTML page has a generated Markdown counterpart (`index.md`
and `privacy.md`). Upgrade documentation is Markdown-first at
`pages/guide/Upgrading.md` rather than duplicated as a top-level page.
The HTML alternate link and llms.txt point to these files. Run
`python3 scripts/generate-page-markdown.py` after editing page content, and
`python3 scripts/generate-page-markdown.py --check` to detect stale copies.
The conversion expands shared includes, includes main content and footer notices,
preserves absolute links, and omits navigation and decorative graphics.

Guide Markdown is the source rather than generated output. Jekyll applies the
shared guide layout and CSS to produce each HTML page, while the deployment
workflow also publishes the `.md` file at the matching guide path. Before
publishing, the script resolves the current-release token so raw Markdown
contains the same concrete version as the HTML rather than a Liquid expression.
Each rendered page advertises that source with `rel="alternate"`, and `llms.txt`
links directly to the most useful Markdown entry points. The sitemap lists the
canonical rendered HTML pages rather than the alternate Markdown copies.

The two generated top-level Markdown alternates are excluded from Jekyll and
copied into the finished artifact after the build. This prevents GitHub Pages'
optional-front-matter plugin from rendering them with a default theme and
overwriting `index.html` or `privacy.html`.

Do not enable Jekyll's `permalink: pretty` setting. Public guide and policy
links intentionally use stable `.html` routes; validation checks representative
rendered paths so a permalink change cannot silently break them.

## Website consent and Google Tag Manager

The head loads local consent.js with defer. It queues denied Consent Mode defaults
before loading container GTM-P7RPRQBZ only after an optional category is granted.
There is intentionally no unconditional GTM script or noscript iframe: either
would contact Google before consent. JavaScript-disabled visits remain untracked.

Advanced choices map to analytics_storage, ad_storage/ad_user_data, and separately
ad_personalization. Personalized advertising requires advertising consent. Choice
storage expires after 180 days; invalid records fail closed. Withdrawal reloads
and attempts cleanup of accessible Google first-party cookies. Cross-tab changes
and back/forward cache restoration reload to honor the latest saved preference.

### Container configuration required before publishing tags

The website cannot sandbox tags inside GTM. In the container, configure additional
consent checks on every analytics, advertising, custom HTML, and third-party tag.
Analytics tags require analytics_storage. Advertising tags require ad_storage and
ad_user_data; personalized advertising additionally requires ad_personalization.
Use svb_consent_update triggers with the corresponding svb_analytics,
svb_advertising, or svb_personalization boolean data-layer variables. Include an
appropriate permitted page-load trigger for returning visitors. Never trigger
optional tags unconditionally on All Pages or Consent Initialization. Built-in
Google consent checks alone may permit cookieless pings when a category is denied.

Verify each consent combination in GTM Preview/Tag Assistant before publishing the
container. Container access and its configured tags are outside this repository;
website tests cover the loader and signals, not tag-level enforcement. Keep the
privacy notice and cookie retention details aligned with the actual published tags.

Run `node --test scripts/tests/consent.test.cjs` and regenerate Markdown after HTML
changes. The preference is website-only; the desktop app has no new telemetry.

Night schedule appears alongside the other capabilities in the same feature-card
grid, with matching illustrations, typography, and spacing. Five cards use two
columns on desktop (two, two, then one), leaving space for a future feature. Feature copy describes the released
product because the site publishes with the release. Keep the visible copy and
llms.txt aligned. The section introduces no tracking or external assets.

The Ubuntu card provides distinct x64 and ARM64 download buttons. The ARM64
URL targets the next release that includes `speaker-volume-bridge-linux-arm64.deb`;
it will become available after the first GA release containing that asset.

The Windows card likewise provides x64 and ARM64 buttons. Publish both
Windows installer assets in a stable release before deploying
the updated website. Generated Markdown and llms.txt carry the same choices.
