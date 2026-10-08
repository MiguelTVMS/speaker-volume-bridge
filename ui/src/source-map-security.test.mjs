import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';

// Exercise the same PostCSS dependency that Vite uses for CSS builds.
const require = createRequire(import.meta.url);
const viteRequire = createRequire(require.resolve('vite/package.json'));
const postcss = viteRequire('postcss');

const sectionMap = { version: 3, sources: ['input.css'], names: [], mappings: 'AAAA' };
const indexedMap = (line) => ({
  version: 3,
  sections: [{ offset: { line, column: 0 }, map: sectionMap }],
});

for (const line of [Number.MAX_SAFE_INTEGER, -1, 0.5]) {
  test(`CSS source maps reject unsafe section offset ${line}`, () => {
    assert.throws(
      () => postcss.parse('a { color: red }', { map: { prev: indexedMap(line) } }),
      /Section offset/,
    );
  });
}

test('CSS source maps still accept a valid indexed section', () => {
  const root = postcss.parse('a { color: red }', {
    map: { prev: indexedMap(0) },
  });
  assert.equal(root.first.selector, 'a');
  assert.equal(root.first.first.value, 'red');
});
