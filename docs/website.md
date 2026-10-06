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

GitHub Pages remains the hosting provider. Website delivery composes the retained,
verified GA site with approved catalogs from `develop`. GA source promotion stays
on `main`; catalog-only delivery can run from `develop` without another application
release. The workflow serializes these deliveries and verifies served content.
See [catalog operations](catalog-operations.md) for the administrator prerequisites,
trusted workflow handoff, durable snapshot adoption and exact-byte verification.
Activation is incomplete until those live acceptance checks are satisfied.

### Catalog publication

First verify an existing release is public. Builds, drafts, uploads and Store
submissions are not public availability evidence. The proposal workflow inspects
actual direct-download packages and their provenance before preparing entries.

Select the `develop` branch and run **Propose update catalog** with the required
`release_tag` input naming that already public release. This is the only manual
input. Do not supply JSON availability records, catalog digests or a selected
feed. The workflow reads current catalogs and independently verifies public
release metadata, package bytes, versions, architectures and publisher channel.
It updates both versioned feeds as applicable: GA direct releases enter v1 and
v2; Alpha/Beta direct releases enter v2 only. It opens a focused proposal PR back
to `develop` with normal CI and human review. It never publishes an application
package or approves or merges its own proposal.

After approved merge, catalog delivery overlays the approved feeds onto the
retained GA snapshot. Verify both public catalogs and every non-catalog website
file. Catalog publication does not require another release or a catalog promotion
to `main`. Scheduled reconciliation can recover missed proposals or delivery, but
does not replace those approvals or served-byte verification.

Operator cases:

- **Initial bootstrap:** adopt the actual served GA snapshot using the documented
  Website bootstrap procedure. Bootstrap deploys nothing. An empty catalog stays
  valid until independently verified release entries are approved and delivered.
- **Missing credentials:** configure the catalog GitHub App, then rerun the
  proposal workflow on `develop` for the existing public release tag. Do not
  dispatch another application release to recover catalog delivery.
- **Retry or conflict:** rerun against current approved `develop`. Review the
  regenerated proposal and retain normal CI and human approval. Do not manually
  combine unverified entries or supply a previously reviewed digest.
- **Store availability:** the automatic direct-release proposal does not establish
  or create Store availability. Keep unavailable or unverified Store targets
  outstanding; a submission or approval cannot substitute for public availability.
- **Withdrawal:** the manual release-tag dispatch has no withdrawal input. Handle
  withdrawn packages through the [reviewed withdrawal procedure](#reviewed-withdrawal)
  below; never restore an older release as a downgrade.
- **Before deployment:** proposal, build or pre-deployment failures leave the
  previous served site in place. Correct the failed prerequisite and retry.
- **After deployment starts:** a deployment error or failed served-byte check is
  an unverified outcome. Pages may already serve the candidate catalogs; do not
  assume rollback or report catalog delivery as verified. Retain the previous and
  candidate snapshots and follow [delivery recovery](catalog-operations.md#independent-catalog-delivery).
  For ambiguous GA delivery, use Website `verify-candidate` without deployment,
  or the documented main recovery run. Retry catalog-only delivery on develop
  when appropriate. Never clear pending state to bypass verification. A public
  application release with pending catalog delivery remains supported.

### Reviewed withdrawal

Use a focused branch from current approved `develop`. Inspect the current catalogs
and review the exact edition, channel, OS, application architecture and version
cutoff before preparing a change. The following records illustrate one Windows
GA target; replace the target and version with the reviewed values, and supply a
non-empty withdrawal reason. Do not run the example unchanged unless it identifies
the actual withdrawn offer. Keep the record files outside the committed catalog
change.

Save the v1 record as `withdrawal-v1.json`:

```json
{
  "operation": "withdraw",
  "target": {
    "edition": "direct_windows",
    "channel": "stable",
    "os": "windows",
    "architecture": "x86_64"
  },
  "version": "1.8.0",
  "reason": "Public package withdrawn after review"
}
```

Save the v2 record as `withdrawal-v2.json` using the same fields plus
`"classification": "GA"` inside `target`. GA uses `channel: stable` in both feeds.
For Alpha or Beta direct targets, use only the v2 record, set the matching
classification and `channel: prereleases`, and do not modify v1. Store editions
(`microsoft_store` and `mac_app_store`) use only the v1 record and preparation
command, even for GA; v2 excludes Store targets. Use both feeds only for direct
GA targets. Repeat for each affected architecture and edition. An explicit reviewed version cutoff records withdrawal intent even
if that offer is absent; without an existing entry or cutoff the tool cannot
infer it. The withdrawal ledger prevents equal or older offers from being restored.

Capture both digests when reviewing the catalogs, then retain those values through
preparation so a changed catalog fails closed:

```sh
v1_expected=$(python3 -c 'import hashlib; from pathlib import Path; print(hashlib.sha256(Path("pages/updates/v1/catalog.json").read_bytes()).hexdigest())')
v2_expected=$(python3 -c 'import hashlib; from pathlib import Path; print(hashlib.sha256(Path("pages/updates/v2/catalog.json").read_bytes()).hexdigest())')
python3 scripts/prepare-update-catalog.py withdrawal-v1.json --catalog pages/updates/v1/catalog.json --expected-sha256 "$v1_expected"
python3 scripts/prepare-update-catalog.py withdrawal-v2.json --catalog pages/updates/v2/catalog.json --expected-sha256 "$v2_expected"
python3 scripts/validate-update-catalog.py pages/updates/v1/catalog.json
python3 scripts/validate-update-catalog.py pages/updates/v2/catalog.json
git diff -- pages/updates/v1/catalog.json pages/updates/v2/catalog.json
```

For a preview-only direct withdrawal, omit the v1 preparation command. For a
Store withdrawal, omit the v2 preparation command and do not create a v2 record. Review the removed
entries and retained tombstones, commit only the intended catalogs, and open a
normal PR to `develop`. Require CI and human approval. After approved merge and
delivery, verify both served feeds and unchanged non-catalog website files.
Neither local preparation nor a passing validator establishes deployed withdrawal.

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

### Combined preview feed

The proposal workflow maintains both v1 (stable-only transport) and v2
(combined GA/Alpha/Beta transport) from the same verified release-tag invocation.
V2 uses exact release-page actions and mandatory publisher classification.
Numeric preview versions are classified from the release-body marker and matching
GitHub prerelease status. V2 excludes Store targets. Classification participates
in duplicate-target identity and withdrawal. Clients select the maximum semantic
version across matching edition, OS and application-architecture candidates under
their saved channel policy. Validate both served catalogs after deployment; a
missing or empty preview feed remains unavailable.
