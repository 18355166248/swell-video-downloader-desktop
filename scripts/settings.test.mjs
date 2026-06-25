import assert from 'node:assert/strict';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import ts from 'typescript';

const sourcePath = resolve('src/lib/settings.ts');
const outputPath = resolve('tmp/settings.test-output.mjs');
mkdirSync(dirname(outputPath), { recursive: true });

const source = readFileSync(sourcePath, 'utf8');
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2022,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
});
writeFileSync(outputPath, transpiled.outputText);

try {
  const {
    DEFAULT_DOWNLOAD_CONCURRENCY,
    MAX_DOWNLOAD_CONCURRENCY,
    MIN_DOWNLOAD_CONCURRENCY,
    normalizeDownloadConcurrency,
  } = await import(`../tmp/settings.test-output.mjs?cache=${Date.now()}`);

  assert.equal(DEFAULT_DOWNLOAD_CONCURRENCY, 3);
  assert.equal(MIN_DOWNLOAD_CONCURRENCY, 1);
  assert.equal(MAX_DOWNLOAD_CONCURRENCY, 8);

  assert.equal(normalizeDownloadConcurrency(null), DEFAULT_DOWNLOAD_CONCURRENCY);
  assert.equal(normalizeDownloadConcurrency(undefined), DEFAULT_DOWNLOAD_CONCURRENCY);
  assert.equal(normalizeDownloadConcurrency(Number.NaN), DEFAULT_DOWNLOAD_CONCURRENCY);
  assert.equal(normalizeDownloadConcurrency(0), MIN_DOWNLOAD_CONCURRENCY);
  assert.equal(normalizeDownloadConcurrency(1), 1);
  assert.equal(normalizeDownloadConcurrency(3.8), 3);
  assert.equal(normalizeDownloadConcurrency(99), MAX_DOWNLOAD_CONCURRENCY);
} finally {
  rmSync(outputPath, { force: true });
}
