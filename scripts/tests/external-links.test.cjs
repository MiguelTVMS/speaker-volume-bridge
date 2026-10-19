const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = fs.readFileSync(
  path.join(__dirname, "..", "..", "pages", "external-links.js"),
  "utf8",
);

test("external links open safely while same-site links stay in the current tab", () => {
  const links = [
    { href: "https://github.com/MiguelTVMS/speaker-volume-bridge", rel: "author" },
    { href: "https://svb.miguel.ms/guide/", rel: "" },
    { href: "mailto:hello@example.com", rel: "" },
  ];
  const context = {
    URL,
    window: { location: { origin: "https://svb.miguel.ms", href: "https://svb.miguel.ms/" } },
    document: { querySelectorAll: () => links },
    Set,
  };

  vm.runInNewContext(source, context);

  assert.equal(links[0].target, "_blank");
  assert.deepEqual(new Set(links[0].rel.split(/\s+/)), new Set(["author", "noopener", "noreferrer"]));
  assert.equal(links[1].target, undefined);
  assert.equal(links[1].rel, "");
  assert.equal(links[2].target, undefined);
});
